//! Unified frame source over ffmpeg.
//!
//! A still image, a video file, and a live camera are all just "a sequence of
//! frames" as far as the renderer is concerned - an image is a one-frame video.
//! That lets every mode share a single decode path.
//!
//! Frames come out already scaled to the destination grid. Doing the downscale
//! in swscale rather than sampling by hand means one output pixel is exactly
//! one terminal cell, and it is where the cell aspect-ratio correction gets
//! applied by the caller choosing `dst`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ffmpeg_next as ffmpeg;
use ffmpeg::format::context::Input as InputContext;
use ffmpeg::format::Pixel;
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use ffmpeg::util::frame::video::Video;

/// Demuxers that expose local capture devices, in preference order.
#[cfg(target_os = "macos")]
const CAMERA_DEMUXERS: &[&str] = &["avfoundation"];
#[cfg(target_os = "linux")]
const CAMERA_DEMUXERS: &[&str] = &["v4l2", "video4linux2"];
#[cfg(target_os = "windows")]
const CAMERA_DEMUXERS: &[&str] = &["dshow", "vfwcap"];

/// A frame scaled to the terminal grid: one RGB triple per character cell,
/// tightly packed (no stride padding).
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Frame {
    /// Returns the RGB triple at `(x, y)`.
    ///
    /// # Panics
    /// Panics if the coordinates are outside the frame.
    #[inline]
    pub fn pixel(&self, x: u32, y: u32) -> (u8, u8, u8) {
        let i = ((y * self.width + x) * 3) as usize;
        (self.data[i], self.data[i + 1], self.data[i + 2])
    }
}

/// What to decode.
pub enum Source {
    /// Anything ffmpeg can demux from disk - a PNG/JPEG (single frame) or an
    /// MP4/MKV/etc.
    File(PathBuf),
    /// A live capture device. `spec` is the platform's device identifier; for
    /// avfoundation on macOS that is an index such as `"0"`.
    Camera {
        spec: String,
        size: Option<(u32, u32)>,
        fps: Option<u32>,
    },
}

/// A decoder plus scaler, yielding RGB frames at a fixed output size.
pub struct FrameSource {
    ictx: InputContext,
    decoder: ffmpeg::decoder::Video,
    stream: usize,
    scaler: Scaler,
    dst: (u32, u32),
    src_format: Pixel,
    src_size: (u32, u32),
    frame_interval: Option<Duration>,
    /// A capture device rather than a file: it never ends, and it is allowed to
    /// keep us waiting between frames.
    live: bool,
    /// EOF reached; the decoder has been told to flush.
    draining: bool,
    /// Flush complete, nothing left to yield.
    done: bool,
}

/// How long to wait before re-polling a source that had no data ready.
const EAGAIN_BACKOFF: Duration = Duration::from_millis(2);

/// Give up on a file that keeps returning EAGAIN (~1s). A live device is
/// allowed to block indefinitely, so this bound does not apply to it.
const EAGAIN_MAX_RETRIES: u32 = 500;

impl FrameSource {
    /// Opens `source` and prepares to emit frames at `dst` = (width, height)
    /// in character cells.
    pub fn open(source: &Source, dst: (u32, u32)) -> Result<Self, ffmpeg::Error> {
        ffmpeg::init()?;
        // libav* logs to stderr, which in a TUI paints straight over the art.
        // A single "deprecated pixel format" warning is enough to corrupt a
        // frame mid-capture, so keep everything below Fatal quiet.
        ffmpeg::util::log::set_level(ffmpeg::util::log::Level::Fatal);
        assert!(dst.0 > 0 && dst.1 > 0, "destination size must be non-zero");

        let ictx = match source {
            Source::File(path) => ffmpeg::format::input(path)?,
            Source::Camera { spec, size, fps } => open_camera(spec, *size, *fps)?,
        };

        // Pull everything we need off the stream before `ictx` is borrowed
        // mutably for packet reads.
        let (stream, frame_interval, parameters) = {
            let s = ictx
                .streams()
                .best(Type::Video)
                .ok_or(ffmpeg::Error::StreamNotFound)?;
            let rate = s.avg_frame_rate();
            // A rate of 0 means "unknown" (still images, some capture devices).
            let interval = if rate.numerator() > 0 && rate.denominator() > 0 {
                Some(Duration::from_secs_f64(
                    rate.denominator() as f64 / rate.numerator() as f64,
                ))
            } else {
                None
            };
            (s.index(), interval, s.parameters())
        };

        let decoder = ffmpeg::codec::context::Context::from_parameters(parameters)?
            .decoder()
            .video()?;

        let src_format = decoder.format();
        let src_size = (decoder.width(), decoder.height());
        let scaler = Scaler::get(
            src_format,
            src_size.0,
            src_size.1,
            Pixel::RGB24,
            dst.0,
            dst.1,
            Flags::BILINEAR,
        )?;

        Ok(Self {
            ictx,
            decoder,
            stream,
            scaler,
            dst,
            src_format,
            src_size,
            frame_interval,
            live: matches!(source, Source::Camera { .. }),
            draining: false,
            done: false,
        })
    }

    /// Nominal time between frames, if the stream declares one. `None` for
    /// still images and devices that do not report a rate.
    pub fn frame_interval(&self) -> Option<Duration> {
        self.frame_interval
    }

    /// Output size in character cells.
    pub fn size(&self) -> (u32, u32) {
        self.dst
    }

    /// Native pixel dimensions of the decoded stream, before scaling. Needed to
    /// work out an aspect-correct cell grid.
    pub fn source_size(&self) -> (u32, u32) {
        self.src_size
    }

    /// Retargets the scaler, e.g. after the terminal is resized. Cheap no-op if
    /// the size is unchanged.
    pub fn resize(&mut self, dst: (u32, u32)) -> Result<(), ffmpeg::Error> {
        if dst == self.dst || dst.0 == 0 || dst.1 == 0 {
            return Ok(());
        }
        self.dst = dst;
        self.rebuild_scaler()
    }

    fn rebuild_scaler(&mut self) -> Result<(), ffmpeg::Error> {
        self.scaler = Scaler::get(
            self.src_format,
            self.src_size.0,
            self.src_size.1,
            Pixel::RGB24,
            self.dst.0,
            self.dst.1,
            Flags::BILINEAR,
        )?;
        Ok(())
    }

    /// Decodes and returns the next frame, or `None` once the stream ends.
    ///
    /// A live camera never returns `None` under normal operation.
    pub fn next_frame(&mut self) -> Option<Result<Frame, ffmpeg::Error>> {
        let mut eagain_retries = 0u32;

        loop {
            // Drain whatever the decoder already has before reading more.
            let mut decoded = Video::empty();
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                return Some(self.scale(&decoded));
            }

            if self.done {
                return None;
            }
            if self.draining {
                // Flushed and fully drained.
                self.done = true;
                return None;
            }

            let mut packet = ffmpeg::codec::packet::Packet::empty();
            match packet.read(&mut self.ictx) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        if let Err(e) = self.decoder.send_packet(&packet) {
                            return Some(Err(e));
                        }
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    self.draining = true;
                    if let Err(e) = self.decoder.send_eof() {
                        return Some(Err(e));
                    }
                }
                // No data ready yet. Capture devices routinely return this
                // between frames - notably on the very first read, before the
                // camera has warmed up - so it means "wait", not "failed".
                Err(ffmpeg::Error::Other { errno })
                    if errno == ffmpeg::util::error::EAGAIN
                        && (self.live || eagain_retries < EAGAIN_MAX_RETRIES) =>
                {
                    eagain_retries += 1;
                    std::thread::sleep(EAGAIN_BACKOFF);
                }
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn scale(&mut self, decoded: &Video) -> Result<Frame, ffmpeg::Error> {
        // Source geometry can change mid-stream (and capture devices often
        // report the real format only once frames start flowing), so rebuild
        // the scaler if the input no longer matches what it was built for.
        if decoded.format() != self.src_format
            || decoded.width() != self.src_size.0
            || decoded.height() != self.src_size.1
        {
            self.src_format = decoded.format();
            self.src_size = (decoded.width(), decoded.height());
            self.rebuild_scaler()?;
        }

        let mut rgb = Video::empty();
        self.scaler.run(decoded, &mut rgb)?;

        // swscale pads each row out to its own stride; copy row by row so the
        // caller gets a tightly packed buffer it can index arithmetically.
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        let stride = rgb.stride(0);
        let src = rgb.data(0);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            let off = y * stride;
            data.extend_from_slice(&src[off..off + w * 3]);
        }

        Ok(Frame {
            width: rgb.width(),
            height: rgb.height(),
            data,
        })
    }
}

/// Finds the platform capture demuxer and opens `spec` through it.
fn open_camera(
    spec: &str,
    size: Option<(u32, u32)>,
    fps: Option<u32>,
) -> Result<InputContext, ffmpeg::Error> {
    ffmpeg::device::register_all();

    let format = CAMERA_DEMUXERS
        .iter()
        .find_map(|name| ffmpeg::device::input::video().find(|f| f.name() == *name))
        .ok_or(ffmpeg::Error::DemuxerNotFound)?;

    let mut options = ffmpeg::Dictionary::new();

    // Left to itself, avfoundation may select a portrait mode (Mac cameras
    // advertise both 1920x1080 and 1080x1920), which renders sideways in a
    // wide terminal. 720p is landscape, universally supported, and plenty of
    // detail for a character grid.
    let size = size.or(if cfg!(target_os = "macos") {
        Some((1280, 720))
    } else {
        None
    });
    if let Some((w, h)) = size {
        options.set("video_size", &format!("{w}x{h}"));
    }

    // avfoundation defaults to 29.97fps (NTSC), but Mac cameras typically
    // advertise exactly 15 and 30 and reject anything else outright with EIO.
    // A bare `--camera` therefore fails unless we pin a rate the device will
    // actually accept. Other backends negotiate sanely, so only default here.
    let fps = fps.or(if cfg!(target_os = "macos") {
        Some(30)
    } else {
        None
    });
    if let Some(fps) = fps {
        options.set("framerate", &fps.to_string());
    }
    // Keep latency down: drop stale frames rather than queueing them, so what
    // renders is what the camera is seeing now.
    options.set("fflags", "nobuffer");

    match ffmpeg::format::open_with(Path::new(spec), &format, options)? {
        ffmpeg::format::context::Context::Input(input) => Ok(input),
        // `open_with` only ever yields Output for an output format, which we
        // never pass here.
        ffmpeg::format::context::Context::Output(_) => Err(ffmpeg::Error::Bug),
    }
}

/// Lists the capture devices ffmpeg can see, for `--list-devices`.
pub fn camera_demuxer_name() -> Option<String> {
    ffmpeg::init().ok()?;
    ffmpeg::device::register_all();
    CAMERA_DEMUXERS
        .iter()
        .find_map(|name| ffmpeg::device::input::video().find(|f| f.name() == *name))
        .map(|f| f.name().to_string())
}

/// Convenience for the common case of decoding a file path.
impl From<PathBuf> for Source {
    fn from(p: PathBuf) -> Self {
        Source::File(p)
    }
}

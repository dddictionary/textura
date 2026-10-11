//! End-to-end checks that the ffmpeg decode path produces usable frames.
//!
//! These run against the sample media committed in the repo, so they exercise
//! real demuxing rather than a synthetic buffer.

use textura::ascii;
use textura::source::{FrameSource, Source};

fn to_ascii(frame: &textura::source::Frame) -> String {
    let mut out = String::with_capacity(((frame.width + 1) * frame.height) as usize);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let (r, g, b) = frame.pixel(x, y);
            out.push(ascii::glyph(ascii::luma(r, g, b), ascii::RAMP) as char);
        }
        out.push('\n');
    }
    out
}

#[test]
fn decodes_a_still_image_to_the_requested_grid() {
    let mut src = FrameSource::open(&Source::File("images/ferris.png".into()), (64, 20))
        .expect("failed to open ferris.png");

    let frame = src
        .next_frame()
        .expect("expected at least one frame")
        .expect("decode failed");

    // The whole point of scaling in swscale: we get exactly the grid we asked
    // for, one RGB triple per cell, with no stride padding left in the buffer.
    assert_eq!((frame.width, frame.height), (64, 20));
    assert_eq!(frame.data.len(), 64 * 20 * 3);

    let art = to_ascii(&frame);
    println!("{art}");

    assert!(
        art.chars().any(|c| c != ' ' && c != '\n'),
        "rendered art was entirely blank"
    );
}

#[test]
fn a_still_image_yields_exactly_one_frame() {
    let mut src = FrameSource::open(&Source::File("images/cat.jpg".into()), (40, 16))
        .expect("failed to open cat.jpg");

    let mut count = 0;
    while let Some(frame) = src.next_frame() {
        frame.expect("decode failed");
        count += 1;
        assert!(count < 100, "still image did not terminate");
    }
    assert_eq!(count, 1, "a still image should decode to a single frame");
}

#[test]
fn resize_retargets_the_output_grid() {
    let mut src = FrameSource::open(&Source::File("images/archlinux.png".into()), (32, 12))
        .expect("failed to open archlinux.png");

    src.resize((50, 18)).expect("resize failed");

    let frame = src
        .next_frame()
        .expect("expected a frame")
        .expect("decode failed");

    assert_eq!((frame.width, frame.height), (50, 18));
    assert_eq!(frame.data.len(), 50 * 18 * 3);
}

/// Synthesizes a short clip with the ffmpeg CLI. Returns false if ffmpeg is
/// unavailable, so the suite still passes outside the dev shell.
fn synth_clip(path: &std::path::Path, fps: u32, seconds: u32) -> bool {
    std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=size=320x240:rate={fps}:duration={seconds}"),
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn decodes_every_frame_of_a_video() {
    let dir = std::env::temp_dir().join("textura-decode-tests");
    std::fs::create_dir_all(&dir).expect("could not create temp dir");
    let clip = dir.join("testsrc.mp4");

    if !synth_clip(&clip, 10, 2) {
        eprintln!("skipping: ffmpeg CLI not available to synthesize a clip");
        return;
    }

    let mut src =
        FrameSource::open(&Source::File(clip), (48, 18)).expect("failed to open synthesized clip");

    // A real video declares a frame rate; a still image does not.
    let interval = src.frame_interval().expect("video should declare a rate");
    assert_eq!(interval.as_millis(), 100, "10fps means 100ms per frame");

    let mut count = 0;
    while let Some(frame) = src.next_frame() {
        let frame = frame.expect("decode failed mid-stream");
        assert_eq!((frame.width, frame.height), (48, 18));
        count += 1;
        assert!(count <= 100, "decode did not terminate");
    }

    // 10fps for 2s. The decoder must drain cleanly at EOF and not drop the
    // tail of the stream.
    assert_eq!(count, 20, "expected every frame, and a clean flush at EOF");
}

#[test]
fn missing_file_is_an_error_not_a_panic() {
    let err = FrameSource::open(&Source::File("images/does-not-exist.png".into()), (16, 8));
    assert!(err.is_err(), "opening a missing file should fail cleanly");
}

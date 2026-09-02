//! Turning a decoded frame into drawable terminal text.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};

use crate::ascii::{self, luma_range as luma_range_of, stretch};
use crate::source::Frame;

/// How a frame should be turned into glyphs.
pub struct Options {
    /// Glyph ramp, dark to light.
    pub ramp: Vec<u8>,
    /// Emit a 24-bit foreground colour per cell, sampled from the source.
    pub color: bool,
    /// Stretch each frame's luma range to fill the ramp.
    ///
    /// Without this, mid-tone-dominated footage collapses onto a handful of
    /// glyphs and the picture reads as flat - which is exactly what a webcam
    /// in ordinary indoor light produces.
    pub normalize: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ramp: ascii::RAMP.to_vec(),
            color: false,
            normalize: true,
        }
    }
}

/// The darkest and brightest luma present in `frame`. Frames are RGB24, so the
/// stride is three.
fn luma_range(frame: &Frame) -> (u8, u8) {
    luma_range_of(&frame.data, 3)
}

/// Renders `frame` to terminal text.
pub fn render(frame: &Frame, opts: &Options) -> Text<'static> {
    let (lo, hi) = if opts.normalize {
        luma_range(frame)
    } else {
        (0, 255)
    };

    let mut lines = Vec::with_capacity(frame.height as usize);

    for y in 0..frame.height {
        if opts.color {
            lines.push(color_line(frame, y, opts, lo, hi));
        } else {
            let mut row = String::with_capacity(frame.width as usize);
            for x in 0..frame.width {
                let (r, g, b) = frame.pixel(x, y);
                let l = stretch(ascii::luma(r, g, b), lo, hi);
                row.push(ascii::glyph(l, &opts.ramp) as char);
            }
            lines.push(Line::from(row));
        }
    }

    Text::from(lines)
}

/// Builds one coloured row, coalescing runs of identical colour into a single
/// span. Adjacent cells frequently share a colour, and every extra span is
/// another style change on the wire, so this materially cuts the per-frame
/// cost of colour mode.
fn color_line(frame: &Frame, y: u32, opts: &Options, lo: u8, hi: u8) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_color: Option<(u8, u8, u8)> = None;

    for x in 0..frame.width {
        let (r, g, b) = frame.pixel(x, y);
        let l = stretch(ascii::luma(r, g, b), lo, hi);
        let ch = ascii::glyph(l, &opts.ramp) as char;

        match run_color {
            Some(c) if c == (r, g, b) => run.push(ch),
            Some(c) => {
                spans.push(Span::styled(
                    std::mem::take(&mut run),
                    Style::default().fg(Color::Rgb(c.0, c.1, c.2)),
                ));
                run.push(ch);
                run_color = Some((r, g, b));
            }
            None => {
                run.push(ch);
                run_color = Some((r, g, b));
            }
        }
    }

    if let Some(c) = run_color {
        spans.push(Span::styled(
            run,
            Style::default().fg(Color::Rgb(c.0, c.1, c.2)),
        ));
    }

    Line::from(spans)
}

/// Renders to a plain string, for writing to a file.
pub fn to_plain_string(frame: &Frame, opts: &Options) -> String {
    let (lo, hi) = if opts.normalize {
        luma_range(frame)
    } else {
        (0, 255)
    };

    let mut out = String::with_capacity(((frame.width + 1) * frame.height) as usize);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let (r, g, b) = frame.pixel(x, y);
            let l = stretch(ascii::luma(r, g, b), lo, hi);
            out.push(ascii::glyph(l, &opts.ramp) as char);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_from(width: u32, height: u32, f: impl Fn(u32, u32) -> (u8, u8, u8)) -> Frame {
        let mut data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                let (r, g, b) = f(x, y);
                data.extend_from_slice(&[r, g, b]);
            }
        }
        Frame {
            width,
            height,
            data,
        }
    }

    #[test]
    fn normalize_rescues_a_low_contrast_frame() {
        // Every pixel within a narrow mid band - the ferris case.
        let frame = frame_from(16, 4, |x, _| {
            let v = 120 + (x as u8 % 8);
            (v, v, v)
        });

        let flat = to_plain_string(
            &frame,
            &Options {
                normalize: false,
                ..Default::default()
            },
        );
        let stretched = to_plain_string(
            &frame,
            &Options {
                normalize: true,
                ..Default::default()
            },
        );

        let distinct = |s: &str| {
            let mut v: Vec<char> = s.chars().filter(|c| *c != '\n').collect();
            v.sort_unstable();
            v.dedup();
            v.len()
        };

        assert!(
            distinct(&stretched) > distinct(&flat),
            "normalize should widen the glyph spread ({} -> {})",
            distinct(&flat),
            distinct(&stretched)
        );
    }

    #[test]
    fn render_shape_matches_the_frame() {
        let frame = frame_from(10, 3, |_, _| (128, 128, 128));
        let text = render(&frame, &Options::default());
        assert_eq!(text.lines.len(), 3);
        for line in &text.lines {
            assert_eq!(line.width(), 10);
        }
    }

    #[test]
    fn color_mode_coalesces_identical_runs() {
        // A uniform row is one colour, so it should collapse to a single span
        // rather than one per cell.
        let frame = frame_from(32, 1, |_, _| (10, 200, 30));
        let text = render(
            &frame,
            &Options {
                color: true,
                ..Default::default()
            },
        );
        assert_eq!(text.lines[0].spans.len(), 1);
        assert_eq!(text.lines[0].width(), 32);
    }

    #[test]
    fn color_mode_splits_on_colour_change() {
        let frame = frame_from(4, 1, |x, _| if x < 2 { (255, 0, 0) } else { (0, 0, 255) });
        let text = render(
            &frame,
            &Options {
                color: true,
                ..Default::default()
            },
        );
        assert_eq!(text.lines[0].spans.len(), 2);
    }

    #[test]
    fn plain_string_has_one_line_per_row() {
        let frame = frame_from(5, 4, |_, _| (0, 0, 0));
        let s = to_plain_string(&frame, &Options::default());
        assert_eq!(s.lines().count(), 4);
        assert!(s.lines().all(|l| l.chars().count() == 5));
    }
}

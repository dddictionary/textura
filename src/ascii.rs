//! Core luminance-to-glyph mapping.
//!
//! This is the hot path: at 30fps across a full terminal we map on the order of
//! a few hundred thousand pixels per second. Ramps are therefore expanded once
//! into a `Vec<char>` and indexed directly, rather than being re-decoded per
//! pixel.
//!
//! Glyphs are `char`, not `u8`, so ramps can use block and braille characters.
//! Indexing a UTF-8 byte slice would slice multi-byte sequences apart and
//! render mojibake.

/// Ramp workbench. Every ramp is ordered **dark to light**: the first glyph is
/// drawn for black, the last for white.
///
/// Pick one by editing the `RAMP` constant below.
pub mod ramps {
    /// The original ramp, with nine leading spaces to crush the darks.
    /// Balanced general purpose choice.
    pub const CLASSIC: &str = "         _.,-=+:;cba!?0123456789$W#@";

    /// Same glyphs, far more space padding. Dark regions go fully empty, which
    /// suits a webcam in dim light where sensor grain would otherwise speckle
    /// the background. High contrast, less midtone detail.
    pub const CLASSIC_HEAVY: &str =
        "                                _.,-=+:;cba!?0123456789$W#@";

    /// No padding at all - every luminance level maps to a visible glyph.
    /// Maximum tonal detail, but noisy: nothing ever reads as empty.
    pub const CLASSIC_TIGHT: &str = "_.,-=+:;cba!?0123456789$W#@";

    /// Classic reversed, for light backgrounds - dense glyphs mark *dark*
    /// areas instead of bright ones. Looks wrong on a dark terminal.
    pub const INVERTED: &str = "@#W$9876543210?!abc;:+=-,._         ";

    /// Ten evenly spaced steps. Clean and readable, a good default when
    /// CLASSIC feels mushy.
    pub const SHORT: &str = " .:-=+*#%@";

    /// The widely used 70-level ramp. Very smooth gradients, but many glyphs
    /// differ in weight between fonts, so it can look uneven.
    pub const LONG: &str =
        " .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

    /// Unicode block shading. Solid and poster-like rather than textual -
    /// usually the best looking option on a video feed.
    pub const BLOCKS: &str = " ░▒▓█";

    /// Block shading with padding, so dark areas stay empty.
    pub const BLOCKS_HEAVY: &str = "    ░▒▓█";

    /// Braille dot densities. Fine grained and slightly dithered looking.
    pub const BRAILLE: &str = " ⠁⠃⠇⠏⠟⠿⡿⣿";

    /// Five levels. Very graphic, almost a posterize effect.
    pub const MINIMAL: &str = " .oO@";

    /// Two levels. Pure threshold - works surprisingly well on high contrast
    /// subjects like a face lit from one side.
    pub const BINARY: &str = " 01";

    /// Punctuation only. Sparse and sketchy.
    pub const SKETCH: &str = "  .·,':;\"!|/\\(){}[]#";
}

// ─────────────────────────────────────────────────────────────────────────────
//  ACTIVE RAMP - uncomment exactly one line.
// ─────────────────────────────────────────────────────────────────────────────
pub const RAMP: &str = ramps::CLASSIC_HEAVY;
// pub const RAMP: &str = ramps::CLASSIC;
// pub const RAMP: &str = ramps::CLASSIC_TIGHT;
// pub const RAMP: &str = ramps::INVERTED;
// pub const RAMP: &str = ramps::SHORT;
// pub const RAMP: &str = ramps::LONG;
// pub const RAMP: &str = ramps::BLOCKS;
// pub const RAMP: &str = ramps::BLOCKS_HEAVY;
// pub const RAMP: &str = ramps::BRAILLE;
// pub const RAMP: &str = ramps::MINIMAL;
// pub const RAMP: &str = ramps::BINARY;
// pub const RAMP: &str = ramps::SKETCH;
// ─────────────────────────────────────────────────────────────────────────────

/// Every ramp in the workbench, as `(name, glyphs)`. Drives `--ramp` name
/// lookup on the CLI and the preset picker on the web build.
pub const PRESETS: &[(&str, &str)] = &[
    ("classic", ramps::CLASSIC),
    ("classic-heavy", ramps::CLASSIC_HEAVY),
    ("classic-tight", ramps::CLASSIC_TIGHT),
    ("inverted", ramps::INVERTED),
    ("short", ramps::SHORT),
    ("long", ramps::LONG),
    ("blocks", ramps::BLOCKS),
    ("blocks-heavy", ramps::BLOCKS_HEAVY),
    ("braille", ramps::BRAILLE),
    ("minimal", ramps::MINIMAL),
    ("binary", ramps::BINARY),
    ("sketch", ramps::SKETCH),
];

/// A ramp expanded for lookup.
///
/// Built once and reused, so the per-pixel cost is a multiply, a divide and an
/// index - no UTF-8 decoding in the hot path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ramp {
    glyphs: Vec<char>,
}

impl Ramp {
    /// Builds a ramp from glyphs ordered dark to light.
    ///
    /// Empty input falls back to [`RAMP`]: a ramp with no glyphs has nothing to
    /// draw, and silently rendering blank is worse than ignoring the request.
    pub fn new(glyphs: &str) -> Ramp {
        let glyphs: Vec<char> = glyphs.chars().collect();
        if glyphs.is_empty() {
            return Ramp {
                glyphs: RAMP.chars().collect(),
            };
        }
        Ramp { glyphs }
    }

    /// Looks up a ramp by preset name, case insensitively.
    pub fn preset(name: &str) -> Option<Ramp> {
        PRESETS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, glyphs)| Ramp::new(glyphs))
    }

    /// Number of glyphs.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Always false - a `Ramp` cannot be empty by construction.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Maps an 8-bit luminance sample to a glyph.
    #[inline]
    pub fn glyph(&self, luma: u8) -> char {
        // Rounded rescale of 0..=255 onto 0..=len-1, in u32 so nothing
        // overflows. `glyphs` is never empty, so `last` cannot underflow.
        let last = self.glyphs.len() as u32 - 1;
        let idx = (luma as u32 * last + 127) / 255;
        self.glyphs[idx as usize]
    }
}

impl Default for Ramp {
    fn default() -> Self {
        Ramp::new(RAMP)
    }
}

/// Rec. 601 luma, matching what ffmpeg's RGB24-to-GRAY8 conversion produces.
#[inline]
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

/// Darkest and brightest luma in a packed pixel buffer.
///
/// `stride` is the bytes per pixel - 3 for the RGB24 frames ffmpeg hands back,
/// 4 for the RGBA a browser canvas produces. Trailing bytes (alpha) are ignored.
pub fn luma_range(data: &[u8], stride: usize) -> (u8, u8) {
    debug_assert!(stride >= 3, "need at least three channels");
    let mut lo = u8::MAX;
    let mut hi = u8::MIN;
    for px in data.chunks_exact(stride) {
        let l = luma(px[0], px[1], px[2]);
        lo = lo.min(l);
        hi = hi.max(l);
    }
    // An empty buffer leaves the sentinels inverted; report a flat range so
    // `stretch` treats it as a no-op rather than underflowing.
    if lo > hi {
        return (0, 0);
    }
    (lo, hi)
}

/// Rescales `l` from `lo..=hi` onto the full 0..=255 range.
///
/// Without this, mid-tone-dominated footage collapses onto a handful of glyphs
/// and reads as flat - exactly what a webcam in ordinary indoor light produces.
#[inline]
pub fn stretch(l: u8, lo: u8, hi: u8) -> u8 {
    if hi <= lo {
        // Flat frame - leave it alone rather than amplifying sensor noise.
        return l;
    }
    let span = (hi - lo) as u32;
    ((l.saturating_sub(lo) as u32 * 255) / span).min(255) as u8
}

/// Converts a packed pixel buffer straight to newline-separated text.
///
/// `stride` is bytes per pixel (3 for RGB, 4 for RGBA). A short buffer is
/// padded with spaces rather than panicking, so a torn frame cannot crash the
/// render loop.
pub fn rgba_to_ascii(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    ramp: &Ramp,
    normalize: bool,
) -> String {
    let (lo, hi) = if normalize {
        luma_range(data, stride)
    } else {
        (0, 255)
    };

    let (w, h) = (width as usize, height as usize);
    // Glyphs may be multi-byte, so reserve by the widest one present.
    let mut out = String::with_capacity((w + 1) * h * 2);

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * stride;
            let ch = match data.get(i..i + 3) {
                Some(px) => ramp.glyph(stretch(luma(px[0], px[1], px[2]), lo, hi)),
                None => ' ',
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_spans_full_range() {
        let r = Ramp::default();
        let first = RAMP.chars().next().unwrap();
        let last = RAMP.chars().last().unwrap();
        assert_eq!(r.glyph(0), first, "black maps to the first glyph");
        assert_eq!(r.glyph(255), last, "white maps to the last glyph");
    }

    #[test]
    fn glyph_is_monotonic() {
        // Brighter input must never select an earlier glyph.
        let r = Ramp::new(ramps::CLASSIC);
        let glyphs: Vec<char> = ramps::CLASSIC.chars().collect();
        let idx_of = |l| {
            let g = r.glyph(l);
            glyphs.iter().rposition(|&c| c == g).unwrap()
        };
        let mut prev = idx_of(0);
        for l in 1..=255u8 {
            let cur = idx_of(l);
            assert!(cur >= prev, "ramp went backwards at luma {l}");
            prev = cur;
        }
    }

    #[test]
    fn every_preset_is_usable() {
        // The real point: multi-byte ramps must survive round-tripping. A
        // byte-indexed ramp would slice these apart into mojibake.
        for (name, glyphs) in PRESETS {
            let r = Ramp::new(glyphs);
            assert_eq!(
                r.len(),
                glyphs.chars().count(),
                "{name}: length must count chars, not bytes"
            );
            for l in 0..=255u8 {
                let g = r.glyph(l);
                assert!(
                    glyphs.contains(g),
                    "{name}: produced {g:?}, which is not in the ramp"
                );
            }
        }
    }

    #[test]
    fn multibyte_ramps_render_real_glyphs() {
        let r = Ramp::new(ramps::BLOCKS);
        assert_eq!(r.len(), 5, "' ░▒▓█' is five glyphs, not fifteen bytes");
        assert_eq!(r.glyph(0), ' ');
        assert_eq!(r.glyph(255), '█');
    }

    #[test]
    fn inverted_runs_the_other_way() {
        let r = Ramp::new(ramps::INVERTED);
        assert_eq!(r.glyph(0), '@', "black is dense on a light background");
        assert_eq!(r.glyph(255), ' ');
    }

    #[test]
    fn empty_ramp_falls_back_to_the_default() {
        assert_eq!(Ramp::new(""), Ramp::default());
    }

    #[test]
    fn preset_lookup_is_case_insensitive() {
        assert!(Ramp::preset("blocks").is_some());
        assert!(Ramp::preset("BLOCKS").is_some());
        assert!(Ramp::preset("nope").is_none());
    }

    #[test]
    fn glyph_stays_in_bounds_for_any_ramp_length() {
        // The +127 rounding term must never push the index past the last entry.
        for len in 1..64usize {
            let r = Ramp::new(&"x".repeat(len));
            for l in 0..=255u8 {
                r.glyph(l);
            }
        }
    }

    #[test]
    fn single_glyph_ramp_does_not_divide_by_zero() {
        let r = Ramp::new("x");
        assert_eq!(r.glyph(0), 'x');
        assert_eq!(r.glyph(255), 'x');
    }

    #[test]
    fn stretch_expands_a_narrow_range() {
        assert_eq!(stretch(100, 100, 140), 0);
        assert_eq!(stretch(140, 100, 140), 255);
    }

    #[test]
    fn stretch_leaves_a_flat_frame_alone() {
        assert_eq!(stretch(70, 70, 70), 70);
        assert_eq!(stretch(0, 200, 10), 0);
    }

    #[test]
    fn luma_range_handles_both_strides_and_empty_input() {
        let rgb = [0u8, 0, 0, 255, 255, 255];
        let rgba = [0u8, 0, 0, 7, 255, 255, 255, 9];
        assert_eq!(luma_range(&rgb, 3), (0, 255));
        assert_eq!(luma_range(&rgba, 4), (0, 255));
        assert_eq!(luma_range(&[], 4), (0, 0));
    }

    #[test]
    fn rgba_to_ascii_shape_and_content() {
        let data = vec![255u8; 2 * 2 * 4];
        let art = rgba_to_ascii(&data, 2, 2, 4, &Ramp::new(ramps::SHORT), false);
        assert_eq!(art, "@@\n@@\n");
    }

    #[test]
    fn rgba_to_ascii_tolerates_a_short_buffer() {
        let data = vec![255u8; 4];
        let art = rgba_to_ascii(&data, 4, 2, 4, &Ramp::default(), false);
        assert_eq!(art.lines().count(), 2);
        assert!(art.lines().all(|l| l.chars().count() == 4));
    }

    #[test]
    fn normalize_widens_the_glyph_spread() {
        let mut data = Vec::new();
        for i in 0..16u8 {
            let v = 120 + (i % 8);
            data.extend_from_slice(&[v, v, v, 255]);
        }
        let distinct = |s: String| {
            let mut v: Vec<char> = s.chars().filter(|c| *c != '\n').collect();
            v.sort_unstable();
            v.dedup();
            v.len()
        };
        let ramp = Ramp::new(ramps::CLASSIC);
        let flat = distinct(rgba_to_ascii(&data, 16, 1, 4, &ramp, false));
        let wide = distinct(rgba_to_ascii(&data, 16, 1, 4, &ramp, true));
        assert!(wide > flat, "normalize should widen spread ({flat} -> {wide})");
    }

    #[test]
    fn luma_matches_rec601_endpoints() {
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(luma(255, 255, 255), 255);
        assert!(luma(0, 255, 0) > luma(255, 0, 0));
        assert!(luma(255, 0, 0) > luma(0, 0, 255));
    }
}

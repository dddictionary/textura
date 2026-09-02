//! Core luminance-to-glyph mapping.
//!
//! This is the hot path: at 30fps across a full terminal we map on the order of
//! a few hundred thousand pixels per second, so the ramp is a plain byte slice
//! and lookup is a single multiply-divide rather than a per-pixel allocation.

/// Glyph ramp ordered dark to light: `RAMP[0]` is rendered for black, the last
/// entry for white.
///
/// The run of leading spaces is deliberate and inherited from the original
/// ramp - it crushes the bottom of the range so dark regions read as empty
/// rather than as noise, which matters on a webcam feed where sensor grain
/// would otherwise speckle the whole background.
pub const RAMP: &[u8] = b"                                _.,-=+:;cba!?0123456789$W#@";

/// Maps an 8-bit luminance sample to a glyph from `ramp`.
///
/// # Panics
/// Panics if `ramp` is empty.
#[inline]
pub fn glyph(luma: u8, ramp: &[u8]) -> u8 {
    assert!(!ramp.is_empty(), "glyph ramp must not be empty");
    // Rounded rescale of 0..=255 onto 0..=len-1, in u32 so nothing overflows.
    let last = ramp.len() as u32 - 1;
    let idx = (luma as u32 * last + 127) / 255;
    ramp[idx as usize]
}

/// Rec. 601 luma, matching what ffmpeg's RGB24-to-GRAY8 conversion produces.
/// Kept here so colour mode can derive a glyph from an RGB frame without
/// paying for a second swscale pass.
#[inline]
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8
}

/// Darkest and brightest luma in a packed pixel buffer.
///
/// `stride` is the bytes per pixel - 3 for the RGB24 frames ffmpeg hands back,
/// 4 for the RGBA a browser canvas produces. Any trailing bytes (alpha) are
/// ignored.
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

/// Converts a packed pixel buffer straight to newline-separated ASCII.
///
/// `stride` is bytes per pixel (3 for RGB, 4 for RGBA). The buffer must hold at
/// least `width * height * stride` bytes; anything beyond that is ignored.
///
/// This is the whole conversion in one call, which is what the WebAssembly
/// binding needs - the browser hands over a canvas buffer and wants text back.
pub fn rgba_to_ascii(
    data: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    ramp: &[u8],
    normalize: bool,
) -> String {
    let (lo, hi) = if normalize {
        luma_range(data, stride)
    } else {
        (0, 255)
    };

    let (w, h) = (width as usize, height as usize);
    let mut out = String::with_capacity((w + 1) * h);

    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * stride;
            // Tolerate a short buffer rather than panicking on a torn frame.
            let ch = match data.get(i..i + 3) {
                Some(px) => glyph(stretch(luma(px[0], px[1], px[2]), lo, hi), ramp),
                None => b' ',
            };
            out.push(ch as char);
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
        assert_eq!(glyph(0, RAMP), b' ', "black maps to the first ramp entry");
        assert_eq!(glyph(255, RAMP), b'@', "white maps to the last ramp entry");
    }

    #[test]
    fn glyph_is_monotonic() {
        // Brighter input must never select a less dense glyph.
        let idx_of = |l| {
            let g = glyph(l, RAMP);
            RAMP.iter().rposition(|&c| c == g).unwrap()
        };
        let mut prev = idx_of(0);
        for l in 1..=255u8 {
            let cur = idx_of(l);
            assert!(cur >= prev, "ramp went backwards at luma {l}");
            prev = cur;
        }
    }

    #[test]
    fn glyph_stays_in_bounds_for_any_ramp_length() {
        // The +127 rounding term must never push the index past the last entry.
        for len in 1..64usize {
            let ramp = vec![b'x'; len];
            for l in 0..=255u8 {
                glyph(l, &ramp);
            }
        }
    }

    #[test]
    fn single_entry_ramp_does_not_divide_by_zero() {
        assert_eq!(glyph(0, b"x"), b'x');
        assert_eq!(glyph(255, b"x"), b'x');
    }

    #[test]
    fn stretch_expands_a_narrow_range() {
        assert_eq!(stretch(100, 100, 140), 0);
        assert_eq!(stretch(140, 100, 140), 255);
    }

    #[test]
    fn stretch_leaves_a_flat_frame_alone() {
        // hi == lo must not divide by zero or blow the image out.
        assert_eq!(stretch(70, 70, 70), 70);
        assert_eq!(stretch(0, 200, 10), 0);
    }

    #[test]
    fn luma_range_handles_both_strides_and_empty_input() {
        // Same pixels, RGB vs RGBA, must agree - alpha is ignored.
        let rgb = [0u8, 0, 0, 255, 255, 255];
        let rgba = [0u8, 0, 0, 7, 255, 255, 255, 9];
        assert_eq!(luma_range(&rgb, 3), (0, 255));
        assert_eq!(luma_range(&rgba, 4), (0, 255));
        // Empty input must not leave the sentinels inverted.
        assert_eq!(luma_range(&[], 4), (0, 0));
    }

    #[test]
    fn rgba_to_ascii_shape_and_content() {
        // 2x2 white, RGBA.
        let data = vec![255u8; 2 * 2 * 4];
        let art = rgba_to_ascii(&data, 2, 2, 4, RAMP, false);
        assert_eq!(art, "@@\n@@\n");
        assert_eq!(art.lines().count(), 2);
    }

    #[test]
    fn rgba_to_ascii_tolerates_a_short_buffer() {
        // A torn frame must not panic - it should pad rather than index past
        // the end.
        let data = vec![255u8; 4];
        let art = rgba_to_ascii(&data, 4, 2, 4, RAMP, false);
        assert_eq!(art.lines().count(), 2);
        assert!(art.lines().all(|l| l.chars().count() == 4));
    }

    #[test]
    fn normalize_widens_the_glyph_spread() {
        // Every pixel in a narrow mid band, the low-contrast webcam case.
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
        let flat = distinct(rgba_to_ascii(&data, 16, 1, 4, RAMP, false));
        let wide = distinct(rgba_to_ascii(&data, 16, 1, 4, RAMP, true));
        assert!(wide > flat, "normalize should widen spread ({flat} -> {wide})");
    }

    #[test]
    fn luma_matches_rec601_endpoints() {
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(luma(255, 255, 255), 255);
        // Green dominates the weighting, blue contributes least.
        assert!(luma(0, 255, 0) > luma(255, 0, 0));
        assert!(luma(255, 0, 0) > luma(0, 0, 255));
    }
}

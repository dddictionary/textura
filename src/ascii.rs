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
    fn luma_matches_rec601_endpoints() {
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(luma(255, 255, 255), 255);
        // Green dominates the weighting, blue contributes least.
        assert!(luma(0, 255, 0) > luma(255, 0, 0));
        assert!(luma(255, 0, 0) > luma(0, 0, 255));
    }
}

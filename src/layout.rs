//! Fitting a picture onto a character grid.
//!
//! Terminal cells are roughly twice as tall as they are wide, so a naive
//! one-pixel-per-cell grid renders everything vertically squashed. Every size
//! decision therefore has to divide the row count by the cell aspect.

/// How many times taller than wide a character cell is. 2.0 is right for most
/// terminals at typical font settings; exposed so it can be tuned per setup.
pub const DEFAULT_CELL_ASPECT: f64 = 2.0;

/// Largest aspect-correct grid that fits inside `bounds`.
///
/// `src` is the source in pixels, `bounds` the available terminal area in
/// cells. The result never exceeds `bounds` in either axis and is at least
/// 1x1, so it is always safe to hand to a scaler.
pub fn fit_grid(src: (u32, u32), bounds: (u16, u16), cell_aspect: f64) -> (u32, u32) {
    let (sw, sh) = (src.0.max(1) as f64, src.1.max(1) as f64);
    let (bw, bh) = (bounds.0.max(1) as f64, bounds.1.max(1) as f64);

    // Width in cells if we let height be the limit, and vice versa; take
    // whichever keeps us inside both.
    let by_width = (bw, bw * sh / sw / cell_aspect);
    let by_height = (bh * cell_aspect * sw / sh, bh);

    let (w, h) = if by_width.1 <= bh { by_width } else { by_height };

    (
        (w.round() as u32).clamp(1, bounds.0.max(1) as u32),
        (h.round() as u32).clamp(1, bounds.1.max(1) as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_exceeds_the_bounds() {
        // Exhaustive over a range of shapes: the grid must always fit.
        for sw in [1u32, 16, 640, 1920, 4000] {
            for sh in [1u32, 9, 480, 1080, 3000] {
                for bw in [1u16, 10, 80, 200] {
                    for bh in [1u16, 5, 24, 60] {
                        let (w, h) = fit_grid((sw, sh), (bw, bh), DEFAULT_CELL_ASPECT);
                        assert!(w >= 1 && h >= 1, "grid collapsed for {sw}x{sh} in {bw}x{bh}");
                        assert!(
                            w <= bw as u32 && h <= bh as u32,
                            "{w}x{h} overflows bounds {bw}x{bh}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn corrects_for_tall_cells() {
        // A square source in a generous area should come out about twice as
        // wide as it is tall, so it *looks* square once rendered.
        let (w, h) = fit_grid((100, 100), (200, 200), 2.0);
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }

    #[test]
    fn a_cell_aspect_of_one_is_a_plain_fit() {
        // With square cells the correction disappears.
        let (w, h) = fit_grid((100, 100), (50, 200), 1.0);
        assert_eq!((w, h), (50, 50));
    }

    #[test]
    fn width_constrained_and_height_constrained_both_work() {
        // Wide source in a narrow box: width is the limit.
        let (w, h) = fit_grid((1920, 1080), (80, 200), 2.0);
        assert_eq!(w, 80);
        assert_eq!(h, (80.0 * 1080.0 / 1920.0 / 2.0f64).round() as u32);

        // Tall source in a short box: height is the limit.
        let (w2, h2) = fit_grid((1080, 1920), (200, 20), 2.0);
        assert_eq!(h2, 20);
        assert!(w2 <= 200);
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert_eq!(fit_grid((0, 0), (0, 0), 2.0), (1, 1));
        assert_eq!(fit_grid((1, 0), (1, 1), 2.0), (1, 1));
    }
}

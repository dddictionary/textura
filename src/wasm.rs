//! WebAssembly bindings.
//!
//! The browser replaces everything `source.rs` does natively: `getUserMedia`
//! is the capture device and `canvas.drawImage` is the scaler. By the time a
//! frame reaches here it is already at grid resolution, so this module only has
//! to do the part that is genuinely shared with the terminal build - turning
//! luminance into glyphs.
//!
//! Colour is deliberately *not* handled here. JavaScript already holds the
//! `ImageData` it passed in, so it can read per-cell colour directly without
//! paying to copy a second buffer back across the wasm boundary.

use wasm_bindgen::prelude::*;

use crate::ascii::{Ramp, PRESETS};
use crate::layout;


/// Converts frames to ASCII. Holds the ramp so it is not re-parsed per frame.
#[wasm_bindgen]
pub struct Converter {
    ramp: Ramp,
    normalize: bool,
}

#[wasm_bindgen]
impl Converter {
    /// Creates a converter using the `classic` ramp.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Converter {
        Converter {
            ramp: Ramp::default(),
            normalize: true,
        }
    }

    /// Sets the ramp to one of the named presets. Unknown names are ignored so
    /// a stale page cannot break the render loop.
    #[wasm_bindgen(js_name = setPreset)]
    pub fn set_preset(&mut self, name: &str) {
        if let Some(ramp) = Ramp::preset(name) {
            self.ramp = ramp;
        }
    }

    /// Sets an arbitrary ramp, ordered dark to light. Empty input is ignored -
    /// an empty ramp has no glyph to render.
    #[wasm_bindgen(js_name = setRamp)]
    pub fn set_ramp(&mut self, ramp: &str) {
        if !ramp.is_empty() {
            self.ramp = Ramp::new(ramp);
        }
    }

    /// Toggles per-frame contrast stretching.
    #[wasm_bindgen(js_name = setNormalize)]
    pub fn set_normalize(&mut self, on: bool) {
        self.normalize = on;
    }

    /// Converts one RGBA frame - straight from `ctx.getImageData().data` - into
    /// newline-separated ASCII.
    pub fn render(&self, rgba: &[u8], width: u32, height: u32) -> String {
        crate::ascii::rgba_to_ascii(rgba, width, height, 4, &self.ramp, self.normalize)
    }
}

impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

/// The available preset names, newline separated.
#[wasm_bindgen(js_name = presetNames)]
pub fn preset_names() -> String {
    PRESETS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Largest aspect-correct grid fitting inside `max_cols` x `max_rows`, as
/// `[cols, rows]`.
///
/// Shared with the terminal build so the browser and the CLI frame a picture
/// identically, including the cell aspect correction.
#[wasm_bindgen(js_name = fitGrid)]
pub fn fit_grid(src_w: u32, src_h: u32, max_cols: u32, max_rows: u32, cell_aspect: f64) -> Vec<u32> {
    let (w, h) = layout::fit_grid(
        (src_w, src_h),
        (max_cols.min(u16::MAX as u32) as u16, max_rows.min(u16::MAX as u32) as u16),
        cell_aspect,
    );
    vec![w, h]
}

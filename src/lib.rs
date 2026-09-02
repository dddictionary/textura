//! Convert images, video files, and a live camera feed into ASCII art.
//!
//! The pipeline is the same for every input: decode, downscale straight to the
//! character grid, map each cell's luminance to a glyph, draw. A still image is
//! simply a one-frame video.
//!
//! `ascii` and `layout` are the portable core and carry no I/O. Everything that
//! needs an operating system - ffmpeg decoding, the terminal UI - sits behind
//! the `native` feature, so the same core can be compiled to WebAssembly and
//! driven by a browser's camera instead.

pub mod ascii;
pub mod layout;

#[cfg(feature = "native")]
pub mod render;
#[cfg(feature = "native")]
pub mod source;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

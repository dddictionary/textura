//! Convert images, video files, and a live camera feed into ASCII art.
//!
//! The pipeline is the same for every input: decode with ffmpeg, let swscale
//! downscale straight to the character grid, map each cell's luminance to a
//! glyph, draw. A still image is simply a one-frame video.

pub mod ascii;
pub mod layout;
pub mod render;
pub mod source;

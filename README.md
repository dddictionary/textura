# ascii-rs

Convert images, video files, and a live camera feed into ASCII art, rendered in your terminal.

Inspired by [The Coding Train's ASCII art in p5.js](https://www.youtube.com/watch?v=55iwMYv8tGI).

## Usage

```bash
textura images/ferris.png          # display an image
textura clip.mp4                   # play a video as ASCII
textura --camera                   # live camera feed
textura --camera --color           # ...in 24-bit colour
textura images/cat.jpg -o cat.txt  # write to a file instead
```

Press `q`, `Esc`, or `Ctrl-C` to quit.

### Options

| Flag | Description |
|---|---|
| `--camera [DEVICE]` | Capture live. On macOS `DEVICE` is an AVFoundation index (default `0`). |
| `-o, --output <FILE>` | Write ASCII to a file instead of displaying it. |
| `--width <COLS>` | Column count when writing to a file (default 100). |
| `-c, --color` | 24-bit foreground colour per cell, sampled from the source. |
| `--no-normalize` | Disable per-frame contrast stretching. |
| `--ramp <GLYPHS>` | Custom glyph ramp, ordered dark to light. |
| `--size <WxH>` | Camera capture resolution, e.g. `1280x720`. |
| `--fps <FPS>` | Camera capture frame rate. |
| `--cell-aspect <RATIO>` | How many times taller than wide a cell is (default 2.0). |

## Streaming

The terminal window *is* the video source: run it fullscreen, then point a
screen-capture source at that window in OBS.

Colour mode looks better on stream, monochrome is cheaper to render. Contrast
normalization is on by default and matters a lot for webcam footage, which is
otherwise mid-tone dominated and reads as flat.

## In the browser

The core compiles to WebAssembly, so the same conversion runs on a web page
against a webcam. Live at [4brar.me/ascii](https://4brar.me/ascii).

ffmpeg is **not** part of that build — it cannot target the browser, and it does
not need to. The browser supplies everything `source.rs` does natively:

| Native | Browser |
| --- | --- |
| ffmpeg capture device | `getUserMedia()` |
| swscale downscale | `canvas.drawImage()` at grid size |
| RGB24 buffer | `getImageData()` → RGBA |

Frames arrive already at one pixel per character cell, so WebAssembly only maps
luminance to glyphs. That is why `ffmpeg-next`, `ratatui` and `clap` sit behind
the `native` feature: turning it off is what makes a wasm32 build possible.

```bash
nix develop -c ./scripts/build-wasm.sh    # -> web/pkg/
```

Output is a ~26KB `.wasm` plus its JS loader. The bundle exports `Converter`
(with `render`, `setPreset`, `setRamp`, `setNormalize`), `fitGrid`, and
`presetNames`.

Colour is deliberately left to the caller: the page already holds the pixel
buffer it passed in, so reading per-cell colour there is cheaper than copying a
second buffer back out of wasm.

> **Note**
> `wasm-bindgen` must match the `wasm-bindgen-cli` version exactly. The crate is
> pinned in `Cargo.toml` and the CLI comes from `flake.nix`; bump them together.

## Development

Requires [Nix](https://nixos.org/) with flakes enabled.

```bash
nix develop     # drops you into a shell with rustc + ffmpeg 8
cargo test
cargo run -- images/ferris.png
```

The dev shell re-execs into your login shell (override with `TEXTURA_SHELL`).
Note that flakes only see git-tracked files, so `git add` new sources before
building.

`ffmpeg-next`'s major version tracks FFmpeg's own, so the crate and the pinned
`ffmpeg_8` in `flake.nix` must be bumped together.

### Features

| Feature | Default | Pulls in |
| --- | --- | --- |
| `native` | yes | `ffmpeg-next`, `ratatui`, `clap` — decoding, TUI, CLI |

`--no-default-features` leaves only `ascii` and `layout`, the portable core with
no I/O. That is what the wasm build uses; the `textura` binary declares
`required-features = ["native"]` so cargo skips it automatically.

### Known gaps

- The v4l2 (Linux) and dshow (Windows) camera paths compile but have never been
  run. The frame-rate and capture-size defaults are macOS-specific and Linux
  will likely want its own.
- The interactive TUI loop is not covered by tests — it needs a real terminal.

## How it works

Every input is treated as a sequence of frames - a still image is just a
one-frame video - so all three modes share one decode path:

```
source ──> ffmpeg decode ──> swscale to the cell grid ──> luma ──> glyph ──> draw
```

Scaling directly to the character grid means one output pixel is exactly one
cell, which is also where the ~2:1 cell aspect correction is applied.

## TODO

- HTML embedding to render output on the web
- Dithering, and alternative ramps (block characters)
- Audio-reactive effects

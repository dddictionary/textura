use std::io::stdout;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
// Use ratatui's own crossterm rather than a second direct dependency, so the
// terminal-mode calls and the backend can never drift onto different versions.
use ratatui::crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size as terminal_size, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{prelude::*, widgets::Paragraph, Terminal};

use textura::ascii::{Ramp, PRESETS};
use textura::layout::{fit_grid, DEFAULT_CELL_ASPECT};
use textura::render::{self, Options};
use textura::source::{FrameSource, Source};

/// Convert images, video, and live camera input into ASCII art.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Image or video file to convert. Omit when using --camera.
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Capture from a live camera. Optionally takes a device spec; on macOS
    /// that is an AVFoundation index, e.g. `--camera 1`.
    #[arg(long, value_name = "DEVICE", num_args = 0..=1, default_missing_value = "0")]
    camera: Option<String>,

    /// Write ASCII to a file instead of displaying it.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Print one frame to stdout and exit, instead of opening the TUI. Handy
    /// for piping, and for comparing ramps side by side.
    #[arg(short, long)]
    print: bool,

    /// Column count for --print and --output. Defaults to the terminal width,
    /// or 100 when stdout is not a terminal.
    #[arg(long, value_name = "COLS")]
    width: Option<u16>,

    /// Emit 24-bit colour per cell, sampled from the source.
    #[arg(short, long)]
    color: bool,

    /// Disable per-frame contrast normalization.
    #[arg(long)]
    no_normalize: bool,

    /// Glyph ramp: a preset name (see --list-ramps) or literal glyphs ordered
    /// dark to light.
    #[arg(long, value_name = "NAME|GLYPHS")]
    ramp: Option<String>,

    /// List the built-in ramp presets and exit.
    #[arg(long)]
    list_ramps: bool,

    /// Camera capture resolution, e.g. 1280x720.
    #[arg(long, value_name = "WxH")]
    size: Option<String>,

    /// Camera capture frame rate.
    #[arg(long, value_name = "FPS")]
    fps: Option<u32>,

    /// How many times taller than wide a character cell is.
    #[arg(long, default_value_t = DEFAULT_CELL_ASPECT, value_name = "RATIO")]
    cell_aspect: f64,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WxH, got {s:?}"))?;
    let w = w.trim().parse().map_err(|_| format!("bad width in {s:?}"))?;
    let h = h.trim().parse().map_err(|_| format!("bad height in {s:?}"))?;
    Ok((w, h))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.list_ramps {
        list_ramps();
        return Ok(());
    }

    let source = match (&args.camera, &args.input) {
        (Some(spec), _) => Source::Camera {
            spec: spec.clone(),
            size: args.size.as_deref().map(parse_size).transpose()?,
            fps: args.fps,
        },
        (None, Some(path)) => Source::File(path.clone()),
        (None, None) => {
            eprintln!("error: provide a FILE or use --camera\n");
            eprintln!("  textura images/ferris.png");
            eprintln!("  textura --camera");
            std::process::exit(2);
        }
    };

    // A bare name selects a preset; anything else is taken literally, so
    // `--ramp " .:#"` still works.
    let ramp = match args.ramp.as_deref() {
        Some(spec) => Ramp::preset(spec).unwrap_or_else(|| Ramp::new(spec)),
        None => Ramp::default(),
    };

    let opts = Options {
        ramp,
        color: args.color,
        normalize: !args.no_normalize,
    };

    // Fall back to the terminal width, then to a sane fixed width when output
    // is redirected and there is no terminal to measure.
    let width = args
        .width
        .or_else(|| terminal_size().ok().map(|(cols, _)| cols))
        .unwrap_or(100);

    match (&args.output, args.print) {
        (Some(path), _) => {
            let art = convert_once(&source, &opts, width, args.cell_aspect)?;
            std::fs::write(path, &art)?;
            eprintln!("Wrote {} lines to {}", art.lines().count(), path.display());
            Ok(())
        }
        (None, true) => {
            print!("{}", convert_once(&source, &opts, width, args.cell_aspect)?);
            Ok(())
        }
        (None, false) => run_tui(&source, &opts, args.cell_aspect),
    }
}

/// Prints each preset with a luminance sweep, so the ramps can be compared at
/// a glance rather than by reading glyph strings.
fn list_ramps() {
    let width = 48;
    for (name, glyphs) in PRESETS {
        let ramp = Ramp::new(glyphs);
        let sweep: String = (0..width)
            .map(|i| ramp.glyph((i * 255 / (width - 1)) as u8))
            .collect();
        println!("{name:>14}  {sweep}");
    }
    println!("\n{:>14}  dark to light. Use a name or literal glyphs:", "");
    println!("{:>14}    textura --camera --ramp blocks", "");
    println!("{:>14}    textura --camera --ramp ' .:-=+*#%@'", "");
}

/// Opens a source, turning ffmpeg's terse errors into something actionable.
///
/// Camera failures are the common case and the least self-explanatory: ffmpeg
/// reports a bare I/O error when a device rejects the requested capture mode.
fn open_source(source: &Source, dst: (u32, u32)) -> Result<FrameSource, Box<dyn std::error::Error>> {
    FrameSource::open(source, dst).map_err(|e| -> Box<dyn std::error::Error> {
        match source {
            Source::Camera { spec, .. } => format!(
                "could not open camera {spec:?}: {e}\n\n\
                 List available devices with:\n  \
                 ffmpeg -f avfoundation -list_devices true -i \"\"\n\n\
                 If the device rejects the default mode, pin one it advertises:\n  \
                 textura --camera {spec} --fps 30 --size 1280x720"
            )
            .into(),
            Source::File(path) => format!("could not open {}: {e}", path.display()).into(),
        }
    })
}

/// One-shot conversion: decode the first frame and return it as text.
///
/// A still image is a one-frame video, so this covers images and the first
/// frame of a clip or camera alike.
fn convert_once(
    source: &Source,
    opts: &Options,
    width: u16,
    cell_aspect: f64,
) -> Result<String, Box<dyn std::error::Error>> {
    // Opened at a provisional size; the real grid needs the source aspect,
    // which is only known once the stream is open.
    let mut src = open_source(source, (width as u32, width as u32))?;
    let grid = fit_grid(src.source_size(), (width, u16::MAX), cell_aspect);
    src.resize(grid)?;

    let frame = src
        .next_frame()
        .ok_or("no frames decoded")?
        .map_err(|e| format!("decode failed: {e}"))?;

    Ok(render::to_plain_string(&frame, opts))
}

/// Restores the terminal. Safe to call more than once.
fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
}

fn run_tui(
    source: &Source,
    opts: &Options,
    cell_aspect: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open before touching the terminal, so a failure prints as ordinary
    // stderr rather than flashing past inside the alternate screen.
    let (cols, rows) = terminal_size()?;
    let src = open_source(source, (cols.max(1) as u32, rows.max(1) as u32))?;

    // Without this, a panic leaves the terminal in raw mode and the user's
    // shell is unusable afterwards.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        default_hook(info);
    }));

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = render_loop(&mut terminal, src, opts, cell_aspect);

    restore();
    terminal.show_cursor()?;
    result
}

fn render_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    mut src: FrameSource,
    opts: &Options,
    cell_aspect: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Fall back to 30fps when the stream does not declare a rate (still images,
    // some capture devices).
    let interval = src
        .frame_interval()
        .unwrap_or_else(|| Duration::from_millis(33));

    let mut last: Option<Text<'static>> = None;
    let mut ended = false;

    loop {
        let frame_start = Instant::now();

        if !ended {
            let area = terminal.size()?;
            let grid = fit_grid(src.source_size(), (area.width, area.height), cell_aspect);
            src.resize(grid)?;

            match src.next_frame() {
                Some(Ok(frame)) => last = Some(render::render(&frame, opts)),
                Some(Err(e)) => {
                    restore();
                    return Err(format!("decode failed: {e}").into());
                }
                // Stream finished. Keep the final frame on screen rather than
                // dropping the user back to a blank terminal.
                None => ended = true,
            }
        }

        if let Some(text) = &last {
            terminal.draw(|f| {
                let area = f.area();
                // `fit_grid` already sized the art to fill one axis; centre it
                // on the other so it sits in the middle of the window rather
                // than hugging the top edge.
                let art_height = (text.lines.len() as u16).min(area.height);
                let top = area.y + area.height.saturating_sub(art_height) / 2;
                let rect = Rect {
                    x: area.x,
                    y: top,
                    width: area.width,
                    height: art_height,
                };
                f.render_widget(Paragraph::new(text.clone()).centered(), rect);
            })?;
        }

        // Spend whatever is left of this frame's budget waiting for input. This
        // is what keeps the loop non-blocking: `poll` doubles as the frame
        // pacer and the key handler.
        let budget = if ended {
            // Nothing left to decode, so there is no rate to keep up with.
            Duration::from_millis(100)
        } else {
            interval.saturating_sub(frame_start.elapsed())
        };

        if event::poll(budget)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release && should_quit(key.code, key.modifiers) {
                    return Ok(());
                }
            }
        }
    }
}

fn should_quit(code: KeyCode, mods: KeyModifiers) -> bool {
    matches!(code, KeyCode::Char('q') | KeyCode::Esc)
        || (mods.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c')))
}

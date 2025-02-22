use crossterm::{event::{self, Event}, terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, execute};
use ratatui::{prelude::*, text::Text, Terminal};
use clap::Parser;
use std::{fs, io::{self, stdout}, path::PathBuf};

#[derive(Parser, Debug)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long, value_name="FILE")]
    path: PathBuf,
}


fn main() -> Result<(), Box<dyn std::error::Error>> {

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let args = Args::parse();
    let text = fs::read_to_string(&args.path).unwrap_or_else(|_| "Failed to read file".to_string());
    
    loop {
        terminal.draw(|frame| draw(frame, &text)).expect("failed to draw frame");

        if matches!(event::read().expect("failed to read event"), Event::Key(_)) {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}




fn draw(frame: &mut Frame, words: &str) {
    let text = Text::raw(words);
    frame.render_widget(text, frame.area());
}

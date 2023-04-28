use std::{env, process, io, thread, time::Duration};
use ascii_rs::Config;

use tui::{
    backend::CrosstermBackend,
    widgets::{Widget, Block, Borders},
    layout::{Layout, Constraint, Direction},
    Terminal
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

// use nokhwa::{
//     nokhwa_initialize,
//     pixel_format::{RgbAFormat, RgbFormat},
//     query,
//     utils::{ApiBackend, RequestedFormat, RequestedFormatType, CameraIndex},
//     Camera,
// };


fn tui_stuff() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default()
            .title("Block")
            .borders(Borders::ALL);
        f.render_widget(block, size);
    })?;

    thread::sleep(Duration::from_millis(5000));

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn main() {
    // let args: Vec<String> = env::args().collect();

    // let config = Config::build(&args).unwrap_or_else(|err| {
    //     eprintln!("Error occurred: {err}");
    //     process::exit(1);
    // });

    // if let Err(e) = ascii_rs::run(config) {
    //     eprintln!("Application error: {e}");
    //     process::exit(1)
    // }

    tui_stuff().unwrap();
    // let format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    // let camera_index = CameraIndex::Index(0);

    // let mut threaded = Camera::new(camera_index, format).unwrap();
    // threaded.open_stream().unwrap();
    // #[allow(clippy::empty_loop)] // keep it running
    // loop {
    //     let frame = threaded.frame().unwrap();
    //     let image = frame.decode_image::<RgbAFormat>().unwrap();
    //     println!(
    //         "{}x{} {} naripoggers",
    //         image.width(),
    //         image.height(),
    //         image.len()
    //     );
    // }


}

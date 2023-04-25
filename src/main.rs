use std::{env, process};

use ascii_rs::Config;

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = Config::build(&args).unwrap_or_else(|err| {
        eprintln!("Error occurred: {err}");
        process::exit(1);
    });

    if let Err(e) = ascii_rs::run(config) {
        eprintln!("Application error: {e}");
        process::exit(1)
    }
}

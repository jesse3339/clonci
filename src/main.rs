pub(crate) mod commands;
pub(crate) mod help;
pub(crate) mod shell;
pub(crate) mod state;

use std::process;

fn main() {
    if let Err(err) = commands::run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

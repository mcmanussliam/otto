pub mod app_error;
pub mod cli;
pub mod config;
pub mod envfile;
pub mod history;
pub mod model;
pub mod notify;
pub mod output;
pub mod runner;
pub mod version;

pub fn run() -> i32 {
    match cli::run_cli() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            err.code()
        }
    }
}

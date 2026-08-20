//! `madake` コマンド本体。

use std::process::ExitCode;

use clap::Parser;
use madake_cli::{run, Cli, HttpClient};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let client = match HttpClient::new(cli.port) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("madake: {e}");
            return ExitCode::FAILURE;
        }
    };
    match run(&cli, &client) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("madake: {e}");
            ExitCode::FAILURE
        }
    }
}

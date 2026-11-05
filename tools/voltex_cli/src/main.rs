//! voltex CLI — `new`, `build`, `run` subcommands. Each subcommand lives in
//! its own module; this file is dispatch + help text.

#![warn(missing_docs)]

use std::process::ExitCode;

mod build;
mod error;
mod help;
mod manifest;
mod new;
mod run;
mod util;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Top-level flags handled before subcommand dispatch.
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("voltex {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let result = match args.get(1).map(|s| s.as_str()) {
        Some("new") => run_subcommand(&args, help::NEW, new::cmd),
        Some("build") => run_subcommand(&args, help::BUILD, build::cmd),
        Some("run") => run_subcommand(&args, help::RUN, run::cmd),
        Some("--help") | Some("-h") | None => {
            print!("{}", help::TOP);
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("voltex: unknown subcommand `{}`\n", other);
            eprint!("{}", help::TOP);
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

/// If the subcommand args contain `--help` / `-h`, print the per-subcommand
/// help text and return success. Otherwise dispatch to the real handler.
fn run_subcommand(
    args: &[String],
    sub_help: &str,
    handler: impl FnOnce(&[String]) -> error::Result<()>,
) -> error::Result<()> {
    let sub_args = &args[2..];
    if sub_args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", sub_help);
        return Ok(());
    }
    handler(sub_args)
}

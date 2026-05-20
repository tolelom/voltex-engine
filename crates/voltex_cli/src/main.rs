//! voltex CLI — `new`, `build`, `run` subcommands. Each subcommand lives in
//! its own module; this file is dispatch + help text.

use std::process::ExitCode;

mod build;
mod error;
mod manifest;
mod new;
mod run;
mod util;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("new") => new::cmd(&args[2..]),
        Some("build") => build::cmd(&args[2..]),
        Some("run") => run::cmd(&args[2..]),
        Some("--help") | Some("-h") | None => {
            print_help();
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("voltex: unknown subcommand `{}`", other);
            print_help();
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

fn print_help() {
    eprintln!(
        "voltex — game engine CLI

USAGE:
    voltex new <name>                            Scaffold a new project
    voltex build [--target <triple>]             Build the project in cwd
    voltex run   [--target <triple>]             Build then launch the exe

OPTIONS (build/run):
    --manifest-path <path>    Use voltex.toml from <path> instead of cwd
    --target <triple>         windows-x64 | linux-x64 | macos-arm64 (default: host)
    --engine-path <repo>      Voltex engine repo root (otherwise VOLTEX_REPO env)
"
    );
}

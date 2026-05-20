//! `voltex run` — build then exec the produced executable.

use std::path::PathBuf;
use std::process::Command;

use crate::build;
use crate::error::{CliError, Result};
use crate::manifest::Manifest;

pub fn cmd(args: &[String]) -> Result<()> {
    // Re-use build's argument parsing by delegating.
    build::cmd(args)?;

    // Resolve the resulting exe path from manifest + current dir.
    let manifest_dir = if let Some(idx) = args.iter().position(|a| a == "--manifest-path") {
        PathBuf::from(args.get(idx + 1).ok_or(CliError::MissingArg("--manifest-path <path>"))?)
    } else {
        std::env::current_dir()?
    };
    let manifest_dir = manifest_dir.canonicalize()?;
    let manifest = Manifest::load_from_dir(&manifest_dir)?;
    let exe = manifest_dir.join("dist").join(if cfg!(windows) {
        format!("{}.exe", manifest.project.name)
    } else {
        manifest.project.name.clone()
    });
    eprintln!("voltex run: launching {}", exe.display());
    let status = Command::new(&exe).status().map_err(CliError::from)?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

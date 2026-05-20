//! User-facing CLI errors. `Display` impl writes the friendly form.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CliError {
    MissingArg(&'static str),
    ManifestNotFound(PathBuf),
    ManifestParse { line: usize, msg: String },
    MissingField { section: &'static str, field: &'static str },
    InvalidValue { field: String, got: String },
    #[allow(dead_code)] // reserved for VFS work in B2
    SceneNotFound(PathBuf),
    NewTargetExists(PathBuf),
    CargoMissing,
    CargoBuildFailed(i32),
    Io(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingArg(name) => {
                write!(f, "missing argument `{}`", name)
            }
            CliError::ManifestNotFound(path) => {
                write!(
                    f,
                    "voltex.toml not found at {} — run `voltex new <name>` to scaffold a project",
                    path.display()
                )
            }
            CliError::ManifestParse { line, msg } => {
                write!(f, "voltex.toml parse error on line {}: {}", line, msg)
            }
            CliError::MissingField { section, field } => {
                write!(
                    f,
                    "missing required field [{}] {} in voltex.toml",
                    section, field
                )
            }
            CliError::InvalidValue { field, got } => {
                write!(f, "invalid value for `{}`: got `{}`", field, got)
            }
            CliError::SceneNotFound(path) => {
                write!(f, "scene file not found: {}", path.display())
            }
            CliError::NewTargetExists(path) => {
                write!(
                    f,
                    "cannot create project: {} already exists",
                    path.display()
                )
            }
            CliError::CargoMissing => write!(
                f,
                "`cargo` not found in PATH — install rustup at https://rustup.rs"
            ),
            CliError::CargoBuildFailed(code) => {
                write!(f, "cargo build failed (exit code {})", code)
            }
            CliError::Io(s) => write!(f, "io error: {}", s),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

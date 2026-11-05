//! User-facing CLI errors. `Display` impl writes the friendly form.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CliError {
    MissingArg(&'static str),
    ManifestNotFound(PathBuf),
    ManifestParse {
        line: usize,
        /// 1-based character column of the offending token.
        column: usize,
        msg: String,
    },
    MissingField {
        section: &'static str,
        field: &'static str,
    },
    InvalidValue {
        field: String,
        got: String,
    },
    NewTargetExists(PathBuf),
    CargoMissing,
    CargoBuildFailed(i32),
    Io(String),
    /// User asked to build for a target that does not match the host triple.
    /// `voltex build` does not cross-compile.
    CrossCompileNotSupported {
        host: String,
        requested: String,
    },
    /// Manifest `targets = [...]` listed only non-host triples — we cannot
    /// build any of them from the current machine.
    HostNotInManifestTargets {
        host: String,
        listed: Vec<String>,
    },
    /// A key was found in `voltex.toml` that the engine does not recognise
    /// for that section. Suggests close matches when possible.
    UnknownField {
        section: String,
        field: String,
        valid: Vec<&'static str>,
        suggestion: Option<&'static str>,
    },
    /// A top-level section was found in `voltex.toml` that the engine does
    /// not recognise.
    UnknownSection {
        name: String,
        valid_prefixes: Vec<&'static str>,
    },
    /// A path referenced by the manifest (scene, scripts dir, assets dir)
    /// does not exist.
    ManifestPathMissing {
        field: &'static str,
        path: PathBuf,
        base_dir: PathBuf,
    },
    /// `cargo build` produced several executables and none is named after
    /// `[project] name` — shipping a guess could package an asset generator.
    AmbiguousExecutable {
        project: String,
        candidates: Vec<PathBuf>,
    },
}

// One arm per variant, in declaration order: each arm is the whole message.
impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingArg(name) => write!(f, "missing argument `{name}`"),
            CliError::ManifestNotFound(path) => write!(
                f,
                "voltex.toml not found at {} — run `voltex new <name>` to scaffold a project",
                path.display()
            ),
            CliError::ManifestParse { line, column, msg } => write!(
                f,
                "voltex.toml parse error on line {line}, column {column}: {msg}"
            ),
            CliError::MissingField { section, field } => {
                write!(f, "missing required field [{section}] {field} in voltex.toml")
            }
            CliError::InvalidValue { field, got } => {
                write!(f, "invalid value for `{field}`: got `{got}`")
            }
            CliError::NewTargetExists(path) => {
                write!(f, "cannot create project: {} already exists", path.display())
            }
            CliError::CargoMissing => write!(
                f,
                "`cargo` not found in PATH — install rustup at https://rustup.rs"
            ),
            CliError::CargoBuildFailed(code) => {
                write!(f, "cargo build failed (exit code {code})")
            }
            CliError::Io(s) => write!(f, "io error: {s}"),
            CliError::CrossCompileNotSupported { host, requested } => write!(
                f,
                "cross-compile not supported.\n  current host: {host}\n  requested target: {requested}\n  hint: run `voltex build --target {requested}` on a {requested} machine."
            ),
            CliError::HostNotInManifestTargets { host, listed } => write!(
                f,
                "manifest targets do not include host triple.\n  current host: {host}\n  manifest targets = {listed:?}\n  hint: add \"{host}\" to the targets array, or pass --target {host} explicitly."
            ),
            CliError::UnknownField {
                section,
                field,
                valid,
                suggestion,
            } => {
                let hint = match suggestion {
                    Some(s) => format!("\n  hint: did you mean `{s}`?"),
                    None => format!("\n  valid keys: {valid:?}"),
                };
                write!(f, "unknown key `{field}` in [{section}] section of voltex.toml{hint}")
            }
            CliError::UnknownSection {
                name,
                valid_prefixes,
            } => write!(
                f,
                "unknown section [{name}] in voltex.toml\n  valid sections: {valid_prefixes:?}"
            ),
            CliError::ManifestPathMissing {
                field,
                path,
                base_dir,
            } => write!(
                f,
                "manifest field `{field}` references missing path\n  path:     {}\n  base dir: {}\n  hint: create the file/directory, or remove the entry from voltex.toml",
                path.display(),
                base_dir.display()
            ),
            CliError::AmbiguousExecutable { project, candidates } => {
                let list: Vec<String> =
                    candidates.iter().map(|p| p.display().to_string()).collect();
                write!(
                    f,
                    "cargo built {} executables but none is named `{project}`\n  candidates: {}\n  hint: rename the game's [[bin]] to `{project}`, or set [project] name to the game binary's name",
                    candidates.len(),
                    list.join(", ")
                )
            }
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

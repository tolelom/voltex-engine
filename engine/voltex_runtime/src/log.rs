//! Minimal structured logger.
//!
//! Five levels (error/warn/info/debug/trace), one filter env var
//! (`VOLTEX_LOG`, defaults to `info`), optional file mirror. No external
//! dependency — uses `std::sync::OnceLock` + `std::sync::Mutex<File>`.
//!
//! Output format:
//! ```text
//! 2026-05-21T12:34:56Z [INFO ] voltex_runtime::app: GPU adapter: ...
//! ```
//!
//! Use the convenience macros so the level-filter check happens BEFORE
//! formatting the message — zero allocation when filtered out:
//!
//! ```ignore
//! use voltex_runtime::{log_info, log_error};
//! log_info!("starting up: {}", app_name);
//! log_error!("scene load failed: {}", err);
//! ```

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Log severity. Higher value = more verbose; the filter rejects messages
/// strictly *above* the configured level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Errors — almost-fatal conditions you always want logged.
    Error = 0,
    /// Warnings — recoverable misconfigurations.
    Warn = 1,
    /// Informational — startup/shutdown, infrequent state changes.
    Info = 2,
    /// Debug — verbose diagnostics, off by default.
    Debug = 3,
    /// Trace — extremely verbose tracing, almost always off.
    Trace = 4,
}

impl Level {
    /// Five-character padded label, e.g. `"INFO "`.
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    /// Parse a case-insensitive level name. Unknown → `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "warn" | "warning" => Some(Level::Warn),
            "info" => Some(Level::Info),
            "debug" => Some(Level::Debug),
            "trace" => Some(Level::Trace),
            _ => None,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label().trim())
    }
}

/// Global logger state. Initialised once via [`init`]; defaults to
/// `Level::Info` + stderr-only if not initialised explicitly.
struct LoggerState {
    level: Level,
    file: Option<Mutex<std::fs::File>>,
}

static LOGGER: OnceLock<LoggerState> = OnceLock::new();

/// Initialise the global logger. Idempotent — only the first call has
/// effect within a process.
///
/// `level`: lowest severity allowed through (e.g. `Level::Info` lets
/// Error / Warn / Info through, drops Debug / Trace).
///
/// `log_file`: when `Some(path)`, every record is mirrored into that
/// file in addition to stderr. The file is opened append-only; the
/// parent directory is created if missing.
pub fn init(level: Level, log_file: Option<PathBuf>) {
    let file = log_file.and_then(|p| open_log_file(&p).ok().map(Mutex::new));
    let _ = LOGGER.set(LoggerState { level, file });
}

/// Initialise from `VOLTEX_LOG` env var. Falls back to `Level::Info`
/// when the var is unset or invalid. No file mirror.
///
/// Examples:
/// ```bash
/// VOLTEX_LOG=debug ./my_game
/// VOLTEX_LOG=warn  ./my_game     # only error+warn
/// ```
pub fn init_from_env() {
    let level = std::env::var("VOLTEX_LOG")
        .ok()
        .and_then(|s| Level::parse(&s))
        .unwrap_or(Level::Info);
    init(level, None);
}

/// True iff `level` would be emitted. Useful for skipping expensive
/// argument formatting in caller code.
pub fn enabled(level: Level) -> bool {
    let st = LOGGER.get_or_init(|| LoggerState {
        level: Level::Info,
        file: None,
    });
    level <= st.level
}

/// Emit one record. Caller is expected to gate on [`enabled`] before
/// formatting if the arguments are expensive — the convenience macros
/// do this automatically.
pub fn log(level: Level, target: &str, message: &fmt::Arguments<'_>) {
    let st = LOGGER.get_or_init(|| LoggerState {
        level: Level::Info,
        file: None,
    });
    if level > st.level {
        return;
    }
    let line = format_record(crate::utc_time::unix_seconds_now(), level, target, message);
    // stderr — best effort, never panic.
    let _ = writeln!(std::io::stderr(), "{}", line.trim_end());
    if let Some(file) = &st.file {
        if let Ok(mut f) = file.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.write_all(b"\n");
            let _ = f.flush();
        }
    }
}

/// Public formatter — used by [`log`] and exposed for tests.
pub fn format_record(
    unix_seconds: i64,
    level: Level,
    target: &str,
    message: &fmt::Arguments<'_>,
) -> String {
    let (y, mo, d, h, mi, s) = crate::utc_time::unix_to_ymd_hms(unix_seconds);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z [{}] {}: {}",
        y,
        mo,
        d,
        h,
        mi,
        s,
        level.label(),
        target,
        message
    )
}

fn open_log_file(path: &Path) -> std::io::Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

// ── Public macros ──────────────────────────────────────────────────────────

/// Log at Error level. Skips argument formatting when filtered out.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        if $crate::log::enabled($crate::log::Level::Error) {
            $crate::log::log(
                $crate::log::Level::Error,
                module_path!(),
                &format_args!($($arg)*),
            );
        }
    }};
}

/// Log at Warn level. Skips argument formatting when filtered out.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        if $crate::log::enabled($crate::log::Level::Warn) {
            $crate::log::log(
                $crate::log::Level::Warn,
                module_path!(),
                &format_args!($($arg)*),
            );
        }
    }};
}

/// Log at Info level. Skips argument formatting when filtered out.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        if $crate::log::enabled($crate::log::Level::Info) {
            $crate::log::log(
                $crate::log::Level::Info,
                module_path!(),
                &format_args!($($arg)*),
            );
        }
    }};
}

/// Log at Debug level. Skips argument formatting when filtered out.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        if $crate::log::enabled($crate::log::Level::Debug) {
            $crate::log::log(
                $crate::log::Level::Debug,
                module_path!(),
                &format_args!($($arg)*),
            );
        }
    }};
}

/// Log at Trace level. Skips argument formatting when filtered out.
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {{
        if $crate::log::enabled($crate::log::Level::Trace) {
            $crate::log::log(
                $crate::log::Level::Trace,
                module_path!(),
                &format_args!($($arg)*),
            );
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_parse_known_strings() {
        assert_eq!(Level::parse("error"), Some(Level::Error));
        assert_eq!(Level::parse("WARN"), Some(Level::Warn));
        assert_eq!(Level::parse("Warning"), Some(Level::Warn));
        assert_eq!(Level::parse(" info "), Some(Level::Info));
        assert_eq!(Level::parse("debug"), Some(Level::Debug));
        assert_eq!(Level::parse("TRACE"), Some(Level::Trace));
    }

    #[test]
    fn level_parse_rejects_unknown() {
        assert_eq!(Level::parse(""), None);
        assert_eq!(Level::parse("verbose"), None);
        assert_eq!(Level::parse("xyzzy"), None);
    }

    #[test]
    fn level_ordering_more_verbose_is_greater() {
        assert!(Level::Trace > Level::Debug);
        assert!(Level::Debug > Level::Info);
        assert!(Level::Info > Level::Warn);
        assert!(Level::Warn > Level::Error);
    }

    #[test]
    fn level_label_is_five_chars_padded() {
        assert_eq!(Level::Error.label().len(), 5);
        assert_eq!(Level::Warn.label().len(), 5);
        assert_eq!(Level::Info.label().len(), 5);
        assert_eq!(Level::Debug.label().len(), 5);
        assert_eq!(Level::Trace.label().len(), 5);
    }

    #[test]
    fn format_record_has_iso8601_timestamp_and_level_and_target() {
        let msg = format_args!("hello {}", "world");
        let line = format_record(1779364800, Level::Info, "voltex::demo", &msg);
        assert!(line.starts_with("2026-05-21T12:00:00Z [INFO ]"));
        assert!(line.contains("voltex::demo:"));
        assert!(line.ends_with("hello world"));
    }

    #[test]
    fn format_record_preserves_level_label() {
        for lvl in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
            let line = format_record(0, lvl, "t", &format_args!("x"));
            assert!(line.contains(&format!("[{}]", lvl.label())));
        }
    }

    #[test]
    fn open_log_file_creates_parent_dir() {
        let tmp = std::env::temp_dir().join(format!(
            "voltex_runtime_log_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let log_path = tmp.join("nested").join("voltex.log");

        let mut f = open_log_file(&log_path).unwrap();
        writeln!(f, "test line").unwrap();
        drop(f);

        assert!(log_path.is_file());
        let body = std::fs::read_to_string(&log_path).unwrap();
        assert!(body.contains("test line"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

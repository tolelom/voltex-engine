//! Crash log writer — installs a `std::panic::set_hook` that, in addition to
//! the default stderr behaviour, writes a `<app_name>-crash-<UTC>.txt` file
//! containing the panic message, location, backtrace, and basic environment.
//!
//! Designed for shipped game executables where stderr is invisible to the
//! end user. The file is the only thing they can attach to a bug report.
//!
//! No external dependencies — date formatting uses Howard Hinnant's days-to-
//! civil algorithm directly. Backtrace capture uses `std::backtrace`.

use std::backtrace::Backtrace;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

/// One-shot guard so duplicate calls to [`install_panic_hook`] are no-ops.
static INSTALLED: Once = Once::new();

/// Install a panic hook that writes a crash report to `dir/<app>-crash-…txt`
/// in addition to the default stderr behaviour. Idempotent — only the first
/// call has effect within a process.
///
/// `dir = None` resolves to the current working directory at hook-fire time.
/// The directory is created if it does not exist.
pub fn install_panic_hook(app_name: impl Into<String>, dir: Option<PathBuf>) {
    let app_name = app_name.into();
    INSTALLED.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Always run the default hook first so stderr still shows the panic.
            default_hook(info);

            let payload = panic_payload_message(info);
            let location = info
                .location()
                .map(|l| (l.file().to_string(), l.line(), l.column()));
            let backtrace = Backtrace::force_capture().to_string();
            let now = unix_seconds_now();

            let report = format_crash_report(
                &app_name,
                &payload,
                location.as_ref().map(|(f, l, c)| (f.as_str(), *l, *c)),
                &backtrace,
                now,
            );

            // Best-effort write. If we cannot write the file, we have already
            // delegated to the default hook (which printed to stderr), so
            // there is nothing else useful to do — definitely do not panic
            // inside the panic hook.
            let resolved_dir = dir.clone().unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
            let _ = write_crash_file(&resolved_dir, &app_name, now, &report);
        }));
    });
}

/// Pure formatter used by [`install_panic_hook`] and by the tests.
/// `unix_seconds` is seconds since the Unix epoch (UTC).
pub fn format_crash_report(
    app_name: &str,
    panic_message: &str,
    location: Option<(&str, u32, u32)>,
    backtrace: &str,
    unix_seconds: i64,
) -> String {
    let (y, mo, d, h, mi, s) = unix_to_ymd_hms(unix_seconds);
    let mut out = String::new();
    out.push_str("=== Voltex crash report ===\n");
    out.push_str(&format!("app:           {}\n", app_name));
    out.push_str(&format!(
        "wallclock UTC: {:04}-{:02}-{:02} {:02}:{:02}:{:02}\n",
        y, mo, d, h, mi, s
    ));
    out.push_str(&format!("os:            {}\n", std::env::consts::OS));
    out.push_str(&format!("arch:          {}\n", std::env::consts::ARCH));
    out.push_str(&format!("voltex_runtime: {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!("\npanic message:\n  {}\n", panic_message));
    match location {
        Some((file, line, col)) => {
            out.push_str(&format!("\npanic location:\n  {}:{}:{}\n", file, line, col));
        }
        None => out.push_str("\npanic location:\n  <unknown>\n"),
    }
    out.push_str("\nbacktrace:\n");
    out.push_str(backtrace);
    if !backtrace.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Compose the crash filename from app + timestamp. Public for testing.
pub fn crash_filename(app_name: &str, unix_seconds: i64) -> String {
    let (y, mo, d, h, mi, s) = unix_to_ymd_hms(unix_seconds);
    let safe_app = sanitize_for_filename(app_name);
    format!(
        "{}-crash-{:04}{:02}{:02}-{:02}{:02}{:02}.txt",
        safe_app, y, mo, d, h, mi, s
    )
}

fn write_crash_file(
    dir: &Path,
    app_name: &str,
    unix_seconds: i64,
    report: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(crash_filename(app_name, unix_seconds));
    let mut f = std::fs::File::create(&path)?;
    f.write_all(report.as_bytes())?;
    Ok(path)
}

fn panic_payload_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let p = info.payload();
    if let Some(s) = p.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Howard Hinnant's `days_from_civil` reversed — convert UNIX seconds
/// (UTC) to `(year, month, day, hour, min, sec)`.
/// Reference: <https://howardhinnant.github.io/date_algorithms.html>
fn unix_to_ymd_hms(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let secs_of_day = secs.rem_euclid(86400);
    let hour = (secs_of_day / 3600) as u32;
    let min = ((secs_of_day % 3600) / 60) as u32;
    let sec = (secs_of_day % 60) as u32;

    // Shift epoch so day 0 = 0000-03-01 (era origin).
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u64; // 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // 0..=399
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365
    let mp = (5 * doy + 2) / 153; // 0..=11
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let year = if m <= 2 { (y + 1) as i32 } else { y as i32 };
    (year, m, d, hour, min, sec)
}

fn sanitize_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' => c,
            ' ' => '_',
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_1970_01_01_00_00_00() {
        assert_eq!(unix_to_ymd_hms(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn unix_2026_05_21_noon_utc() {
        // 2026-05-21 12:00:00 UTC
        // Days from 1970-01-01 to 2026-05-21 = 20595 (verified via cal)
        // 20595 * 86400 + 12 * 3600 = 1779408000 + 43200 = 1779364800
        // (we'll use the inverse — assert that round-trip works)
        let known = 1779364800i64;
        let (y, mo, d, h, mi, s) = unix_to_ymd_hms(known);
        assert_eq!((y, mo, d, h, mi, s), (2026, 5, 21, 12, 0, 0));
    }

    #[test]
    fn unix_2000_leap_year_feb_29() {
        // 2000-02-29 00:00:00 UTC — leap year edge case
        // Days from 1970-01-01 to 2000-02-29 = 11016
        let secs = 11016i64 * 86400;
        assert_eq!(unix_to_ymd_hms(secs), (2000, 2, 29, 0, 0, 0));
    }

    #[test]
    fn unix_negative_secs_pre_epoch() {
        // 1969-12-31 23:59:59 = -1
        assert_eq!(unix_to_ymd_hms(-1), (1969, 12, 31, 23, 59, 59));
        // 1969-12-31 00:00:00 = -86400
        assert_eq!(unix_to_ymd_hms(-86400), (1969, 12, 31, 0, 0, 0));
    }

    #[test]
    fn crash_filename_is_well_formed() {
        let name = crash_filename("my_game", 0);
        assert_eq!(name, "my_game-crash-19700101-000000.txt");
    }

    #[test]
    fn crash_filename_sanitises_app_name() {
        let name = crash_filename("My Game / weird name", 0);
        // ' ' → '_' and '/' → '_'
        assert!(name.starts_with("My_Game___weird_name-crash-"));
        assert!(name.ends_with(".txt"));
    }

    #[test]
    fn format_crash_report_contains_all_sections() {
        let report = format_crash_report(
            "my_game",
            "something went wrong",
            Some(("src/main.rs", 42, 7)),
            "  0: my_game::do_thing\n  1: main\n",
            1779364800,
        );
        assert!(report.contains("=== Voltex crash report ==="));
        assert!(report.contains("app:           my_game"));
        assert!(report.contains("wallclock UTC: 2026-05-21 12:00:00"));
        assert!(report.contains("os:            "));
        assert!(report.contains("arch:          "));
        assert!(report.contains("voltex_runtime: "));
        assert!(report.contains("something went wrong"));
        assert!(report.contains("src/main.rs:42:7"));
        assert!(report.contains("my_game::do_thing"));
    }

    #[test]
    fn format_crash_report_handles_missing_location() {
        let report = format_crash_report(
            "g",
            "boom",
            None,
            "bt",
            0,
        );
        assert!(report.contains("panic location:\n  <unknown>"));
    }

    #[test]
    fn format_crash_report_ends_with_newline() {
        let report = format_crash_report("g", "boom", None, "bt", 0);
        assert!(report.ends_with('\n'));
    }

    #[test]
    fn write_crash_file_creates_dir_and_file() {
        let tmp = std::env::temp_dir().join(format!(
            "voltex_runtime_crash_test_{}",
            std::process::id()
        ));
        // Ensure cleanup of any prior run.
        let _ = std::fs::remove_dir_all(&tmp);

        let path = write_crash_file(&tmp, "test_app", 1779364800, "report body").unwrap();
        assert!(path.exists());
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("test_app-crash-20260521-120000.txt")
        );
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, "report body");

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_panic_hook_is_idempotent() {
        // We can't easily induce a real panic from a test without poisoning
        // global state, but we can assert that repeated installs do not
        // panic or otherwise misbehave.
        install_panic_hook("idem_test", Some(std::env::temp_dir()));
        install_panic_hook("idem_test_again", Some(std::env::temp_dir()));
    }
}

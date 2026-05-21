//! Cross-cutting helpers shared by subcommands.

use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

/// Escape `s` so it can be embedded inside a Rust `"..."` literal.
pub fn escape_for_rust_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Canonicalize `p`, then strip the Windows `\\?\` verbatim prefix for
/// drive-letter paths so Cargo's `path = "..."` dependency parser can
/// consume the result.
///
/// `std::fs::canonicalize` on Windows always returns paths in the
/// extended-length `\\?\C:\...` form, which Cargo rejects as
/// `invalid path url '//?/C:\...'`. UNC paths (`\\?\UNC\server\share\...`)
/// are left untouched because rewriting them changes their meaning.
pub fn canonicalize_friendly(p: &Path) -> Result<PathBuf> {
    let canon = p.canonicalize().map_err(CliError::from)?;
    Ok(strip_verbatim_prefix(canon))
}

#[cfg(windows)]
fn strip_verbatim_prefix(p: PathBuf) -> PathBuf {
    let Some(s) = p.to_str() else { return p };
    let Some(rest) = s.strip_prefix(r"\\?\") else { return p };
    if rest.starts_with("UNC\\") {
        return p;
    }
    // Drive-letter form: "C:\..." — second byte is ':'.
    if rest.len() >= 2 && rest.as_bytes()[1] == b':' {
        return PathBuf::from(rest);
    }
    p
}

#[cfg(not(windows))]
fn strip_verbatim_prefix(p: PathBuf) -> PathBuf {
    p
}

pub fn copy_optional_file(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst).map_err(CliError::from)?;
    Ok(())
}

pub fn copy_optional_dir(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if !src.is_dir() {
        return Err(CliError::Io(format!(
            "{} exists but is not a directory",
            src.display()
        )));
    }
    copy_dir_recursive(src, dst)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ty.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_for_rust_string_handles_special_chars() {
        assert_eq!(escape_for_rust_string(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_for_rust_string("plain"), "plain");
    }

    #[test]
    fn copy_optional_file_missing_src_is_noop() {
        let src = std::env::temp_dir().join("voltex_cli_util_missing_src.txt");
        let dst = std::env::temp_dir().join("voltex_cli_util_missing_dst.txt");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
        assert!(copy_optional_file(&src, &dst).is_ok());
        assert!(!dst.exists());
    }

    #[cfg(windows)]
    #[test]
    fn strip_verbatim_prefix_strips_drive_letter() {
        let input = PathBuf::from(r"\\?\C:\foo\bar");
        assert_eq!(strip_verbatim_prefix(input), PathBuf::from(r"C:\foo\bar"));
    }

    #[cfg(windows)]
    #[test]
    fn strip_verbatim_prefix_leaves_unc_alone() {
        let input = PathBuf::from(r"\\?\UNC\server\share\foo");
        assert_eq!(strip_verbatim_prefix(input.clone()), input);
    }

    #[cfg(windows)]
    #[test]
    fn strip_verbatim_prefix_leaves_non_verbatim_alone() {
        let input = PathBuf::from(r"C:\foo");
        assert_eq!(strip_verbatim_prefix(input.clone()), input);
    }

    #[cfg(not(windows))]
    #[test]
    fn strip_verbatim_prefix_is_noop_on_unix() {
        let input = PathBuf::from("/foo/bar");
        assert_eq!(strip_verbatim_prefix(input.clone()), input);
    }

    #[test]
    fn canonicalize_friendly_returns_non_verbatim_path() {
        // Pre-create a real directory so canonicalize succeeds on every host.
        let dir = std::env::temp_dir().join("voltex_cli_canon_friendly");
        std::fs::create_dir_all(&dir).unwrap();
        let canon = canonicalize_friendly(&dir).unwrap();
        let s = canon.to_string_lossy();
        assert!(
            !s.starts_with(r"\\?\"),
            "canonicalize_friendly still returned verbatim path: {s}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_optional_dir_recurses() {
        let root = std::env::temp_dir().join("voltex_cli_util_copy_dir");
        let _ = std::fs::remove_dir_all(&root);
        let src = root.join("src");
        let dst = root.join("dst");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("a.txt"), "a").unwrap();
        std::fs::write(src.join("nested").join("b.txt"), "b").unwrap();

        copy_optional_dir(&src, &dst).unwrap();
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("nested").join("b.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}

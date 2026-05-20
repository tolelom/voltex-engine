//! Cross-cutting helpers shared by subcommands.

use std::path::Path;

use crate::error::{CliError, Result};

/// Escape `s` so it can be embedded inside a Rust `"..."` literal.
pub fn escape_for_rust_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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

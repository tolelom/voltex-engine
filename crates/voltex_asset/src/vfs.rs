//! Tiny virtual filesystem. Two backends: real disk and an embedded `.vpkg`
//! archive. Consumers (runtime, asset loader) read by relative path without
//! caring which backend they got.

use std::path::{Path, PathBuf};

use crate::error::{AssetError, Result};
use crate::vpkg::{VpkgEntry, VpkgReader};

pub enum Vfs {
    Filesystem(PathBuf),
    Archive(VpkgReaderOwned),
}

/// Owning version of `VpkgReader` so the runtime can drop the original byte
/// source (a `&'static [u8]` from `include_bytes!`) and keep reading.
pub struct VpkgReaderOwned {
    bytes: &'static [u8],
    entries: Vec<VpkgEntry>,
}

impl VpkgReaderOwned {
    fn from_static(bytes: &'static [u8]) -> Result<Self> {
        // Reuse the borrowed parser then materialise the entries.
        let reader = VpkgReader::parse(bytes)?;
        let entries = reader.entries().to_vec();
        Ok(Self { bytes, entries })
    }

    pub fn read(&self, path: &str) -> Option<&[u8]> {
        let entry = self.entries.iter().find(|e| e.path == path)?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        Some(&self.bytes[start..end])
    }

    pub fn list(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.path.as_str())
    }
}

impl Vfs {
    pub fn fs(base: impl Into<PathBuf>) -> Self {
        Vfs::Filesystem(base.into())
    }

    pub fn archive_from_bytes(bytes: &'static [u8]) -> Result<Self> {
        Ok(Vfs::Archive(VpkgReaderOwned::from_static(bytes)?))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        match self {
            Vfs::Filesystem(base) => std::fs::read(base.join(path)).map_err(AssetError::from),
            Vfs::Archive(reader) => reader
                .read(path)
                .map(|b| b.to_vec())
                .ok_or_else(|| AssetError::Generic(format!("vfs: missing `{}`", path))),
        }
    }

    pub fn exists(&self, path: &str) -> bool {
        match self {
            Vfs::Filesystem(base) => base.join(path).exists(),
            Vfs::Archive(reader) => reader.read(path).is_some(),
        }
    }

    pub fn list(&self) -> Vec<String> {
        match self {
            Vfs::Filesystem(base) => list_dir_recursive(base, base),
            Vfs::Archive(reader) => reader.list().map(|s| s.to_string()).collect(),
        }
    }
}

fn list_dir_recursive(base: &Path, dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Ok(ty) = entry.file_type() {
            if ty.is_dir() {
                out.extend(list_dir_recursive(base, &path));
            } else if ty.is_file() {
                if let Ok(rel) = path.strip_prefix(base) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpkg::write_vpkg;

    fn unique_root(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("voltex_asset_vfs_test_{}_{}", label, stamp))
    }

    #[test]
    fn filesystem_read_returns_bytes() {
        let root = unique_root("fs_read");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.txt"), "hello").unwrap();
        let vfs = Vfs::fs(&root);
        let data = vfs.read("a.txt").unwrap();
        assert_eq!(data, b"hello");
        assert!(vfs.exists("a.txt"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn filesystem_missing_path_errors() {
        let root = unique_root("fs_missing");
        std::fs::create_dir_all(&root).unwrap();
        let vfs = Vfs::fs(&root);
        assert!(vfs.read("nope.txt").is_err());
        assert!(!vfs.exists("nope.txt"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn archive_read_returns_bytes() {
        let entries = vec![("script.lua".to_string(), b"-- hi".to_vec())];
        let bytes: &'static [u8] = Box::leak(write_vpkg(&entries).into_boxed_slice());
        let vfs = Vfs::archive_from_bytes(bytes).unwrap();
        assert_eq!(vfs.read("script.lua").unwrap(), b"-- hi");
        assert!(vfs.exists("script.lua"));
    }

    #[test]
    fn archive_unknown_path_errors() {
        let bytes: &'static [u8] = Box::leak(write_vpkg(&[]).into_boxed_slice());
        let vfs = Vfs::archive_from_bytes(bytes).unwrap();
        assert!(vfs.read("missing.lua").is_err());
        assert!(!vfs.exists("missing.lua"));
    }

    #[test]
    fn vfs_exists_matches_read() {
        let root = unique_root("exists_match");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("b.txt"), "b").unwrap();
        let vfs = Vfs::fs(&root);
        assert_eq!(vfs.exists("sub/b.txt"), vfs.read("sub/b.txt").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn filesystem_list_returns_relative_paths() {
        let root = unique_root("fs_list");
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::write(root.join("scripts").join("a.lua"), "x").unwrap();
        std::fs::write(root.join("b.txt"), "y").unwrap();
        let vfs = Vfs::fs(&root);
        let mut listed = vfs.list();
        listed.sort();
        assert!(listed.contains(&"b.txt".to_string()));
        assert!(listed.contains(&"scripts/a.lua".to_string()));
        let _ = std::fs::remove_dir_all(&root);
    }
}

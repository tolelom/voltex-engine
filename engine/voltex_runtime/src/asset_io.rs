//! Shared asset file access for the runtime's path resolvers: where relative
//! paths are read from (Vfs → `asset_root` → cwd) and the canonical cache-key
//! spelling of a path.

use std::path::{Path, PathBuf};

/// Canonical cache-key form of an asset path: forward slashes, no leading
/// `./`. `with_audio_files` seeds, editor-persisted paths, and Vfs keys all
/// meet in the caches — without one spelling, the same file can occupy two
/// slots (double decode, hot-reload split). Loaders still receive the
/// original path (filesystem resolution handles either spelling).
pub(crate) fn normalize_path_key(path: &Path) -> PathBuf {
    let mut s = voltex_asset::vfs_key(&path.to_string_lossy());
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_string();
    }
    PathBuf::from(s)
}

/// Read asset bytes: the Vfs (when present **and** it has the entry) wins,
/// then a **relative** path is tried under `asset_root` (cwd-independent game
/// assets, friction #6), then plain fs as the last resort. The Vfs key is the
/// path's forward-slash string form — matching the editor's persisted format
/// (v1.0.99). Unlike [`normalize_path_key`], a leading `./` is **not**
/// stripped from the Vfs key.
///
/// Used for TexturePath / MeshPath / AudioPath. (Scene loading does not go
/// through here — it ignores `asset_root` — and `with_audio_files` uses
/// [`root_relative_if_file`].)
pub(crate) fn read_asset(
    vfs: Option<&voltex_asset::Vfs>,
    asset_root: Option<&Path>,
    path: &Path,
) -> Result<Vec<u8>, String> {
    if let Some(vfs) = vfs {
        let key = voltex_asset::vfs_key(&path.to_string_lossy());
        if vfs.exists(&key) {
            return vfs.read(&key).map_err(|e| e.to_string());
        }
    }
    if path.is_relative() {
        if let Some(root) = asset_root {
            let rooted = root.join(path);
            match std::fs::read(&rooted) {
                Ok(bytes) => return Ok(bytes),
                // 파일이 root에 존재하는데 다른 이유(권한/공유 위반)로 못
                // 읽었다면 그 에러를 그대로 — cwd 폴백은 진단을 어긋나게
                // 하고, cwd의 동명 파일을 잘못 로드할 수 있다.
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(format!("{}: {e}", rooted.display()));
                }
                Err(_) => {}
            }
        }
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

/// Boot-time rule for `RuntimeConfig::with_audio_files` paths: a relative
/// `path` becomes `asset_root/path` **only if that file exists**; otherwise
/// (absolute path, no root, or not under the root) `path` is returned as-is.
///
/// Deliberately not [`read_asset`]: there is no Vfs lookup, and an existing
/// but unreadable rooted file still wins here (the decode then reports it).
/// The returned path is also what the hot-reload watcher polls.
pub(crate) fn root_relative_if_file(asset_root: Option<&Path>, path: &Path) -> PathBuf {
    match asset_root {
        Some(root) if path.is_relative() && root.join(path).is_file() => root.join(path),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_asset_prefers_vfs_and_falls_back_to_fs() {
        let dir = std::env::temp_dir().join(format!("voltex_read_asset_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let loose = dir.join("loose.png");
        std::fs::write(&loose, b"from-fs").unwrap();

        // Archive vfs with one entry under the editor's canonical key form.
        let bytes = voltex_asset::write_vpkg(&[(
            "assets/hero.png".to_string(),
            b"from-vfs".to_vec(),
        )]);
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let vfs = voltex_asset::Vfs::archive_from_bytes(leaked).expect("vpkg parses");

        // In the archive → vfs wins (no fs file needed).
        assert_eq!(
            read_asset(Some(&vfs), None, Path::new("assets/hero.png")).unwrap(),
            b"from-vfs"
        );
        // Backslash spelling normalizes onto the same archive key.
        assert_eq!(
            read_asset(Some(&vfs), None, Path::new("assets\\hero.png")).unwrap(),
            b"from-vfs"
        );
        // Not in the archive → fs fallback.
        assert_eq!(read_asset(Some(&vfs), None, &loose).unwrap(), b"from-fs");
        // Present in neither → Err.
        assert!(read_asset(Some(&vfs), None, Path::new("missing.png")).is_err());
        // No vfs at all → plain fs behaviour.
        assert_eq!(read_asset(None, None, &loose).unwrap(), b"from-fs");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_asset_resolves_relative_paths_against_asset_root() {
        let root =
            std::env::temp_dir().join(format!("voltex_asset_root_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("textures")).unwrap();
        std::fs::write(root.join("textures/hero.png"), b"rooted").unwrap();

        // Relative path + asset root → resolves under the root, regardless of
        // cwd (the file does not exist relative to the test cwd).
        assert_eq!(
            read_asset(None, Some(&root), Path::new("textures/hero.png")).unwrap(),
            b"rooted"
        );
        // Absolute paths bypass the root.
        assert_eq!(
            read_asset(None, Some(&root), &root.join("textures/hero.png")).unwrap(),
            b"rooted"
        );
        // Missing everywhere → Err.
        assert!(read_asset(None, Some(&root), Path::new("textures/nope.png")).is_err());
        // Vfs still wins over the root for bundled entries.
        let bytes = voltex_asset::write_vpkg(&[(
            "textures/hero.png".to_string(),
            b"from-vfs".to_vec(),
        )]);
        let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let vfs = voltex_asset::Vfs::archive_from_bytes(leaked).expect("vpkg parses");
        assert_eq!(
            read_asset(Some(&vfs), Some(&root), Path::new("textures/hero.png")).unwrap(),
            b"from-vfs"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}

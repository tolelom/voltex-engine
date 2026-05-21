//! `.vpkg` — minimal archive format used by `voltex build` to embed scene +
//! scripts + assets inside the produced executable.
//!
//! ```text
//! magic        = b"VPKG"            (4 bytes)
//! version      = 1u32 LE            (4 bytes)
//! entry_count  = u32 LE             (4 bytes)
//! entries:
//!   path_len   u32 LE
//!   path       path_len UTF-8 bytes (forward slashes)
//!   offset     u32 LE
//!   size       u32 LE
//! blobs        concatenated, in entry order
//! ```
//!
//! No compression, no checksums — those belong in v1.1+. We deliberately keep
//! the format as boring as possible so any tooling can read it with `read +
//! parse_index` and no external deps.

use crate::error::{AssetError, Result};

/// Magic bytes at the start of every `.vpkg` archive.
pub const VPKG_MAGIC: &[u8; 4] = b"VPKG";
/// Format version this crate writes and accepts on parse.
pub const VPKG_VERSION: u32 = 1;

/// One file entry in a `.vpkg` index.
#[derive(Debug, Clone)]
pub struct VpkgEntry {
    /// Forward-slash-delimited relative path inside the archive.
    pub path: String,
    /// Absolute byte offset of this blob within the archive.
    pub offset: u32,
    /// Byte length of this blob.
    pub size: u32,
}

/// Build a `.vpkg` archive from `entries` (path, bytes) pairs. Order is
/// preserved.
pub fn write_vpkg(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    // First compute header / index size so we know absolute blob offsets.
    let mut header_size = 4 + 4 + 4; // magic + version + count
    for (path, _) in entries {
        header_size += 4 + path.len() + 4 + 4;
    }

    let total_blob_size: usize = entries.iter().map(|(_, b)| b.len()).sum();
    let mut out = Vec::with_capacity(header_size + total_blob_size);
    out.extend_from_slice(VPKG_MAGIC);
    out.extend_from_slice(&VPKG_VERSION.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    let mut current_offset = header_size as u32;
    for (path, bytes) in entries {
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path.as_bytes());
        out.extend_from_slice(&current_offset.to_le_bytes());
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        current_offset += bytes.len() as u32;
    }

    for (_, bytes) in entries {
        out.extend_from_slice(bytes);
    }
    out
}

/// Reader over a borrowed `.vpkg` byte slice.
#[derive(Debug)]
pub struct VpkgReader<'a> {
    bytes: &'a [u8],
    entries: Vec<VpkgEntry>,
}

impl<'a> VpkgReader<'a> {
    /// Parses the header + index. Returns `Err` if the magic/version is wrong
    /// or the index is truncated / refers to out-of-range blobs.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 12 {
            return Err(AssetError::Generic("vpkg too short for header".into()));
        }
        if &bytes[0..4] != VPKG_MAGIC {
            return Err(AssetError::Generic("vpkg bad magic".into()));
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != VPKG_VERSION {
            return Err(AssetError::Generic(format!(
                "vpkg version {} unsupported (expected {})",
                version, VPKG_VERSION
            )));
        }
        let count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        let mut entries = Vec::with_capacity(count);
        let mut cursor = 12usize;
        for _ in 0..count {
            if cursor + 4 > bytes.len() {
                return Err(AssetError::Generic("vpkg truncated path_len".into()));
            }
            let plen = u32::from_le_bytes([
                bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3],
            ]) as usize;
            cursor += 4;
            if cursor + plen + 8 > bytes.len() {
                return Err(AssetError::Generic("vpkg truncated entry".into()));
            }
            let path = std::str::from_utf8(&bytes[cursor..cursor + plen])
                .map_err(|_| AssetError::Generic("vpkg path is not UTF-8".into()))?
                .to_string();
            cursor += plen;
            let offset = u32::from_le_bytes([
                bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3],
            ]);
            cursor += 4;
            let size = u32::from_le_bytes([
                bytes[cursor], bytes[cursor + 1], bytes[cursor + 2], bytes[cursor + 3],
            ]);
            cursor += 4;
            entries.push(VpkgEntry { path, offset, size });
        }
        // Validate every blob region is within the archive.
        for e in &entries {
            let end = e.offset as usize + e.size as usize;
            if end > bytes.len() {
                return Err(AssetError::Generic(format!(
                    "vpkg blob out of range for `{}`",
                    e.path
                )));
            }
        }
        Ok(Self { bytes, entries })
    }

    /// Borrows the parsed entry list, in original order.
    pub fn entries(&self) -> &[VpkgEntry] {
        &self.entries
    }

    /// Iterates over all entry paths.
    pub fn list(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.path.as_str())
    }

    /// Returns the blob bytes for `path`, or `None` if not in the archive.
    pub fn read(&self, path: &str) -> Option<&[u8]> {
        let entry = self.entries.iter().find(|e| e.path == path)?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        Some(&self.bytes[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_file() {
        let entries = vec![("hello.txt".to_string(), b"Hello, world!".to_vec())];
        let archive = write_vpkg(&entries);
        let reader = VpkgReader::parse(&archive).unwrap();
        assert_eq!(reader.entries().len(), 1);
        assert_eq!(reader.read("hello.txt"), Some(b"Hello, world!".as_ref()));
    }

    #[test]
    fn roundtrip_multiple_files_preserves_order() {
        let entries = vec![
            ("a.txt".to_string(), b"aaa".to_vec()),
            ("b/c.txt".to_string(), b"bbb".to_vec()),
            ("d/e/f.txt".to_string(), b"ccc".to_vec()),
        ];
        let archive = write_vpkg(&entries);
        let reader = VpkgReader::parse(&archive).unwrap();
        let paths: Vec<&str> = reader.list().collect();
        assert_eq!(paths, vec!["a.txt", "b/c.txt", "d/e/f.txt"]);
        assert_eq!(reader.read("b/c.txt"), Some(b"bbb".as_ref()));
        assert_eq!(reader.read("d/e/f.txt"), Some(b"ccc".as_ref()));
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut bytes = write_vpkg(&[]);
        bytes[0] = b'X';
        let err = VpkgReader::parse(&bytes).unwrap_err();
        matches!(err, AssetError::Generic(_));
    }

    #[test]
    fn parse_rejects_truncated_index() {
        let bytes = vec![b'V', b'P', b'K', b'G', 1, 0, 0, 0, 5, 0, 0, 0]; // 5 entries but no data
        let err = VpkgReader::parse(&bytes).unwrap_err();
        matches!(err, AssetError::Generic(_));
    }

    #[test]
    fn empty_archive_has_zero_entries() {
        let archive = write_vpkg(&[]);
        let reader = VpkgReader::parse(&archive).unwrap();
        assert_eq!(reader.entries().len(), 0);
        assert!(reader.read("anything").is_none());
    }

    #[test]
    fn read_unknown_path_returns_none() {
        let archive = write_vpkg(&[("only.txt".to_string(), b"x".to_vec())]);
        let reader = VpkgReader::parse(&archive).unwrap();
        assert!(reader.read("missing.txt").is_none());
    }
}

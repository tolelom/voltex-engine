//! `.vpkg` archive robustness: write/parse round-trips random archives, and
//! the parser never panics or aborts on corrupted or hostile bytes (it reads
//! game packages that may be truncated on disk or tampered with).

use voltex_asset::vpkg::{write_vpkg, VpkgReader};

struct XorShift(u32);
impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn random_archive(rng: &mut XorShift) -> Vec<(String, Vec<u8>)> {
    (0..rng.next() % 12)
        .map(|i| {
            let path = format!("assets/{i}/file_{}.bin", rng.next() % 1000);
            let data = (0..rng.next() % 300).map(|_| rng.next() as u8).collect();
            (path, data)
        })
        .collect()
}

#[test]
fn write_then_parse_round_trips() {
    let mut rng = XorShift(0x00F0_0D01);
    for case in 0..500 {
        let entries = random_archive(&mut rng);
        let bytes = write_vpkg(&entries);
        let reader = VpkgReader::parse(&bytes).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(reader.entries().len(), entries.len(), "case {case}");
        for (path, data) in &entries {
            // Duplicate paths resolve to the first entry.
            let first = entries.iter().find(|(p, _)| p == path).unwrap();
            assert_eq!(reader.read(path), Some(first.1.as_slice()), "case {case}: {path}");
            let _ = data;
        }
    }
}

#[test]
fn huge_declared_entry_count_is_rejected_without_allocating() {
    // 12-byte file claiming 4 billion entries: the parser pre-allocated
    // `count` entries (~160 GB) before reading any of them and aborted.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&write_vpkg(&[])[0..8]); // magic + version
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(VpkgReader::parse(&bytes).is_err());
}

#[test]
fn parser_survives_corrupted_archives() {
    let mut rng = XorShift(0xBAD_F11E);
    for _ in 0..20_000 {
        let mut bytes = write_vpkg(&random_archive(&mut rng));
        for _ in 0..1 + rng.next() % 6 {
            let i = rng.next() as usize % bytes.len();
            bytes[i] = rng.next() as u8;
        }
        bytes.truncate(rng.next() as usize % (bytes.len() + 1));
        if let Ok(reader) = VpkgReader::parse(&bytes) {
            let paths: Vec<String> = reader.list().map(str::to_string).collect();
            for p in paths {
                let _ = reader.read(&p);
            }
        }
    }
}

//! Progressive JPEG support (SOF2 — successive scans).
//!
//! Currently unwired — the baseline JPEG decoder in `jpg/` handles SOF0
//! only. These items are the planned API surface for progressive decoding
//! and are intentionally `pub` so a future progressive decoder (in this
//! crate or downstream) can wire them up without an API churn. F8 left
//! them `pub` for the same reason — see audit note in `docs/dev/AUDIT-BACKLOG.md`.

/// Progressive JPEG scan parameters.
#[derive(Debug, Clone)]
pub struct ScanParams {
    /// Spectral selection start (DCT coefficient index).
    pub ss: u8,
    /// Spectral selection end (DCT coefficient index).
    pub se: u8,
    /// Successive approximation bit position high.
    pub ah: u8,
    /// Successive approximation bit position low.
    pub al: u8,
    /// Component indices included in this scan.
    pub components: Vec<u8>,
}

/// Detect if a JPEG is progressive (SOF2 marker present).
pub fn is_progressive_jpeg(data: &[u8]) -> bool {
    // Scan for SOF2 marker (0xFF 0xC2)
    for i in 0..data.len().saturating_sub(1) {
        if data[i] == 0xFF && data[i + 1] == 0xC2 {
            return true;
        }
    }
    false
}

/// Parse SOS markers to extract scan parameters.
pub fn parse_scan_params(data: &[u8]) -> Vec<ScanParams> {
    let mut scans = Vec::new();
    let mut i = 0;
    while i < data.len().saturating_sub(1) {
        if data[i] == 0xFF && data[i + 1] == 0xDA {
            // SOS marker found
            if i + 2 < data.len() {
                let len = ((data[i + 2] as usize) << 8) | data[i + 3] as usize;
                if i + 2 + len <= data.len() {
                    let num_components = data[i + 4] as usize;
                    let param_offset = i + 4 + 1 + num_components * 2;
                    if param_offset + 2 < data.len() {
                        let ss = data[param_offset];
                        let se = data[param_offset + 1];
                        let ah_al = data[param_offset + 2];
                        let ah = ah_al >> 4;
                        let al = ah_al & 0x0F;
                        let mut components = Vec::new();
                        for c in 0..num_components {
                            components.push(data[i + 5 + c * 2]);
                        }
                        scans.push(ScanParams {
                            ss,
                            se,
                            ah,
                            al,
                            components,
                        });
                    }
                }
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    scans
}

/// A 64-element DCT coefficient block.
pub type DctBlock = [i32; 64];

/// Accumulate a scan's coefficients into the coefficient buffer.
/// For first DC scan (ss=0, ah=0): set DC coefficient
/// For refinement (ah>0): refine existing coefficient precision
/// For AC scan: set AC coefficients in range [ss..=se]
pub fn merge_scan_coefficients(block: &mut DctBlock, scan_block: &DctBlock, params: &ScanParams) {
    if params.ah == 0 {
        // First scan for this range: copy coefficients
        for i in params.ss as usize..=params.se as usize {
            if i < 64 {
                block[i] = scan_block[i] << params.al;
            }
        }
    } else {
        // Refinement: add lower bits
        for i in params.ss as usize..=params.se as usize {
            if i < 64 {
                let bit = (scan_block[i] & 1) << params.al;
                if block[i] > 0 {
                    block[i] |= bit;
                } else if block[i] < 0 {
                    block[i] -= bit;
                } else if bit != 0 {
                    block[i] = bit;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_progressive_sof2() {
        // Minimal JPEG-like data with SOF2 marker
        let data = [0xFF, 0xD8, 0xFF, 0xC2, 0x00, 0x0B];
        assert!(is_progressive_jpeg(&data));
    }

    #[test]
    fn test_is_not_progressive_sof0() {
        // SOF0 (baseline)
        let data = [0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B];
        assert!(!is_progressive_jpeg(&data));
    }

    #[test]
    fn test_parse_baseline_scan() {
        // Minimal SOS with Ss=0, Se=63, Ah=0, Al=0
        // SOS marker: FF DA, length=0x0C, 3 components, then Ss Se AhAl
        let mut data = vec![0xFF, 0xD8]; // SOI
        data.extend_from_slice(&[0xFF, 0xDA]); // SOS marker
        data.extend_from_slice(&[0x00, 0x0C]); // length = 12
        data.push(3); // num_components
                      // 3 components: (id, table)
        data.extend_from_slice(&[1, 0x00, 2, 0x11, 3, 0x11]);
        data.push(0); // Ss = 0
        data.push(63); // Se = 63
        data.push(0); // Ah=0, Al=0

        let scans = parse_scan_params(&data);
        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].ss, 0);
        assert_eq!(scans[0].se, 63);
        assert_eq!(scans[0].ah, 0);
        assert_eq!(scans[0].al, 0);
    }

    #[test]
    fn test_merge_first_scan_dc() {
        let mut block = [0i32; 64];
        let mut scan_block = [0i32; 64];
        scan_block[0] = 100; // DC coefficient
        let params = ScanParams {
            ss: 0,
            se: 0,
            ah: 0,
            al: 2,
            components: vec![1],
        };
        merge_scan_coefficients(&mut block, &scan_block, &params);
        assert_eq!(block[0], 100 << 2); // shifted by Al
    }

    #[test]
    fn test_merge_ac_scan() {
        let mut block = [0i32; 64];
        let mut scan_block = [0i32; 64];
        scan_block[1] = 50;
        scan_block[5] = 30;
        let params = ScanParams {
            ss: 1,
            se: 5,
            ah: 0,
            al: 0,
            components: vec![1],
        };
        merge_scan_coefficients(&mut block, &scan_block, &params);
        assert_eq!(block[1], 50);
        assert_eq!(block[5], 30);
        assert_eq!(block[0], 0); // DC not touched
        assert_eq!(block[6], 0); // outside range
    }

    #[test]
    fn test_empty_data() {
        assert!(!is_progressive_jpeg(&[]));
        assert!(parse_scan_params(&[]).is_empty());
    }
}

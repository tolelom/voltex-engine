//! OGG container parser.
//!
//! Parses OGG bitstream pages and extracts Vorbis packets.
//! Reference: <https://www.xiph.org/ogg/doc/framing.html>

/// An OGG page header.
#[derive(Debug, Clone)]
pub struct OggPage {
    /// Header type flags (0x01 = continuation, 0x02 = BOS, 0x04 = EOS).
    pub header_type: u8,
    /// Granule position (PCM sample position).
    pub granule_position: u64,
    /// Bitstream serial number.
    pub serial: u32,
    /// Page sequence number.
    pub page_sequence: u32,
    /// Number of segments in this page.
    pub segment_count: u8,
    /// The segment table (each entry is a segment length, 0..255).
    pub segment_table: Vec<u8>,
    /// Raw packet data of this page (concatenated segments).
    pub data: Vec<u8>,
}

/// Parse all OGG pages from raw bytes.
pub fn parse_ogg_pages(data: &[u8]) -> crate::Result<Vec<OggPage>> {
    let mut pages = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        if offset + 27 > data.len() {
            break;
        }

        // Capture pattern "OggS"
        if &data[offset..offset + 4] != b"OggS" {
            return Err(format!("Invalid OGG capture pattern at offset {}", offset).into());
        }

        let version = data[offset + 4];
        if version != 0 {
            return Err(format!("Unsupported OGG version: {}", version).into());
        }

        let header_type = data[offset + 5];

        let granule_position = u64::from_le_bytes([
            data[offset + 6],
            data[offset + 7],
            data[offset + 8],
            data[offset + 9],
            data[offset + 10],
            data[offset + 11],
            data[offset + 12],
            data[offset + 13],
        ]);

        let serial = u32::from_le_bytes([
            data[offset + 14],
            data[offset + 15],
            data[offset + 16],
            data[offset + 17],
        ]);

        let page_sequence = u32::from_le_bytes([
            data[offset + 18],
            data[offset + 19],
            data[offset + 20],
            data[offset + 21],
        ]);

        // CRC at offset+22..+26 (skip verification for simplicity)

        let segment_count = data[offset + 26] as usize;

        if offset + 27 + segment_count > data.len() {
            return Err("OGG page segment table extends beyond data".into());
        }

        let segment_table: Vec<u8> = data[offset + 27..offset + 27 + segment_count].to_vec();

        let total_data_size: usize = segment_table.iter().map(|&s| s as usize).sum();
        let data_start = offset + 27 + segment_count;

        if data_start + total_data_size > data.len() {
            return Err("OGG page data extends beyond file".into());
        }

        let page_data = data[data_start..data_start + total_data_size].to_vec();

        pages.push(OggPage {
            header_type,
            granule_position,
            serial,
            page_sequence,
            segment_count: segment_count as u8,
            segment_table,
            data: page_data,
        });

        offset = data_start + total_data_size;
    }

    if pages.is_empty() {
        return Err("No OGG pages found".into());
    }

    Ok(pages)
}

/// Extract Vorbis packets from parsed OGG pages.
///
/// Packets can span multiple segments (segment length = 255 means continuation).
/// Packets can also span multiple pages (header_type bit 0x01 = continuation).
pub fn extract_packets(pages: &[OggPage]) -> crate::Result<Vec<Vec<u8>>> {
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut current_packet: Vec<u8> = Vec::new();

    for page in pages {
        let mut data_offset = 0;

        for (seg_idx, &seg_len) in page.segment_table.iter().enumerate() {
            let seg_data = &page.data[data_offset..data_offset + seg_len as usize];
            current_packet.extend_from_slice(seg_data);
            data_offset += seg_len as usize;

            // A segment length < 255 terminates the current packet.
            // A segment length of exactly 255 means the packet continues in the next segment.
            if seg_len < 255 {
                if !current_packet.is_empty() {
                    packets.push(std::mem::take(&mut current_packet));
                }
            }
            // If seg_len == 255 and this is the last segment of the page,
            // the packet continues on the next page.
            let _ = seg_idx; // suppress unused warning
        }
    }

    // If there's remaining data in current_packet (ended with 255-byte segments
    // and no terminating segment), flush it as a final packet.
    if !current_packet.is_empty() {
        packets.push(current_packet);
    }

    Ok(packets)
}

/// Convenience function: parse OGG container and extract all Vorbis packets.
pub fn parse_ogg(data: &[u8]) -> crate::Result<Vec<Vec<u8>>> {
    let pages = parse_ogg_pages(data)?;
    extract_packets(&pages)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal OGG page from raw packet data.
    fn build_ogg_page(
        header_type: u8,
        granule: u64,
        serial: u32,
        page_seq: u32,
        packets_data: &[&[u8]],
    ) -> Vec<u8> {
        // Build segment table and concatenated data
        let mut segment_table = Vec::new();
        let mut page_data = Vec::new();

        for (i, packet) in packets_data.iter().enumerate() {
            let len = packet.len();
            // Write full 255-byte segments
            let full_segments = len / 255;
            let remainder = len % 255;

            for _ in 0..full_segments {
                segment_table.push(255u8);
            }
            // Terminating segment (< 255), even if 0 to signal end of packet
            segment_table.push(remainder as u8);

            page_data.extend_from_slice(packet);
        }

        let segment_count = segment_table.len();
        let mut out = Vec::new();

        // Capture pattern
        out.extend_from_slice(b"OggS");
        // Version
        out.push(0);
        // Header type
        out.push(header_type);
        // Granule position
        out.extend_from_slice(&granule.to_le_bytes());
        // Serial
        out.extend_from_slice(&serial.to_le_bytes());
        // Page sequence
        out.extend_from_slice(&page_seq.to_le_bytes());
        // CRC (dummy zeros)
        out.extend_from_slice(&[0u8; 4]);
        // Segment count
        out.push(segment_count as u8);
        // Segment table
        out.extend_from_slice(&segment_table);
        // Data
        out.extend_from_slice(&page_data);

        out
    }

    #[test]
    fn parse_single_page() {
        let packet = b"hello vorbis";
        let page_bytes = build_ogg_page(0x02, 0, 1, 0, &[packet.as_slice()]);
        let pages = parse_ogg_pages(&page_bytes).expect("parse failed");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].header_type, 0x02);
        assert_eq!(pages[0].serial, 1);
        assert_eq!(pages[0].page_sequence, 0);
        assert_eq!(pages[0].data, packet);
    }

    #[test]
    fn parse_multiple_pages() {
        let p1 = build_ogg_page(0x02, 0, 1, 0, &[b"first"]);
        let p2 = build_ogg_page(0x00, 100, 1, 1, &[b"second"]);
        let mut data = p1;
        data.extend_from_slice(&p2);

        let pages = parse_ogg_pages(&data).expect("parse failed");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_sequence, 0);
        assert_eq!(pages[1].page_sequence, 1);
        assert_eq!(pages[1].granule_position, 100);
    }

    #[test]
    fn extract_single_packet() {
        let page_bytes = build_ogg_page(0x02, 0, 1, 0, &[b"packet_one"]);
        let packets = parse_ogg(&page_bytes).expect("parse_ogg failed");
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0], b"packet_one");
    }

    #[test]
    fn extract_multiple_packets_single_page() {
        let page_bytes = build_ogg_page(0x02, 0, 1, 0, &[b"pkt1", b"pkt2", b"pkt3"]);
        let packets = parse_ogg(&page_bytes).expect("parse_ogg failed");
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0], b"pkt1");
        assert_eq!(packets[1], b"pkt2");
        assert_eq!(packets[2], b"pkt3");
    }

    #[test]
    fn extract_large_packet_spanning_segments() {
        // Create a packet larger than 255 bytes
        let large_packet: Vec<u8> = (0..600).map(|i| (i % 256) as u8).collect();
        let page_bytes = build_ogg_page(0x02, 0, 1, 0, &[&large_packet]);
        let packets = parse_ogg(&page_bytes).expect("parse_ogg failed");
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0], large_packet);
    }

    #[test]
    fn invalid_capture_pattern() {
        let data = b"NotOGGdata";
        let result = parse_ogg_pages(data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("capture pattern"));
    }

    #[test]
    fn empty_data() {
        let result = parse_ogg_pages(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn page_header_fields() {
        let page_bytes = build_ogg_page(0x04, 12345, 42, 7, &[b"data"]);
        let pages = parse_ogg_pages(&page_bytes).expect("parse failed");
        assert_eq!(pages[0].header_type, 0x04); // EOS
        assert_eq!(pages[0].granule_position, 12345);
        assert_eq!(pages[0].serial, 42);
        assert_eq!(pages[0].page_sequence, 7);
    }
}

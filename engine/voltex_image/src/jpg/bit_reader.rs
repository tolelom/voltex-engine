//! JPEG entropy-coded data reader: `BitReader` reads MSB-first bits, skips 0xFF00
//! byte stuffing, decodes Huffman symbols and resynchronises at restart markers.

use super::headers::HuffTable;

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bit_pos: u8,
    current: u8,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8], start: usize) -> Self {
        Self {
            data,
            pos: start,
            bit_pos: 0,
            current: 0,
        }
    }

    fn read_byte(&mut self) -> crate::Result<u8> {
        if self.pos >= self.data.len() {
            return Err("Unexpected end of scan data".into());
        }
        let byte = self.data[self.pos];
        self.pos += 1;
        if byte == 0xFF {
            if self.pos >= self.data.len() {
                return Err("Unexpected end after 0xFF".into());
            }
            let next = self.data[self.pos];
            if next == 0x00 {
                self.pos += 1; // skip stuffed 0x00
            } else if (0xD0..=0xD7).contains(&next) {
                // RST marker — skip marker byte and read next actual byte
                self.pos += 1;
                return self.read_byte();
            } else {
                return Err("Marker found in scan data".into());
            }
        }
        Ok(byte)
    }

    fn ensure_bits(&mut self) -> crate::Result<()> {
        if self.bit_pos == 0 {
            self.current = self.read_byte()?;
            self.bit_pos = 8;
        }
        Ok(())
    }

    fn read_bit(&mut self) -> crate::Result<u8> {
        self.ensure_bits()?;
        self.bit_pos -= 1;
        Ok((self.current >> self.bit_pos) & 1)
    }

    pub(super) fn read_bits(&mut self, count: u8) -> crate::Result<u16> {
        let mut val: u16 = 0;
        for _ in 0..count {
            val = (val << 1) | self.read_bit()? as u16;
        }
        Ok(val)
    }

    pub(super) fn decode_huffman(&mut self, table: &HuffTable) -> crate::Result<u8> {
        let mut code: u16 = 0;
        for len in 0..16 {
            code = (code << 1) | self.read_bit()? as u16;
            if table.maxcode[len] >= 0 && code as i32 <= table.maxcode[len] {
                let idx = table.offsets[len] as usize + (code - table.mincode[len]) as usize;
                return Ok(table.symbols[idx]);
            }
        }
        Err("Invalid Huffman code".into())
    }

    /// Skip to next byte-aligned position (and handle RST markers)
    fn align_to_byte(&mut self) {
        self.bit_pos = 0;
        self.current = 0;
    }

    /// Find and skip RST marker in the byte stream
    pub(super) fn skip_to_rst_marker(&mut self) -> crate::Result<()> {
        // Align to byte boundary
        self.align_to_byte();
        // Look for 0xFF 0xDn marker
        loop {
            if self.pos >= self.data.len() {
                return Err("Unexpected end looking for RST marker".into());
            }
            if self.data[self.pos] == 0xFF && self.pos + 1 < self.data.len() {
                let next = self.data[self.pos + 1];
                if (0xD0..=0xD7).contains(&next) {
                    self.pos += 2;
                    return Ok(());
                }
            }
            self.pos += 1;
        }
    }

    pub(super) fn scan_end_pos(&self) -> usize {
        self.pos
    }
}

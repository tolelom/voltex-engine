// ---------------------------------------------------------------------------
// Bitstream reader and small numeric utilities used across vorbis sub-modules.
// ---------------------------------------------------------------------------

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0..7 within current byte
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub(super) fn bits_remaining(&self) -> usize {
        if self.byte_pos >= self.data.len() {
            return 0;
        }
        (self.data.len() - self.byte_pos) * 8 - self.bit_pos as usize
    }

    pub(super) fn read_bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err("read_bits: n > 32".to_string());
        }
        let mut result: u32 = 0;
        let mut bits_read = 0u32;

        while bits_read < n {
            if self.byte_pos >= self.data.len() {
                return Err("read_bits: unexpected end of data".to_string());
            }
            let remaining_in_byte = 8 - self.bit_pos as u32;
            let to_read = (n - bits_read).min(remaining_in_byte);
            let mask = (1u32 << to_read) - 1;
            let bits = ((self.data[self.byte_pos] >> self.bit_pos) as u32) & mask;
            result |= bits << bits_read;
            bits_read += to_read;
            self.bit_pos += to_read as u8;
            if self.bit_pos >= 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Ok(result)
    }

    pub(super) fn read_bit(&mut self) -> Result<bool, String> {
        Ok(self.read_bits(1)? != 0)
    }

    pub(super) fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_bits(8)? as u8)
    }

    pub(super) fn read_u16(&mut self) -> Result<u16, String> {
        Ok(self.read_bits(16)? as u16)
    }

    pub(super) fn read_u32(&mut self) -> Result<u32, String> {
        self.read_bits(32)
    }
}

pub(super) fn ilog(x: u32) -> u32 {
    if x == 0 {
        0
    } else {
        32 - x.leading_zeros()
    }
}

pub(super) fn float32_unpack(val: u32) -> f32 {
    let mantissa = (val & 0x1fffff) as f32;
    let sign = if val & 0x80000000 != 0 { -1.0f32 } else { 1.0f32 };
    let exponent = ((val >> 21) & 0x3ff) as i32 - 788;
    sign * mantissa * (2.0f32).powi(exponent)
}

pub(super) fn lookup1_values(entries: u32, dimensions: u32) -> u32 {
    if dimensions == 0 {
        return 0;
    }
    // floor(entries^(1/dimensions))
    let val = (entries as f64).powf(1.0 / dimensions as f64).floor() as u32;
    // Verify: val^dimensions <= entries < (val+1)^dimensions
    // Use a simple iterative check
    let mut r = val;
    loop {
        let mut power = 1u64;
        let mut ok = true;
        for _ in 0..dimensions {
            power = power.saturating_mul((r + 1) as u64);
            if power > entries as u64 {
                ok = false;
                break;
            }
        }
        if ok {
            r += 1;
        } else {
            break;
        }
    }
    r
}

pub(super) fn bit_reverse(val: u32, bits: u8) -> u32 {
    let mut result = 0u32;
    let mut v = val;
    for _ in 0..bits {
        result = (result << 1) | (v & 1);
        v >>= 1;
    }
    result
}

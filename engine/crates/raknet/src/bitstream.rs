//! MSB-first BitStream reader/writer matching RakNet's bit-level serialization.

use anyhow::{bail, Result};

/// MSB-first bit writer for RakNet/2016 Roblox bit-packed packets.
#[derive(Default, Clone)]
pub struct BitWriter {
    buf: Vec<u8>,
    bit_offset: u8, // 0..8 bits currently occupied in the last byte
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            buf: Vec::with_capacity(bytes),
            bit_offset: 0,
        }
    }

    pub fn write_bit(&mut self, bit: bool) {
        if self.bit_offset == 0 {
            self.buf.push(0);
        }
        if bit {
            let idx = self.buf.len() - 1;
            self.buf[idx] |= 1 << (7 - self.bit_offset);
        }
        self.bit_offset = (self.bit_offset + 1) & 7;
    }

    pub fn write_bits(&mut self, value: u64, count: u8) {
        for i in (0..count).rev() {
            self.write_bit(((value >> i) & 1) != 0);
        }
    }

    pub fn align_byte(&mut self) {
        self.bit_offset = 0;
    }

    pub fn write_u8(&mut self, v: u8) {
        if self.bit_offset == 0 {
            self.buf.push(v);
        } else {
            self.write_bits(v as u64, 8);
        }
    }

    pub fn write_u16_be(&mut self, v: u16) {
        self.write_u8((v >> 8) as u8);
        self.write_u8((v & 0xFF) as u8);
    }

    pub fn write_u32_be(&mut self, v: u32) {
        for b in v.to_be_bytes() {
            self.write_u8(b);
        }
    }

    pub fn write_f32_be(&mut self, v: f32) {
        for b in v.to_be_bytes() {
            self.write_u8(b);
        }
    }

    /// Writes an unsigned LEB128 varint (used in 2016 Roblox schema & data streams).
    pub fn write_varint(&mut self, mut v: u32) {
        loop {
            let mut byte = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            self.write_u8(byte);
            if v == 0 {
                break;
            }
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if self.bit_offset == 0 {
            self.buf.extend_from_slice(bytes);
        } else {
            for &b in bytes {
                self.write_u8(b);
            }
        }
    }

    pub fn write_string_varint(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_varint(bytes.len() as u32);
        self.write_bytes(bytes);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// MSB-first bit reader for RakNet/2016 Roblox bit-packed packets.
pub struct BitReader<'a> {
    buf: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, bit_pos: 0 }
    }

    pub fn remaining_bits(&self) -> usize {
        (self.buf.len() * 8).saturating_sub(self.bit_pos)
    }

    pub fn read_bit(&mut self) -> Result<bool> {
        if self.remaining_bits() == 0 {
            bail!("BitReader EOF");
        }
        let byte_idx = self.bit_pos / 8;
        let bit_idx = 7 - (self.bit_pos % 8);
        self.bit_pos += 1;
        Ok(((self.buf[byte_idx] >> bit_idx) & 1) != 0)
    }

    pub fn read_bits(&mut self, count: u8) -> Result<u64> {
        if self.remaining_bits() < count as usize {
            bail!("BitReader EOF: needed {count} bits");
        }
        let mut out = 0u64;
        for _ in 0..count {
            out = (out << 1) | (self.read_bit()? as u64);
        }
        Ok(out)
    }

    pub fn align_byte(&mut self) {
        let rem = self.bit_pos % 8;
        if rem != 0 {
            self.bit_pos += 8 - rem;
        }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        if self.bit_pos % 8 == 0 {
            let idx = self.bit_pos / 8;
            if idx >= self.buf.len() {
                bail!("BitReader EOF reading u8");
            }
            self.bit_pos += 8;
            Ok(self.buf[idx])
        } else {
            Ok(self.read_bits(8)? as u8)
        }
    }

    pub fn read_u16_be(&mut self) -> Result<u16> {
        let hi = self.read_u8()? as u16;
        let lo = self.read_u8()? as u16;
        Ok((hi << 8) | lo)
    }

    pub fn read_u32_be(&mut self) -> Result<u32> {
        let b0 = self.read_u8()?;
        let b1 = self.read_u8()?;
        let b2 = self.read_u8()?;
        let b3 = self.read_u8()?;
        Ok(u32::from_be_bytes([b0, b1, b2, b3]))
    }

    pub fn read_f32_be(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.read_u32_be()?))
    }

    pub fn read_varint(&mut self) -> Result<u32> {
        let mut out = 0u32;
        let mut shift = 0u32;
        for _ in 0..5 {
            let byte = self.read_u8()?;
            out |= ((byte & 0x7F) as u32) << shift;
            if (byte & 0x80) == 0 {
                return Ok(out);
            }
            shift += 7;
        }
        bail!("varint overflow (>5 bytes)");
    }

    pub fn read_string_varint(&mut self) -> Result<String> {
        let len = self.read_varint()? as usize;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(self.read_u8()?);
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

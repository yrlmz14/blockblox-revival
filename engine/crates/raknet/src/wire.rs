//! Low-level binary reader/writer for RakNet wire primitives.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use anyhow::{bail, Result};

/// Standard RakNet offline message magic (16 bytes).
pub const OFFLINE_MESSAGE_ID: [u8; 16] = [
    0x00, 0xFF, 0xFF, 0x00, 0xFE, 0xFE, 0xFE, 0xFE, 0xFD, 0xFD, 0xFD, 0xFD, 0x12, 0x34, 0x56, 0x78,
];

/// Cursor-based reader over a RakNet packet slice.
pub struct WireReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> WireReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            bail!(
                "unexpected EOF: needed {len} bytes, only {} remaining",
                self.remaining()
            );
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_bool(&mut self) -> Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_u16_be(&mut self) -> Result<u16> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_u24_le(&mut self) -> Result<u32> {
        let b = self.read_bytes(3)?;
        Ok((b[0] as u32) | ((b[1] as u32) << 8) | ((b[2] as u32) << 16))
    }

    pub fn read_u32_be(&mut self) -> Result<u32> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_i32_be(&mut self) -> Result<i32> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_u64_be(&mut self) -> Result<u64> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn read_f32_be(&mut self) -> Result<f32> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_magic(&mut self) -> Result<()> {
        let magic = self.read_bytes(16)?;
        if magic != OFFLINE_MESSAGE_ID {
            bail!("invalid RakNet offline magic");
        }
        Ok(())
    }

    /// Reads a RakNet IPv4 `SystemAddress` (version=4, 4 bitwise-inverted IPv4 octets, u16 BE port).
    pub fn read_system_address(&mut self) -> Result<SocketAddr> {
        let version = self.read_u8()?;
        if version != 4 {
            // Skip IPv6 (24 bytes after version) if encountered, fallback to unspecified IPv4.
            if version == 6 && self.remaining() >= 28 {
                let _ = self.read_bytes(28)?;
                return Ok(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)));
            }
            bail!("unsupported RakNet address version: {version}");
        }
        let ip = self.read_bytes(4)?;
        let port = self.read_u16_be()?;
        let addr = Ipv4Addr::new(!ip[0], !ip[1], !ip[2], !ip[3]);
        Ok(SocketAddr::V4(SocketAddrV4::new(addr, port)))
    }

    /// Reads a 2-byte big-endian length-prefixed UTF-8 string.
    pub fn read_string_u16(&mut self) -> Result<String> {
        let len = self.read_u16_be()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }

    /// Reads a 4-byte big-endian length-prefixed UTF-8 string (used in Roblox 2016 packets).
    pub fn read_string_u32(&mut self) -> Result<String> {
        let len = self.read_u32_be()? as usize;
        let bytes = self.read_bytes(len)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Growable writer for RakNet binary packets.
#[derive(Default)]
pub struct WireWriter {
    buf: Vec<u8>,
}

impl WireWriter {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(128),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    pub fn write_u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn write_bool(&mut self, v: bool) {
        self.buf.push(if v { 1 } else { 0 });
    }

    pub fn write_u16_be(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_u24_le(&mut self, v: u32) {
        self.buf.push((v & 0xFF) as u8);
        self.buf.push(((v >> 8) & 0xFF) as u8);
        self.buf.push(((v >> 16) & 0xFF) as u8);
    }

    pub fn write_u32_be(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_i32_be(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_u64_be(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_f32_be(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_magic(&mut self) {
        self.buf.extend_from_slice(&OFFLINE_MESSAGE_ID);
    }

    /// Writes a RakNet IPv4 `SystemAddress` (version=4, 4 bitwise-inverted octets, u16 BE port).
    pub fn write_system_address(&mut self, addr: SocketAddr) {
        let (octets, port) = match addr.ip() {
            IpAddr::V4(v4) => (v4.octets(), addr.port()),
            IpAddr::V6(_) => ([0, 0, 0, 0], addr.port()),
        };
        self.write_u8(4);
        self.write_u8(!octets[0]);
        self.write_u8(!octets[1]);
        self.write_u8(!octets[2]);
        self.write_u8(!octets[3]);
        self.write_u16_be(port);
    }

    pub fn write_string_u16(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_u16_be(bytes.len().min(u16::MAX as usize) as u16);
        self.write_bytes(&bytes[..bytes.len().min(u16::MAX as usize)]);
    }

    pub fn write_string_u32(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_u32_be(bytes.len() as u32);
        self.write_bytes(bytes);
    }
}

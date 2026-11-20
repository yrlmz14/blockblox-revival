//! RakNet connected datagram framing, ACK/NAK ranges, reliability layer, and split reassembly.

use std::collections::HashMap;

use anyhow::{bail, Result};

use crate::wire::{WireReader, WireWriter};

/// Outermost RakNet connected datagram header flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RakNetFlags(pub u8);

impl RakNetFlags {
    pub const VALID: u8 = 0x80;
    pub const ACK: u8 = 0x40;
    pub const NAK: u8 = 0x20;
    pub const PACKET_PAIR: u8 = 0x10;
    pub const CONTINUOUS_SEND: u8 = 0x08;
    pub const NEEDS_B_AND_AS: u8 = 0x04;

    pub fn data() -> Self {
        Self(Self::VALID | Self::NEEDS_B_AND_AS) // 0x84
    }

    pub fn ack() -> Self {
        Self(Self::VALID | Self::ACK) // 0xC0
    }

    pub fn nak() -> Self {
        Self(Self::VALID | Self::NAK) // 0xA0
    }

    pub fn is_valid(self) -> bool {
        self.0 & Self::VALID != 0
    }

    pub fn is_ack(self) -> bool {
        self.is_valid() && (self.0 & Self::ACK != 0)
    }

    pub fn is_nak(self) -> bool {
        self.is_valid() && (self.0 & Self::NAK != 0) && !self.is_ack()
    }
}

/// Inclusive range of 24-bit datagram sequence numbers in an ACK or NAK packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckRange {
    pub min: u32,
    pub max: u32,
}

impl AckRange {
    pub fn single(seq: u32) -> Self {
        Self { min: seq, max: seq }
    }
}

/// Encodes an ACK or NAK datagram.
pub fn encode_ack(is_nak: bool, ranges: &[AckRange]) -> Vec<u8> {
    let mut w = WireWriter::with_capacity(3 + ranges.len() * 7);
    let flags = if is_nak {
        RakNetFlags::nak()
    } else {
        RakNetFlags::ack()
    };
    w.write_u8(flags.0);
    w.write_u16_be(ranges.len() as u16);
    for r in ranges {
        if r.min == r.max {
            w.write_bool(true);
            w.write_u24_le(r.min);
        } else {
            w.write_bool(false);
            w.write_u24_le(r.min);
            w.write_u24_le(r.max);
        }
    }
    w.into_bytes()
}

/// Decodes an ACK or NAK datagram (expecting the flags byte to have been read or peeked).
pub fn decode_ack(buf: &[u8]) -> Result<(bool, Vec<AckRange>)> {
    let mut r = WireReader::new(buf);
    let flags = RakNetFlags(r.read_u8()?);
    if !flags.is_ack() && !flags.is_nak() {
        bail!("not an ACK/NAK datagram: 0x{:02X}", flags.0);
    }
    let count = r.read_u16_be()? as usize;
    let mut ranges = Vec::with_capacity(count);
    for _ in 0..count {
        let is_single = r.read_bool()?;
        let min = r.read_u24_le()?;
        let max = if is_single { min } else { r.read_u24_le()? };
        ranges.push(AckRange { min, max });
    }
    Ok((flags.is_nak(), ranges))
}

/// RakNet reliability modes (3-bit field in reliability header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reliability {
    Unreliable = 0,
    UnreliableSequenced = 1,
    Reliable = 2,
    ReliableOrdered = 3,
    ReliableSequenced = 4,
    UnreliableWithAckReceipt = 5,
    ReliableWithAckReceipt = 6,
    ReliableOrderedWithAckReceipt = 7,
}

impl Reliability {
    pub fn from_u8(v: u8) -> Result<Self> {
        Ok(match v & 0x07 {
            0 => Self::Unreliable,
            1 => Self::UnreliableSequenced,
            2 => Self::Reliable,
            3 => Self::ReliableOrdered,
            4 => Self::ReliableSequenced,
            5 => Self::UnreliableWithAckReceipt,
            6 => Self::ReliableWithAckReceipt,
            7 => Self::ReliableOrderedWithAckReceipt,
            _ => unreachable!(),
        })
    }

    pub fn is_reliable(self) -> bool {
        matches!(
            self,
            Self::Reliable
                | Self::ReliableOrdered
                | Self::ReliableSequenced
                | Self::ReliableWithAckReceipt
                | Self::ReliableOrderedWithAckReceipt
        )
    }

    pub fn is_sequenced(self) -> bool {
        matches!(self, Self::UnreliableSequenced | Self::ReliableSequenced)
    }

    pub fn is_ordered(self) -> bool {
        matches!(
            self,
            Self::UnreliableSequenced
                | Self::ReliableOrdered
                | Self::ReliableSequenced
                | Self::ReliableOrderedWithAckReceipt
        )
    }
}

/// Split packet metadata attached when `has_split` bit is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitInfo {
    pub count: u32,
    pub id: u16,
    pub index: u32,
}

/// One encapsulated message inside a RakNet connected data datagram (`0x80..0x8F`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncapsulatedPacket {
    pub reliability: Reliability,
    pub reliable_index: Option<u32>,
    pub sequence_index: Option<u32>,
    pub order_index: Option<u32>,
    pub order_channel: Option<u8>,
    pub split: Option<SplitInfo>,
    pub payload: Vec<u8>,
}

/// Decoded connected data datagram containing one or more encapsulated packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDatagram {
    pub flags: RakNetFlags,
    pub sequence_number: u32,
    pub packets: Vec<EncapsulatedPacket>,
}

impl DataDatagram {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(64);
        w.write_u8(self.flags.0);
        w.write_u24_le(self.sequence_number);

        for pkt in &self.packets {
            let mut header = ((pkt.reliability as u8) & 0x07) << 5;
            if pkt.split.is_some() {
                header |= 0x10;
            }
            w.write_u8(header);
            let bit_len = (pkt.payload.len() * 8) as u16;
            w.write_u16_be(bit_len);

            if pkt.reliability.is_reliable() {
                w.write_u24_le(pkt.reliable_index.unwrap_or(0));
            }
            if pkt.reliability.is_sequenced() {
                w.write_u24_le(pkt.sequence_index.unwrap_or(0));
            }
            if pkt.reliability.is_ordered() {
                w.write_u24_le(pkt.order_index.unwrap_or(0));
                w.write_u8(pkt.order_channel.unwrap_or(0));
            }
            if let Some(split) = pkt.split {
                w.write_u32_be(split.count);
                w.write_u16_be(split.id);
                w.write_u32_be(split.index);
            }
            w.write_bytes(&pkt.payload);
        }

        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let flags = RakNetFlags(r.read_u8()?);
        if !flags.is_valid() || flags.is_ack() || flags.is_nak() {
            bail!("not a RakNet data datagram: 0x{:02X}", flags.0);
        }
        let sequence_number = r.read_u24_le()?;
        let mut packets = Vec::new();

        while r.remaining() >= 3 {
            let header = r.read_u8()?;
            let reliability = Reliability::from_u8((header >> 5) & 0x07)?;
            let has_split = (header & 0x10) != 0;
            let bit_len = r.read_u16_be()? as usize;
            let byte_len = bit_len.div_ceil(8);

            let reliable_index = if reliability.is_reliable() {
                Some(r.read_u24_le()?)
            } else {
                None
            };
            let sequence_index = if reliability.is_sequenced() {
                Some(r.read_u24_le()?)
            } else {
                None
            };
            let (order_index, order_channel) = if reliability.is_ordered() {
                (Some(r.read_u24_le()?), Some(r.read_u8()?))
            } else {
                (None, None)
            };
            let split = if has_split {
                Some(SplitInfo {
                    count: r.read_u32_be()?,
                    id: r.read_u16_be()?,
                    index: r.read_u32_be()?,
                })
            } else {
                None
            };

            let payload = r.read_bytes(byte_len)?.to_vec();
            packets.push(EncapsulatedPacket {
                reliability,
                reliable_index,
                sequence_index,
                order_index,
                order_channel,
                split,
                payload,
            });
        }

        Ok(Self {
            flags,
            sequence_number,
            packets,
        })
    }
}

/// Splits a large payload into MTU-safe `EncapsulatedPacket` fragments.
pub fn fragment_payload(
    payload: &[u8],
    mtu: u16,
    split_id: u16,
    mut next_reliable_index: impl FnMut() -> u32,
    order_index: u32,
    order_channel: u8,
) -> Vec<EncapsulatedPacket> {
    // Reserve ~64 bytes for IP/UDP + RakNet datagram + reliability + split headers.
    let max_chunk = (mtu as usize).saturating_sub(64).max(256);
    if payload.len() <= max_chunk {
        return vec![EncapsulatedPacket {
            reliability: Reliability::ReliableOrdered,
            reliable_index: Some(next_reliable_index()),
            sequence_index: None,
            order_index: Some(order_index),
            order_channel: Some(order_channel),
            split: None,
            payload: payload.to_vec(),
        }];
    }

    let chunks: Vec<&[u8]> = payload.chunks(max_chunk).collect();
    let count = chunks.len() as u32;
    chunks
        .into_iter()
        .enumerate()
        .map(|(idx, chunk)| EncapsulatedPacket {
            reliability: Reliability::ReliableOrdered,
            reliable_index: Some(next_reliable_index()),
            sequence_index: None,
            order_index: Some(order_index),
            order_channel: Some(order_channel),
            split: Some(SplitInfo {
                count,
                id: split_id,
                index: idx as u32,
            }),
            payload: chunk.to_vec(),
        })
        .collect()
}

/// Reassembles fragmented RakNet split packets by `(split_id, split_count)`.
#[derive(Default)]
pub struct SplitAssembler {
    buckets: HashMap<u16, (u32, HashMap<u32, Vec<u8>>)>,
}

impl SplitAssembler {
    /// Inserts a split fragment. Returns the complete reassembled payload once all parts arrive.
    pub fn push(&mut self, info: SplitInfo, fragment: Vec<u8>) -> Option<Vec<u8>> {
        if info.count == 0 || info.index >= info.count || info.count > 4096 {
            return None;
        }
        let entry = self
            .buckets
            .entry(info.id)
            .or_insert_with(|| (info.count, HashMap::with_capacity(info.count as usize)));
        entry.1.insert(info.index, fragment);
        if entry.1.len() as u32 == entry.0 {
            let (count, mut parts) = self.buckets.remove(&info.id)?;
            let mut full = Vec::new();
            for i in 0..count {
                full.extend_from_slice(&parts.remove(&i)?);
            }
            Some(full)
        } else {
            None
        }
    }
}

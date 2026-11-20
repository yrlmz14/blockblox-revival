//! Clean-room 2016 Roblox RakNet UDP protocol & legacy RCC adapter.
//!
//! Designed to run alongside the WebSocket server in `revival-server` so that
//! both modern cross-platform clients and legacy 2016 RakNet peers share the
//! same authoritative `DataModel` and `PhysicsWorld`.

pub mod bitstream;
pub mod packets;
pub mod pcap;
pub mod reliability;
pub mod schema;
pub mod server;
pub mod wire;

pub use bitstream::{BitReader, BitWriter};
pub use packets::{
    PacketId, DEFAULT_RAKNET_PORT, DEFAULT_RAKNET_PROTOCOL_VERSION,
    DEFAULT_ROBLOX_2016_PROTOCOL_VERSION,
};
pub use reliability::{
    decode_ack, encode_ack, fragment_payload, AckRange, DataDatagram, EncapsulatedPacket,
    RakNetFlags, Reliability, SplitAssembler, SplitInfo,
};
pub use schema::{
    ClassDescriptor, EnumDescriptor, NetworkSchema, PhysicsPacket, PhysicsPartState,
    PropertyDescriptor,
};
pub use server::{run_raknet_server, wrap_reliable_datagram, RakNetConfig, RakNetEvent};
pub use wire::OFFLINE_MESSAGE_ID;

/// Wireshark-style one-line packet dissector for inspecting raw UDP RakNet datagrams.
pub fn inspect_packet(raw: &[u8]) -> String {
    if raw.is_empty() {
        return "EMPTY (0 bytes)".into();
    }

    if packets::is_offline_message(raw) {
        return format!(
            "OFFLINE {} (0x{:02X}, {} bytes)",
            PacketId::name(raw[0]),
            raw[0],
            raw.len()
        );
    }

    let flags = RakNetFlags(raw[0]);
    if flags.is_ack() || flags.is_nak() {
        let kind = if flags.is_nak() { "NAK" } else { "ACK" };
        return match decode_ack(raw) {
            Ok((_, ranges)) => format!("{kind} ({} ranges, {} bytes)", ranges.len(), raw.len()),
            Err(e) => format!("{kind} <invalid: {e}>"),
        };
    }

    if flags.is_valid() {
        return match DataDatagram::decode(raw) {
            Ok(dg) => {
                let inner: Vec<String> = dg
                    .packets
                    .iter()
                    .map(|p| {
                        let pid = p.payload.first().copied().unwrap_or(0);
                        let split = p
                            .split
                            .map(|s| format!(" split={}/{}", s.index + 1, s.count))
                            .unwrap_or_default();
                        format!(
                            "{}(0x{:02X}, {:?}, {}B{})",
                            PacketId::name(pid),
                            pid,
                            p.reliability,
                            p.payload.len(),
                            split
                        )
                    })
                    .collect();
                format!(
                    "DATA seq={} [{}]",
                    dg.sequence_number,
                    inner.join(", ")
                )
            }
            Err(e) => format!("DATA <decode error: {e}> ({} bytes)", raw.len()),
        };
    }

    format!(
        "RAW {} (0x{:02X}, {} bytes)",
        PacketId::name(raw[0]),
        raw[0],
        raw.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn ack_roundtrip_single_and_ranged() {
        let ranges = vec![AckRange::single(7), AckRange { min: 10, max: 15 }];
        let encoded = encode_ack(false, &ranges);
        let (is_nak, decoded) = decode_ack(&encoded).unwrap();
        assert!(!is_nak);
        assert_eq!(decoded, ranges);
    }

    #[test]
    fn split_fragment_and_reassemble() {
        let payload: Vec<u8> = (0..3500u32).map(|i| (i % 251) as u8).collect();
        let mut r_idx = 0u32;
        let frags = fragment_payload(&payload, 600, 42, || {
            let cur = r_idx;
            r_idx += 1;
            cur
        }, 0, 0);
        assert!(frags.len() > 1);

        let mut assembler = SplitAssembler::default();
        let mut out = None;
        for f in frags {
            out = assembler.push(f.split.unwrap(), f.payload);
        }
        assert_eq!(out.unwrap(), payload);
    }

    #[test]
    fn schema_sync_and_physics_bitstream_roundtrip() {
        let schema = NetworkSchema::default_2016();
        let bytes = schema.encode(PacketId::SchemaSync);
        let decoded = NetworkSchema::decode(&bytes).unwrap();
        assert_eq!(decoded, schema);

        let phys = PhysicsPacket {
            parts: vec![PhysicsPartState {
                referent_id: 77,
                position: [1.5, 10.25, -4.0],
                velocity: [0.0, -9.8, 2.0],
                angular_velocity: [0.1, 0.2, 0.3],
                humanoid_state: 8,
            }],
        };
        let pbytes = phys.encode();
        let pdecoded = PhysicsPacket::decode(&pbytes).unwrap();
        assert_eq!(pdecoded, phys);
    }

    #[test]
    fn pcap_write_and_parse_raknet_session() {
        let client = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 64496);
        let server = SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_RAKNET_PORT);
        let req1 = packets::OpenConnectionRequest1 {
            protocol_version: 5,
            mtu_size: 1200,
        }
        .encode();
        let ack = encode_ack(false, &[AckRange::single(0)]);
        let pcap_bytes = pcap::write_pcap_udp(&[(client, server, &req1), (server, client, &ack)]);
        let records = pcap::parse_pcap_udp(&pcap_bytes).unwrap();
        assert_eq!(records.len(), 2);
        assert!(inspect_packet(&records[0].payload).contains("ID_OPEN_CONNECTION_REQUEST_1"));
        assert!(inspect_packet(&records[1].payload).starts_with("ACK"));
    }
}

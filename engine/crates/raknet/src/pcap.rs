//! Pure-Rust `.pcap` (libpcap) reader & writer for inspecting captured 2016 Roblox RakNet traffic.

use std::net::{Ipv4Addr, SocketAddrV4};

use anyhow::{bail, Result};

/// One extracted UDP datagram from a `.pcap` capture file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcapUdpRecord {
    pub index: usize,
    pub ts_sec: u32,
    pub ts_usec: u32,
    pub src: SocketAddrV4,
    pub dst: SocketAddrV4,
    pub payload: Vec<u8>,
}

/// Parses a standard libpcap (`.pcap`) file and extracts all IPv4 UDP datagrams.
pub fn parse_pcap_udp(bytes: &[u8]) -> Result<Vec<PcapUdpRecord>> {
    if bytes.len() < 24 {
        bail!("file too short for PCAP global header");
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let little_endian = match magic {
        0xA1B2_C3D4 | 0xA1B2_3C4D => true,
        0xD4C3_B2A1 | 0x4D3C_B2A1 => false,
        other => bail!("unsupported PCAP magic: 0x{other:08X} (expected classic .pcap)"),
    };

    let read_u32 = |slice: &[u8]| -> u32 {
        let arr: [u8; 4] = slice.try_into().unwrap();
        if little_endian {
            u32::from_le_bytes(arr)
        } else {
            u32::from_be_bytes(arr)
        }
    };

    let link_type = read_u32(&bytes[20..24]);
    let mut pos = 24;
    let mut out = Vec::new();
    let mut idx = 0usize;

    while pos + 16 <= bytes.len() {
        let ts_sec = read_u32(&bytes[pos..pos + 4]);
        let ts_usec = read_u32(&bytes[pos + 4..pos + 8]);
        let incl_len = read_u32(&bytes[pos + 8..pos + 12]) as usize;
        pos += 16;
        if pos + incl_len > bytes.len() {
            break;
        }
        let frame = &bytes[pos..pos + incl_len];
        pos += incl_len;
        idx += 1;

        if let Some((src, dst, udp_payload)) = extract_ipv4_udp(link_type, frame) {
            out.push(PcapUdpRecord {
                index: idx,
                ts_sec,
                ts_usec,
                src,
                dst,
                payload: udp_payload.to_vec(),
            });
        }
    }

    Ok(out)
}

fn extract_ipv4_udp(link_type: u32, frame: &[u8]) -> Option<(SocketAddrV4, SocketAddrV4, &[u8])> {
    let ip = match link_type {
        // DLT_NULL (BSD/macOS loopback: 4-byte family header)
        0 => {
            if frame.len() < 4 {
                return None;
            }
            &frame[4..]
        }
        // DLT_EN10MB (Ethernet II: 14-byte header, EtherType 0x0800 = IPv4)
        1 => {
            if frame.len() < 14 || frame[12] != 0x08 || frame[13] != 0x00 {
                return None;
            }
            &frame[14..]
        }
        // DLT_RAW (Raw IP)
        101 => frame,
        // DLT_LINUX_SLL (Linux cooked capture: 16-byte header)
        113 => {
            if frame.len() < 16 || frame[14] != 0x08 || frame[15] != 0x00 {
                return None;
            }
            &frame[16..]
        }
        _ => return None,
    };

    if ip.len() < 20 || (ip[0] >> 4) != 4 {
        return None;
    }
    let ihl = ((ip[0] & 0x0F) as usize) * 4;
    if ip.len() < ihl + 8 || ip[9] != 17 {
        // Protocol 17 = UDP
        return None;
    }

    let src_ip = Ipv4Addr::new(ip[12], ip[13], ip[14], ip[15]);
    let dst_ip = Ipv4Addr::new(ip[16], ip[17], ip[18], ip[19]);
    let udp = &ip[ihl..];
    let src_port = u16::from_be_bytes([udp[0], udp[1]]);
    let dst_port = u16::from_be_bytes([udp[2], udp[3]]);
    let udp_len = u16::from_be_bytes([udp[4], udp[5]]) as usize;
    if udp_len < 8 || udp.len() < udp_len {
        return None;
    }
    Some((
        SocketAddrV4::new(src_ip, src_port),
        SocketAddrV4::new(dst_ip, dst_port),
        &udp[8..udp_len],
    ))
}

/// Writes a sequence of IPv4 UDP datagrams into a classic libpcap (`.pcap`) byte buffer (`DLT_RAW`).
pub fn write_pcap_udp(records: &[(SocketAddrV4, SocketAddrV4, &[u8])]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + records.len() * 128);
    // Global header (little-endian, v2.4, snaplen=65535, DLT_RAW=101)
    out.extend_from_slice(&0xA1B2_C3D4u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&65535u32.to_le_bytes());
    out.extend_from_slice(&101u32.to_le_bytes());

    for (i, (src, dst, payload)) in records.iter().enumerate() {
        let udp_len = (8 + payload.len()) as u16;
        let total_len = 20 + udp_len;

        let mut ip_udp = Vec::with_capacity(total_len as usize);
        // Minimal IPv4 header (20 bytes)
        ip_udp.push(0x45);
        ip_udp.push(0);
        ip_udp.extend_from_slice(&total_len.to_be_bytes());
        ip_udp.extend_from_slice(&0u16.to_be_bytes());
        ip_udp.extend_from_slice(&0u16.to_be_bytes());
        ip_udp.push(64);
        ip_udp.push(17); // UDP
        ip_udp.extend_from_slice(&0u16.to_be_bytes());
        ip_udp.extend_from_slice(&src.ip().octets());
        ip_udp.extend_from_slice(&dst.ip().octets());
        // UDP header (8 bytes)
        ip_udp.extend_from_slice(&src.port().to_be_bytes());
        ip_udp.extend_from_slice(&dst.port().to_be_bytes());
        ip_udp.extend_from_slice(&udp_len.to_be_bytes());
        ip_udp.extend_from_slice(&0u16.to_be_bytes());
        ip_udp.extend_from_slice(payload);

        // Per-packet PCAP header
        out.extend_from_slice(&(i as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(ip_udp.len() as u32).to_le_bytes());
        out.extend_from_slice(&(ip_udp.len() as u32).to_le_bytes());
        out.extend_from_slice(&ip_udp);
    }

    out
}

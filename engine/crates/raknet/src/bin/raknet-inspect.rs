//! CLI tool for dissecting Wireshark `.pcap` captures of 2012–2016 Roblox RakNet UDP traffic.
//!
//! Usage:
//!   cargo run -p revival-raknet --bin raknet-inspect -- <capture.pcap> [port]

use std::{collections::HashMap, env, fs, net::SocketAddrV4};

use anyhow::{bail, Result};
use revival_raknet::{
    inspect_packet,
    pcap::parse_pcap_udp,
    DataDatagram, PacketId, RakNetFlags, SplitAssembler,
};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        bail!("usage: raknet-inspect <capture.pcap> [udp_port_filter]");
    }
    let path = &args[1];
    let port_filter: Option<u16> = args.get(2).and_then(|s| s.parse().ok());

    let bytes = fs::read(path)?;
    let records = parse_pcap_udp(&bytes)?;
    let mut assemblers: HashMap<(SocketAddrV4, SocketAddrV4), SplitAssembler> = HashMap::new();

    println!(
        "Loaded {} UDP datagram(s) from {path}{}",
        records.len(),
        port_filter
            .map(|p| format!(" (filtering port {p})"))
            .unwrap_or_default()
    );

    let mut shown = 0usize;
    for rec in records {
        if let Some(p) = port_filter {
            if rec.src.port() != p && rec.dst.port() != p {
                continue;
            }
        }
        shown += 1;
        let summary = inspect_packet(&rec.payload);
        println!(
            "#{:<5} {:>4}.{:06} {:>21} -> {:<21} {}",
            rec.index, rec.ts_sec, rec.ts_usec, rec.src, rec.dst, summary
        );

        // Also reassemble any split packets in this UDP flow
        if !rec.payload.is_empty() {
            let flags = RakNetFlags(rec.payload[0]);
            if flags.is_valid() && !flags.is_ack() && !flags.is_nak() {
                if let Ok(dg) = DataDatagram::decode(&rec.payload) {
                    let asm = assemblers.entry((rec.src, rec.dst)).or_default();
                    for pkt in dg.packets {
                        if let Some(split) = pkt.split {
                            if let Some(full) = asm.push(split, pkt.payload) {
                                let pid = full.first().copied().unwrap_or(0);
                                println!(
                                    "      ↳ REASSEMBLED {} (0x{:02X}, {} bytes)",
                                    PacketId::name(pid),
                                    pid,
                                    full.len()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    println!("Displayed {shown} RakNet/UDP packet(s).");
    Ok(())
}

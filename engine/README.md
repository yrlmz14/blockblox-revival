# Revival Engine Workspace

## Crates
- `crates/core`: `DataModel`, `.rbxl`/`.rbxlx` loader (`rbx-dom`), and sandboxed `Luau` VM (`mlua`).
- `crates/net`: Shared WebSocket protocol (`ClientMessage`, `ServerMessage`, `ServerInfo`).
- `crates/raknet`: Clean-room 2016 Legacy RCC (RakNet UDP framing, reliability, split-packet reassembly, and 2016 Roblox `0x81..0x97` protocol packets).
- `crates/server`: Lightweight `axum` + `tokio` dual-stack headless server (HTTP/WS + 2016 RakNet UDP on `:53640`) optimized for **Raspberry Pi Zero 2 W** (512 MB RAM, `aarch64-unknown-linux-gnu`).
- `crates/client`: Cross-platform client core (`wgpu` + `winit`) for macOS, Android `arm64-v8a`, and iOS WebAssembly.

## Running the Server Locally
```bash
cargo run -p revival-server
# Optional: load a custom place file & enable live RakNet packet tracing
PLACE_FILE=./my_place.rbxlx PORT=8080 RAKNET_PORT=53640 RAKNET_TRACE=1 cargo run -p revival-server
```

## Inspecting Wireshark `.pcap` Captures (2012–2016 RakNet)
```bash
cargo run -p revival-raknet --bin raknet-inspect -- capture.pcap 53640
```

## Building for Raspberry Pi Zero 2 W (`aarch64`)
```bash
rustup target add aarch64-unknown-linux-gnu
# Using cargo-zigbuild or aarch64-linux-gnu-gcc linker:
cargo build -p revival-server --release --target aarch64-unknown-linux-gnu
```
The release profile uses `opt-level = "s"`, `lto = true`, and `strip = true` to keep the binary small and RAM usage minimal on the Pi Zero 2 W's 512 MB RAM.

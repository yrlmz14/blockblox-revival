# Revival Engine Workspace

## Crates
- `crates/core`: `DataModel`, `.rbxl`/`.rbxlx` loader (`rbx-dom`), and sandboxed `Luau` VM (`mlua`).
- `crates/net`: Shared WebSocket protocol (`ClientMessage`, `ServerMessage`, `ServerInfo`).
- `crates/server`: Lightweight `axum` + `tokio` headless server optimized for **Raspberry Pi Zero 2 W** (512 MB RAM, `aarch64-unknown-linux-gnu`).
- `crates/client`: Cross-platform client core (`wgpu` + `winit`) for macOS, Android `arm64-v8a`, and iOS WebAssembly.

## Running the Server Locally
```bash
cargo run -p revival-server
# Optional: load a custom place file
PLACE_FILE=./my_place.rbxlx PORT=8080 cargo run -p revival-server
```

## Building for Raspberry Pi Zero 2 W (`aarch64`)
```bash
rustup target add aarch64-unknown-linux-gnu
# Using cargo-zigbuild or aarch64-linux-gnu-gcc linker:
cargo build -p revival-server --release --target aarch64-unknown-linux-gnu
```
The release profile uses `opt-level = "s"`, `lto = true`, and `strip = true` to keep the binary small and RAM usage minimal on the Pi Zero 2 W's 512 MB RAM.

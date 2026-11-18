package org.libreblox.app

import android.app.NativeActivity

/**
 * NativeActivity host for the Rust `revival-client` (`librevival_client.so`) 3D engine.
 * Reads `pending_place.rbxl` / `pending_ws.txt` from `filesDir` via `android_main`.
 */
class RevivalGameActivity : NativeActivity()

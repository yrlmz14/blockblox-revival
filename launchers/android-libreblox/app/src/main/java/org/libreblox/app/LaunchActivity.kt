package org.libreblox.app

import android.app.Activity
import android.os.Bundle

/** Target of the home-screen shortcut: apply flags, launch Roblox, vanish. */
class LaunchActivity : Activity() {
    override fun onCreate(b: Bundle?) {
        super.onCreate(b)
        Boot.launch(this, intent?.getStringExtra("uri"), null)
        finish()
    }
}

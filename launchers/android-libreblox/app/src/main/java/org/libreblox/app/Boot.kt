package org.libreblox.app

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.mutableStateListOf
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku
import java.io.BufferedReader
import java.io.InputStreamReader

/** Runs shell commands (uid shell) through Shizuku. Ported from Dorfikstrap. */
object Sh {
    const val PKG = "com.roblox.client"
    const val FLAGS_FILE = "/data/local/tmp/ClientAppSettings.json"
    const val SHIZUKU_PKG = "moe.shizuku.privileged.api"

    enum class State { NOT_INSTALLED, NOT_RUNNING, NEEDS_PERMISSION, READY }

    fun state(ctx: Context): State = try {
        when {
            !Shizuku.pingBinder() -> {
                val installed = try { ctx.packageManager.getPackageInfo(SHIZUKU_PKG, 0); true } catch (_: Exception) { false }
                if (installed) State.NOT_RUNNING else State.NOT_INSTALLED
            }
            Shizuku.isPreV11() -> State.NOT_RUNNING
            Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED -> State.NEEDS_PERMISSION
            else -> State.READY
        }
    } catch (_: Throwable) { State.NOT_INSTALLED }

    fun ready(): Boolean = try {
        Shizuku.pingBinder() && !Shizuku.isPreV11() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
    } catch (_: Throwable) { false }

    fun requestPermission() { try { if (Shizuku.pingBinder() && !Shizuku.isPreV11()) Shizuku.requestPermission(1) } catch (_: Throwable) {} }

    /** Shizuku.newProcess is private in the API; reached by reflection. */
    private fun spawn(cmd: String): Process {
        val m = Shizuku::class.java.getDeclaredMethod("newProcess", Array<String>::class.java, Array<String>::class.java, String::class.java)
        m.isAccessible = true
        return m.invoke(null, arrayOf("sh", "-c", cmd), null, null) as Process
    }

    /** Blocking. Returns stdout+stderr (max 400 lines). */
    fun run(cmd: String): String {
        val sb = StringBuilder()
        try {
            val p = spawn("$cmd 2>&1")
            val r = BufferedReader(InputStreamReader(p.inputStream))
            var n = 0
            while (n++ < 400) { sb.append(r.readLine() ?: break).append('\n') }
            p.waitFor()
        } catch (t: Throwable) {
            sb.append("ERROR: ").append(t.cause ?: t).append('\n')
        }
        return sb.toString()
    }
}

data class Flag(val name: String, val value: String, val enabled: Boolean = true)

/** The user's FastFlag list (persisted) and its ClientAppSettings.json form. */
class FlagStore(ctx: Context) {
    private val sp = ctx.getSharedPreferences("flags", Context.MODE_PRIVATE)
    val flags = mutableStateListOf<Flag>()

    init {
        try {
            val a = JSONArray(sp.getString("list", "[]"))
            for (i in 0 until a.length()) {
                val o = a.getJSONObject(i)
                flags += Flag(o.getString("n"), o.getString("v"), o.optBoolean("e", true))
            }
        } catch (_: Exception) {}
    }

    var master: Boolean
        get() = sp.getBoolean("master", true)
        set(v) { sp.edit().putBoolean("master", v).apply() }

    /** Apply the flags automatically when a game is started from LibreBlox. */
    var applyOnPlay: Boolean
        get() = sp.getBoolean("apply_on_play", true)
        set(v) { sp.edit().putBoolean("apply_on_play", v).apply() }

    fun save() {
        val a = JSONArray()
        flags.forEach { a.put(JSONObject().put("n", it.name).put("v", it.value).put("e", it.enabled)) }
        sp.edit().putString("list", a.toString()).apply()
    }

    /** Adds or updates (and enables). Returns false if name/value contain forbidden characters. */
    fun put(name: String, value: String): Boolean {
        if (!NAME.matches(name) || !VALUE.matches(value)) return false
        val i = flags.indexOfFirst { it.name == name }
        if (i < 0) flags += Flag(name, value) else flags[i] = Flag(name, value, true)
        save()
        return true
    }

    fun setEnabled(name: String, on: Boolean) {
        val i = flags.indexOfFirst { it.name == name }
        if (i >= 0) { flags[i] = flags[i].copy(enabled = on); save() }
    }

    fun remove(name: String) { flags.removeAll { it.name == name }; save() }
    fun clear() { flags.clear(); save() }
    fun enabledCount() = flags.count { it.enabled }

    fun json(): String {
        val o = JSONObject()
        flags.filter { it.enabled }.forEach { o.put(it.name, it.value) }
        return o.toString()
    }

    /** Imports a JSON object of flags. Returns (added, skipped), or null if the text is not a JSON object. */
    fun importJson(text: String): Pair<Int, Int>? = try {
        var clean = text.replace("﻿", "").trim()
        if (clean.startsWith("```")) {
            clean = clean.replaceFirst(Regex("^```[a-zA-Z]*"), "")
            if (clean.endsWith("```")) clean = clean.dropLast(3)
        }
        clean = clean.trim().replace(Regex(",\\s*([}\\]])"), "$1")
        val o = JSONObject(clean)
        var ok = 0; var bad = 0
        for (k in o.keys()) {
            val v = o.get(k)
            if (v is JSONObject || v is JSONArray || v == JSONObject.NULL) { bad++; continue }
            if (put(k, v.toString())) ok++ else bad++
        }
        ok to bad
    } catch (_: Exception) { null }

    /** Shell command that writes the file. Values are validated, so single quotes cannot occur. */
    fun writeCommand(): String =
        if (!master || enabledCount() == 0) "rm -f ${Sh.FLAGS_FILE}; echo 'no flags: file removed'"
        else "printf '%s' '${json()}' > ${Sh.FLAGS_FILE} && chmod 644 ${Sh.FLAGS_FILE} && cat ${Sh.FLAGS_FILE}"

    companion object {
        val NAME = Regex("[A-Za-z0-9_.]{2,100}")
        val VALUE = Regex("[A-Za-z0-9_.:/ -]{0,100}")

        /** name, value, description. Experimental, not verified against Roblox's allowlist. */
        val PRESETS = listOf(
            Triple("FFlagDebugDisplayFPS", "true", "FPS counter"),
            Triple("DFIntTaskSchedulerTargetFps", "120", "Target FPS 120"),
            Triple("FIntDebugForceMSAASamples", "1", "Disable MSAA"),
            Triple("FIntRenderShadowIntensity", "0", "No shadows"),
        )
    }
}

object Boot {
    fun robloxInstalled(ctx: Context) = ctx.packageManager.getLaunchIntentForPackage(Sh.PKG) != null

    /** Writes the flag file (when Shizuku is ready) and force-stops Roblox so it re-reads it. Blocking. */
    fun apply(ctx: Context, restart: Boolean): String {
        if (!Sh.ready()) return "Shizuku is not ready"
        val store = FlagStore(ctx)
        val out = Sh.run(store.writeCommand())
        if (restart) Sh.run("am force-stop ${Sh.PKG}")
        return out
    }

    /** Applies the flags (if enabled), then opens a roblox:// link (or the Roblox home screen when uri is null). */
    fun launch(ctx: Context, uri: String?, fallbackWeb: String?) {
        val app = ctx.applicationContext
        Thread {
            if (FlagStore(app).applyOnPlay) apply(app, true)
            Handler(Looper.getMainLooper()).post {
                val flags = Intent.FLAG_ACTIVITY_NEW_TASK
                try {
                    val i = if (uri != null && uri.startsWith("roblox://")) Intent(Intent.ACTION_VIEW, Uri.parse(uri)).setPackage(Sh.PKG)
                    else app.packageManager.getLaunchIntentForPackage(Sh.PKG) ?: throw IllegalStateException("Roblox not installed")
                    app.startActivity(i.addFlags(flags))
                } catch (_: Exception) {
                    fallbackWeb?.let { openWeb(app, it) }
                }
            }
        }.start()
    }
}

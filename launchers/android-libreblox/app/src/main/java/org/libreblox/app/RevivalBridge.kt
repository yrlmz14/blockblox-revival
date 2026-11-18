package org.libreblox.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import java.net.HttpURLConnection
import java.net.URL

/**
 * Bridge between the LibreBlox Android UI, the Raspberry Pi Zero 2 W revival server,
 * and the native `revival-client` Rust library (`arm64-v8a`).
 */
object RevivalBridge {
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("revival_client")
            nativeLoaded = true
        } catch (_: UnsatisfiedLinkError) {
            nativeLoaded = false
        }
    }

    fun isNativeEngineAvailable(): Boolean = nativeLoaded

    external fun nativeStartOfflineBytes(placeBytes: ByteArray): Boolean
    external fun nativeStartOnlineWs(wsUrl: String): Boolean

    suspend fun fetchPiServers(baseUrl: String): List<Game> = withContext(Dispatchers.IO) {
        val url = URL("${baseUrl.trimEnd('/')}/api/servers")
        val conn = (url.openConnection() as HttpURLConnection).apply {
            requestMethod = "GET"
            connectTimeout = 4000
            readTimeout = 4000
        }
        try {
            val body = conn.inputStream.bufferedReader().use { it.readText() }
            val arr = JSONArray(body)
            List(arr.length()) { i ->
                val obj = arr.getJSONObject(i)
                Game(
                    universeId = (i + 1).toLong(),
                    placeId = (i + 1).toLong(),
                    name = obj.optString("name", "Revival Server"),
                    players = obj.optLong("players", 0L),
                )
            }
        } finally {
            conn.disconnect()
        }
    }
}

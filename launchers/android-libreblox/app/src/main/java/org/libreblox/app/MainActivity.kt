package org.libreblox.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background,
                ) {
                    RevivalLauncherScreen()
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RevivalLauncherScreen() {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var serverUrl by remember { mutableStateOf("http://192.168.1.100:8080") }
    var statusText by remember {
        val crashFile = File(ctx.filesDir, "last_crash.txt")
        val crashMsg = if (crashFile.exists()) crashFile.readText() else null
        mutableStateOf(
            when {
                !RevivalBridge.isNativeEngineAvailable() ->
                    "Native engine failed to load: ${RevivalBridge.lastLoadError ?: "unknown error"}"
                !crashMsg.isNullOrBlank() -> crashMsg
                else -> "Native engine (librevival_client.so) ready. No roblox.com login required."
            }
        )
    }

    val pickPlace = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(),
    ) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        if (!RevivalBridge.isNativeEngineAvailable()) {
            statusText = "Cannot launch: ${RevivalBridge.lastLoadError ?: "librevival_client.so missing"}"
            return@rememberLauncherForActivityResult
        }
        try {
            val bytes = ctx.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            if (bytes == null || bytes.isEmpty()) {
                statusText = "Could not read selected file."
                return@rememberLauncherForActivityResult
            }
            File(ctx.filesDir, "pending_place.rbxl").writeBytes(bytes)
            File(ctx.filesDir, "pending_ws.txt").delete()
            statusText = "Loaded ${bytes.size} bytes. Launching 3D engine..."
            ctx.startActivity(Intent(ctx, RevivalGameActivity::class.java))
        } catch (e: Exception) {
            statusText = "Error loading file: ${e.message}"
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("BlockBlox Revival", fontWeight = FontWeight.Bold) },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        "Offline Mode (.rbxl / .rbxlx)",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Text(
                        "Plays directly on your phone using the Rust + wgpu + Luau engine. No internet or roblox.com login needed.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Button(
                            onClick = {
                                File(ctx.filesDir, "pending_place.rbxl").delete()
                                File(ctx.filesDir, "pending_ws.txt").delete()
                                statusText = "Launching offline baseplate..."
                                ctx.startActivity(Intent(ctx, RevivalGameActivity::class.java))
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Play Baseplate")
                        }
                        FilledTonalButton(
                            onClick = { pickPlace.launch(arrayOf("*/*")) },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Open .rbxl")
                        }
                    }
                }
            }

            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant,
                ),
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        "Online Revival Server",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                    OutlinedTextField(
                        value = serverUrl,
                        onValueChange = { serverUrl = it },
                        label = { Text("Server URL (Pi Zero 2 W / x86_64)") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        FilledTonalButton(
                            onClick = {
                                scope.launch {
                                    statusText = "Checking $serverUrl..."
                                    try {
                                        val list = RevivalBridge.fetchPiServers(serverUrl)
                                        val first = list.firstOrNull()
                                        statusText = if (first != null) {
                                            "Online: ${first.name} (${first.players} players)"
                                        } else {
                                            "Server reachable, 0 rooms listed."
                                        }
                                    } catch (e: Exception) {
                                        statusText = "Server unreachable: ${e.message}"
                                    }
                                }
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Check Server")
                        }
                        Button(
                            onClick = {
                                val wsUrl = serverUrl.trim()
                                    .replace("https://", "wss://")
                                    .replace("http://", "ws://")
                                    .trimEnd('/') + "/ws"
                                File(ctx.filesDir, "pending_place.rbxl").delete()
                                File(ctx.filesDir, "pending_ws.txt").writeText(wsUrl)
                                statusText = "Joining $wsUrl..."
                                ctx.startActivity(Intent(ctx, RevivalGameActivity::class.java))
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Text("Join Online")
                        }
                    }
                }
            }

            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = statusText,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

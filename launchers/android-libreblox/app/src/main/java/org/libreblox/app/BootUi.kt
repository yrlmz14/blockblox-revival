package org.libreblox.app

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import rikka.shizuku.Shizuku

/** Built-in bootstrapper (ported from Dorfikstrap): Shizuku status, FastFlags, launch shortcut. */
@Composable
fun BootstrapperPage() {
    val ctx = LocalContext.current
    val store = remember { FlagStore(ctx) }
    var tab by remember { mutableIntStateOf(0) }
    var shizuku by remember { mutableStateOf(Sh.state(ctx)) }

    DisposableEffect(Unit) {
        val refresh = { shizuku = Sh.state(ctx) }
        val perm = Shizuku.OnRequestPermissionResultListener { _, _ -> refresh() }
        val got = Shizuku.OnBinderReceivedListener { refresh() }
        val dead = Shizuku.OnBinderDeadListener { refresh() }
        try {
            Shizuku.addRequestPermissionResultListener(perm)
            Shizuku.addBinderReceivedListenerSticky(got)
            Shizuku.addBinderDeadListener(dead)
        } catch (_: Throwable) {}
        onDispose {
            try {
                Shizuku.removeRequestPermissionResultListener(perm)
                Shizuku.removeBinderReceivedListener(got)
                Shizuku.removeBinderDeadListener(dead)
            } catch (_: Throwable) {}
        }
    }

    val titles = listOf(R.string.boot_status, R.string.boot_flags, R.string.boot_shortcut)
    ScrollableTabRow(tab, containerColor = androidx.compose.ui.graphics.Color.Transparent, edgePadding = 0.dp) {
        titles.forEachIndexed { i, t -> Tab(tab == i, { tab = i }, text = { Text(stringResource(t)) }) }
    }
    when (tab) {
        0 -> StatusTab(store, shizuku) { shizuku = Sh.state(ctx) }
        1 -> FlagsTab(store)
        else -> ShortcutTab()
    }
}

@Composable
private fun StatusTab(store: FlagStore, shizuku: Sh.State, refresh: () -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    var applyOnPlay by remember { mutableStateOf(store.applyOnPlay) }
    var msg by remember { mutableStateOf<String?>(null) }
    val roblox = Boot.robloxInstalled(ctx)

    Panel {
        Text(stringResource(if (roblox) R.string.roblox_installed else R.string.roblox_missing), style = MaterialTheme.typography.titleMedium)
        Text(
            stringResource(
                when (shizuku) {
                    Sh.State.NOT_INSTALLED -> R.string.shizuku_not_installed
                    Sh.State.NOT_RUNNING -> R.string.shizuku_not_running
                    Sh.State.NEEDS_PERMISSION -> R.string.shizuku_needs_perm
                    Sh.State.READY -> R.string.shizuku_ready
                },
            ) + if (shizuku == Sh.State.READY) " · " + stringResource(R.string.flags_active_fmt, if (store.master) store.enabledCount() else 0) else "",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        when (shizuku) {
            Sh.State.NOT_INSTALLED -> FilledTonalButton({ openWeb(ctx, "https://shizuku.rikka.app/download/") }) { Text(stringResource(R.string.get_shizuku)) }
            Sh.State.NOT_RUNNING -> FilledTonalButton({
                ctx.packageManager.getLaunchIntentForPackage(Sh.SHIZUKU_PKG)?.let { ctx.startActivity(it.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
            }) { Text(stringResource(R.string.open_shizuku)) }
            Sh.State.NEEDS_PERMISSION -> Button({ Sh.requestPermission() }) { Text(stringResource(R.string.allow_shizuku)) }
            Sh.State.READY -> {}
        }
        OutlinedButton(refresh) { Text(stringResource(R.string.retry)) }
    }
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.apply_on_play), style = MaterialTheme.typography.titleMedium)
                Text(stringResource(R.string.apply_on_play_desc), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Switch(applyOnPlay, { applyOnPlay = it; store.applyOnPlay = it })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton({
                if (!Sh.ready()) msg = ctx.getString(R.string.shizuku_not_ready)
                else scope.launch {
                    withContext(Dispatchers.IO) { Boot.apply(ctx, false) }
                    msg = ctx.getString(R.string.applied)
                }
            }, Modifier.weight(1f)) { Text(stringResource(R.string.apply)) }
            Button({ Boot.launch(ctx, null, null); }, Modifier.weight(1f), enabled = roblox) { Text(stringResource(R.string.apply_launch)) }
        }
        msg?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary) }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FlagsTab(store: FlagStore) {
    var master by remember { mutableStateOf(store.master) }
    var dialog by remember { mutableStateOf<Flag?>(null) }
    var adding by remember { mutableStateOf(false) }
    var confirmClear by remember { mutableStateOf(false) }
    var note by remember { mutableStateOf<String?>(null) }
    val ctx = LocalContext.current

    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(stringResource(R.string.use_flags), Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
            Switch(master, { master = it; store.master = it })
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button({ adding = true }, Modifier.weight(1f)) { Text(stringResource(R.string.add_flag)) }
            OutlinedButton({ confirmClear = true }, Modifier.weight(1f)) { Text(stringResource(R.string.delete_all)) }
        }
        Text(stringResource(R.string.flags_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        note?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
    }
    Panel {
        Text(stringResource(R.string.presets), style = MaterialTheme.typography.titleMedium)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FlagStore.PRESETS.forEach { (n, v, d) -> AssistChip({ store.put(n, v) }, { Text("$d ($n=$v)") }) }
        }
    }
    Panel {
        if (store.flags.isEmpty()) Text(stringResource(R.string.no_flags), color = MaterialTheme.colorScheme.onSurfaceVariant)
        store.flags.toList().forEach { f ->
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f).clickable { dialog = f }) {
                    Text(f.name, style = MaterialTheme.typography.bodyMedium)
                    Text("= ${f.value}", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Switch(f.enabled, { store.setEnabled(f.name, it) })
                IconButton({ store.remove(f.name) }) { Icon(Icons.Default.Close, stringResource(R.string.delete)) }
            }
        }
    }

    if (confirmClear) AlertDialog(
        onDismissRequest = { confirmClear = false },
        title = { Text(stringResource(R.string.delete_all_q)) }, text = { Text(stringResource(R.string.delete_all_msg)) },
        confirmButton = {
            TextButton({
                confirmClear = false; store.clear()
                if (Sh.ready()) Thread { Sh.run("rm -f ${Sh.FLAGS_FILE}") }.start()
            }) { Text(stringResource(R.string.delete)) }
        },
        dismissButton = { TextButton({ confirmClear = false }) { Text(stringResource(R.string.cancel)) } },
    )

    dialog?.let { f ->
        var value by remember(f) { mutableStateOf(f.value) }
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.edit_flag)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(f.name, fontFamily = FontFamily.Monospace)
                    OutlinedTextField(value, { value = it }, label = { Text(stringResource(R.string.flag_value)) }, singleLine = true)
                }
            },
            confirmButton = {
                TextButton({ note = if (store.put(f.name, value.trim())) null else ctx.getString(R.string.invalid_value); dialog = null }) { Text(stringResource(R.string.ok)) }
            },
            dismissButton = { TextButton({ dialog = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    if (adding) {
        var json by remember { mutableStateOf(false) }
        var name by remember { mutableStateOf("") }
        var value by remember { mutableStateOf("") }
        var text by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { adding = false },
            title = { Text(stringResource(R.string.add_flag)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        FilterChip(!json, { json = false }, { Text(stringResource(R.string.name_value)) })
                        FilterChip(json, { json = true }, { Text("JSON") })
                    }
                    if (!json) {
                        OutlinedTextField(name, { name = it }, label = { Text(stringResource(R.string.flag_name)) }, singleLine = true)
                        OutlinedTextField(value, { value = it }, label = { Text(stringResource(R.string.flag_value)) }, singleLine = true)
                    } else {
                        OutlinedTextField(text, { text = it }, minLines = 6, placeholder = { Text("{\"FFlagDebugDisplayFPS\": \"true\"}") })
                    }
                }
            },
            confirmButton = {
                TextButton({
                    note = if (!json) {
                        if (store.put(name.trim(), value.trim())) null else ctx.getString(R.string.invalid_name_value)
                    } else {
                        val r = store.importJson(text)
                        if (r == null) ctx.getString(R.string.invalid_json) else if (r.second > 0) ctx.getString(R.string.added_skipped_fmt, r.first, r.second) else null
                    }
                    adding = false
                }) { Text(stringResource(R.string.add)) }
            },
            dismissButton = { TextButton({ adding = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun ShortcutTab() {
    val ctx = LocalContext.current
    val sp = remember { ctx.getSharedPreferences("lb", android.content.Context.MODE_PRIVATE) }
    var name by remember { mutableStateOf(sp.getString("shortcut_name", "Roblox") ?: "Roblox") }
    var msg by remember { mutableStateOf<String?>(null) }
    Panel {
        Text(stringResource(R.string.shortcut_info), style = MaterialTheme.typography.bodyMedium)
        OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.shortcut_name)) }, singleLine = true)
        Button({
            val n = name.trim().ifEmpty { "Roblox" }
            sp.edit().putString("shortcut_name", n).apply()
            if (!ShortcutManagerCompat.isRequestPinShortcutSupported(ctx)) msg = ctx.getString(R.string.no_pin_support)
            else {
                val info = ShortcutInfoCompat.Builder(ctx, "launch_roblox")
                    .setShortLabel(n)
                    .setIcon(IconCompat.createWithResource(ctx, R.mipmap.ic_launcher))
                    .setIntent(Intent(ctx, LaunchActivity::class.java).setAction(Intent.ACTION_VIEW))
                    .build()
                ShortcutManagerCompat.requestPinShortcut(ctx, info, null)
            }
        }) { Text(stringResource(R.string.create_shortcut)) }
        msg?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
    }
}

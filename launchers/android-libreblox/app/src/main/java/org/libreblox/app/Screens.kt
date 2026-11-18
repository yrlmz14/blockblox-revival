package org.libreblox.app

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import java.util.Locale
import kotlinx.coroutines.launch

// ---------------- helpers ----------------

fun compact(n: Long): String = when {
    n >= 1_000_000_000 -> String.format(Locale.getDefault(), "%.1fB", n / 1e9)
    n >= 1_000_000 -> String.format(Locale.getDefault(), "%.1fM", n / 1e6)
    n >= 1_000 -> String.format(Locale.getDefault(), "%.1fK", n / 1e3)
    else -> n.toString()
}

fun likePercent(g: Game): Int? = (g.up + g.down).takeIf { it > 0 }?.let { (g.up * 100 / it).toInt() }

const val DORFIKSTRAP = "org.dorfikstrap.app"

fun dorfikstrapInstalled(ctx: Context): Boolean = try {
    ctx.packageManager.getPackageInfo(DORFIKSTRAP, 0); true
} catch (e: Exception) { false }

/** Starts a roblox:// link: FastFlags are applied first (built-in bootstrapper, needs Shizuku). */
fun startRoblox(ctx: Context, uri: String, fallbackWeb: String) = Boot.launch(ctx, uri, fallbackWeb)

fun openPlay(ctx: Context, placeId: Long) =
    startRoblox(ctx, "roblox://experiences/start?placeId=$placeId", "https://www.roblox.com/games/$placeId")

fun openWeb(ctx: Context, url: String) {
    try { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) } catch (_: Exception) {}
}

@Composable
fun <T> rememberLoad(key: Any, block: suspend () -> T): Pair<Load<T>, () -> Unit> {
    var state by remember(key) { mutableStateOf<Load<T>>(Load.Busy) }
    var attempt by remember(key) { mutableIntStateOf(0) }
    LaunchedEffect(key, attempt) {
        state = Load.Busy
        state = try { Load.Ok(block()) } catch (e: Exception) { Load.Err(e.message ?: e.toString()) }
    }
    return state to { attempt++ }
}

@Composable
fun Centered(content: @Composable () -> Unit) = Box(Modifier.fillMaxSize().padding(24.dp), Alignment.Center) { content() }

@Composable
fun ErrorBox(msg: String, onRetry: () -> Unit) = Centered {
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.error_generic, msg), style = MaterialTheme.typography.bodyMedium)
        FilledTonalButton(onClick = onRetry) { Text(stringResource(R.string.retry)) }
    }
}

@Composable
fun SearchField(value: String, hint: String, onChange: (String) -> Unit, onSearch: () -> Unit, modifier: Modifier = Modifier) {
    OutlinedTextField(
        value = value, onValueChange = onChange, singleLine = true, modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(28.dp),
        placeholder = { Text(hint) },
        leadingIcon = { Icon(Icons.Default.Search, null) },
        trailingIcon = {
            if (value.isNotEmpty()) IconButton(onClick = { onChange(""); onSearch() }) { Icon(Icons.Default.Clear, stringResource(R.string.search_clear)) }
        },
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = { onSearch() }),
    )
}

@Composable
fun GameCard(g: Game, onClick: () -> Unit) {
    Card(
        onClick = onClick, shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        AsyncImage(
            model = g.icon, contentDescription = null, contentScale = ContentScale.Crop,
            modifier = Modifier.fillMaxWidth().aspectRatio(1f).clip(RoundedCornerShape(topStart = 16.dp, topEnd = 16.dp))
                .let { it },
        )
        Column(Modifier.padding(10.dp)) {
            Text(g.name, maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.titleSmall, minLines = 2)
            val parts = listOfNotNull(
                if (g.players > 0) stringResource(R.string.playing_fmt, compact(g.players)) else null,
                likePercent(g)?.let { stringResource(R.string.likes_fmt, it) },
            )
            if (parts.isNotEmpty()) Text(parts.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
fun GameGrid(list: List<Game>, header: (@Composable () -> Unit)? = null, onGame: (Game) -> Unit) {
    LazyVerticalGrid(
        columns = GridCells.Adaptive(150.dp), contentPadding = PaddingValues(16.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp),
        modifier = Modifier.fillMaxSize(),
    ) {
        if (header != null) item(span = { GridItemSpan(maxLineSpan) }) { header() }
        items(list, key = { it.universeId }) { GameCard(it) { onGame(it) } }
    }
}

// ---------------- tabs ----------------

@Composable
fun UserRow(u: User, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(16.dp, 8.dp), verticalAlignment = Alignment.CenterVertically) {
        AsyncImage(u.avatar, null, Modifier.size(52.dp).clip(CircleShape), contentScale = ContentScale.Crop)
        Spacer(Modifier.width(14.dp))
        Column {
            Text(u.display, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("@${u.name}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

// ---------------- detail screens ----------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DetailScaffold(title: String, onBack: () -> Unit, content: @Composable (Modifier) -> Unit) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                navigationIcon = { IconButton(onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back)) } },
            )
        },
    ) { content(Modifier.padding(it)) }
}

@Composable
fun Stat(label: String, value: String, modifier: Modifier = Modifier) {
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(value, style = MaterialTheme.typography.titleMedium)
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun GameScreen(base: Game, onBack: () -> Unit, onUser: (Long) -> Unit) {
    val ctx = LocalContext.current
    val (load, retry) = rememberLoad(base.universeId) { Api.gameDetails(base) }
    DetailScaffold(base.name, onBack) { m ->
        when (val l = load) {
            Load.Idle, Load.Busy -> Box(m) { Centered { CircularProgressIndicator() } }
            is Load.Err -> Box(m) { ErrorBox(l.msg, retry) }
            is Load.Ok -> {
                val d = l.v
                Column(m.fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        AsyncImage(d.game.icon, null, Modifier.size(104.dp).clip(RoundedCornerShape(20.dp)), contentScale = ContentScale.Crop)
                        Spacer(Modifier.width(16.dp))
                        Column {
                            Text(d.game.name, style = MaterialTheme.typography.titleLarge)
                            if (d.creator.isNotEmpty()) Text(stringResource(R.string.by_fmt, d.creator), color = MaterialTheme.colorScheme.onSurfaceVariant)
                            likePercent(d.game)?.let { Text(stringResource(R.string.likes_fmt, it), style = MaterialTheme.typography.bodyMedium) }
                        }
                    }
                    val cfg = LocalCfg.current
                    Button({ cfg.recordPlayed(d.game); openPlay(ctx, d.game.placeId) }, Modifier.fillMaxWidth().height(52.dp),
                        colors = androidx.compose.material3.ButtonDefaults.buttonColors(containerColor = RobloxGreen, contentColor = androidx.compose.ui.graphics.Color.White)) {
                        Icon(Icons.Default.PlayArrow, null)
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.play))
                    }
                    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(20.dp)) {
                        Row(Modifier.fillMaxWidth().padding(vertical = 14.dp)) {
                            Stat(stringResource(R.string.stat_playing), compact(d.game.players), Modifier.weight(1f))
                            Stat(stringResource(R.string.stat_visits), compact(d.visits), Modifier.weight(1f))
                            Stat(stringResource(R.string.stat_favorites), compact(d.favorites), Modifier.weight(1f))
                            if (d.maxPlayers > 0) Stat(stringResource(R.string.stat_max_players), d.maxPlayers.toString(), Modifier.weight(1f))
                        }
                    }
                    if (d.shots.isNotEmpty()) {
                        Text(stringResource(R.string.screenshots), style = MaterialTheme.typography.titleMedium)
                        LazyRow(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            items(d.shots) { AsyncImage(it, null, Modifier.height(170.dp).aspectRatio(16f / 9f).clip(RoundedCornerShape(14.dp)), contentScale = ContentScale.Crop) }
                        }
                    }
                    if (d.description.isNotBlank()) {
                        Text(stringResource(R.string.description), style = MaterialTheme.typography.titleMedium)
                        Text(d.description, style = MaterialTheme.typography.bodyMedium)
                    }
                    TextButton({ openWeb(ctx, "https://www.roblox.com/games/${d.game.placeId}") }) { Text(stringResource(R.string.open_web)) }
                }
            }
        }
    }
}

// ---------------- login ----------------

@android.annotation.SuppressLint("SetJavaScriptEnabled")
@Composable
fun LoginScreen(onBack: () -> Unit, onDone: () -> Unit) {
    val ctx = LocalContext.current
    var done by remember { mutableStateOf(false) }
    DetailScaffold(stringResource(R.string.login), onBack) { m ->
        androidx.compose.ui.viewinterop.AndroidView(
            modifier = m.fillMaxSize(),
            factory = { c ->
                android.webkit.WebView(c).apply {
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    val cm = android.webkit.CookieManager.getInstance()
                    cm.setAcceptCookie(true)
                    cm.setAcceptThirdPartyCookies(this, true)
                    webViewClient = object : android.webkit.WebViewClient() {
                        override fun onPageFinished(view: android.webkit.WebView, url: String?) {
                            if (done) return
                            val cookies = cm.getCookie("https://www.roblox.com").orEmpty()
                            val v = cookies.split(";").map { it.trim() }
                                .firstOrNull { it.startsWith(".ROBLOSECURITY=") }?.removePrefix(".ROBLOSECURITY=")
                            if (!v.isNullOrEmpty()) {
                                done = true
                                Session.save(ctx, v)
                                onDone()
                            }
                        }
                    }
                    // Start from a logged-out page so another account can be added.
                    cm.removeAllCookies { loadUrl("https://www.roblox.com/login") }
                }
            },
            onRelease = { it.destroy() },
        )
    }
}


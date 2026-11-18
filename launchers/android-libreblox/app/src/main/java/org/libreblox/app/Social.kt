package org.libreblox.app

import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

val RobloxGreen = Color(0xFF00B06F)
private val PresenceGreen = Color(0xFF00B06F)
private val PresenceBlue = Color(0xFF3B8CFF)

@Composable
fun NeedLogin(onLogin: () -> Unit) = Centered {
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.login_required), style = MaterialTheme.typography.bodyLarge)
        Button(onLogin) { Text(stringResource(R.string.login)) }
    }
}

@Composable
fun SectionHeader(text: String) =
    Text(text, Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium)

/** Roblox-style square tile used in horizontal rows. */
@Composable
fun GameTile(g: Game, onClick: () -> Unit) {
    Column(Modifier.width(132.dp).clickable(onClick = onClick)) {
        AsyncImage(g.icon, null, Modifier.size(132.dp).clip(RoundedCornerShape(14.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
        Text(g.name, Modifier.padding(top = 6.dp), maxLines = 2, minLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall)
        val parts = listOfNotNull(likePercent(g)?.let { "$it%" }, if (g.players > 0) compact(g.players) else null)
        if (parts.isNotEmpty()) Text(parts.joinToString(" · "), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun GameRow(title: String, games: Load<List<Game>>, onRetry: () -> Unit, onGame: (Game) -> Unit) {
    SectionHeader(title)
    when (games) {
        Load.Idle, Load.Busy -> Box(Modifier.fillMaxWidth().height(170.dp), Alignment.Center) { CircularProgressIndicator() }
        is Load.Err -> Row(Modifier.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("…", color = MaterialTheme.colorScheme.error)
            TextButton(onRetry) { Text(stringResource(R.string.retry)) }
        }
        is Load.Ok -> LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            items(games.v, key = { it.universeId }) { GameTile(it) { onGame(it) } }
        }
    }
}

@Composable
fun PresenceAvatar(f: Friend, size: Int, onClick: () -> Unit) {
    Box(Modifier.size(size.dp).clickable(onClick = onClick)) {
        AsyncImage(f.user.avatar, null, Modifier.size(size.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
        val t = f.presence?.type ?: 0
        if (t > 0) Box(
            Modifier.size((size / 4).dp).align(Alignment.BottomEnd)
                .background(MaterialTheme.colorScheme.background, CircleShape).padding(2.dp)
                .background(if (t == 3) PresenceBlue else PresenceGreen, CircleShape),
        )
    }
}

// ---------------- Home ----------------

private val FOR_YOU = Regex("recommend|for you|based|because|friends|revisit|continue|similar", RegexOption.IGNORE_CASE)

@Composable
private fun SortRow(vm: AppVm, sort: Sort, onGame: (Game) -> Unit) {
    LaunchedEffect(sort.id) { vm.ensureRow(sort.id) }
    GameRow(sort.title, vm.rowData[sort.id] ?: Load.Busy, { vm.ensureRow(sort.id) }, onGame)
}

@Composable
fun HomeTab(
    vm: AppVm, modifier: Modifier, forYou: Boolean,
    onGame: (Game) -> Unit, onUser: (Long) -> Unit, onLogin: () -> Unit, onAddFriends: () -> Unit, onContinue: () -> Unit,
) {
    val cfg = LocalCfg.current
    val personal = vm.sorts.filter { FOR_YOU.containsMatchIn(it.title) || FOR_YOU.containsMatchIn(it.id) }.ifEmpty { vm.sorts.take(3) }
    val sorts = cfg.orderedSorts(if (forYou) personal else vm.sorts).filter { it.id !in cfg.hiddenSorts }
    LazyColumn(modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
        if (forYou) {
            if (vm.account == null) item {
                Row(Modifier.fillMaxWidth().padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(stringResource(R.string.login_required), Modifier.weight(1f), color = MaterialTheme.colorScheme.onSurfaceVariant)
                    FilledTonalButton(onLogin) { Text(stringResource(R.string.login)) }
                }
            } else if (cfg.on("home_addfriends")) item {
                FilledTonalButton(onAddFriends, Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Text(stringResource(R.string.add_friends))
                }
            }
            val fr = vm.friends
            if (cfg.on("home_friends") && vm.account != null && fr is Load.Ok && fr.v.isNotEmpty()) item {
                SectionHeader(stringResource(R.string.friends_count_fmt, fr.v.size))
                LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    items(fr.v) { f ->
                        Column(Modifier.width(76.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                            PresenceAvatar(f, 64) { onUser(f.user.id) }
                            Text(f.user.display, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall, modifier = Modifier.padding(top = 4.dp))
                            if (f.presence?.type == 2 && f.presence.location.isNotEmpty()) Text(f.presence.location, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall, color = RobloxGreen)
                        }
                    }
                }
            }
            if (cfg.on("home_continue") && cfg.history.isNotEmpty()) item {
                Text(
                    stringResource(R.string.continue_title) + "  ›",
                    Modifier.clickable(onClick = onContinue).padding(horizontal = 16.dp, vertical = 8.dp),
                    style = MaterialTheme.typography.titleMedium,
                )
                LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    items(cfg.history.take(15)) { GameTile(it.asGame()) { onGame(it.asGame()) } }
                }
            }
        }
        if (vm.homeError != null) item { ErrorBox(vm.homeError ?: "", vm::loadHome) }
        if (vm.homeBusy) item { Box(Modifier.fillMaxWidth().height(200.dp), Alignment.Center) { CircularProgressIndicator() } }
        items(sorts.size) { i -> SortRow(vm, sorts[i], onGame) }
    }
}

// ---------------- Search ----------------

@Composable
fun SearchTab(vm: AppVm, modifier: Modifier, onGame: (Game) -> Unit, onUser: (User) -> Unit) {
    Column(modifier.fillMaxSize()) {
        if (vm.searchPeople)
            SearchField(vm.userQuery, stringResource(R.string.search_people), { vm.userQuery = it }, vm::searchUsers, Modifier.padding(16.dp, 8.dp))
        else
            SearchField(vm.gameQuery, stringResource(R.string.search_games), { vm.gameQuery = it }, vm::searchGames, Modifier.padding(16.dp, 8.dp))
        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(!vm.searchPeople, { vm.searchPeople = false }, { Text(stringResource(R.string.tab_games)) })
            FilterChip(vm.searchPeople, { vm.searchPeople = true }, { Text(stringResource(R.string.tab_people)) })
        }
        if (vm.searchPeople) {
            when (val s = vm.userSearch) {
                Load.Idle -> Centered { Text(stringResource(R.string.people_hint), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                Load.Busy -> Centered { CircularProgressIndicator() }
                is Load.Err -> ErrorBox(s.msg, vm::searchUsers)
                is Load.Ok -> if (s.v.isEmpty()) Centered { Text(stringResource(R.string.no_results)) }
                else LazyColumn(Modifier.fillMaxSize()) { items(s.v, key = { it.id }) { UserRow(it) { onUser(it) } } }
            }
        } else {
            when (val s = vm.gameSearch) {
                Load.Idle -> Centered { Text(stringResource(R.string.games_hint), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                Load.Busy -> Centered { CircularProgressIndicator() }
                is Load.Err -> ErrorBox(s.msg, vm::searchGames)
                is Load.Ok -> if (s.v.isEmpty()) Centered { Text(stringResource(R.string.no_results)) } else GameGrid(s.v, onGame = onGame)
            }
        }
    }
}

// ---------------- Friends ----------------

@Composable
fun FriendsTab(vm: AppVm, modifier: Modifier, onUser: (Long) -> Unit, onLogin: () -> Unit) {
    val ctx = LocalContext.current
    if (vm.account == null) { Box(modifier) { NeedLogin(onLogin) }; return }
    Column(modifier.fillMaxSize()) {
        when (val f = vm.friends) {
            Load.Idle, Load.Busy -> Centered { CircularProgressIndicator() }
            is Load.Err -> ErrorBox(f.msg, vm::loadFriends)
            is Load.Ok -> if (f.v.isEmpty()) Centered { Text(stringResource(R.string.no_friends)) }
            else LazyColumn(Modifier.fillMaxSize()) {
                items(f.v) { fr ->
                    val p = fr.presence
                    Row(Modifier.fillMaxWidth().clickable { onUser(fr.user.id) }.padding(16.dp, 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        PresenceAvatar(fr, 52) { onUser(fr.user.id) }
                        Spacer(Modifier.width(14.dp))
                        Column(Modifier.weight(1f)) {
                            Text(fr.user.display, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            val status = when (p?.type ?: 0) {
                                2 -> if (p!!.location.isNotEmpty()) stringResource(R.string.friend_in_game_fmt, p.location) else stringResource(R.string.friend_in_game)
                                3 -> stringResource(R.string.friend_studio)
                                1 -> stringResource(R.string.friend_online)
                                else -> stringResource(R.string.friend_offline)
                            }
                            Text(status, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        }
                        if (p != null && p.type == 2 && p.placeId != 0L) {
                            Button({ openJoin(ctx, p.placeId, p.gameId) }, colors = ButtonDefaults.buttonColors(containerColor = RobloxGreen, contentColor = Color.White)) {
                                Text(stringResource(R.string.join))
                            }
                        }
                    }
                }
            }
        }
    }
}

fun openJoin(ctx: android.content.Context, placeId: Long, gameId: String) =
    startRoblox(ctx, "roblox://experiences/start?placeId=$placeId" + if (gameId.isNotEmpty()) "&gameInstanceId=$gameId" else "",
        "https://www.roblox.com/games/$placeId")

// ---------------- Chat ----------------

@Composable
fun ChatTab(vm: AppVm, modifier: Modifier, filter: String, onChat: (Convo) -> Unit, onLogin: () -> Unit) {
    if (vm.account == null) { Box(modifier) { NeedLogin(onLogin) }; return }
    Column(modifier.fillMaxSize()) {
        when (val c = vm.convos) {
            Load.Idle, Load.Busy -> Centered { CircularProgressIndicator() }
            is Load.Err -> ErrorBox(c.msg, vm::loadConvos)
            is Load.Ok -> if (c.v.isEmpty()) Centered { Text(stringResource(R.string.chat_empty)) }
            else LazyColumn(Modifier.fillMaxSize()) {
                items(c.v.filter { filter.isBlank() || it.title.contains(filter, true) }) { cv ->
                    Row(Modifier.fillMaxWidth().clickable { onChat(cv) }.padding(16.dp, 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        AsyncImage(cv.avatar, null, Modifier.size(48.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
                        Spacer(Modifier.width(14.dp))
                        Text(cv.title, Modifier.weight(1f), style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        if (cv.unread) Box(Modifier.size(10.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
                    }
                }
            }
        }
    }
}

@Composable
fun ChatScreen(vm: AppVm, convo: Convo, onBack: () -> Unit) {
    val me = vm.account?.id ?: 0L
    val cookie = remember { vm.cookie() }
    val scope = rememberCoroutineScope()
    val (load, retry) = rememberLoad(convo.id) { Api.messages(convo.id, cookie ?: throw ApiException(401, "no session")) }
    var text by remember { mutableStateOf("") }
    var sendError by remember { mutableStateOf<String?>(null) }
    DetailScaffold(convo.title, onBack) { m ->
        Column(m.fillMaxSize().imePadding()) {
            Box(Modifier.weight(1f)) {
                when (val l = load) {
                    Load.Idle, Load.Busy -> Centered { CircularProgressIndicator() }
                    is Load.Err -> ErrorBox(l.msg, retry)
                    is Load.Ok -> {
                        val state = rememberLazyListState()
                        LaunchedEffect(l.v.size) { if (l.v.isNotEmpty()) state.scrollToItem(l.v.size - 1) }
                        LazyColumn(Modifier.fillMaxSize(), state = state, contentPadding = PaddingValues(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                            items(l.v) { msg ->
                                val mine = msg.senderId == me
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start) {
                                    Text(
                                        msg.text,
                                        Modifier.widthIn(max = 300.dp).clip(RoundedCornerShape(16.dp))
                                            .background(if (mine) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surfaceContainerHigh)
                                            .padding(12.dp, 8.dp),
                                        color = if (mine) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface,
                                    )
                                }
                            }
                        }
                    }
                }
            }
            sendError?.let { Text(it, Modifier.padding(horizontal = 12.dp), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
            Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(text, { text = it }, Modifier.weight(1f), shape = RoundedCornerShape(24.dp), placeholder = { Text(stringResource(R.string.chat_input)) }, maxLines = 4)
                Spacer(Modifier.width(8.dp))
                Button({
                    val t = text.trim()
                    if (t.isEmpty() || cookie == null) return@Button
                    text = ""
                    scope.launch {
                        try { Api.sendMessage(convo.id, t, cookie); sendError = null; retry() }
                        catch (e: Exception) { sendError = e.message; text = t }
                    }
                }) { Text(stringResource(R.string.chat_send)) }
            }
        }
    }
}

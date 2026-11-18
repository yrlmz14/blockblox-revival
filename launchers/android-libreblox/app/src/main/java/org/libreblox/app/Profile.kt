package org.libreblox.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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

@Composable
private fun <T> Section(title: String, key: Any, load: suspend () -> List<T>, content: @Composable (List<T>) -> Unit) {
    val (state, _) = rememberLoad(key, load)
    when (val s = state) {
        is Load.Ok -> if (s.v.isNotEmpty()) {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                content(s.v)
            }
        }
        is Load.Err, Load.Idle -> {}
        Load.Busy -> Box(Modifier.fillMaxWidth().height(32.dp), Alignment.CenterStart) { CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp) }
    }
}

@Composable
private fun EntryCard(e: Entry, onClick: () -> Unit) {
    Column(Modifier.width(120.dp).clickable(onClick = onClick)) {
        AsyncImage(e.image, null, Modifier.size(120.dp).clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
        Text(e.title, Modifier.padding(top = 4.dp), maxLines = 2, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelMedium)
        if (e.subtitle.isNotBlank()) Text(e.subtitle, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun EntryRow(items: List<Entry>, onItem: ((Entry) -> Unit)? = null) {
    val ctx = LocalContext.current
    LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) { items(items) { e -> EntryCard(e) { if (onItem != null) onItem(e) else openWeb(ctx, e.url) } } }
}

private enum class Confirm { UNFRIEND, BLOCK }

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun UserScreen(
    vm: AppVm, id: Long, onBack: () -> Unit, onGame: (Game) -> Unit, onUser: (User) -> Unit,
    onChat: (Convo) -> Unit, onInventory: () -> Unit, onFavorites: () -> Unit, onGroup: (Long) -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val cookie = remember(vm.account) { vm.cookie() }
    val me = vm.account
    val own = me?.id == id
    val (load, retry) = rememberLoad(id) { Api.userDetails(id) }
    val title = (load as? Load.Ok)?.v?.user?.display ?: ""

    DetailScaffold(title, onBack) { m ->
        when (val l = load) {
            Load.Idle, Load.Busy -> Box(m) { Centered { CircularProgressIndicator() } }
            is Load.Err -> Box(m) { ErrorBox(l.msg, retry) }
            is Load.Ok -> {
                val d = l.v
                val isFriend = (vm.friends as? Load.Ok)?.v?.any { it.user.id == id } == true
                var unfriended by remember(id) { mutableStateOf(false) }
                var following by remember(id) { mutableStateOf<Boolean?>(null) }
                var msg by remember(id) { mutableStateOf<String?>(null) }
                var confirm by remember(id) { mutableStateOf<Confirm?>(null) }
                var presence by remember(id) { mutableStateOf<Presence?>(null) }
                val friendNow = isFriend && !unfriended

                androidx.compose.runtime.LaunchedEffect(id, cookie) {
                    if (cookie != null) {
                        presence = runCatching { Api.presenceOf(id, cookie) }.getOrNull()
                        if (!own) following = runCatching { Api.isFollowing(id, cookie) }.getOrNull()
                    }
                }
                fun act(ok: String, block: suspend (String) -> Unit) {
                    val c = cookie ?: run { vm.stack.add(Screen.LoginS); return }
                    scope.launch { msg = try { block(c); ok } catch (e: Exception) { e.message } }
                }

                Column(m.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    // skin on top
                    Box(Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceContainer), Alignment.Center) {
                        AsyncImage(d.bigAvatar, null, Modifier.fillMaxWidth().height(320.dp), contentScale = ContentScale.Fit)
                    }
                    Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            AsyncImage(d.headshot, null, Modifier.size(72.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
                            Spacer(Modifier.width(14.dp))
                            Column {
                                val (prem, _) = rememberLoad(id to "prem") { runCatching { Api.isPremium(id, cookie) }.getOrDefault(false) }
                                NameLine(d.user.display, d.user.verified, (prem as? Load.Ok)?.v == true, MaterialTheme.typography.headlineSmall)
                                Text("@${d.user.name}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                                val p = presence
                                if (p != null && p.type > 0) Row(verticalAlignment = Alignment.CenterVertically) {
                                    Box(Modifier.size(9.dp).background(if (p.type == 3) Color(0xFF3B8CFF) else RobloxGreen, CircleShape))
                                    Spacer(Modifier.width(6.dp))
                                    Text(
                                        when (p.type) {
                                            2 -> if (p.location.isNotEmpty()) stringResource(R.string.friend_in_game_fmt, p.location) else stringResource(R.string.friend_in_game)
                                            3 -> stringResource(R.string.friend_studio)
                                            else -> stringResource(R.string.friend_online)
                                        },
                                        style = MaterialTheme.typography.bodySmall, color = RobloxGreen, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                        }
                        if (d.banned) Text(stringResource(R.string.banned), color = MaterialTheme.colorScheme.error)
                        if (d.description.isNotBlank()) Text(d.description, style = MaterialTheme.typography.bodyMedium)

                        // social links, previous usernames, join date
                        val (soc, _) = rememberLoad(id to "soc") { runCatching { Api.socials(id, cookie) }.getOrDefault(emptyList()) }
                        (soc as? Load.Ok)?.v?.takeIf { it.isNotEmpty() }?.let { links ->
                            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                links.forEach { (n, u) -> AssistChip({ openWeb(ctx, u) }, { Text(n) }) }
                            }
                        }
                        val (names, _) = rememberLoad(id to "names") { runCatching { Api.usernameHistory(id) }.getOrDefault(emptyList()) }
                        (names as? Load.Ok)?.v?.takeIf { it.isNotEmpty() }?.let {
                            Text(stringResource(R.string.prev_names_fmt, it.joinToString(", ")), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        if (d.created.isNotEmpty()) Text(stringResource(R.string.joined_fmt, d.created), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)

                        // actions
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            val p = presence
                            if (!own && p != null && p.type == 2 && p.placeId != 0L)
                                Button({ openJoin(ctx, p.placeId, p.gameId) }, colors = ButtonDefaults.buttonColors(containerColor = RobloxGreen, contentColor = Color.White)) { Text(stringResource(R.string.join)) }
                            if (!own) FilledTonalButton({
                                act("") { c -> onChat(Convo(Api.newConversation(id, c), d.user.display, false, d.headshot)) }
                            }) { Text(stringResource(R.string.prof_chat)) }
                            if (own) FilledTonalButton({}) { Text(stringResource(R.string.change_display)) }
                            if (!own) {
                                val f = following
                                FilledTonalButton({
                                    act(ctx.getString(R.string.action_done)) { c ->
                                        Api.friendAction(id, if (f == true) "unfollow" else "follow", c); following = f != true
                                    }
                                }) { Text(stringResource(if (f == true) R.string.unfollow else R.string.follow)) }
                            }
                            FilledTonalButton(onInventory) { Text(stringResource(R.string.prof_inventory)) }
                            FilledTonalButton(onFavorites) { Text(stringResource(R.string.drawer_favorites)) }
                            if (!own && friendNow) OutlinedButton({ confirm = Confirm.UNFRIEND }) { Text(stringResource(R.string.unfriend)) }
                            if (!own) OutlinedButton({ confirm = Confirm.BLOCK }) { Text(stringResource(R.string.block_user)) }
                            if (!own) OutlinedButton({ openWeb(ctx, "https://www.roblox.com/abusereport/userprofile?id=$id") }) { Text(stringResource(R.string.report)) }
                        }
                        msg?.takeIf { it.isNotEmpty() }?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary) }

                        Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(20.dp)) {
                            Row(Modifier.fillMaxWidth().padding(vertical = 14.dp)) {
                                Stat(stringResource(R.string.friends), compact(d.friends), Modifier.weight(1f))
                                Stat(stringResource(R.string.followers), compact(d.followers), Modifier.weight(1f))
                                Stat(stringResource(R.string.following), compact(d.following), Modifier.weight(1f))
                            }
                        }

                        Section(stringResource(R.string.wearing_title), id to "wear", { Api.wearing(id) }) { EntryRow(it) }
                        Section(stringResource(R.string.drawer_favorites), id to "fav", { Api.favoriteGames(id, cookie) }) { games ->
                            LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) { items(games) { g -> Box(Modifier.width(140.dp)) { GameCard(g) { onGame(g) } } } }
                        }
                        if (d.friendList.isNotEmpty()) {
                            Text(stringResource(R.string.friends_list), style = MaterialTheme.typography.titleMedium)
                            LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                                items(d.friendList) { f ->
                                    Column(Modifier.width(76.dp).clickable { onUser(f) }, horizontalAlignment = Alignment.CenterHorizontally) {
                                        AsyncImage(f.avatar, null, Modifier.size(64.dp).clip(CircleShape), contentScale = ContentScale.Crop)
                                        Text(f.display, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.labelSmall)
                                    }
                                }
                            }
                        }
                        Section(stringResource(R.string.drawer_communities), id to "groups", { Api.groups(id, cookie) }) { gl -> EntryRow(gl) { onGroup(it.id) } }
                        Section(stringResource(R.string.badges_title), id to "badges", { Api.badges(id) }) { EntryRow(it) }
                        if (d.games.isNotEmpty()) {
                            Text(stringResource(R.string.user_games), style = MaterialTheme.typography.titleMedium)
                            LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                items(d.games) { g -> Box(Modifier.width(140.dp)) { GameCard(g) { onGame(g) } } }
                            }
                        }
                        TextButton({ openWeb(ctx, "https://www.roblox.com/users/${d.user.id}/profile") }) { Text(stringResource(R.string.open_web)) }
                    }
                }

                confirm?.let { c ->
                    AlertDialog(
                        onDismissRequest = { confirm = null },
                        title = { Text(stringResource(if (c == Confirm.UNFRIEND) R.string.unfriend else R.string.block_user) + ": " + d.user.display) },
                        text = { Text(stringResource(R.string.are_you_sure)) },
                        confirmButton = {
                            TextButton({
                                confirm = null
                                act(ctx.getString(R.string.action_done)) { ck ->
                                    if (c == Confirm.UNFRIEND) { Api.friendAction(id, "unfriend", ck); unfriended = true; vm.loadFriends() }
                                    else Api.block(id, ck)
                                }
                            }) { Text(stringResource(R.string.ok)) }
                        },
                        dismissButton = { TextButton({ confirm = null }) { Text(stringResource(R.string.cancel)) } },
                    )
                }
            }
        }
    }
}

@Composable
fun UserFavoritesScreen(vm: AppVm, id: Long, onBack: () -> Unit, onGame: (Game) -> Unit) {
    val (state, reload) = rememberLoad(id) { Api.favoriteGames(id, vm.cookie()) }
    DetailScaffold(stringResource(R.string.drawer_favorites), onBack) { m ->
        Box(m) {
            when (val s = state) {
                is Load.Ok -> if (s.v.isEmpty()) Centered { Text(stringResource(R.string.no_results)) } else GameGrid(s.v, onGame = onGame)
                is Load.Err -> ErrorBox(s.msg, reload)
                else -> Centered { CircularProgressIndicator() }
            }
        }
    }
}


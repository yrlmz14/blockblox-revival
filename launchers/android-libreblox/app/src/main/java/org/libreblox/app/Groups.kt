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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

/** A name followed by the verified check and/or the Premium star. */
@Composable
fun NameLine(text: String, verified: Boolean, premium: Boolean, style: TextStyle = MaterialTheme.typography.titleMedium, color: Color = Color.Unspecified) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = style, color = color, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
        if (verified) Icon(Icons.Default.CheckCircle, stringResource(R.string.verified), Modifier.padding(start = 4.dp).size(18.dp), tint = Color(0xFF3B8CFF))
        if (premium) Icon(Icons.Default.Star, stringResource(R.string.premium), Modifier.padding(start = 4.dp).size(18.dp), tint = Color(0xFFFFC107))
    }
}

@Composable
private fun PostCard(poster: String, verified: Boolean, role: String, date: String, body: String, onOpen: (() -> Unit)? = null, onClickUser: (() -> Unit)? = null) {
    val ctx = LocalContext.current
    val clip = LocalClipboardManager.current
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(20.dp), modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f).let { if (onClickUser != null) it.clickable(onClick = onClickUser) else it }) {
                    NameLine(poster, verified, false)
                    Text(listOf(role, date.take(10)).filter { it.isNotEmpty() }.joinToString(" · "), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                TextButton({ clip.setText(AnnotatedString(body)) }) { Text(stringResource(R.string.copy)) }
            }
            Text(body, style = MaterialTheme.typography.bodyMedium, maxLines = if (onOpen != null) 6 else Int.MAX_VALUE, overflow = TextOverflow.Ellipsis,
                modifier = if (onOpen != null) Modifier.clickable(onClick = onOpen) else Modifier)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class, androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun GroupScreen(vm: AppVm, id: Long, onBack: () -> Unit, onUser: (Long) -> Unit, onGame: (Game) -> Unit, onGroup: (Long) -> Unit, onMembers: (String) -> Unit) {
    val ctx = LocalContext.current
    val clip = LocalClipboardManager.current
    val scope = rememberCoroutineScope()
    val me = vm.account
    val cookie = remember(me) { vm.cookie() }
    val (load, retry) = rememberLoad(id) { Api.groupInfo(id, cookie) }
    val title = (load as? Load.Ok)?.v?.name ?: ""
    var tab by rememberSaveable(id) { mutableIntStateOf(0) }
    var myRole by remember(id, me) { mutableStateOf<String?>(null) }
    var left by remember(id) { mutableStateOf(false) }
    var confirmLeave by remember(id) { mutableStateOf(false) }
    var msg by remember(id) { mutableStateOf<String?>(null) }
    var pages by remember(id) { mutableIntStateOf(1) }

    androidx.compose.runtime.LaunchedEffect(id, me) {
        myRole = if (me != null) runCatching { Api.roleIn(me.id, id) }.getOrNull() else null
    }

    DetailScaffold(title, onBack) { m ->
        when (val l = load) {
            Load.Idle, Load.Busy -> Box(m) { Centered { CircularProgressIndicator() } }
            is Load.Err -> Box(m) { ErrorBox(l.msg, retry) }
            is Load.Ok -> {
                val g = l.v
                val member = !myRole.isNullOrEmpty() && !left
                val (premium, _) = rememberLoad(g.ownerId to "prem") { runCatching { Api.isPremium(g.ownerId, cookie) }.getOrDefault(false) }
                val ownerPremium = (premium as? Load.Ok)?.v == true
                Column(m.fillMaxSize().verticalScroll(rememberScrollState()).padding(bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    // banner: the group icon, enlarged and blurred (Roblox has no public banner endpoint)
                    Box(Modifier.fillMaxWidth().height(140.dp).background(MaterialTheme.colorScheme.surfaceContainerHigh)) {
                        AsyncImage(g.icon, null, Modifier.fillMaxSize().blur(18.dp).alpha(0.7f), contentScale = ContentScale.Crop)
                    }
                    Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        Row(Modifier.padding(top = (-44).dp), verticalAlignment = Alignment.Bottom) {
                            AsyncImage(g.icon, null, Modifier.size(88.dp).clip(RoundedCornerShape(20.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh), contentScale = ContentScale.Crop)
                        }
                        Column(Modifier.padding(top = (-36).dp)) {
                            NameLine(g.name, g.verified, false, MaterialTheme.typography.headlineSmall)
                            Row(Modifier.clickable { onUser(g.ownerId) }, verticalAlignment = Alignment.CenterVertically) {
                                Text(stringResource(R.string.by) + " ", color = MaterialTheme.colorScheme.onSurfaceVariant)
                                NameLine(g.ownerName, g.ownerVerified, ownerPremium, MaterialTheme.typography.bodyMedium, MaterialTheme.colorScheme.primary)
                            }
                        }
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            if (member) OutlinedButton({ confirmLeave = true }) { Text(stringResource(R.string.leave_group)) }
                            else if (g.publicEntry) Button({
                                val c = cookie ?: run { vm.stack.add(Screen.LoginS); return@Button }
                                scope.launch { msg = try { Api.joinGroup(id, c); left = false; myRole = Api.roleIn(me?.id ?: 0, id); ctx.getString(R.string.action_done) } catch (e: Exception) { e.message } }
                            }) { Text(stringResource(R.string.join_group)) }
                            FilledTonalButton({ clip.setText(AnnotatedString("https://www.roblox.com/communities/$id")) }) { Text(stringResource(R.string.copy_link)) }
                            OutlinedButton({ openWeb(ctx, "https://www.roblox.com/abusereport/group?id=$id") }) { Text(stringResource(R.string.report)) }
                        }
                        msg?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.primary) }
                        Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer), shape = RoundedCornerShape(20.dp)) {
                            Row(Modifier.fillMaxWidth().padding(vertical = 14.dp)) {
                                Column(Modifier.weight(1f).clickable { onMembers(g.name) }, horizontalAlignment = Alignment.CenterHorizontally) {
                                    Text(compact(g.members), style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
                                    Text(stringResource(R.string.members), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                                Stat(stringResource(R.string.member_rank), if (member) myRole ?: "" else "—", Modifier.weight(1f))
                            }
                        }
                        if (g.description.isNotBlank()) Text(g.description, style = MaterialTheme.typography.bodyMedium)
                        val (names, _) = rememberLoad(id to "names") { runCatching { Api.nameHistory(id, cookie) }.getOrDefault(emptyList()) }
                        (names as? Load.Ok)?.v?.takeIf { it.size > 1 }?.let {
                            Text(stringResource(R.string.prev_names_fmt, it.drop(1).joinToString(", ")), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    PrimaryTabRow(tab, containerColor = MaterialTheme.colorScheme.background) {
                        listOf(R.string.grp_home, R.string.grp_posts, R.string.grp_events, R.string.grp_affiliates).forEachIndexed { i, t ->
                            Tab(tab == i, { tab = i }, text = { Text(stringResource(t)) })
                        }
                    }
                    Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        when (tab) {
                            0 -> {
                                Text(stringResource(R.string.grp_announcements), style = MaterialTheme.typography.titleMedium)
                                val sh = g.shout
                                if (sh == null) Text(stringResource(R.string.grp_no_announcements), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                else {
                                    PostCard(sh.poster, sh.posterVerified, sh.role, sh.created, sh.body, onClickUser = { onUser(sh.posterId) })
                                    TextButton({ tab = 1 }) { Text(stringResource(R.string.see_more_posts)) }
                                }
                                val (games, _) = rememberLoad(id to "games") { Api.groupGames(id) }
                                Text(stringResource(R.string.grp_places), style = MaterialTheme.typography.titleMedium)
                                when (val gs = games) {
                                    is Load.Ok -> {
                                        if (gs.v.isEmpty()) Text(stringResource(R.string.no_results), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                        gs.v.take(pages * 8).forEach { pl ->
                                            ListItem(
                                                headlineContent = { Text(pl.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                                supportingContent = {
                                                    Text(listOfNotNull(likePercent(pl)?.let { "👍 $it%" }, "▶ ${compact(pl.players)}").joinToString("  ·  "))
                                                },
                                                leadingContent = { AsyncImage(pl.icon, null, Modifier.size(56.dp).clip(RoundedCornerShape(14.dp)), contentScale = ContentScale.Crop) },
                                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                                modifier = Modifier.clickable { onGame(pl) },
                                            )
                                        }
                                        if (gs.v.size > pages * 8) TextButton({ pages++ }) { Text(stringResource(R.string.show_more)) }
                                    }
                                    is Load.Err -> Text(gs.msg, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                                    else -> CircularProgressIndicator(Modifier.size(24.dp))
                                }
                            }
                            1 -> {
                                Text(stringResource(R.string.grp_announcements), style = MaterialTheme.typography.headlineSmall)
                                g.shout?.let { sh -> PostCard(sh.poster, sh.posterVerified, sh.role, sh.created, sh.body, onClickUser = { onUser(sh.posterId) }) }
                                val (posts, _) = rememberLoad(id to "wall") { Api.wallPosts(id, cookie) }
                                when (val p = posts) {
                                    is Load.Ok -> {
                                        if (p.v.isNotEmpty()) Text(stringResource(R.string.grp_wall), style = MaterialTheme.typography.titleLarge)
                                        p.v.forEach { PostCard(it.poster, it.verified, it.role, it.created, it.body, onClickUser = { onUser(it.posterId) }) }
                                        if (p.v.isEmpty() && g.shout == null) Text(stringResource(R.string.grp_no_announcements), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    }
                                    is Load.Err -> if (g.shout == null) Text(stringResource(R.string.grp_wall_closed), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    else -> CircularProgressIndicator(Modifier.size(24.dp))
                                }
                            }
                            2 -> {
                                val (ev, _) = rememberLoad(id to "events") { Api.groupEvents(id) }
                                when (val e = ev) {
                                    is Load.Ok -> {
                                        if (e.v.isEmpty()) Text(stringResource(R.string.grp_no_events), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                        e.v.forEach { x ->
                                            ListItem(
                                                headlineContent = { Text(x.title, maxLines = 2, overflow = TextOverflow.Ellipsis) },
                                                supportingContent = { Text(x.subtitle) },
                                                leadingContent = { AsyncImage(x.image, null, Modifier.size(64.dp).clip(RoundedCornerShape(14.dp)), contentScale = ContentScale.Crop) },
                                                trailingContent = { FilledTonalButton({}) { Text(stringResource(R.string.notify_me)) } },
                                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                            )
                                        }
                                    }
                                    is Load.Err -> Text(stringResource(R.string.grp_no_events), color = MaterialTheme.colorScheme.onSurfaceVariant)
                                    else -> CircularProgressIndicator(Modifier.size(24.dp))
                                }
                            }
                            else -> {
                                listOf("allies" to R.string.grp_allies, "enemies" to R.string.grp_enemies).forEach { (kind, label) ->
                                    val (rel, _) = rememberLoad(id to kind) { Api.affiliates(id, kind) }
                                    (rel as? Load.Ok)?.v?.takeIf { it.isNotEmpty() }?.let { list ->
                                        Text(stringResource(label), style = MaterialTheme.typography.titleMedium)
                                        list.forEach { x ->
                                            ListItem(
                                                headlineContent = { Text(x.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                                                supportingContent = { Text(x.subtitle) },
                                                leadingContent = { AsyncImage(x.image, null, Modifier.size(56.dp).clip(RoundedCornerShape(14.dp)), contentScale = ContentScale.Crop) },
                                                colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                                                modifier = Modifier.clickable { onGroup(x.id) },
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if (confirmLeave) AlertDialog(
                    onDismissRequest = { confirmLeave = false },
                    title = { Text(stringResource(R.string.leave_group) + ": " + g.name) }, text = { Text(stringResource(R.string.are_you_sure)) },
                    confirmButton = {
                        TextButton({
                            confirmLeave = false
                            val c = cookie ?: return@TextButton
                            scope.launch { msg = try { Api.leaveGroup(id, me?.id ?: 0, c); left = true; ctx.getString(R.string.action_done) } catch (e: Exception) { e.message } }
                        }) { Text(stringResource(R.string.ok)) }
                    },
                    dismissButton = { TextButton({ confirmLeave = false }) { Text(stringResource(R.string.cancel)) } },
                )
            }
        }
    }
}

/** Members of a group by role ("All" or any role). */
@Composable
fun MembersScreen(id: Long, name: String, onBack: () -> Unit, onUser: (Long) -> Unit) {
    val (roles, _) = rememberLoad(id) { Api.groupRoles(id) }
    var role by remember(id) { mutableStateOf(0L) }
    val (members, retry) = rememberLoad(id to role) { Api.members(id, role) }
    DetailScaffold(stringResource(R.string.members) + " · " + name, onBack) { m ->
        Column(m.fillMaxSize()) {
            LazyRow(contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                item { FilterChip(role == 0L, { role = 0L }, { Text(stringResource(R.string.all)) }) }
                items((roles as? Load.Ok)?.v ?: emptyList()) { r -> FilterChip(role == r.id, { role = r.id }, { Text("${r.name} · ${compact(r.members)}") }) }
            }
            when (val s = members) {
                is Load.Ok -> if (s.v.isEmpty()) Centered { Text(stringResource(R.string.no_results)) } else androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize()) {
                    items(s.v.size) { i ->
                        val u = s.v[i]
                        ListItem(
                            headlineContent = { NameLine(u.display, u.verified, false) },
                            supportingContent = { Text("@${u.name}") },
                            leadingContent = { AsyncImage(u.avatar, null, Modifier.size(48.dp).clip(CircleShape), contentScale = ContentScale.Crop) },
                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                            modifier = Modifier.clickable { onUser(u.id) },
                        )
                    }
                }
                is Load.Err -> ErrorBox(s.msg, retry)
                else -> Centered { CircularProgressIndicator() }
            }
        }
    }
}

// ---------------- inventory by category ----------------

private val INV_TYPES = listOf(
    R.string.inv_collectibles to -1, R.string.inv_hats to 8, R.string.inv_hair to 41, R.string.inv_face_acc to 42, R.string.inv_neck to 43,
    R.string.inv_shoulder to 44, R.string.inv_front to 45, R.string.inv_back to 46, R.string.inv_waist to 47,
    R.string.inv_shirts to 11, R.string.inv_pants to 12, R.string.inv_tshirts to 2, R.string.inv_faces to 18, R.string.inv_gear to 19,
)

@Composable
fun InventoryScreen(vm: AppVm, userId: Long?, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val uid = userId ?: vm.account?.id ?: 0L
    var type by remember { mutableIntStateOf(-1) }
    val cookie = remember(vm.account) { vm.cookie() }
    val (state, retry) = rememberLoad(uid to type) {
        if (type == -1) Api.collectibles(uid, cookie) else Api.inventory(uid, type, cookie)
    }
    DetailScaffold(stringResource(R.string.drawer_inventory), onBack) { m ->
        if (uid == 0L) { Box(m) { NeedLogin { vm.stack.add(Screen.LoginS) } }; return@DetailScaffold }
        Column(m.fillMaxSize()) {
            LazyRow(contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                items(INV_TYPES) { (label, t) -> FilterChip(type == t, { type = t }, { Text(stringResource(label)) }) }
            }
            when (val s = state) {
                is Load.Ok -> if (s.v.isEmpty()) Centered { Text(stringResource(R.string.no_results), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                else androidx.compose.foundation.lazy.LazyColumn(Modifier.fillMaxSize()) {
                    items(s.v.size) { i ->
                        val e = s.v[i]
                        ListItem(
                            headlineContent = { Text(e.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                            supportingContent = if (e.subtitle.isNotBlank()) ({ Text(e.subtitle) }) else null,
                            leadingContent = { AsyncImage(e.image, null, Modifier.size(56.dp).clip(RoundedCornerShape(14.dp)), contentScale = ContentScale.Crop) },
                            colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                            modifier = Modifier.clickable { openWeb(ctx, e.url) },
                        )
                    }
                }
                is Load.Err -> if (s.msg.startsWith("HTTP 403") || s.msg.startsWith("HTTP 401")) Centered { Text(stringResource(R.string.inv_hidden), color = MaterialTheme.colorScheme.onSurfaceVariant) } else ErrorBox(s.msg, retry)
                else -> Centered { CircularProgressIndicator() }
            }
        }
    }
}

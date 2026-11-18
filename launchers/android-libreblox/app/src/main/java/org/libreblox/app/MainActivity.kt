package org.libreblox.app

import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AccountCircle
import androidx.compose.material.icons.filled.Build
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.ExitToApp
import androidx.compose.material.icons.filled.Face
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.Share
import androidx.compose.material.icons.filled.Create
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.ShoppingCart
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.NavigationDrawerItemDefaults
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.FilledTonalButton
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val vm by viewModels<AppVm>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val cfg = Cfg(this)
        setContent {
            val d = LocalDensity.current
            CompositionLocalProvider(
                LocalCfg provides cfg,
                LocalDensity provides Density(d.density, d.fontScale * cfg.textScale),
            ) {
                LibreTheme(cfg.theme()) {
                    Surface(color = MaterialTheme.colorScheme.background) { Root(vm, cfg) }
                }
            }
        }
    }
}

@Composable
fun LibreTheme(spec: ThemeSpec, content: @Composable () -> Unit) {
    val ctx = LocalContext.current
    val dark = if (spec.id == "system") isSystemInDarkTheme() else spec.dark
    val base = when {
        spec.id == "system" && Build.VERSION.SDK_INT >= 31 -> if (dark) dynamicDarkColorScheme(ctx) else dynamicLightColorScheme(ctx)
        dark -> darkColorScheme()
        else -> lightColorScheme()
    }
    fun c(k: String, d: Color) = spec.colors[k]?.let { Color(it) } ?: d
    val scheme = base.copy(
        primary = c("primary", base.primary), onPrimary = c("onPrimary", base.onPrimary),
        secondary = c("secondary", base.secondary),
        background = c("background", base.background), onBackground = c("onBackground", base.onBackground),
        surface = c("surface", base.surface), onSurface = c("onSurface", base.onSurface),
        onSurfaceVariant = c("onSurfaceVariant", base.onSurfaceVariant), error = c("error", base.error),
        surfaceContainer = c("surfaceContainer", base.surfaceContainer),
        surfaceContainerHigh = c("surfaceContainerHigh", base.surfaceContainerHigh),
        surfaceContainerHighest = c("surfaceContainerHighest", base.surfaceContainerHighest),
    )
    MaterialTheme(colorScheme = scheme, content = content)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Root(vm: AppVm, cfg: Cfg) {
    val ctx = LocalContext.current
    val haptic = LocalHapticFeedback.current
    val tap = rememberTap(haptic, cfg)
    var tab by rememberSaveable { mutableStateOf("home") }
    var homeTab by rememberSaveable { mutableStateOf(0) }
    var chatFilter by rememberSaveable { mutableStateOf("") }
    var showSwitch by remember { mutableStateOf(false) }
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val top = vm.stack.lastOrNull()
    val pop = { vm.stack.removeAt(vm.stack.lastIndex); Unit }
    val push = { s: Screen -> vm.stack.add(s); Unit }
    val login = { push(Screen.LoginS) }
    androidx.compose.runtime.LaunchedEffect(vm.account) {
        val a = vm.account
        if (a != null) cfg.setOwner(a.id) else if (Session.activeId(ctx) == 0L) cfg.setOwner(0L)
    }
    BackHandler(top != null) { pop() }
    BackHandler(top == null && drawer.isOpen) { scope.launch { drawer.close() } }

    when (top) {
        is Screen.GameS -> GameScreen(top.game, pop, onUser = { push(Screen.UserS(it)) })
        is Screen.UserS -> UserScreen(vm, top.id, pop, onGame = { push(Screen.GameS(it)) }, onUser = { push(Screen.UserS(it.id)) },
            onChat = { push(Screen.ChatS(it)) }, onInventory = { push(Screen.UserInvS(top.id)) }, onFavorites = { push(Screen.UserFavS(top.id)) }, onGroup = { push(Screen.GroupS(it)) })
        is Screen.UserInvS -> InventoryScreen(vm, top.id, pop)
        is Screen.GroupS -> GroupScreen(vm, top.id, pop, onUser = { push(Screen.UserS(it)) }, onGame = { push(Screen.GameS(it)) }, onGroup = { push(Screen.GroupS(it)) }, onMembers = { push(Screen.MembersS(top.id, it)) })
        is Screen.MembersS -> MembersScreen(top.id, top.name, pop) { push(Screen.UserS(it)) }
        is Screen.UserFavS -> UserFavoritesScreen(vm, top.id, pop) { push(Screen.GameS(it)) }
        is Screen.ChatS -> ChatScreen(vm, top.convo, pop)
        Screen.LoginS -> LoginScreen(pop, onDone = { pop(); vm.refreshAccount() })
        Screen.SearchS -> SearchScreen(vm, pop, { push(Screen.GameS(it)) }, { push(Screen.UserS(it.id)) })
        Screen.SettingsS -> SettingsScreen(vm, pop) { push(Screen.DocsS) }
        Screen.DocsS -> DocsScreen(pop)
        Screen.ContinueS -> ContinueScreen(pop) { push(Screen.GameS(it)) }
        Screen.FriendsS -> FriendsScreen(vm, pop, { push(Screen.UserS(it)) }, { login() })
        Screen.AddFriendsS -> AddFriendsScreen(vm, pop) { push(Screen.UserS(it)) }
        Screen.NewChatS -> NewChatScreen(vm, pop)
        Screen.FavoritesS -> FavoritesScreen(vm, pop) { push(Screen.GameS(it)) }
        Screen.CommunitiesS -> ListLoadScreen(stringResource(R.string.drawer_communities), vm, pop, onItem = { push(Screen.GroupS(it.id)) }) { id, c -> Api.groups(id, c) }
        Screen.InventoryS -> InventoryScreen(vm, null, pop)
        Screen.TradesS -> TradesScreen(vm, pop)
        Screen.MarketS -> MarketScreen(pop)
        null -> {
            fun go(id: String) {
                tap()
                scope.launch { drawer.close() }
                when (id) {
                    "continue" -> push(Screen.ContinueS)
                    "friends" -> push(Screen.FriendsS)
                    "settings" -> push(Screen.SettingsS)
                    "messages" -> tab = "chat"
                    "switch" -> showSwitch = true
                    "logout" -> vm.logout()
                    "favorites" -> push(Screen.FavoritesS)
                    "communities" -> push(Screen.CommunitiesS)
                    "inventory" -> push(Screen.InventoryS)
                    "trades" -> push(Screen.TradesS)
                    "help" -> openWeb(ctx, "https://en.help.roblox.com/")
                    "learn" -> openWeb(ctx, "https://create.roblox.com/docs")
                    "create" -> openWeb(ctx, "https://create.roblox.com/")
                }
            }
            ModalNavigationDrawer(
                drawerState = drawer,
                drawerContent = {
                    val a = cfg.drawerAlpha
                    ModalDrawerSheet(
                        modifier = Modifier.width(cfg.drawerWidth.dp),
                        drawerContainerColor = MaterialTheme.colorScheme.background.copy(alpha = a),
                    ) {
                        Column(Modifier.verticalScroll(rememberScrollState()).padding(12.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            val me = vm.account
                            if (cfg.on("drawer_header")) Card(
                                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh.copy(alpha = a)),
                                shape = RoundedCornerShape(24.dp), modifier = Modifier.fillMaxWidth(),
                            ) {
                                Column(Modifier.fillMaxWidth().padding(16.dp)) {
                                    if (me == null) {
                                        Text(stringResource(R.string.app_name), style = MaterialTheme.typography.titleLarge)
                                        FilledTonalButton({ scope.launch { drawer.close() }; login() }) { Text(stringResource(R.string.login)) }
                                    } else {
                                        AsyncImage(me.avatar, null, Modifier.size(64.dp).clip(CircleShape), contentScale = ContentScale.Crop)
                                        Spacer(Modifier.height(10.dp))
                                        Text(me.display, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        Text("@${me.name}", color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                        if (cfg.on("drawer_balance")) vm.balance?.let {
                                            Spacer(Modifier.height(8.dp))
                                            AssistChip({}, { Text(stringResource(R.string.robux_fmt, compact(it))) })
                                        }
                                    }
                                }
                            }
                            Card(
                                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer.copy(alpha = a)),
                                shape = RoundedCornerShape(24.dp), modifier = Modifier.fillMaxWidth(),
                            ) {
                                Column(Modifier.padding(vertical = 6.dp, horizontal = 6.dp)) {
                                    cfg.drawerOrder.filter { it !in cfg.drawerHidden }.forEach { id ->
                                        if (me == null && id in listOf("switch", "logout", "inventory", "trades")) return@forEach
                                        NavigationDrawerItem(
                                            label = { Text(stringResource(drawerLabel(id))) }, selected = false, onClick = { go(id) },
                                            icon = { Icon(drawerIcon(id), null) },
                                            colors = NavigationDrawerItemDefaults.colors(unselectedContainerColor = androidx.compose.ui.graphics.Color.Transparent),
                                        )
                                    }
                                }
                            }
                        }
                    }
                },
            ) {
                val blurDp by androidx.compose.animation.core.animateDpAsState(
                    if (drawer.targetValue == DrawerValue.Open) cfg.drawerBlur.dp else 0.dp, label = "blur",
                )
                Scaffold(
                    modifier = if (blurDp > 0.dp) Modifier.blur(blurDp) else Modifier,
                    containerColor = MaterialTheme.colorScheme.background,
                    topBar = {
                        TopAppBar(
                            title = {
                                if (tab == "home" && cfg.on("top_search") && cfg.onDef("top_searchfield", false)) Surface(
                                    onClick = { tap(); push(Screen.SearchS) }, shape = RoundedCornerShape(24.dp),
                                    color = MaterialTheme.colorScheme.surfaceContainerHigh, modifier = Modifier.fillMaxWidth().height(44.dp),
                                ) {
                                    Row(Modifier.padding(horizontal = 14.dp), verticalAlignment = Alignment.CenterVertically) {
                                        Icon(Icons.Default.Search, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                        Spacer(Modifier.width(8.dp))
                                        Text(stringResource(R.string.search_games), color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1)
                                    }
                                }
                            },
                            navigationIcon = {
                                if (tab == "home") IconButton({ tap(); scope.launch { drawer.open() } }) { Icon(Icons.Default.Menu, null) }
                            },
                            actions = {
                                when (tab) {
                                    "chat" -> {
                                        IconButton({ tap(); push(Screen.SearchS) }) { Icon(Icons.Default.Search, null) }
                                        IconButton({ tap(); push(Screen.NewChatS) }) { Icon(Icons.Default.Add, null) }
                                    }
                                    "home" -> {
                                        if (cfg.on("top_search") && !cfg.onDef("top_searchfield", false)) IconButton({ tap(); push(Screen.SearchS) }) { Icon(Icons.Default.Search, null) }
                                        if (cfg.on("top_robux")) IconButton({ tap(); openWeb(ctx, "https://www.roblox.com/upgrades/robux") }) { Icon(Icons.Default.ShoppingCart, null) }
                                        if (cfg.on("top_messages")) IconButton({ tap(); tab = "chat" }) { Icon(Icons.Default.Email, null) }
                                    }
                                }
                            },
                            colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                        )
                    },
                    bottomBar = {
                        NavigationBar {
                            cfg.navOrder.filter { it !in cfg.navHidden }.forEach { id ->
                                val icon = when (id) { "home" -> Icons.Default.Home; "chat" -> Icons.Default.Email; else -> Icons.Default.Person }
                                val label = when (id) { "home" -> R.string.tab_home; "chat" -> R.string.tab_chat; else -> R.string.tab_me }
                                NavigationBarItem(tab == id, { tap(); tab = id }, { Icon(icon, null) }, label = { Text(stringResource(label)) })
                            }
                        }
                    },
                ) { pad ->
                    val m = Modifier.padding(pad)
                    when (if (tab in cfg.navOrder && tab !in cfg.navHidden) tab else cfg.navOrder.first { it !in cfg.navHidden }) {
                        "home" -> {
                            val tabs = listOfNotNull(
                                if (cfg.on("tab_foryou")) true else null,
                                if (cfg.on("tab_charts")) false else null,
                            ).ifEmpty { listOf(true) }
                            val idx = homeTab.coerceIn(0, tabs.lastIndex)
                            Column(m) {
                                if (tabs.size > 1) PrimaryTabRow(idx, containerColor = MaterialTheme.colorScheme.background) {
                                    tabs.forEachIndexed { i, forYou ->
                                        Tab(idx == i, { homeTab = i }, text = { Text(stringResource(if (forYou) R.string.for_you else R.string.charts)) })
                                    }
                                }
                                HomeTab(
                                    vm, Modifier, forYou = tabs[idx],
                                    onGame = { push(Screen.GameS(it)) }, onUser = { push(Screen.UserS(it)) },
                                    onLogin = { login() }, onAddFriends = { push(Screen.AddFriendsS) }, onContinue = { push(Screen.ContinueS) },
                                )
                            }
                        }
                        "chat" -> ChatTab(vm, m, chatFilter, { push(Screen.ChatS(it)) }, { login() })
                        else -> MeTab(vm, m, onLogin = { login() }, onProfile = { vm.account?.let { push(Screen.UserS(it.id)) } })
                    }
                }
            }
        }
    }

    if (showSwitch) {
        val accs = Session.accounts(ctx)
        AlertDialog(
            onDismissRequest = { showSwitch = false },
            title = { Text(stringResource(R.string.drawer_switch)) },
            text = {
                Column {
                    accs.forEach { a ->
                        Text(
                            a.display + "  @" + a.name + if (a.id == Session.activeId(ctx)) "  ✓" else "",
                            Modifier.fillMaxWidth().clickable { showSwitch = false; vm.switchAccount(a.id) }.padding(vertical = 12.dp),
                        )
                    }
                }
            },
            confirmButton = { TextButton({ showSwitch = false; login() }) { Text(stringResource(R.string.add_account)) } },
            dismissButton = { TextButton({ showSwitch = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private fun drawerIcon(id: String): ImageVector = when (id) {
    "favorites" -> Icons.Default.Favorite
    "continue" -> Icons.Default.PlayArrow
    "friends" -> Icons.Default.Face
    "communities" -> Icons.Default.Share
    "inventory" -> Icons.Default.List
    "trades" -> Icons.Default.DateRange
    "messages" -> Icons.Default.Email
    "settings" -> Icons.Default.Settings
    "help" -> Icons.Default.Info
    "learn" -> Icons.Default.Star
    "create" -> Icons.Default.Create
    "switch" -> Icons.Default.AccountCircle
    else -> Icons.Default.ExitToApp
}

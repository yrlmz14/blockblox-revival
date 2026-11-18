package org.libreblox.app

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch

sealed interface Load<out T> {
    data object Idle : Load<Nothing>
    data object Busy : Load<Nothing>
    data class Ok<T>(val v: T) : Load<T>
    data class Err(val msg: String) : Load<Nothing>
}

sealed interface Screen {
    data class GameS(val game: Game) : Screen
    data class UserS(val id: Long) : Screen
    data class ChatS(val convo: Convo) : Screen
    data object LoginS : Screen
    data object SearchS : Screen
    data object SettingsS : Screen
    data object DocsS : Screen
    data object ContinueS : Screen
    data object FriendsS : Screen
    data object AddFriendsS : Screen
    data object NewChatS : Screen
    data object FavoritesS : Screen
    data object CommunitiesS : Screen
    data object InventoryS : Screen
    data object TradesS : Screen
    data object MarketS : Screen
    data class UserInvS(val id: Long) : Screen
    data class UserFavS(val id: Long) : Screen
    data class GroupS(val id: Long) : Screen
    data class MembersS(val id: Long, val name: String) : Screen
}

class AppVm(private val app: Application) : AndroidViewModel(app) {
    val stack = mutableStateListOf<Screen>()

    // discover
    var sorts by mutableStateOf<List<Sort>>(emptyList())
    val rowData = mutableStateMapOf<String, Load<List<Game>>>()
    var homeError by mutableStateOf<String?>(null)
    var homeBusy by mutableStateOf(false)

    // search
    var searchPeople by mutableStateOf(false)
    var gameQuery by mutableStateOf("")
    var gameSearch by mutableStateOf<Load<List<Game>>>(Load.Idle)
    var userQuery by mutableStateOf("")
    var userSearch by mutableStateOf<Load<List<User>>>(Load.Idle)

    // account + social
    var account by mutableStateOf<User?>(null)
    var balance by mutableStateOf<Long?>(null)
    var friends by mutableStateOf<Load<List<Friend>>>(Load.Idle)
    var convos by mutableStateOf<Load<List<Convo>>>(Load.Idle)
    var requests by mutableStateOf<Load<List<User>>>(Load.Idle)

    init {
        loadHome()
        refreshAccount()
    }

    private fun err(e: Throwable) = Load.Err(e.message ?: e.toString())
    fun cookie(): String? = Session.load(app)

    fun loadHome() {
        homeBusy = true
        homeError = null
        rowData.clear()
        viewModelScope.launch {
            try {
                sorts = runCatching { Api.sorts(cookie()) }.getOrDefault(emptyList())
                    .ifEmpty { listOf(Sort("top-playing-now", "Popular")) }
            } catch (e: Exception) {
                homeError = e.message ?: e.toString()
            }
            homeBusy = false
        }
    }

    /** Loads one chart row the first time it is shown. */
    fun ensureRow(id: String) {
        if (rowData[id] != null && rowData[id] !is Load.Err) return
        rowData[id] = Load.Busy
        viewModelScope.launch {
            rowData[id] = try { Load.Ok(Api.sortGames(id, cookie())) } catch (e: Exception) { err(e) }
        }
    }

    fun refreshAccount() {
        val cookie = Session.load(app) ?: run { clearSocial(); account = null; return }
        viewModelScope.launch {
            // Keep the previous value on network errors; only a rejected cookie logs the user out.
            try {
                val me = Api.me(cookie)
                if (me == null) { Session.clear(app); clearSocial(); account = null; refreshAfterSwitch() }
                else {
                    Session.register(app, me.id, me.name, me.display)
                    val changed = account?.id != me.id
                    account = me
                    loadSocial()
                    if (changed) loadHome()
                }
            } catch (_: Exception) {}
        }
    }

    private fun refreshAfterSwitch() {
        if (Session.load(app) != null) refreshAccount() else loadHome()
    }

    fun switchAccount(id: Long) {
        Session.switchTo(app, id)
        clearSocial(); account = null; balance = null
        refreshAccount()
    }

    private fun clearSocial() {
        friends = Load.Idle; convos = Load.Idle; requests = Load.Idle; balance = null
    }

    fun loadSocial() {
        loadFriends(); loadConvos(); loadBalance()
    }

    fun loadBalance() {
        val me = account ?: return
        val c = cookie() ?: return
        viewModelScope.launch { balance = try { Api.robux(me.id, c) } catch (_: Exception) { null } }
    }

    fun loadFriends() {
        val me = account ?: return
        val c = cookie() ?: return
        friends = Load.Busy
        viewModelScope.launch { friends = try { Load.Ok(Api.friends(me.id, c)) } catch (e: Exception) { err(e) } }
    }

    fun loadConvos() {
        val me = account ?: return
        val c = cookie() ?: return
        convos = Load.Busy
        viewModelScope.launch { convos = try { Load.Ok(Api.conversations(me.id, c)) } catch (e: Exception) { err(e) } }
    }

    fun loadRequests() {
        val c = cookie() ?: return
        requests = Load.Busy
        viewModelScope.launch { requests = try { Load.Ok(Api.incomingRequests(c)) } catch (e: Exception) { err(e) } }
    }

    fun logout() {
        Session.clear(app)
        account = null
        clearSocial()
        try {
            android.webkit.CookieManager.getInstance().removeAllCookies(null)
            android.webkit.CookieManager.getInstance().flush()
        } catch (_: Exception) {}
        if (Session.load(app) != null) refreshAccount() else loadHome()
    }

    fun searchGames() {
        val q = gameQuery.trim()
        if (q.isEmpty()) { gameSearch = Load.Idle; return }
        gameSearch = Load.Busy
        viewModelScope.launch { gameSearch = try { Load.Ok(Api.searchGames(q)) } catch (e: Exception) { err(e) } }
    }

    fun searchUsers() {
        val q = userQuery.trim()
        if (q.isEmpty()) { userSearch = Load.Idle; return }
        userSearch = Load.Busy
        viewModelScope.launch { userSearch = try { Load.Ok(Api.searchUsers(q)) } catch (e: Exception) { err(e) } }
    }
}

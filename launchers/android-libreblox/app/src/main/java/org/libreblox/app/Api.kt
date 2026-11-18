package org.libreblox.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.util.UUID

data class Presence(val type: Int, val location: String, val placeId: Long, val gameId: String)
data class Friend(val user: User, val presence: Presence?)
data class Convo(val id: String, val title: String, val unread: Boolean, val avatar: String?)
data class Msg(val id: String, val senderId: Long, val text: String, val sent: String)

class ApiException(val code: Int, val url: String) : Exception("HTTP $code\n$url")

data class Game(
    val universeId: Long,
    val placeId: Long,
    val name: String,
    val players: Long = 0,
    val up: Long = 0,
    val down: Long = 0,
    val icon: String? = null,
)

data class GameDetails(
    val game: Game,
    val description: String,
    val creator: String,
    val visits: Long,
    val favorites: Long,
    val maxPlayers: Int,
    val genre: String,
    val shots: List<String>,
)

data class Entry(val id: Long, val title: String, val subtitle: String, val image: String?, val url: String)

data class Shout(val body: String, val posterId: Long, val poster: String, val posterVerified: Boolean, val role: String, val created: String)

data class GroupInfo(
    val id: Long, val name: String, val description: String, val verified: Boolean,
    val ownerId: Long, val ownerName: String, val ownerVerified: Boolean,
    val members: Long, val publicEntry: Boolean, val icon: String?, val shout: Shout?,
)

data class Role(val id: Long, val name: String, val rank: Int, val members: Long)

data class Post(val id: Long, val posterId: Long, val poster: String, val verified: Boolean, val role: String, val created: String, val body: String)

data class Sort(val id: String, val title: String)

data class User(val id: Long, val name: String, val display: String, val verified: Boolean = false, val avatar: String? = null)

data class UserDetails(
    val user: User,
    val description: String,
    val created: String,
    val banned: Boolean,
    val friends: Long,
    val followers: Long,
    val following: Long,
    val bigAvatar: String?,
    val headshot: String?,
    val friendList: List<User>,
    val games: List<Game>,
)

/**
 * Thin client for Roblox's public web endpoints. They are unofficial and may change without notice,
 * so every field is read defensively.
 */
object Api {
    private val session = UUID.randomUUID().toString()

    private var csrf: String? = null

    private suspend fun request(url: String, method: String = "GET", cookie: String? = null, body: String? = null): String =
        withContext(Dispatchers.IO) {
            for (attempt in 0..1) {
                val c = URL(url).openConnection() as HttpURLConnection
                try {
                    c.requestMethod = method
                    c.connectTimeout = 15000
                    c.readTimeout = 20000
                    c.setRequestProperty("Accept", "application/json")
                    c.setRequestProperty("User-Agent", "LibreBlox/0.3 (Android)")
                    if (cookie != null) c.setRequestProperty("Cookie", ".ROBLOSECURITY=$cookie")
                    if (method != "GET") csrf?.let { c.setRequestProperty("x-csrf-token", it) }
                    if (body != null) {
                        c.doOutput = true
                        c.setRequestProperty("Content-Type", "application/json")
                        c.outputStream.use { it.write(body.toByteArray()) }
                    }
                    val code = c.responseCode
                    if (code == 403 && method != "GET" && attempt == 0) {
                        val t = c.getHeaderField("x-csrf-token")
                        if (t != null) { csrf = t; continue }
                    }
                    if (code !in 200..299) throw ApiException(code, url)
                    return@withContext c.inputStream.bufferedReader().use { it.readText() }
                } finally {
                    c.disconnect()
                }
            }
            throw ApiException(403, url)
        }

    private suspend fun get(url: String, cookie: String? = null): JSONObject = JSONObject(request(url, "GET", cookie))
    private suspend fun getArr(url: String, cookie: String?): JSONArray = JSONArray(request(url, "GET", cookie))

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    private fun JSONArray?.objects(): List<JSONObject> =
        if (this == null) emptyList() else (0 until length()).mapNotNull { optJSONObject(it) }

    /** The account the session cookie belongs to, or null when it is no longer valid. */
    suspend fun me(cookie: String): User? = try {
        val j = get("https://users.roblox.com/v1/users/authenticated", cookie)
        val id = j.optLong("id")
        if (id == 0L) null else User(id, j.optString("name"), j.optString("displayName").ifEmpty { j.optString("name") }, avatar = headshots(listOf(id))[id])
    } catch (e: ApiException) {
        if (e.code == 401 || e.code == 403) null else throw e
    }

    // ---------------- thumbnails ----------------

    private suspend fun gameIcons(ids: List<Long>): Map<Long, String> {
        if (ids.isEmpty()) return emptyMap()
        val j = runCatching {
            get("https://thumbnails.roblox.com/v1/games/icons?universeIds=${ids.joinToString(",")}&returnPolicy=PlaceHolder&size=256x256&format=Png&isCircular=false")
        }.getOrNull() ?: return emptyMap()
        return j.optJSONArray("data").objects().mapNotNull {
            val u = it.optString("imageUrl")
            if (u.isNotEmpty()) it.optLong("targetId") to u else null
        }.toMap()
    }

    private suspend fun headshots(ids: List<Long>): Map<Long, String> {
        if (ids.isEmpty()) return emptyMap()
        val j = runCatching {
            get("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds=${ids.joinToString(",")}&size=150x150&format=Png&isCircular=false")
        }.getOrNull() ?: return emptyMap()
        return j.optJSONArray("data").objects().mapNotNull {
            val u = it.optString("imageUrl")
            if (u.isNotEmpty()) it.optLong("targetId") to u else null
        }.toMap()
    }

    private suspend fun List<Game>.withIcons(): List<Game> {
        val icons = gameIcons(map { it.universeId }.distinct().take(100))
        return map { it.copy(icon = icons[it.universeId]) }
    }

    private suspend fun List<User>.withAvatars(): List<User> {
        val a = headshots(map { it.id }.distinct().take(100))
        return map { it.copy(avatar = a[it.id]) }
    }

    /** Names for a list of ids: Roblox's friends endpoints return bare ids. */
    suspend fun usersByIds(ids: List<Long>): List<User> {
        val out = mutableListOf<User>()
        for (chunk in ids.distinct().chunked(100)) {
            val body = JSONObject().put("userIds", JSONArray(chunk)).put("excludeBannedUsers", false).toString()
            val j = runCatching { JSONObject(request("https://users.roblox.com/v1/users", "POST", null, body)) }.getOrNull() ?: continue
            out += j.optJSONArray("data").objects().map {
                User(it.optLong("id"), it.optString("name"), it.optString("displayName").ifEmpty { it.optString("name") }, it.optBoolean("hasVerifiedBadge"))
            }
        }
        val byId = out.associateBy { it.id }
        return ids.mapNotNull { byId[it] }
    }

    private fun friendIds(j: JSONObject): List<Long> =
        (j.optJSONArray("data") ?: j.optJSONArray("PageItems")).objects().map { it.optLong("id") }.filter { it != 0L }

    private fun gameFrom(o: JSONObject) = Game(
        universeId = o.optLong("universeId"),
        placeId = o.optLong("rootPlaceId"),
        name = o.optString("name"),
        players = o.optLong("playerCount"),
        up = o.optLong("totalUpVotes"),
        down = o.optLong("totalDownVotes"),
    )

    // ---------------- discover ----------------

    suspend fun sorts(cookie: String? = null): List<Sort> {
        val j = get("https://apis.roblox.com/explore-api/v1/get-sorts?sessionId=$session&device=phone&country=all", cookie)
        val list = j.optJSONArray("sorts").objects()
            .filter { it.optString("contentType", "Games").equals("Games", true) && it.optString("sortId").isNotEmpty() }
            .map { Sort(it.optString("sortId"), it.optString("sortDisplayName").ifEmpty { it.optString("sortId") }) }
        return list
    }

    suspend fun sortGames(sortId: String, cookie: String? = null): List<Game> {
        val j = get("https://apis.roblox.com/explore-api/v1/get-sort-content?sessionId=$session&device=phone&country=all&sortId=${enc(sortId)}", cookie)
        return j.optJSONArray("games").objects().map(::gameFrom).filter { it.universeId != 0L }.withIcons()
    }

    // ---------------- search ----------------

    suspend fun searchGames(query: String): List<Game> {
        val j = get("https://apis.roblox.com/search-api/omni-search?searchQuery=${enc(query)}&sessionId=$session&pageType=all")
        val out = mutableListOf<Game>()
        for (g in j.optJSONArray("searchResults").objects()) {
            if (!g.optString("contentGroupType").equals("Game", true)) continue
            out += g.optJSONArray("contents").objects().map(::gameFrom)
        }
        return out.filter { it.universeId != 0L }.withIcons()
    }

    suspend fun searchUsers(query: String): List<User> {
        val j = get("https://users.roblox.com/v1/users/search?keyword=${enc(query)}&limit=25")
        return j.optJSONArray("data").objects().map {
            User(it.optLong("id"), it.optString("name"), it.optString("displayName").ifEmpty { it.optString("name") }, it.optBoolean("hasVerifiedBadge"))
        }.withAvatars()
    }

    // ---------------- details ----------------

    suspend fun gameDetails(base: Game): GameDetails {
        val id = base.universeId
        val j = get("https://games.roblox.com/v1/games?universeIds=$id").optJSONArray("data").objects().firstOrNull()
            ?: throw ApiException(404, "universe $id")
        val votes = runCatching { get("https://games.roblox.com/v1/games/votes?universeIds=$id").optJSONArray("data").objects().firstOrNull() }.getOrNull()
        val icon = base.icon ?: gameIcons(listOf(id))[id]
        val shots = runCatching {
            get("https://thumbnails.roblox.com/v1/games/multiget/thumbnails?universeIds=$id&countPerUniverse=8&size=768x432&format=Png&isCircular=false")
                .optJSONArray("data").objects().firstOrNull()?.optJSONArray("thumbnails").objects()
                .mapNotNull { it.optString("imageUrl").ifEmpty { null } }
        }.getOrNull().orEmpty()
        val game = Game(
            universeId = id,
            placeId = j.optLong("rootPlaceId").takeIf { it != 0L } ?: base.placeId,
            name = j.optString("name").ifEmpty { base.name },
            players = j.optLong("playing"),
            up = votes?.optLong("upVotes") ?: base.up,
            down = votes?.optLong("downVotes") ?: base.down,
            icon = icon,
        )
        return GameDetails(
            game = game,
            description = j.optString("description"),
            creator = j.optJSONObject("creator")?.optString("name").orEmpty(),
            visits = j.optLong("visits"),
            favorites = j.optLong("favoritedCount"),
            maxPlayers = j.optInt("maxPlayers"),
            genre = j.optString("genre"),
            shots = shots,
        )
    }

    private suspend fun count(url: String): Long = runCatching { get(url).optLong("count") }.getOrDefault(0L)

    suspend fun userDetails(id: Long): UserDetails = coroutineScope {
        val profile = async { get("https://users.roblox.com/v1/users/$id") }
        val big = async {
            runCatching {
                get("https://thumbnails.roblox.com/v1/users/avatar?userIds=$id&size=420x420&format=Png&isCircular=false")
                    .optJSONArray("data").objects().firstOrNull()?.optString("imageUrl")?.ifEmpty { null }
            }.getOrNull()
        }
        val head = async { headshots(listOf(id))[id] }
        val friends = async {
            runCatching {
                val ids = friendIds(get("https://friends.roblox.com/v1/users/$id/friends")).take(30)
                usersByIds(ids).withAvatars()
            }.getOrDefault(emptyList())
        }
        val games = async {
            runCatching {
                get("https://games.roblox.com/v2/users/$id/games?limit=12&sortOrder=Desc").optJSONArray("data").objects().map {
                    Game(it.optLong("id"), it.optJSONObject("rootPlace")?.optLong("id") ?: 0L, it.optString("name"))
                }.withIcons()
            }.getOrDefault(emptyList())
        }
        val nFriends = async { count("https://friends.roblox.com/v1/users/$id/friends/count") }
        val nFollowers = async { count("https://friends.roblox.com/v1/users/$id/followers/count") }
        val nFollowing = async { count("https://friends.roblox.com/v1/users/$id/followings/count") }
        val j = profile.await()
        val user = User(id, j.optString("name"), j.optString("displayName").ifEmpty { j.optString("name") }, j.optBoolean("hasVerifiedBadge"))
        val bigUrl = big.await()
        UserDetails(
            user = user.copy(avatar = bigUrl),
            description = j.optString("description"),
            created = j.optString("created").take(10),
            banned = j.optBoolean("isBanned"),
            friends = nFriends.await(),
            followers = nFollowers.await(),
            following = nFollowing.await(),
            bigAvatar = bigUrl,
            headshot = head.await(),
            friendList = friends.await(),
            games = games.await(),
        )
    }

    // ---------------- account: friends, favorites, chat ----------------

    suspend fun presence(ids: List<Long>, cookie: String): Map<Long, Presence> {
        if (ids.isEmpty()) return emptyMap()
        val body = JSONObject().put("userIds", JSONArray(ids.take(200))).toString()
        val j = JSONObject(request("https://presence.roblox.com/v1/presence/users", "POST", cookie, body))
        return j.optJSONArray("userPresences").objects().associate {
            it.optLong("userId") to Presence(it.optInt("userPresenceType"), it.optString("lastLocation"), it.optLong("rootPlaceId").takeIf { p -> p != 0L } ?: it.optLong("placeId"), it.optString("gameId"))
        }
    }

    suspend fun friends(myId: Long, cookie: String): List<Friend> {
        val ids = friendIds(get("https://friends.roblox.com/v1/users/$myId/friends", cookie))
        val users = usersByIds(ids).withAvatars()
        val pres = runCatching { presence(users.map { it.id }, cookie) }.getOrDefault(emptyMap())
        return users.map { Friend(it, pres[it.id]) }.sortedWith(compareByDescending<Friend> { it.presence?.type ?: 0 }.thenBy { it.user.display.lowercase() })
    }

    private const val CHAT = "https://apis.roblox.com/platform-chat-api/v1"

    private fun idOf(o: JSONObject): String {
        for (k in listOf("id", "conversation_id", "conversationId")) {
            val v = o.opt(k)
            if (v != null && v != JSONObject.NULL && v.toString().isNotEmpty()) return v.toString()
        }
        return ""
    }

    suspend fun conversations(myId: Long, cookie: String): List<Convo> {
        val j = get("$CHAT/get-user-conversations?include_user_data=true&pageSize=30", cookie)
        val raw = (j.optJSONArray("conversations") ?: JSONArray()).objects()
        // Some responses wrap each entry as {"conversation": {...}}.
        val arr = raw.map { it.optJSONObject("conversation") ?: it }
        val good = arr.filter { idOf(it).isNotEmpty() }
        if (arr.isNotEmpty() && good.isEmpty()) {
            val f = arr.first()
            throw Exception("Unexpected chat format: " + f.keys().asSequence().joinToString { k -> k + "=" + f.opt(k).toString().take(40) })
        }
        fun others(c: JSONObject): List<Long> {
            val a = c.optJSONArray("participant_user_ids") ?: return emptyList()
            return (0 until a.length()).map { a.optLong(it) }.filter { it != myId && it != 0L }
        }
        val allIds = good.flatMap { others(it) }.distinct()
        val users = usersByIds(allIds).associateBy { it.id }
        val avatars = headshots(good.mapNotNull { others(it).firstOrNull() }.distinct().take(100))
        return good.map { c ->
            val o = others(c)
            val title = c.optString("name").takeIf { it.isNotEmpty() && it != "null" }
                ?: o.mapNotNull { users[it]?.display }.joinToString(", ").ifEmpty { "Chat" }
            Convo(idOf(c), title, c.optInt("unread_message_count") > 0, o.firstOrNull()?.let { avatars[it] })
        }
    }

    /** Message text: the field is a string in some responses and a nested object in others. */
    private fun textOf(v: Any?): String = when (v) {
        null, JSONObject.NULL -> ""
        is String -> v
        is JSONObject -> listOf("text", "content", "message", "body", "value").firstNotNullOfOrNull { k -> textOf(v.opt(k)).takeIf { it.isNotEmpty() } } ?: ""
        is JSONArray -> (0 until v.length()).joinToString(" ") { textOf(v.opt(it)) }.trim()
        else -> v.toString()
    }

    suspend fun messages(conversationId: String, cookie: String): List<Msg> {
        val j = get("$CHAT/get-conversation-messages?conversation_id=${enc(conversationId)}", cookie)
        val arr = (j.optJSONArray("messages") ?: j.optJSONArray("data")).objects()
        return arr.map {
            var text = listOf("content", "text", "message", "body").firstNotNullOfOrNull { k -> textOf(it.opt(k)).takeIf { t -> t.isNotEmpty() } } ?: ""
            // Unknown shape: show the field names instead of an empty bubble, so the format can be fixed.
            if (text.isEmpty()) text = "[" + it.keys().asSequence().joinToString(",") + "]"
            val sender = it.optLong("sender_user_id").takeIf { x -> x != 0L } ?: it.optJSONObject("sender")?.optLong("id") ?: it.optLong("senderId")
            Msg(it.optString("id"), sender, text, it.optString("created_at").ifEmpty { it.optString("createdAt") })
        }.sortedBy { it.sent }
    }

    suspend fun sendMessage(conversationId: String, text: String, cookie: String) {
        val body = JSONObject().put("conversation_id", conversationId)
            .put("messages", JSONArray().put(JSONObject().put("content", text))).toString()
        request("$CHAT/send-messages", "POST", cookie, body)
    }

    // ---------------- balance, friend requests ----------------

    suspend fun robux(userId: Long, cookie: String): Long =
        get("https://economy.roblox.com/v1/users/$userId/currency", cookie).optLong("robux")

    suspend fun incomingRequests(cookie: String): List<User> {
        val j = get("https://friends.roblox.com/v1/my/friends/requests?limit=50&sortOrder=Desc", cookie)
        val ids = (j.optJSONArray("data") ?: JSONArray()).objects().map { it.optLong("id") }.filter { it != 0L }
        return usersByIds(ids).withAvatars()
    }

    suspend fun friendAction(userId: Long, action: String, cookie: String) {
        // action: request-friendship | accept-friend-request | decline-friend-request
        request("https://friends.roblox.com/v1/users/$userId/$action", "POST", cookie, "{}")
    }

    suspend fun newConversation(userId: Long, cookie: String): String {
        val body = JSONObject().put("conversations", JSONArray().put(
            JSONObject().put("type", "one_to_one").put("participant_user_ids", JSONArray().put(userId)),
        )).put("include_user_data", true).toString()
        val j = JSONObject(request("$CHAT/create-conversations", "POST", cookie, body))
        val c = (j.optJSONArray("conversations") ?: JSONArray()).objects().firstOrNull() ?: throw Exception("No conversation returned")
        return idOf(c.optJSONObject("conversation") ?: c).ifEmpty { throw Exception("No conversation id: " + c.keys().asSequence().toList()) }
    }
    // ---------------- lists: favorites, communities, inventory, trades, catalog ----------------

    private suspend fun assetThumbs(ids: List<Long>): Map<Long, String> {
        if (ids.isEmpty()) return emptyMap()
        val j = runCatching {
            get("https://thumbnails.roblox.com/v1/assets?assetIds=${ids.distinct().take(100).joinToString(",")}&size=150x150&format=Png&isCircular=false")
        }.getOrNull() ?: return emptyMap()
        return j.optJSONArray("data").objects().mapNotNull {
            val u = it.optString("imageUrl"); if (u.isNotEmpty()) it.optLong("targetId") to u else null
        }.toMap()
    }

    suspend fun favoriteGames(userId: Long, cookie: String?): List<Game> {
        val j = get("https://games.roblox.com/v2/users/$userId/favorite/games?limit=50&sortOrder=Desc", cookie)
        return j.optJSONArray("data").objects().map {
            Game(it.optLong("id"), it.optJSONObject("rootPlace")?.optLong("id") ?: 0L, it.optString("name"), it.optLong("playing"))
        }.filter { it.universeId != 0L }.withIcons()
    }

    suspend fun groups(userId: Long, cookie: String?): List<Entry> {
        val j = get("https://groups.roblox.com/v1/users/$userId/groups/roles?includeLocked=false", cookie)
        val rows = j.optJSONArray("data").objects()
        val icons = runCatching {
            get("https://thumbnails.roblox.com/v1/groups/icons?groupIds=${rows.mapNotNull { it.optJSONObject("group")?.optLong("id") }.take(100).joinToString(",")}&size=150x150&format=Png&isCircular=false")
                .optJSONArray("data").objects().associate { it.optLong("targetId") to it.optString("imageUrl") }
        }.getOrDefault(emptyMap())
        return rows.mapNotNull {
            val g = it.optJSONObject("group") ?: return@mapNotNull null
            Entry(g.optLong("id"), g.optString("name"), (it.optJSONObject("role")?.optString("name") ?: "") + " · " + g.optLong("memberCount") + " members",
                icons[g.optLong("id")], "https://www.roblox.com/communities/${g.optLong("id")}")
        }
    }

    suspend fun collectibles(userId: Long, cookie: String?): List<Entry> {
        val j = get("https://inventory.roblox.com/v1/users/$userId/assets/collectibles?sortOrder=Desc&limit=50", cookie)
        val rows = j.optJSONArray("data").objects()
        val th = assetThumbs(rows.map { it.optLong("assetId") })
        return rows.map {
            val id = it.optLong("assetId")
            Entry(id, it.optString("name"), if (it.has("recentAveragePrice")) "RAP ${it.optLong("recentAveragePrice")}" else "", th[id], "https://www.roblox.com/catalog/$id")
        }
    }

    suspend fun trades(kind: String, cookie: String): List<Entry> {
        val j = get("https://trades.roblox.com/v1/trades/$kind?limit=25&sortOrder=Desc", cookie)
        return j.optJSONArray("data").objects().map {
            val u = it.optJSONObject("user")
            Entry(it.optLong("id"), u?.optString("displayName") ?: u?.optString("name") ?: "Trade", it.optString("status") + " · " + it.optString("created").take(10), null,
                "https://www.roblox.com/trades")
        }
    }

    suspend fun catalog(query: String): List<Entry> {
        val j = get("https://catalog.roblox.com/v1/search/items/details?Keyword=${enc(query)}&Limit=30&SortType=0")
        val rows = j.optJSONArray("data").objects()
        val th = assetThumbs(rows.filter { it.optString("itemType").equals("Asset", true) }.map { it.optLong("id") })
        return rows.map {
            val id = it.optLong("id")
            val price = if (it.has("price")) "R$ ${it.optLong("price")}" else if (it.optBoolean("priceStatus")) "" else "Free / off-sale"
            val bundle = it.optString("itemType").equals("Bundle", true)
            Entry(id, it.optString("name"), listOf(it.optString("creatorName"), price).filter { x -> x.isNotEmpty() }.joinToString(" · "), th[id],
                "https://www.roblox.com/" + (if (bundle) "bundles/" else "catalog/") + id)
        }
    }
    // ---------------- profile sections ----------------

    suspend fun usernameHistory(id: Long): List<String> =
        get("https://users.roblox.com/v1/users/$id/username-history?limit=25&sortOrder=Desc").optJSONArray("data").objects().map { it.optString("name") }.filter { it.isNotEmpty() }

    /** Social links the user shows on the profile: (label, url). */
    suspend fun socials(id: Long, cookie: String?): List<Pair<String, String>> {
        val j = get("https://accountinformation.roblox.com/v1/users/$id/promotion-channels", cookie)
        fun link(key: String, base: String): Pair<String, String>? {
            val v = j.optString(key).takeIf { it.isNotBlank() && it != "null" } ?: return null
            return key.replaceFirstChar { it.uppercase() } to (if (v.startsWith("http")) v else base + v.trimStart('@'))
        }
        return listOfNotNull(
            link("twitter", "https://x.com/"), link("youtube", "https://youtube.com/"), link("twitch", "https://twitch.tv/"),
            link("facebook", "https://facebook.com/"), link("guilded", "https://guilded.gg/"),
        )
    }

    suspend fun wearing(id: Long): List<Entry> {
        val ids = get("https://avatar.roblox.com/v1/users/$id/currently-wearing").optJSONArray("assetIds")
            ?.let { a -> (0 until a.length()).map { a.optLong(it) } } ?: return emptyList()
        if (ids.isEmpty()) return emptyList()
        val body = JSONObject().put("items", JSONArray().also { a -> ids.take(50).forEach { a.put(JSONObject().put("itemType", "Asset").put("id", it)) } }).toString()
        val rows = runCatching { JSONObject(request("https://catalog.roblox.com/v1/catalog/items/details", "POST", null, body)).optJSONArray("data").objects() }.getOrDefault(emptyList())
        val th = assetThumbs(ids)
        val byId = rows.associateBy { it.optLong("id") }
        return ids.map { aid ->
            val r = byId[aid]
            val price = when {
                r == null -> ""
                r.has("price") -> "R$ ${r.optLong("price")}"
                r.optString("priceStatus").isNotEmpty() -> r.optString("priceStatus")
                else -> ""
            }
            Entry(aid, r?.optString("name") ?: "#$aid", price, th[aid], "https://www.roblox.com/catalog/$aid")
        }
    }

    suspend fun badges(id: Long): List<Entry> {
        val rows = get("https://badges.roblox.com/v1/users/$id/badges?limit=12&sortOrder=Desc").optJSONArray("data").objects()
        val icons = runCatching {
            get("https://thumbnails.roblox.com/v1/badges/icons?badgeIds=${rows.map { it.optLong("id") }.joinToString(",")}&size=150x150&format=Png&isCircular=false")
                .optJSONArray("data").objects().associate { it.optLong("targetId") to it.optString("imageUrl") }
        }.getOrDefault(emptyMap())
        return rows.map { Entry(it.optLong("id"), it.optString("name"), it.optString("description").take(80), icons[it.optLong("id")], "https://www.roblox.com/badges/${it.optLong("id")}") }
    }

    suspend fun presenceOf(id: Long, cookie: String): Presence? = presence(listOf(id), cookie)[id]

    suspend fun isFollowing(id: Long, cookie: String): Boolean {
        val body = JSONObject().put("targetUserIds", JSONArray().put(id)).toString()
        return JSONObject(request("https://friends.roblox.com/v1/user/following-exists", "POST", cookie, body))
            .optJSONArray("followings").objects().firstOrNull()?.optBoolean("isFollowing") ?: false
    }

    suspend fun block(userId: Long, cookie: String) { request("https://accountsettings.roblox.com/v1/users/$userId/block", "POST", cookie, "{}") }
    // ---------------- groups (communities) ----------------

    suspend fun groupInfo(id: Long, cookie: String?): GroupInfo = coroutineScope {
        val j = get("https://groups.roblox.com/v1/groups/$id", cookie)
        val icon = async {
            runCatching {
                get("https://thumbnails.roblox.com/v1/groups/icons?groupIds=$id&size=420x420&format=Png&isCircular=false")
                    .optJSONArray("data").objects().firstOrNull()?.optString("imageUrl")?.ifEmpty { null }
            }.getOrNull()
        }
        val o = j.optJSONObject("owner")
        val sh = j.optJSONObject("shout")
        val shout = sh?.takeIf { it.optString("body").isNotEmpty() }?.let {
            val p = it.optJSONObject("poster")
            val pid = p?.optLong("userId") ?: 0L
            val role = runCatching { roleIn(pid, id) }.getOrNull().orEmpty()
            Shout(it.optString("body"), pid, p?.optString("displayName")?.ifEmpty { p.optString("username") } ?: "", p?.optBoolean("hasVerifiedBadge") ?: false, role, it.optString("updated").ifEmpty { it.optString("created") })
        }
        GroupInfo(
            id, j.optString("name"), j.optString("description"), j.optBoolean("hasVerifiedBadge"),
            o?.optLong("userId") ?: 0L, o?.optString("displayName")?.ifEmpty { o.optString("username") } ?: "", o?.optBoolean("hasVerifiedBadge") ?: false,
            j.optLong("memberCount"), j.optBoolean("publicEntryAllowed"), icon.await(), shout,
        )
    }

    /** The role name of a user in a group, or "" when not a member. */
    suspend fun roleIn(userId: Long, groupId: Long): String {
        if (userId == 0L) return ""
        val rows = get("https://groups.roblox.com/v2/users/$userId/groups/roles").optJSONArray("data").objects()
        return rows.firstOrNull { it.optJSONObject("group")?.optLong("id") == groupId }?.optJSONObject("role")?.optString("name").orEmpty()
    }

    suspend fun groupRoles(id: Long): List<Role> =
        get("https://groups.roblox.com/v1/groups/$id/roles").optJSONArray("roles").objects()
            .map { Role(it.optLong("id"), it.optString("name"), it.optInt("rank"), it.optLong("memberCount")) }.sortedByDescending { it.rank }

    suspend fun nameHistory(id: Long, cookie: String?): List<String> =
        get("https://groups.roblox.com/v1/groups/$id/name-history?limit=25&sortOrder=Desc", cookie).optJSONArray("data").objects().map { it.optString("name") }.filter { it.isNotEmpty() }

    /** Members of a role (roleId 0 = everyone), with avatars. */
    suspend fun members(groupId: Long, roleId: Long): List<User> {
        val url = if (roleId == 0L) "https://groups.roblox.com/v1/groups/$groupId/users?limit=100&sortOrder=Asc"
        else "https://groups.roblox.com/v1/groups/$groupId/roles/$roleId/users?limit=100&sortOrder=Asc"
        val rows = get(url).optJSONArray("data").objects().map { it.optJSONObject("user") ?: it }
        return rows.map { User(it.optLong("userId"), it.optString("username"), it.optString("displayName").ifEmpty { it.optString("username") }, it.optBoolean("hasVerifiedBadge")) }
            .filter { it.id != 0L }.withAvatars()
    }

    suspend fun wallPosts(groupId: Long, cookie: String?): List<Post> =
        get("https://groups.roblox.com/v2/groups/$groupId/wall/posts?limit=20&sortOrder=Desc", cookie).optJSONArray("data").objects().map {
            val p = it.optJSONObject("poster"); val u = p?.optJSONObject("user")
            Post(it.optLong("id"), u?.optLong("userId") ?: 0L, u?.optString("displayName")?.ifEmpty { u.optString("username") } ?: "", u?.optBoolean("hasVerifiedBadge") ?: false,
                p?.optJSONObject("role")?.optString("name").orEmpty(), it.optString("created"), it.optString("body"))
        }

    /** Experiences of a group, with live player counts and like ratio. */
    suspend fun groupGames(groupId: Long): List<Game> {
        val rows = get("https://games.roblox.com/v2/groups/$groupId/games?accessFilter=2&limit=50&sortOrder=Desc").optJSONArray("data").objects()
        val games = rows.map { Game(it.optLong("id"), it.optJSONObject("rootPlace")?.optLong("id") ?: 0L, it.optString("name")) }.filter { it.universeId != 0L }
        if (games.isEmpty()) return games
        val ids = games.joinToString(",") { it.universeId.toString() }
        val playing = runCatching { get("https://games.roblox.com/v1/games?universeIds=$ids").optJSONArray("data").objects().associate { it.optLong("id") to it.optLong("playing") } }.getOrDefault(emptyMap())
        val votes = runCatching { get("https://games.roblox.com/v1/games/votes?universeIds=$ids").optJSONArray("data").objects().associate { it.optLong("id") to (it.optLong("upVotes") to it.optLong("downVotes")) } }.getOrDefault(emptyMap())
        return games.map { it.copy(players = playing[it.universeId] ?: 0, up = votes[it.universeId]?.first ?: 0, down = votes[it.universeId]?.second ?: 0) }.withIcons()
    }

    /** Best-effort: experience events of the group's games (the endpoint is not documented). */
    suspend fun groupEvents(groupId: Long): List<Entry> {
        val out = mutableListOf<Entry>()
        for (g in groupGames(groupId).take(8)) {
            val j = runCatching { get("https://apis.roblox.com/virtual-events/v1/universes/${g.universeId}/virtual-events") }.getOrNull() ?: continue
            for (e in (j.optJSONArray("data") ?: j.optJSONArray("virtualEvents")).objects()) {
                val start = e.optJSONObject("eventTime")?.optString("startUtc").orEmpty().ifEmpty { e.optString("startUtc") }
                out += Entry(e.optLong("id"), e.optString("title").ifEmpty { e.optString("displayTitle") }, start.take(16).replace('T', ' ') + " · " + g.name, g.icon, "https://www.roblox.com/games/${g.placeId}")
            }
        }
        return out
    }

    suspend fun affiliates(groupId: Long, kind: String): List<Entry> {
        val j = get("https://groups.roblox.com/v1/groups/$groupId/relationships/$kind?model.startRowIndex=0&model.maxRows=50")
        val rows = j.optJSONArray("relatedGroups").objects()
        val icons = runCatching {
            get("https://thumbnails.roblox.com/v1/groups/icons?groupIds=${rows.map { it.optLong("id") }.joinToString(",")}&size=150x150&format=Png&isCircular=false")
                .optJSONArray("data").objects().associate { it.optLong("targetId") to it.optString("imageUrl") }
        }.getOrDefault(emptyMap())
        return rows.map { Entry(it.optLong("id"), it.optString("name"), it.optLong("memberCount").toString() + " members", icons[it.optLong("id")], "") }
    }

    suspend fun joinGroup(groupId: Long, cookie: String) { request("https://groups.roblox.com/v1/groups/$groupId/users", "POST", cookie, "{}") }
    suspend fun leaveGroup(groupId: Long, userId: Long, cookie: String) { request("https://groups.roblox.com/v1/groups/$groupId/users/$userId", "DELETE", cookie, null) }

    suspend fun isPremium(userId: Long, cookie: String?): Boolean =
        request("https://premiumfeatures.roblox.com/v1/users/$userId/validate-membership", "GET", cookie).trim().equals("true", true)

    // ---------------- inventory by category ----------------

    suspend fun inventory(userId: Long, typeId: Int, cookie: String?): List<Entry> {
        val rows = get("https://inventory.roblox.com/v2/users/$userId/inventory/$typeId?limit=50&sortOrder=Desc", cookie).optJSONArray("data").objects()
        val th = assetThumbs(rows.map { it.optLong("assetId") })
        return rows.map {
            val id = it.optLong("assetId")
            Entry(id, it.optString("assetName").ifEmpty { it.optString("name") }, it.optString("created").take(10), th[id], "https://www.roblox.com/catalog/$id")
        }
    }
}

package org.libreblox.app

import android.content.Context
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedback
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import org.json.JSONArray
import org.json.JSONObject

/** A colour theme: a base (dark/light) plus colour overrides, loadable from JSON. */
data class ThemeSpec(val id: String, val name: String, val dark: Boolean, val colors: Map<String, Int>, val builtIn: Boolean = true)

val BUILTIN_THEMES: List<ThemeSpec> = listOf(
    ThemeSpec("roblox", "Roblox", true, mapOf(
        "primary" to 0xFF335FFF.toInt(), "onPrimary" to 0xFFFFFFFF.toInt(), "secondary" to 0xFF00B06F.toInt(),
        "background" to 0xFF232527.toInt(), "surface" to 0xFF232527.toInt(), "onBackground" to 0xFFFFFFFF.toInt(), "onSurface" to 0xFFFFFFFF.toInt(),
        "onSurfaceVariant" to 0xFFBDBEBE.toInt(),
        "surfaceContainer" to 0xFF2E3033.toInt(), "surfaceContainerHigh" to 0xFF393B3D.toInt(), "surfaceContainerHighest" to 0xFF45474A.toInt(),
    )),
    ThemeSpec("mocha", "Catppuccin Mocha", true, mapOf(
        "primary" to 0xFFCBA6F7.toInt(), "onPrimary" to 0xFF11111B.toInt(), "secondary" to 0xFFA6E3A1.toInt(),
        "background" to 0xFF1E1E2E.toInt(), "surface" to 0xFF1E1E2E.toInt(), "onBackground" to 0xFFCDD6F4.toInt(), "onSurface" to 0xFFCDD6F4.toInt(),
        "onSurfaceVariant" to 0xFFA6ADC8.toInt(), "error" to 0xFFF38BA8.toInt(),
        "surfaceContainer" to 0xFF181825.toInt(), "surfaceContainerHigh" to 0xFF313244.toInt(), "surfaceContainerHighest" to 0xFF45475A.toInt(),
    )),
    ThemeSpec("latte", "Catppuccin Latte", false, mapOf(
        "primary" to 0xFF8839EF.toInt(), "onPrimary" to 0xFFEFF1F5.toInt(), "secondary" to 0xFF40A02B.toInt(),
        "background" to 0xFFEFF1F5.toInt(), "surface" to 0xFFEFF1F5.toInt(), "onBackground" to 0xFF4C4F69.toInt(), "onSurface" to 0xFF4C4F69.toInt(),
        "onSurfaceVariant" to 0xFF6C6F85.toInt(), "error" to 0xFFD20F39.toInt(),
        "surfaceContainer" to 0xFFE6E9EF.toInt(), "surfaceContainerHigh" to 0xFFCCD0DA.toInt(), "surfaceContainerHighest" to 0xFFBCC0CC.toInt(),
    )),
    ThemeSpec("dracula", "Dracula", true, mapOf(
        "primary" to 0xFFBD93F9.toInt(), "onPrimary" to 0xFF282A36.toInt(), "secondary" to 0xFF50FA7B.toInt(),
        "background" to 0xFF282A36.toInt(), "surface" to 0xFF282A36.toInt(), "onBackground" to 0xFFF8F8F2.toInt(), "onSurface" to 0xFFF8F8F2.toInt(),
        "onSurfaceVariant" to 0xFFBFC2D6.toInt(), "error" to 0xFFFF5555.toInt(),
        "surfaceContainer" to 0xFF21222C.toInt(), "surfaceContainerHigh" to 0xFF343746.toInt(), "surfaceContainerHighest" to 0xFF44475A.toInt(),
    )),
    ThemeSpec("amoled", "AMOLED", true, mapOf(
        "primary" to 0xFF66BB6A.toInt(), "background" to 0xFF000000.toInt(), "surface" to 0xFF000000.toInt(),
        "surfaceContainer" to 0xFF0E0E0E.toInt(), "surfaceContainerHigh" to 0xFF161616.toInt(), "surfaceContainerHighest" to 0xFF1E1E1E.toInt(),
    )),
    ThemeSpec("system", "System", false, emptyMap()),
    ThemeSpec("light", "Light", false, emptyMap()),
    ThemeSpec("dark", "Dark", true, emptyMap()),
)

private fun parseColor(s: String): Int? = try {
    val t = s.trim().removePrefix("#")
    when (t.length) {
        6 -> (0xFF000000 or t.toLong(16)).toInt()
        8 -> t.toLong(16).toInt()
        else -> null
    }
} catch (e: Exception) { null }

fun colorHex(c: Int) = "#%06X".format(c and 0xFFFFFF)

/** Parses a custom theme; returns null when the text is not a valid theme. */
fun parseTheme(text: String): ThemeSpec? = try {
    val j = JSONObject(text.trim())
    val name = j.optString("name").trim().ifEmpty { return null }
    val colors = mutableMapOf<String, Int>()
    val c = j.optJSONObject("colors") ?: return null
    for (k in c.keys()) parseColor(c.optString(k))?.let { colors[k] = it }
    if (colors.isEmpty()) null
    else ThemeSpec("custom:" + name.lowercase(), name, !j.optString("base", "dark").equals("light", true), colors, false)
} catch (e: Exception) { null }

fun themeToJson(t: ThemeSpec): String {
    val c = JSONObject()
    t.colors.forEach { (k, v) -> c.put(k, colorHex(v)) }
    return JSONObject().put("name", t.name).put("base", if (t.dark) "dark" else "light").put("colors", c).toString(2)
}

enum class NavItem(val id: String) { HOME("home"), CHAT("chat"), ME("me") }

val DRAWER_ITEMS = listOf("favorites", "continue", "friends", "communities", "inventory", "trades", "messages", "settings", "help", "learn", "create", "switch", "logout")

data class Played(val universeId: Long, val placeId: Long, val name: String, val icon: String?, val at: Long)

/** App-wide user settings, backed by SharedPreferences, readable as Compose state. */
class Cfg(ctx: Context) {
    private val p = ctx.getSharedPreferences("lb", Context.MODE_PRIVATE)

    var themeId by mutableStateOf(p.getString("theme_id", "roblox") ?: "roblox")
    val customThemes = mutableStateListOf<ThemeSpec>().also { l ->
        try {
            val a = JSONArray(p.getString("themes", "[]"))
            for (i in 0 until a.length()) parseTheme(a.getString(i))?.let { l += it }
        } catch (_: Exception) {}
    }
    var textScale by mutableFloatStateOf(p.getFloat("text_scale", 1f))
    var haptics by mutableStateOf(p.getBoolean("haptics", true))
    var drawerAlpha by mutableFloatStateOf(p.getFloat("drawer_alpha", 1f))
    var viaDorfikstrap by mutableStateOf(p.getBoolean("via_dorfikstrap", true))
    var drawerWidth by mutableFloatStateOf(p.getFloat("drawer_width", 300f))
    var drawerBlur by mutableFloatStateOf(p.getFloat("drawer_blur", 16f))

    /** Show/hide switches for home blocks, top-bar buttons etc. A missing key means "shown". */
    val toggles = mutableStateMapOf<String, Boolean>().also { m ->
        try { val o = JSONObject(p.getString("toggles", "{}")); for (k in o.keys()) m[k] = o.getBoolean(k) } catch (_: Exception) {}
    }
    fun on(key: String) = toggles[key] != false
    fun onDef(key: String, def: Boolean) = toggles[key] ?: def
    fun set(key: String, v: Boolean) { toggles[key] = v; save() }

    val hiddenSorts = mutableStateListOf<String>().also { it += split("hidden_sorts", emptyList()) }
    val sortOrder = mutableStateListOf<String>().also { it += split("sort_order", emptyList()) }
    fun orderedSorts(all: List<Sort>): List<Sort> {
        val idx = sortOrder.withIndex().associate { it.value to it.index }
        return all.sortedBy { idx[it.id] ?: Int.MAX_VALUE }
    }

    val navOrder = mutableStateListOf<String>().also { it += split("nav_order", NavItem.values().map { n -> n.id }) }
    val navHidden = mutableStateListOf<String>().also { it += split("nav_hidden", emptyList()) }
    val drawerOrder = mutableStateListOf<String>().also { l ->
        val saved = split("drawer_order", DRAWER_ITEMS)
        l += saved.filter { it in DRAWER_ITEMS } + DRAWER_ITEMS.filter { it !in saved }
    }
    val drawerHidden = mutableStateListOf<String>().also { it += split("drawer_hidden", emptyList()) }

    /** "Continue" history is kept per account (key history_<id>; the guest/legacy one is plain "history"). */
    private var owner = try { Session.activeId(ctx) } catch (_: Exception) { 0L }
    private fun histKey() = if (owner == 0L) "history" else "history_$owner"
    private fun loadHistory(): List<Played> = try {
        val a = JSONArray(p.getString(histKey(), "[]"))
        (0 until a.length()).map { i ->
            val o = a.getJSONObject(i)
            Played(o.optLong("u"), o.optLong("p"), o.optString("n"), o.optString("i").ifEmpty { null }, o.optLong("t"))
        }
    } catch (_: Exception) { emptyList() }
    val history = mutableStateListOf<Played>().also { it += loadHistory() }

    fun setOwner(id: Long) {
        if (id == owner) return
        owner = id
        history.clear(); history += loadHistory()
    }

    private fun split(key: String, def: List<String>): List<String> =
        p.getString(key, null)?.split(",")?.filter { it.isNotEmpty() } ?: def

    fun theme(): ThemeSpec = (BUILTIN_THEMES + customThemes).firstOrNull { it.id == themeId } ?: BUILTIN_THEMES[0]

    fun save() {
        p.edit()
            .putString("theme_id", themeId)
            .putString("themes", JSONArray().also { a -> customThemes.forEach { a.put(themeToJson(it)) } }.toString())
            .putFloat("text_scale", textScale).putBoolean("haptics", haptics).putFloat("drawer_alpha", drawerAlpha)
            .putBoolean("via_dorfikstrap", viaDorfikstrap).putFloat("drawer_width", drawerWidth).putFloat("drawer_blur", drawerBlur)
            .putString("toggles", JSONObject().also { o -> toggles.forEach { (k, v) -> o.put(k, v) } }.toString())
            .putString("hidden_sorts", hiddenSorts.joinToString(",")).putString("sort_order", sortOrder.joinToString(","))
            .putString("nav_order", navOrder.joinToString(",")).putString("nav_hidden", navHidden.joinToString(","))
            .putString("drawer_order", drawerOrder.joinToString(",")).putString("drawer_hidden", drawerHidden.joinToString(","))
            .apply()
    }

    fun recordPlayed(g: Game) {
        history.removeAll { it.universeId == g.universeId }
        history.add(0, Played(g.universeId, g.placeId, g.name, g.icon, System.currentTimeMillis()))
        while (history.size > 60) history.removeAt(history.lastIndex)
        val a = JSONArray()
        history.forEach { a.put(JSONObject().put("u", it.universeId).put("p", it.placeId).put("n", it.name).put("i", it.icon ?: "").put("t", it.at)) }
        p.edit().putString(histKey(), a.toString()).apply()
    }

    fun resetLayout() {
        navOrder.clear(); navOrder += NavItem.values().map { it.id }
        navHidden.clear()
        drawerOrder.clear(); drawerOrder += DRAWER_ITEMS
        drawerHidden.clear()
        toggles.clear(); hiddenSorts.clear(); sortOrder.clear()
        save()
    }
}

val LocalCfg = compositionLocalOf<Cfg> { error("Cfg not provided") }

/** Haptic tick, when enabled in settings. */
@Composable
fun rememberTap(haptic: HapticFeedback, cfg: Cfg): () -> Unit = { if (cfg.haptics) haptic.performHapticFeedback(HapticFeedbackType.TextHandleMove) }

fun Played.asGame() = Game(universeId, placeId, name, icon = icon)

val THEME_DOCS = """
LibreBlox themes are JSON. Paste one in Settings > Customization > Import theme.

{
  "name": "My theme",
  "base": "dark",            // "dark" or "light": decides what is not overridden
  "colors": {
    "primary": "#CBA6F7",    // buttons, selected items
    "onPrimary": "#11111B",  // text/icons on primary
    "secondary": "#A6E3A1",
    "background": "#1E1E2E", // screen background
    "surface": "#1E1E2E",
    "onBackground": "#CDD6F4",
    "onSurface": "#CDD6F4",  // main text
    "onSurfaceVariant": "#A6ADC8", // secondary text
    "error": "#F38BA8",
    "surfaceContainer": "#181825",        // cards
    "surfaceContainerHigh": "#313244",    // tiles, bubbles
    "surfaceContainerHighest": "#45475A"
  }
}

Colours are #RRGGBB or #AARRGGBB. Missing keys keep the base theme's value.
Comments (//) are only for explanation: remove them before importing.
Tip: "Copy current theme" exports the selected theme as a starting point.
""".trimIndent()

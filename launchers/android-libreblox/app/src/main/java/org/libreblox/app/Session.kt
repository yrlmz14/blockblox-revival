package org.libreblox.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

data class Acc(val id: Long, val name: String, val display: String, val cookie: String)

/**
 * Stores Roblox session cookies (several accounts) encrypted with a key that never leaves the Android Keystore.
 * A freshly logged-in cookie is first kept as "pending" until the account it belongs to is known.
 */
object Session {
    private const val ALIAS = "lb_session"
    private const val PREF = "lb_session"

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val g = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        g.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return g.generateKey()
    }

    private fun encrypt(plain: String): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, key())
        val ct = c.doFinal(plain.toByteArray())
        return Base64.encodeToString(c.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(ct, Base64.NO_WRAP)
    }

    private fun decrypt(v: String): String? = try {
        val (iv, ct) = v.split(":")
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP)))
        String(c.doFinal(Base64.decode(ct, Base64.NO_WRAP)))
    } catch (e: Exception) {
        null
    }

    private class State(var active: Long, var pending: String?, val accounts: MutableList<Acc>)

    private fun read(ctx: Context): State {
        val p = ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE)
        val st = State(0, null, mutableListOf())
        // Legacy single-cookie value from earlier versions counts as pending.
        p.getString("s", null)?.let { decrypt(it) }?.let { st.pending = it }
        val blob = p.getString("accs", null)?.let { decrypt(it) } ?: return st
        try {
            val j = JSONObject(blob)
            st.active = j.optLong("active")
            st.pending = j.optString("pending").ifEmpty { null } ?: st.pending
            val a = j.optJSONArray("list") ?: JSONArray()
            for (i in 0 until a.length()) {
                val o = a.getJSONObject(i)
                st.accounts += Acc(o.optLong("id"), o.optString("name"), o.optString("display"), o.optString("cookie"))
            }
        } catch (_: Exception) {}
        return st
    }

    private fun write(ctx: Context, st: State) {
        val a = JSONArray()
        st.accounts.forEach { a.put(JSONObject().put("id", it.id).put("name", it.name).put("display", it.display).put("cookie", it.cookie)) }
        val j = JSONObject().put("active", st.active).put("pending", st.pending ?: "").put("list", a)
        ctx.getSharedPreferences(PREF, Context.MODE_PRIVATE).edit().putString("accs", encrypt(j.toString())).remove("s").apply()
    }

    /** Cookie to use now: the pending one right after login, otherwise the active account's. */
    fun load(ctx: Context): String? {
        val st = read(ctx)
        return st.pending ?: st.accounts.firstOrNull { it.id == st.active }?.cookie
    }

    fun accounts(ctx: Context): List<Acc> = read(ctx).accounts
    fun activeId(ctx: Context): Long = read(ctx).active

    /** Right after the login page finished: remember the cookie until the account is known. */
    fun save(ctx: Context, cookie: String) {
        val st = read(ctx)
        st.pending = cookie
        write(ctx, st)
    }

    /** The cookie in use belongs to [id]: store it as that account and make it active. */
    fun register(ctx: Context, id: Long, name: String, display: String) {
        val st = read(ctx)
        val cookie = st.pending ?: st.accounts.firstOrNull { it.id == st.active }?.cookie ?: return
        st.accounts.removeAll { it.id == id }
        st.accounts += Acc(id, name, display, cookie)
        st.active = id
        st.pending = null
        write(ctx, st)
    }

    fun switchTo(ctx: Context, id: Long) {
        val st = read(ctx)
        if (st.accounts.any { it.id == id }) { st.active = id; st.pending = null; write(ctx, st) }
    }

    /** Logs out the active account; another stored account (if any) becomes active. */
    fun clear(ctx: Context) {
        val st = read(ctx)
        if (st.pending != null) {
            st.pending = null
        } else {
            st.accounts.removeAll { it.id == st.active }
            st.active = st.accounts.firstOrNull()?.id ?: 0
        }
        write(ctx, st)
    }
}

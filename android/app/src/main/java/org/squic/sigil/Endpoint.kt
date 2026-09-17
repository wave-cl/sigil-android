package org.squic.sigil

import android.content.Context

/**
 * The UnifiedPush endpoint, kept where the wake service can read it without
 * the window being up. Rust holds the same value while it runs; this is the
 * copy that survives the process.
 */
object Endpoint {
    private const val PREFS = "sigil"
    private const val KEY = "endpoint"

    @JvmStatic
    fun get(ctx: Context): String? =
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, null)

    @JvmStatic
    fun set(ctx: Context, url: String?) {
        ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().apply {
            if (url == null) remove(KEY) else putString(KEY, url)
        }.apply()
    }
}

package org.squic.sigil

import android.app.Activity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

/**
 * What the system draws over the surface: the status bar, the gesture bar,
 * a cutout, and the keyboard while it is up. An edge-to-edge window is
 * never resized for any of them; egui gets the whole surface and nothing
 * in the stack says which parts are covered. This listens for the
 * platform's answer and hands it to Rust, in pixels, every time it changes
 * -- which for the keyboard is how the composer gets out from under it.
 */
object Insets {
    fun watch(activity: Activity) {
        WindowCompat.setDecorFitsSystemWindows(activity.window, false)
        val view = activity.window.decorView
        ViewCompat.setOnApplyWindowInsetsListener(view) { _, insets ->
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
            Native.insets(bars.top, maxOf(bars.bottom, ime.bottom), bars.left, bars.right)
            insets
        }
        ViewCompat.requestApplyInsets(view)
    }
}

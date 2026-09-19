package org.squic.sigil

import android.app.Activity

/**
 * The window, for the things that need an *Activity* rather than a Context.
 *
 * `Native.init` runs from `SigilApp`, so the context Rust holds is the
 * application's. Starting an activity for a result needs the activity
 * itself, and the first attempt at this reached for the raw `jobject`
 * android-activity leaves in `ndk_context` and handed it to Kotlin. That
 * segfaulted the process the moment the paperclip was pressed -- a
 * SIGSEGV inside `Files.pick`, not an exception, because a bad reference is
 * not a null one and Kotlin never got the chance to check it.
 *
 * So the activity is tracked on this side, where its lifecycle is known.
 * Rust asks for a pick and never touches an activity reference.
 */
object Host {
    @Volatile
    private var current: Activity? = null

    fun attach(activity: Activity) {
        current = activity
    }

    /** Only if it is still ours: a new window may already have attached. */
    fun detach(activity: Activity) {
        if (current === activity) current = null
    }

    /** The window, or null when there is none -- which is a real state. */
    fun get(): Activity? = current
}

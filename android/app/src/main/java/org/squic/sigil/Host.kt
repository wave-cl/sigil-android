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

    /**
     * Leave the application: Back was pressed with nothing behind the
     * screen, which on Android means going.
     *
     * Called from Rust rather than from a Back handler here, because the
     * press does not always arrive here -- winit takes `KEYCODE_BACK` off
     * the `NativeActivity` input queue and marks it handled, so the
     * interface is where the decision is made.
     *
     * **`moveTaskToBack`, and not `finish`.** `finish` was tried first, on
     * the reasoning that Back off the last screen ends the application. It
     * does not end this one: eframe's loop belongs to the *process*, which
     * outlives the activity, and the activity it was given is gone -- so the
     * app came back to its splash screen and stayed there, drawing nothing,
     * until something killed the process. Seen on the handset, twice, half a
     * minute each time with the window alive and the session caught up
     * behind it.
     *
     * Going to the back is also the better behaviour, which is the part the
     * first reasoning had backwards: the task stays where the person left
     * it, and reopening is instant rather than a fresh connect.
     *
     * `nonRoot = true` because this activity *is* the task root, which is
     * the only case that matters here.
     */
    @JvmStatic
    fun leave() {
        current?.moveTaskToBack(true)
    }
}

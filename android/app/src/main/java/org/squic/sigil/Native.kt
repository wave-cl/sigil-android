package org.squic.sigil

/**
 * The Rust library's JNI surface. Every function here is exported by
 * `crates/sigil-android/src/jni.rs` under this exact class name; the
 * Kotlin side is the caller in every direction that starts on the phone
 * (a wake, a picked file, a pressed notification) and Rust calls back into
 * [Notifier], [Vault] and [Files] for the rest.
 *
 * Nothing that decides anything lives on this side. Kotlin is the glue the
 * platform requires; sigil-phone is the client.
 */
object Native {
    init {
        System.loadLibrary("sigil_android")
    }

    /** The application context, once, from [SigilApp]. */
    external fun init(context: android.content.Context)

    /**
     * SIP-47's wake window, run to completion on the calling thread: connect,
     * register the endpoint, catch up, write, then notify through [Notifier].
     * Returns a one-line report for the log. `filesDir` is where the store and
     * the identity live; `endpoint` is the UnifiedPush endpoint or null.
     */
    external fun wake(filesDir: String, endpoint: String?, budgetSecs: Int): String

    /** The distributor gave (or changed) the endpoint. Remembered for the next connect. */
    external fun endpoint(url: String?)

    /** Files chosen for an outstanding pick, or none. Paths in the app's cache. */
    external fun picked(paths: Array<String>?)

    /** A notification was pressed: which account, exchange and conversation it led to. */
    external fun pressed(identity: String, exchange: String, channelHex: String)

    /** A `sigil://` link arrived while the app was running. Offered, never acted on silently. */
    external fun link(url: String)
}

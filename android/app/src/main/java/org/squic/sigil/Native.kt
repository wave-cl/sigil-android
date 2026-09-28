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

    /**
     * Refuse a ring, with nothing drawn: connect, say it, close. Runs to
     * completion on the calling thread, which must not be the main one.
     * `seq` is the invitation's, or -1 where the notification did not carry
     * it and the live ring in the conversation is the one to refuse.
     *
     * **Returns whether the refusal actually went out**, so the caller can
     * put the ring back when it did not. The detail is in the log; a caller
     * that had to read prose to decide would one day read it wrong.
     */
    external fun decline(
        filesDir: String,
        exchange: String,
        channelHex: String,
        seq: Long,
        budgetSecs: Int,
    ): Boolean

    /** A notification was pressed: which account, exchange and conversation it led to. */
    external fun pressed(identity: String, exchange: String, channelHex: String, answer: Boolean)

    /**
     * The notice a live call stands behind was pressed: take the person back
     * to the call. Queued for the next pass, like every other press.
     */
    external fun showCall(identity: String)

    /**
     * Hang up, from that notice's own button. Nothing is drawn and the window
     * is not brought forward: ending a call from the shade is what somebody
     * does when they do *not* want the application in front of them.
     */
    external fun hangUp(identity: String)

    /** A `sigil://` link arrived while the app was running. Offered, never acted on silently. */
    external fun link(url: String)

    /**
     * The system's Back was pressed. Queued for the next pass, where it
     * becomes the key event the shell reads.
     *
     * Only for the phones where the press does not reach winit on its own;
     * see [MainActivity] for which those are and why this is not the path
     * the handset takes.
     */
    external fun back()

    /**
     * Whether sigil has anywhere to go back to. Asked by [MainActivity]
     * *before* it hands a press over, because the framework leaves the app
     * when nothing answers and the decision cannot wait for a frame.
     *
     * The last frame's answer. Being one frame stale costs at most a press
     * that does nothing; the alternative is blocking the system's thread on
     * the interface.
     */
    external fun canGoBack(): Boolean

    /**
     * What the system draws over the surface, in pixels: status bar, gesture
     * bar or keyboard, cutouts. From [Insets], whenever it changes.
     */
    external fun insets(top: Int, bottom: Int, left: Int, right: Int)

    /**
     * Whether the phone is in its dark theme, at start and whenever it
     * changes. winit reports no theme on Android, so without this sigil
     * was dark on every phone, whatever the phone was.
     */
    external fun theme(dark: Boolean)

    /**
     * How large the reader asked for text to be, at start and whenever it
     * changes: `Configuration.fontScale`. Nothing in winit or eframe reads
     * it, so without this the phone drew one size of type whatever the
     * system had been set to.
     */
    external fun textScale(scale: Float)
}

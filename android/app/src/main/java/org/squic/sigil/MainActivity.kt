package org.squic.sigil

import android.app.NativeActivity
import android.content.Intent
import android.content.res.Configuration
import android.os.Bundle
import android.view.WindowManager
import org.unifiedpush.android.connector.UnifiedPush

/**
 * The window. winit's NativeActivity hosts the Rust library named in the
 * manifest, whose `android_main` is eframe's entry; this class exists for
 * the two things a NativeActivity cannot see on its own -- the intent that
 * launched it, and intents that arrive while it runs -- and for the
 * notification channels, which must exist before anything posts.
 */
class MainActivity : NativeActivity() {
    private companion object {
        const val PERMISSIONS = 0x5162
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Before anything that might need a window (the file chooser does).
        Host.attach(this)
        Notifier.ensureChannels(this)
        Vault.ensureKey()
        // A call can ring with the screen locked; the flags in the manifest
        // turn it on, and this keeps it on while the window is up.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        Insets.watch(this)
        tellTheme(resources.configuration)
        askForWhatCallsNeed()
        // SIP-45: choose a distributor -- the one already chosen, else the
        // one the platform offers, which includes this app's own embedded
        // FCM one when nothing else is installed -- and register with it.
        // The endpoint arrives at PushServiceImpl.onNewEndpoint.
        UnifiedPush.tryUseCurrentOrDefaultDistributor(this) { found ->
            if (found) UnifiedPush.register(this)
        }
        handle(intent)
        takeBack()
    }

    /**
     * **Back, for the phones where it does not reach winit.**
     *
     * There are two ways Back can arrive. On the handset sigil is developed
     * against it comes through the `NativeActivity` input queue, where winit
     * reads it, maps `KEYCODE_BACK` to `BrowserBack` and marks it handled --
     * so the shell gets it and none of this runs. Confirmed on the device:
     * with a log on [systemBack], Back navigated and the log never printed
     * once, because that OEM turns the dispatcher hook off for us
     * (`OplusPredictiveBackController: should not
     * HookOnBackInvokedCallbackEnabled`).
     *
     * The other way is the one `targetSdk` 36 opts into on a stock phone:
     * `WindowOnBackDispatcher` takes the gesture ahead of the input queue,
     * and an application that registers nothing gets the framework's default
     * -- finishing the activity, from any screen, with no step back at all.
     * This is for those, and it is **unverified**: this hardware cannot
     * enter that path, so it has been reasoned from the platform's contract
     * and not seen working. Test it on a stock device before believing it.
     *
     * Registered on 33 and above, where the dispatcher exists;
     * [onBackPressed] is the same decision for the phones below it. Both
     * call [systemBack], and only one of them can fire for a given press --
     * a dispatcher that takes the gesture does not also queue the key.
     */
    private fun takeBack() {
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            onBackInvokedDispatcher.registerOnBackInvokedCallback(
                android.window.OnBackInvokedDispatcher.PRIORITY_DEFAULT,
            ) { systemBack() }
        }
    }

    /**
     * Ask the interface first, and leave only when it has nothing behind the
     * screen: a menu to close, a step of history, or an app on screen, which
     * on a phone always has one more step -- close the conversation, then
     * show the identity it belongs to.
     *
     * Leaving goes through [Host.leave], which says why it moves the task to
     * the back rather than finishing: finishing this activity leaves eframe's
     * loop with no window and the app comes back to a splash screen it never
     * gets past.
     */
    private fun systemBack() {
        if (Native.canGoBack()) Native.back() else Host.leave()
    }

    @Deprecated("The framework calls this below API 33; above it, takeBack's callback")
    override fun onBackPressed() {
        systemBack()
    }

    /**
     * The phone's light or dark, as the system has it. `uiMode` is in the
     * manifest's configChanges, so a change reaches here rather than
     * recreating the window.
     */
    private fun tellTheme(config: Configuration) {
        val night = config.uiMode and Configuration.UI_MODE_NIGHT_MASK
        Native.theme(night == Configuration.UI_MODE_NIGHT_YES)
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        tellTheme(newConfig)
    }

    override fun onDestroy() {
        // Only if this window is still the one Host holds: a recreation
        // attaches the new one before the old one is destroyed.
        Host.detach(this)
        super.onDestroy()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handle(intent)
    }

    /**
     * Two kinds of intent reach the window: a press on one of sigil's own
     * notifications, which names where it led, and a `sigil://` link from
     * anywhere else. Both are handed to Rust as data; neither is acted on
     * here.
     */
    private fun handle(intent: Intent?) {
        intent ?: return
        val identity = intent.getStringExtra(Notifier.EXTRA_IDENTITY)
        val exchange = intent.getStringExtra(Notifier.EXTRA_EXCHANGE)
        val channel = intent.getStringExtra(Notifier.EXTRA_CHANNEL)
        // **Back to a call in progress**, from the notice it stands behind.
        // Not a conversation: the person may be nowhere near the one the call
        // started in, and a call is a screen rather than a place in one.
        if (intent.getBooleanExtra(Notifier.EXTRA_SHOW_CALL, false) && identity != null) {
            Native.showCall(identity)
        }
        if (identity != null && exchange != null && channel != null) {
            val answering = intent.getBooleanExtra(Notifier.EXTRA_ANSWER, false)
            // **The ring comes down on the press, not when the call is up.**
            // It is posted ongoing, so nobody can swipe it away, and it was
            // taken down by `endRing` only once the answer had been through
            // the window, the session and the far end -- seconds during which
            // a call somebody had already accepted went on ringing at them.
            // Decline has worked this way from the start; Answer had not.
            //
            // Safe to take down early here because this *is* the window
            // opening: the conversation comes up with the ring card on it, so
            // an answer that does not go through is still in front of the
            // person with both buttons on it.
            if (answering) {
                Notifier.endRing(this, channel)
            }
            // Answer is a press that also answers; an ordinary press is not.
            Native.pressed(identity, exchange, channel, answering)
        }
        if (intent.action == Intent.ACTION_VIEW) {
            intent.dataString?.let { if (it.startsWith("sigil://")) Native.link(it) }
        }
    }

    /**

     * The microphone and notifications are runtime permissions: the manifest
     * declares them, and Android grants them only through a prompt. Nothing
     * prompted, so every call ended the moment it connected -- the microphone
     * would not open -- and no notification was ever shown. Asked once, at
     * the start; a refusal is remembered by the system and not asked again
     * every launch.
     */
    private fun askForWhatCallsNeed() {
        val wanted = mutableListOf(android.Manifest.permission.RECORD_AUDIO)
        if (android.os.Build.VERSION.SDK_INT >= 33) {
            wanted += android.Manifest.permission.POST_NOTIFICATIONS
        }
        val missing = wanted.filter {
            checkSelfPermission(it) != android.content.pm.PackageManager.PERMISSION_GRANTED
        }
        if (missing.isNotEmpty()) {
            requestPermissions(missing.toTypedArray(), PERMISSIONS)
        }
    }


    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        Files.onResult(this, requestCode, resultCode, data)
    }
}

package org.squic.sigil

import android.app.NativeActivity
import android.content.Intent
import android.os.Bundle
import android.view.WindowManager

/**
 * The window. winit's NativeActivity hosts the Rust library named in the
 * manifest, whose `android_main` is eframe's entry; this class exists for
 * the two things a NativeActivity cannot see on its own -- the intent that
 * launched it, and intents that arrive while it runs -- and for the
 * notification channels, which must exist before anything posts.
 */
class MainActivity : NativeActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Notifier.ensureChannels(this)
        Vault.ensureKey()
        // A call can ring with the screen locked; the flags in the manifest
        // turn it on, and this keeps it on while the window is up.
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        handle(intent)
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
        if (identity != null && exchange != null && channel != null) {
            Native.pressed(identity, exchange, channel)
        }
        if (intent.action == Intent.ACTION_VIEW) {
            intent.dataString?.let { if (it.startsWith("sigil://")) Native.link(it) }
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        Files.onResult(this, requestCode, resultCode, data)
    }
}

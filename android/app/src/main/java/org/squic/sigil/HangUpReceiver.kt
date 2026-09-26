package org.squic.sigil

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

/**
 * Hang up, from the button on the notice a live call stands behind.
 *
 * **A broadcast, with nothing drawn.** Ending a call from the shade is what
 * somebody does when they do *not* want sigil in front of them -- they left
 * it to look something up and the call is over. An activity intent would
 * bring the window up to do it, which makes the quickest way off a call also
 * the slowest way back to whatever the person was actually doing.
 *
 * Nothing here ends anything. The call belongs to the running session, which
 * is in this very process -- the foreground service that posted this notice
 * is what keeps it alive -- so this hands the identity to Rust and the window
 * ends the call on its next pass, by the same path the card's own Hang up
 * button takes. There is no second way for a call to end, and therefore no
 * second way to forget to tell the exchange.
 */
class HangUpReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val identity = intent.getStringExtra(Notifier.EXTRA_IDENTITY) ?: return
        Log.i("sigil", "hang up, from the call's notice")
        Native.hangUp(identity)
    }
}

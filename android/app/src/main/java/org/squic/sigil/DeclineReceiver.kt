package org.squic.sigil

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlin.concurrent.thread

/**
 * The ring notification's Decline.
 *
 * **A receiver, not an activity.** Answer opens sigil, because a call is a
 * screen and that is what answering asks for. Refusing one is the opposite:
 * bringing the application to the front to say no is precisely what somebody
 * pressing Decline did not want. So this does the work with nothing drawn
 * and nothing started.
 *
 * `goAsync` because the work is a round trip to the exchange -- connect, say
 * it, close -- and `onReceive` may not block. The platform allows a receiver
 * about ten seconds; the budget below is under it, and a decline that ran out
 * leaves the caller ringing out, which is where they were anyway.
 */
class DeclineReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val exchange = intent.getStringExtra(Notifier.EXTRA_EXCHANGE) ?: ""
        val channel = intent.getStringExtra(Notifier.EXTRA_CHANNEL) ?: return
        val seq = intent.getLongExtra(EXTRA_SEQ, -1L)
        // Down at once, before the round trip: the press has to land even
        // though the saying of it takes a second or two.
        Notifier.endRing(context, channel)
        val ctx = context.applicationContext
        val finish = goAsync()
        thread(name = "sigil-decline") {
            try {
                Log.i("sigil", Native.decline(ctx.filesDir.absolutePath, exchange, channel, seq, BUDGET_SECS))
            } catch (e: Throwable) {
                Log.w("sigil", "the decline failed: $e")
            } finally {
                finish.finish()
            }
        }
    }

    companion object {
        /** Under the platform's ten seconds for a receiver, with room to close. */
        const val BUDGET_SECS = 7
        const val EXTRA_SEQ = "org.squic.sigil.SEQ"
    }
}

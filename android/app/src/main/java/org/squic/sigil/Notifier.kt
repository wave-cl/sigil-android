package org.squic.sigil

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.RingtoneManager
import android.os.Build

/**
 * The platform's notification surface, and the only consumer of what the
 * phone composes (SIP-47). Called from Rust with words already chosen under
 * the person's privacy setting; this side adds nothing to them.
 *
 * Three channels: messages, calls (with a full-screen intent, so a ring
 * reaches a locked screen), and the quiet one the foreground services stand
 * behind while they work.
 */
object Notifier {
    const val MESSAGES = "messages"
    const val CALLS = "calls"
    const val QUIET = "quiet"

    const val EXTRA_IDENTITY = "org.squic.sigil.identity"
    const val EXTRA_EXCHANGE = "org.squic.sigil.exchange"
    const val EXTRA_CHANNEL = "org.squic.sigil.channel"

    /** The press was Answer on a ring, not an ordinary press. */
    const val EXTRA_ANSWER = "org.squic.sigil.answer"

    /**
     * The press was on a live call's notice, and means "back to the call".
     * Not a conversation: a call is a screen rather than a place in one, and
     * the person may be nowhere near the conversation it started in.
     */
    const val EXTRA_SHOW_CALL = "org.squic.sigil.showcall"

    fun ensureChannels(ctx: Context) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(MESSAGES, ctx.getString(R.string.channel_messages), NotificationManager.IMPORTANCE_HIGH).apply {
                description = ctx.getString(R.string.channel_messages_about)
                // The launcher's dot: the platform counts these itself, which
                // is why sigil-platform's badge reports itself absent on a phone.
                setShowBadge(true)
            }
        )
        nm.createNotificationChannel(
            NotificationChannel(CALLS, ctx.getString(R.string.channel_calls), NotificationManager.IMPORTANCE_HIGH).apply {
                description = ctx.getString(R.string.channel_calls_about)
                setSound(
                    RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE),
                    AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_NOTIFICATION_RINGTONE).build()
                )
                enableVibration(true)
            }
        )
        nm.createNotificationChannel(
            NotificationChannel(QUIET, ctx.getString(R.string.channel_quiet), NotificationManager.IMPORTANCE_LOW).apply {
                description = ctx.getString(R.string.channel_quiet_about)
                setShowBadge(false)
            }
        )
    }

    /**
     * A message notification. One per conversation: `tag` keys it, so a later
     * window replaces rather than stacks, as SIP-47 asks. Called from Rust.
     *
     * # Why this is not `MessagingStyle`
     *
     * It is the obvious modernisation -- the style that puts a conversation
     * in the shade's Conversations section, with the sender's face and their
     * words as bubbles -- and it cannot be used here without giving away
     * what SIP-47 exists to withhold.
     *
     * `MessagingStyle` is built around a `Person` and models a conversation.
     * `Privacy` has three levels and only the first has both to give:
     *
     *  - `SenderAndText` -- who wrote and what. The style would fit.
     *  - `SenderOnly` -- who wrote, and *nothing of what*. The style would
     *    draw a named person with an empty bubble, which is a worse way of
     *    saying less than a line is.
     *  - `FactOnly` -- that something arrived, and nothing else: no sender,
     *    no conversation, and **one notice for everything** rather than one
     *    per conversation. There is no `Person` and no conversation to model.
     *
     * And this side could not tell them apart if it wanted to. It is handed
     * `title` and `body` already composed under the setting, and adds nothing
     * to them -- which is the whole reason the decision lives where the
     * setting does. Reaching for a sender here would mean this file deciding
     * what may be disclosed, from two strings that cannot say.
     *
     * Doing it properly means Rust passing the sender separately and the
     * level deciding whether to, with the style used only at
     * `SenderAndText`, plus a long-lived dynamic shortcut per conversation
     * for the Conversations section to accept it. That is a real design and
     * a cross-language one; it is not a `setStyle` call.
     */
    @JvmStatic
    fun message(
        ctx: Context,
        identity: String,
        exchange: String,
        channelHex: String?,
        title: String,
        body: String,
        count: Int,
        sound: Boolean,
    ) {
        val open = Intent(ctx, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            putExtra(EXTRA_IDENTITY, identity)
            putExtra(EXTRA_EXCHANGE, exchange)
            channelHex?.let { putExtra(EXTRA_CHANNEL, it) }
        }
        val tag = channelHex ?: "sigil"
        val pending = PendingIntent.getActivity(
            ctx, tag.hashCode(), open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val n = Notification.Builder(ctx, MESSAGES)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(title)
            .setContentText(body)
            .setNumber(count)
            .setContentIntent(pending)
            .setAutoCancel(true)
            .setCategory(Notification.CATEGORY_MESSAGE)
            .setDefaults(if (sound) Notification.DEFAULT_SOUND else 0)
            .build()
        ctx.getSystemService(NotificationManager::class.java).notify(tag, 1, n)
    }

    /**
     * A ring. Full-screen so it reaches a locked screen.
     *
     * **Two different intents, and that is the point.** Pressing the body
     * opens the conversation; pressing Answer opens it *and answers*. They
     * were the same intent once, so Answer only ever opened the window and
     * the call went on ringing behind it -- a button that did nothing.
     * They differ by `EXTRA_ANSWER`, and because a `PendingIntent` is
     * matched without its extras, the two need **different request codes**
     * or the platform hands out the first one twice.
     *
     * The phone still does not answer without showing the conversation: the
     * window comes up and the call is answered there, by the same path the
     * Answer button takes.
     */
    @JvmStatic
    fun ring(ctx: Context, identity: String, exchange: String, channelHex: String, from: String) {
        fun leadingTo(answer: Boolean, code: Int): PendingIntent {
            val open = Intent(ctx, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                putExtra(EXTRA_IDENTITY, identity)
                putExtra(EXTRA_EXCHANGE, exchange)
                putExtra(EXTRA_CHANNEL, channelHex)
                if (answer) putExtra(EXTRA_ANSWER, true)
            }
            return PendingIntent.getActivity(
                ctx, code, open,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }
        val show = leadingTo(false, channelHex.hashCode())
        val answer = leadingTo(true, channelHex.hashCode() xor 0x5ADD)
        // **Decline goes nowhere.** Answer opens sigil, because a call is a
        // screen; refusing one does not want the application brought to the
        // front, so this is a broadcast the receiver handles with nothing
        // drawn. The notification had Answer alone, so the only way to
        // refuse a call from the shade was to open it and find the red
        // handset.
        val decline = PendingIntent.getBroadcast(
            ctx,
            channelHex.hashCode() xor 0x0DEC,
            Intent(ctx, DeclineReceiver::class.java).apply {
                putExtra(EXTRA_IDENTITY, identity)
                putExtra(EXTRA_EXCHANGE, exchange)
                putExtra(EXTRA_CHANNEL, channelHex)
                // So a refusal that could not go out can post the same ring
                // again, rather than a nameless one.
                putExtra(DeclineReceiver.EXTRA_FROM, from)
            },
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        // Who is calling goes in the title. The body says what is
        // happening, and used to carry the *channel's* description --
        // "Somebody is calling" -- which is settings text about the
        // category, not about this call, and reads as though the caller
        // were unknown when the title names them.
        val who = from.ifBlank { ctx.getString(R.string.calling_unknown) }
        val n = Notification.Builder(ctx, CALLS)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(who)
            .setContentText(ctx.getString(R.string.calling))
            .setCategory(Notification.CATEGORY_CALL)
            .setFullScreenIntent(show, true)
            .setContentIntent(show)
            // Ongoing: a ring is not dismissed by swiping it away. It is
            // taken down by `endRing` when the call is answered, declined
            // or given up on, which is the only thing that knows.
            .setOngoing(true)
            .also { b ->
                // **The platform's own shape for a ring, where there is
                // one.** `CallStyle` is what every other phone call on this
                // device uses: the caller named as a `Person`, Answer and
                // Decline drawn as call buttons in their own colours rather
                // than as two words in a row of generic actions, and the
                // notification ranked as a call instead of as a message that
                // happens to say "Calling". A ring built out of plain
                // actions is the one notification on the shade that looks
                // less like a call than the rest of them.
                //
                // `CATEGORY_CALL` and the full-screen intent above were
                // already saying this to the ranker; this says it to the
                // person. The style supplies its own two actions, so the
                // hand-built pair stays on the other branch rather than
                // being added twice.
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                    val caller = android.app.Person.Builder()
                        .setName(who)
                        .setImportant(true)
                        .build()
                    b.setStyle(
                        Notification.CallStyle.forIncomingCall(caller, decline, answer)
                    )
                } else {
                    // API 26-30: the same two things to press, named.
                    b.addAction(
                        Notification.Action.Builder(
                            null, ctx.getString(R.string.answer), answer
                        ).build()
                    )
                    b.addAction(
                        Notification.Action.Builder(
                            null, ctx.getString(R.string.decline), decline
                        ).build()
                    )
                }
            }
            .build()
        ctx.getSystemService(NotificationManager::class.java).notify(channelHex, 2, n)
    }

    /** Whether the platform lets sigil post at all. Called from Rust. */
    @JvmStatic
    fun enabled(ctx: Context): Boolean =
        ctx.getSystemService(NotificationManager::class.java).areNotificationsEnabled()

    /** Take a ring down: answered elsewhere, or over. */
    @JvmStatic
    fun endRing(ctx: Context, channelHex: String) {
        ctx.getSystemService(NotificationManager::class.java).cancel(channelHex, 2)
    }

    /**
     * Take down what was said about a conversation, because it has been
     * read.
     *
     * **A ring could be withdrawn and a message could not.** One notice is
     * posted per conversation and only the ring had a way down, so reading a
     * conversation in the window left its notification on the shade — and
     * the only ways to be rid of it were to tap it, which opens the
     * conversation just finished, or to swipe each one away by hand.
     *
     * The same tag the notice was posted under, and the id beside the
     * ring's: `message` uses 1 and `ring` uses 2, so a conversation can have
     * both and lose them one at a time.
     */
    @JvmStatic
    fun endMessage(ctx: Context, channelHex: String) {
        ctx.getSystemService(NotificationManager::class.java).cancel(channelHex, 1)
    }
}

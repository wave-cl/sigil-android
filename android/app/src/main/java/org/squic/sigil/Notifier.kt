package org.squic.sigil

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.media.AudioAttributes
import android.media.RingtoneManager

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
            .addAction(Notification.Action.Builder(null, ctx.getString(R.string.answer), answer).build())
            .addAction(Notification.Action.Builder(null, ctx.getString(R.string.decline), decline).build())
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
}

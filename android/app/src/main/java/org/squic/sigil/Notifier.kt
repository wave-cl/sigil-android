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
     * A ring. Full-screen so it reaches a locked screen; Answer and Decline
     * lead into the window, where the call is answered exactly as on a
     * desktop -- the phone does not answer a call from a notification
     * without showing it.
     */
    @JvmStatic
    fun ring(ctx: Context, identity: String, exchange: String, channelHex: String, from: String) {
        val open = Intent(ctx, MainActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
            putExtra(EXTRA_IDENTITY, identity)
            putExtra(EXTRA_EXCHANGE, exchange)
            putExtra(EXTRA_CHANNEL, channelHex)
        }
        val pending = PendingIntent.getActivity(
            ctx, channelHex.hashCode(), open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val n = Notification.Builder(ctx, CALLS)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(from)
            .setContentText(ctx.getString(R.string.channel_calls_about))
            .setCategory(Notification.CATEGORY_CALL)
            .setFullScreenIntent(pending, true)
            .setContentIntent(pending)
            .setOngoing(true)
            .setAutoCancel(true)
            .addAction(Notification.Action.Builder(null, ctx.getString(R.string.answer), pending).build())
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

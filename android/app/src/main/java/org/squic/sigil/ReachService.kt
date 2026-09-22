package org.squic.sigil

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log

/**
 * Reachable without a distributor: the process kept alive, with the
 * connection to the exchange open, so a call or a message reaches the
 * shade while sigil is not in front.
 *
 * A phone with no push distributor is woken by nothing (SIP-45), and a
 * process that is not in front is stopped by the system within minutes --
 * so without this, sigil could be called only while it was on screen. This
 * is what a messenger does on a phone without push: a quiet ongoing notice,
 * a `specialUse` foreground service, and the battery pays for it. It is the
 * person's choice, on the Phone tab, and off by default.
 *
 * Started and stopped from Rust as the setting changes and at launch; the
 * connection is the session's, and the service only keeps the process.
 */
class ReachService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val open = Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        val pending = PendingIntent.getActivity(
            this, 0, open,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notice = Notification.Builder(this, Notifier.QUIET)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(getString(R.string.reachable))
            .setContentText(getString(R.string.reachable_about))
            .setContentIntent(pending)
            .setOngoing(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            try {
                startForeground(5, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
            } catch (e: Exception) {
                // Said, and stopped: a service the system will not keep in
                // the foreground is one it will kill without a word, and the
                // Phone tab would go on saying the phone is reachable.
                Log.e(TAG, "no foreground service to stay reachable with", e)
                stopSelf()
                return START_NOT_STICKY
            }
        } else {
            startForeground(5, notice)
        }
        return START_STICKY
    }

    companion object {
        private const val TAG = "sigil"

        @JvmStatic
        fun begin(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, ReachService::class.java))
        }

        @JvmStatic
        fun end(ctx: Context) {
            ctx.stopService(Intent(ctx, ReachService::class.java))
        }
    }
}

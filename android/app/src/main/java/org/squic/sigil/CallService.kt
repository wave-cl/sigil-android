package org.squic.sigil

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * A call in progress. The microphone is open, and a phone must not hold a
 * microphone open in silence: this keeps the process alive with the screen
 * off and puts the fact on the status bar, with a way to hang up that leads
 * back into the window where the call is.
 *
 * Started and stopped from Rust as a call begins and ends -- the call is
 * the session's, and the service only reports it.
 */
class CallService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val with = intent?.getStringExtra(EXTRA_WITH) ?: ""
        val open = Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        val pending = android.app.PendingIntent.getActivity(
            this, 0, open,
            android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE
        )
        val notice = Notification.Builder(this, Notifier.QUIET)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(getString(R.string.in_call))
            .setContentText(with)
            .setContentIntent(pending)
            .setOngoing(true)
            .setCategory(Notification.CATEGORY_CALL)
            .build()
        val types = ServiceInfo.FOREGROUND_SERVICE_TYPE_PHONE_CALL or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(4, notice, types)
        } else {
            startForeground(4, notice)
        }
        return START_STICKY
    }

    companion object {
        const val EXTRA_WITH = "org.squic.sigil.with"

        @JvmStatic
        fun begin(ctx: Context, with: String) {
            ctx.startForegroundService(Intent(ctx, CallService::class.java).putExtra(EXTRA_WITH, with))
        }

        @JvmStatic
        fun end(ctx: Context) {
            ctx.stopService(Intent(ctx, CallService::class.java))
        }
    }
}

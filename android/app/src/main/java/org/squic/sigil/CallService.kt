package org.squic.sigil

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log

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
        // **Both types, with a fallback -- not because the permission is
        // missing.** From Android 14 the `phoneCall` type wants
        // MANAGE_OWN_CALLS, the default dialer, or the call-screening role,
        // and the manifest declares MANAGE_OWN_CALLS, which is a normal
        // permission granted at install. So this should be allowed.
        //
        // The fallback is for the case where it is not anyway. A refused
        // `startForeground` throws, and this is called as a call begins: the
        // *call* must not be what breaks over a notification category. What
        // the service is actually for is a microphone that is already open,
        // and `microphone` alone says that and needs only RECORD_AUDIO. The
        // log says which was taken, because "the call ended when the screen
        // did" and "the app died when the call started" look the same from
        // outside and have opposite fixes.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            val both = ServiceInfo.FOREGROUND_SERVICE_TYPE_PHONE_CALL or
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
            try {
                startForeground(4, notice, both)
            } catch (e: Exception) {
                Log.w(TAG, "phoneCall foreground type refused, using microphone alone", e)
                try {
                    startForeground(4, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
                } catch (e2: Exception) {
                    // Nothing else to try. Stop rather than sit as a service
                    // that is not in the foreground, which the system will
                    // kill anyway and without saying why.
                    Log.e(TAG, "no foreground service for this call", e2)
                    stopSelf()
                    return START_NOT_STICKY
                }
            }
        } else {
            startForeground(4, notice)
        }
        return START_STICKY
    }

    companion object {
        private const val TAG = "sigil"
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

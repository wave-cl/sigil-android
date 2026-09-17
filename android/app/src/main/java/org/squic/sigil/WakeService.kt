package org.squic.sigil

import android.app.Notification
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import kotlin.concurrent.thread

/**
 * The wake window, as a short foreground service. The platform grants a
 * foreground service a bounded run and shows the quiet notice while it
 * lasts, which is the honest shape for work done while nobody is looking;
 * the window itself is Rust's, run on a thread of its own, and this stops
 * itself the moment it returns.
 */
class WakeService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notice = Notification.Builder(this, Notifier.QUIET)
            .setSmallIcon(R.drawable.ic_stat_sigil)
            .setContentTitle(getString(R.string.waking))
            .setOngoing(true)
            .build()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(3, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(3, notice)
        }
        thread(name = "sigil-wake") {
            val report = try {
                Native.wake(filesDir.absolutePath, Endpoint.get(this), BUDGET_SECS)
            } catch (e: Throwable) {
                "the wake window failed: $e"
            }
            Log.i("sigil", report)
            stopSelf(startId)
        }
        return START_NOT_STICKY
    }

    companion object {
        /** Under the platform's short-service ceiling, with room for the close. */
        const val BUDGET_SECS = 20

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, WakeService::class.java))
        }
    }
}

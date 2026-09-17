package org.squic.sigil

import android.app.Application

/**
 * The process, before any activity or service: hands Rust the application
 * context it posts notifications and reads the key store through, so the
 * wake service can work with no window up.
 */
class SigilApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Native.init(this)
        Notifier.ensureChannels(this)
    }
}

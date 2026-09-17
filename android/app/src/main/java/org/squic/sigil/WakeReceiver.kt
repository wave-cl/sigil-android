package org.squic.sigil

import android.content.Context
import org.unifiedpush.android.connector.MessagingReceiver
import org.unifiedpush.android.connector.data.PushEndpoint
import org.unifiedpush.android.connector.data.PushMessage
import org.unifiedpush.android.connector.FailedReason

/**
 * SIP-45, as it reaches the phone: the distributor delivers four bytes and
 * this starts the window. The bytes are not read -- there is nothing in
 * them to read, and a wake that carried more would be one to distrust.
 *
 * A new endpoint is handed to Rust, which registers it on the next connect
 * (SIP-47 §Connecting, step 2); an unregistration clears it, so the phone
 * stops telling exchanges about an address nothing answers at.
 */
class WakeReceiver : MessagingReceiver() {
    override fun onMessage(context: Context, message: PushMessage, instance: String) {
        WakeService.start(context)
    }

    override fun onNewEndpoint(context: Context, endpoint: PushEndpoint, instance: String) {
        Native.endpoint(endpoint.url)
    }

    override fun onUnregistered(context: Context, instance: String) {
        Native.endpoint(null)
    }

    override fun onRegistrationFailed(context: Context, reason: FailedReason, instance: String) {
        Native.endpoint(null)
    }
}

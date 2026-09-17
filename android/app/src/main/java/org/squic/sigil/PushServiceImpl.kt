package org.squic.sigil

import org.unifiedpush.android.connector.*
import org.unifiedpush.android.connector.data.*

/**
 * SIP-45, as it reaches the phone: the distributor delivers four bytes and
 * this starts the window. The bytes are not read -- there is nothing in
 * them to read, and a wake that carried more would be one to distrust.
 *
 * A new endpoint is handed to Rust, which registers it on the next connect
 * (SIP-47 §Connecting, step 2); an unregistration clears it, so the phone
 * stops telling exchanges about an address nothing answers at.
 */
class PushServiceImpl : PushService() {
    override fun onMessage(message: PushMessage, instance: String) {
        WakeService.start(this)
    }

    override fun onNewEndpoint(endpoint: PushEndpoint, instance: String) {
        Endpoint.set(this, endpoint.url)
        Native.endpoint(endpoint.url)
    }

    override fun onUnregistered(instance: String) {
        Endpoint.set(this, null)
        Native.endpoint(null)
    }

    override fun onRegistrationFailed(reason: FailedReason, instance: String) {
        Endpoint.set(this, null)
        Native.endpoint(null)
    }
}

package org.squic.sigil

import org.unifiedpush.android.embedded_fcm_distributor.EmbeddedDistributorReceiver
import org.unifiedpush.android.embedded_fcm_distributor.Gateway

/**
 * The bridge SIP-47 describes, for a phone with no UnifiedPush distributor
 * installed: the application itself is the distributor, over Firebase
 * Cloud Messaging, and its endpoint is an address at a gateway the
 * publisher runs. The gateway forwards a POST to FCM as a high-priority
 * data message carrying the body -- `wake` -- and nothing else, and answers
 * 404 for a token FCM no longer knows so the exchange forgets it (SIP-45).
 *
 * The connector prefers a real distributor; this one is offered only when
 * nothing else is installed. What the gateway sees is what SIP-45 lets a
 * distributor see: *when*, per token. Set the gateway with the Gradle
 * property `sigil.wakeProxy`; with none configured there is no gateway and
 * the distributor stays out of the way.
 *
 * `vapid` is empty: SIP-45's wake carries no VAPID authorisation, and the
 * gateway must accept a bare POST. A gateway that insists on one is not
 * one this stack can use.
 */
class FcmDistributor : EmbeddedDistributorReceiver() {
    override val gateway: Gateway?
        get() {
            val proxy = BuildConfig.WAKE_PROXY
            if (proxy.isBlank()) return null
            return object : Gateway {
                override val vapid: String = ""
                override fun getEndpoint(token: String): String = "$proxy/wpfcm?t=$token"
            }
        }
}

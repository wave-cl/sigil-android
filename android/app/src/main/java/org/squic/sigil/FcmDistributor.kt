package org.squic.sigil

import android.content.Context
import org.unifiedpush.android.embedded_fcm_distributor.EmbeddedDistributorReceiver

/**
 * The bridge SIP-47 describes, for a phone with no UnifiedPush distributor
 * installed: the application itself is the distributor, over Firebase
 * Cloud Messaging, and its endpoint is an address at a rewrite proxy the
 * publisher runs. The proxy forwards a POST to FCM as a high-priority data
 * message with the body -- `wake` -- and nothing else, and answers 404 for a
 * token FCM no longer knows so the exchange forgets it (SIP-45).
 *
 * Used only when nothing else is installed; the connector prefers a real
 * distributor. What the bridge sees is what SIP-45 lets a distributor see:
 * *when*, per token. Set the proxy in `strings.xml` (`wake_proxy`); with
 * none configured this distributor stays out of the way.
 */
class FcmDistributor : EmbeddedDistributorReceiver() {
    override fun getEndpoint(context: Context, token: String, instance: String): String {
        val proxy = context.getString(R.string.wake_proxy)
        return "$proxy/FCM?v2&instance=$instance&token=$token"
    }
}

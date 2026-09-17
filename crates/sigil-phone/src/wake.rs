//! SIP-45, the device's side: leave an endpoint with the exchange, on the
//! connection the session already holds.
//!
//! Done on every connect (SIP-47 §Connecting, step 2): re-registering is
//! idempotent and cheap, and a phone that forgot to would go quiet at the
//! end of the last `ttl`, silently. The endpoint is the distributor's --
//! whatever UnifiedPush application the person installed, or the
//! publisher's bridge on a phone with nothing else -- and the exchange
//! makes nothing of it but a place to post four bytes to.

use sqex_proto::refusal::Refusal;
use sqex_proto::wake::{MAX_TTL, Register, forget as forget_body};

/// What came of asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registered {
    /// The exchange keeps the endpoint until `ttl` runs out, by its clock.
    Kept,
    /// This exchange does not wake devices (SIP-45 is additive, and an
    /// exchange from before it answers `not_found`). The phone keeps its
    /// stream open as long as the platform lets it, which is what it did.
    Unsupported,
    /// Refused, with the exchange's reason.
    Refused(String),
    /// The connection failed under the request.
    Failed(String),
}

/// `POST /wake/register`. `ttl` is clamped to SIP-45's 30 days.
pub async fn register(client: &mut sqnr::Client, endpoint: &str, ttl: u32) -> Registered {
    let req = Register {
        ttl: ttl.clamp(1, MAX_TTL),
        endpoint: endpoint.to_string(),
    };
    match client.post("/wake/register", req.encode()).await {
        Ok((200, _)) => Registered::Kept,
        Ok((404, _)) => Registered::Unsupported,
        Ok((code, body)) => Registered::Refused(said(code, &body)),
        Err(e) => Registered::Failed(e),
    }
}

/// `POST /wake/forget`: the person turned wakes off, or the distributor
/// was uninstalled. A registration left behind would have the exchange
/// posting to an address nothing answers at, until its `ttl`.
pub async fn forget(client: &mut sqnr::Client) -> Registered {
    match client.post("/wake/forget", forget_body()).await {
        Ok((200, _)) => Registered::Kept,
        Ok((404, _)) => Registered::Unsupported,
        Ok((code, body)) => Registered::Refused(said(code, &body)),
        Err(e) => Registered::Failed(e),
    }
}

/// A refusal in words: the typed code when the body is one, the status
/// otherwise.
fn said(code: u16, body: &[u8]) -> String {
    match Refusal::decode(body) {
        Ok(r) => format!("{} ({code})", r),
        Err(_) => format!("status {code}"),
    }
}

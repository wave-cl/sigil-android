//! SIP-47's wake window.
//!
//! Woken, the phone has a budget it does not choose -- tens of seconds on
//! the platforms that exist -- and MUST do the following in this order,
//! stopping cleanly wherever the budget runs out:
//!
//! 1. connect, subscribe, register the endpoint, top up prekeys;
//! 2. fetch what is new, open what it can, **write it to the store**;
//! 3. only then compose notifications, from the store;
//! 4. close the stream and stop.
//!
//! Steps 1 and 2 are `sigil_chat::session` doing what it always does; the
//! session subscribes before it reconciles, tops up its prekeys before it
//! asks for anything, and absorbs every entry into the store before it
//! publishes a state that mentions it. What this module adds is the
//! registration, the waiting, the order of 3 after 2, and 4. The order of 2
//! and 3 is the rule the SIP exists for: opening an envelope sealed to a
//! one-time prekey spends the prekey, and a phone that composed first and
//! was suspended before writing would hold a notification for a message
//! nothing can open again.
//!
//! [`Outcome::steps`] records the order things happened in, so a test can
//! assert it rather than trust this comment.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use sigil_chat::session::{self, LinkState};
use sigil_net::Dial;
use sqnr_core::SoftwareSigner;

use crate::notify::{self, Privacy};
use crate::phone::{Phone, Ring};
use crate::wake::{self, Registered};

/// Where the exchange should post `wake`, and for how long to keep it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distributor {
    pub url: String,
    /// At most 30 days (SIP-45); at least the platform's suspension horizon.
    pub ttl_secs: u32,
}

/// One wake window: what to connect as, where, and what to do with what
/// arrives.
pub struct Window {
    pub dial: Dial,
    pub signer: SoftwareSigner,
    /// Where the store lives. `None` is the real `~/.sqex/chat` -- on a
    /// phone, under the directory the host set `HOME` to. Tests always pass
    /// one.
    pub store_at: Option<PathBuf>,
    /// The endpoint to leave with the exchange, if the phone has one.
    pub distributor: Option<Distributor>,
    /// The whole window. The platform's budget, less a margin for the close.
    pub budget: Duration,
    /// How long with nothing new arriving before the window is considered
    /// caught up. Measured from the moment the exchange answered the
    /// conversation list, since the per-channel fetches follow it.
    pub settle: Duration,
    pub privacy: Privacy,
}

impl Window {
    /// Sensible numbers for a phone: a 20 s budget and a second of quiet.
    pub fn new(dial: impl Into<Dial>, signer: SoftwareSigner) -> Window {
        Window {
            dial: dial.into(),
            signer,
            store_at: None,
            distributor: None,
            budget: Duration::from_secs(20),
            settle: Duration::from_secs(1),
            privacy: Privacy::default(),
        }
    }
}

/// What happened, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// The link came up: subscribed, and prekeys being topped up.
    Connected,
    /// The endpoint was offered to the exchange, with its answer.
    Registered(Registered),
    /// The exchange answered the conversation list.
    Synced,
    /// Nothing new arrived for `settle`: caught up, as far as this window
    /// can tell.
    Settled,
    /// Notifications and rings were handed to the phone. Always after
    /// `Synced`, and after everything is in the store.
    Said { notified: usize, rang: usize },
    /// The stream was closed and the store lock released.
    Closed,
}

/// The window's report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub connected: bool,
    /// `None` when there was no distributor to register, or the link never
    /// came up to register it on.
    pub registered: Option<Registered>,
    pub synced: bool,
    /// The budget ran out before the window settled. What was written stays
    /// written; what was not said is said next time.
    pub ran_out: bool,
    /// Messages from others this device did not hold when the window opened.
    pub arrivals: usize,
    pub notified: usize,
    pub rang: usize,
    pub trouble: Option<String>,
    pub steps: Vec<Step>,
    pub took: Duration,
}

/// Run one window. Returns once the stream is closed and the store lock is
/// free, so the next window -- or the foreground application -- can open it.
pub async fn run(window: Window, phone: &dyn Phone) -> Outcome {
    let started = Instant::now();
    let deadline = started + window.budget;
    let mut out = Outcome::default();
    let mut handle = session::start(window.dial, window.signer, window.store_at, || {});

    let mut asked_to_register = false;
    let mut quiet_since: Option<Instant> = None;
    let mut seen = 0usize;
    loop {
        let now = Instant::now();
        if now >= deadline {
            out.ran_out = true;
            tracing::warn!("the wake window ran out before it settled");
            break;
        }
        let state = handle.state();
        if let Some(trouble) = state.trouble.clone() {
            out.trouble = Some(trouble);
            break;
        }
        if state.link == LinkState::Up && !out.connected {
            out.connected = true;
            out.steps.push(Step::Connected);
        }
        // SIP-47 §Connecting, step 2: the endpoint, on the connection the
        // session just made. Once; the session re-dials on its own if the
        // link drops, and a window is too short to care.
        if out.connected
            && !asked_to_register
            && let Some(distributor) = &window.distributor
            && let Some((mut client, _)) = handle.connection().now()
        {
            asked_to_register = true;
            let answer = wake::register(&mut client, &distributor.url, distributor.ttl_secs).await;
            match &answer {
                Registered::Kept => tracing::info!("wake endpoint registered"),
                other => tracing::warn!(?other, "wake endpoint not registered"),
            }
            out.steps.push(Step::Registered(answer.clone()));
            out.registered = Some(answer);
        }
        if state.synced && !out.synced {
            out.synced = true;
            out.steps.push(Step::Synced);
            quiet_since = Some(now);
        }
        if out.synced {
            let n = handle.unseen().len() + handle.ringing().len();
            if n != seen {
                seen = n;
                quiet_since = Some(now);
            }
            if quiet_since.is_some_and(|q| now.duration_since(q) >= window.settle) {
                out.steps.push(Step::Settled);
                break;
            }
        }
        let wait = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(100));
        let _ = tokio::time::timeout(wait, handle.changed()).await;
    }

    // Step 3, and only now: everything the session reports is in its store
    // already, since it absorbs before it publishes. `unseen`, not
    // `arrivals`: what this device did not hold when the window opened,
    // which for a phone woken from sleep is everything it missed -- and
    // "arrivals" is what arrived while it watched, which is nothing.
    if out.connected {
        let unseen = handle.unseen();
        out.arrivals = unseen.len();
        for notification in notify::compose(&unseen, window.privacy) {
            if phone.notify(&notification) {
                out.notified += 1;
            }
        }
        for ring in handle.ringing() {
            if ring.mine || ring.answered {
                continue;
            }
            let ring = Ring {
                channel: ring.channel,
                seq: ring.seq,
                from: ring.from,
                label: ring.label.clone(),
                direct: ring.peer.is_some(),
            };
            if phone.ring(&ring) {
                out.rang += 1;
            }
        }
        if out.notified + out.rang > 0 {
            out.steps.push(Step::Said {
                notified: out.notified,
                rang: out.rang,
            });
        }
    }

    // Step 4. Waiting for the close is what makes the next open safe: the
    // store lock is the session's until its task has ended, and a window
    // that returned before then would have the next one refused as "another
    // client is already using this account".
    let closing = handle.close();
    let gone = Instant::now() + Duration::from_secs(5);
    while !closing.is_finished() && Instant::now() < gone {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    if !closing.is_finished() {
        tracing::warn!("the session did not end within five seconds of being closed");
    }
    out.steps.push(Step::Closed);
    out.took = started.elapsed();
    out
}

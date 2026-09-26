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

use sigil_chat::session::{self, Cmd, LinkState};
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
    /// What this person has muted, and whether they are on do-not-disturb.
    /// Read from the same `quiet.json` the running client writes; a wake
    /// window that ignored it would wake the phone for the one
    /// conversation they said not to.
    pub quiet: sigil::Quiet,
    /// Which exchange this window is for, as the roster names it -- empty
    /// for the identity's default one. A mute is keyed by exchange *and*
    /// channel, so this has to be the same string the client muted under.
    pub exchange: String,
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
            quiet: sigil::Quiet::default(),
            exchange: String::new(),
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
    /// A ring was refused: the live signal and the durable entry both sent.
    Declined,
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
    /// A ring this run was asked to refuse, and did.
    pub declined: bool,
    pub trouble: Option<String>,
    pub steps: Vec<Step>,
    pub took: Duration,
}

/// **Refuse a ring with nothing on screen.**
///
/// The shade offers Answer and Decline. Answering opens the application,
/// which is right: a call is a screen, and somebody who presses Answer is
/// asking for it. Refusing is the opposite — bringing the application
/// forward to say no is precisely what the person did not ask for — so this
/// connects, says it, and closes, drawing nothing.
///
/// It says the two things `Cmd::Decline` says: the live SIP-36 signal, for a
/// caller who is listening at that instant, and the durable `CALL_DECLINED`
/// entry, for one who is not. The second is why this cannot simply drop the
/// notification and do nothing: a caller whose client was not listening
/// would otherwise hear the call ring out, and learn nothing about it.
///
/// Connecting is all it waits for. A decline names the call it refuses, so
/// it needs neither the conversation list nor anything fetched.
pub async fn decline(window: Window, channel: [u8; 32], seq: Option<u64>) -> Outcome {
    let started = Instant::now();
    let deadline = started + window.budget;
    let mut out = Outcome::default();
    let mut handle = session::start(window.dial, window.signer, window.store_at, || {});
    let mut said: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if now >= deadline {
            out.ran_out = true;
            tracing::warn!("the decline ran out before the link came up");
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
        // **The `seq` when the caller of this knows it, and otherwise the
        // ring itself.** A notification posted by the wake window carries
        // the invitation's `seq`; one posted by the running client carries
        // only a `Target`, which has no room for it. Rather than teach the
        // whole notification path a new field for one button, this finds
        // the live ring in the conversation -- which is the session's own
        // knowledge, and the thing being refused.
        if out.connected && said.is_none() {
            let refuse = seq.or_else(|| {
                handle
                    .ringing()
                    .iter()
                    .filter(|r| r.channel == channel && !r.mine)
                    .map(|r| r.seq)
                    .max()
            });
            if let Some(seq) = refuse {
                handle.send(Cmd::Decline { channel, seq });
                said = Some(now);
                out.declined = true;
                out.steps.push(Step::Declined);
            }
        }
        // The ring leaving the session's own list is its word that the
        // refusal went out. **With a floor under it**, because a session
        // that kept the ring for any reason would otherwise hold this open
        // to the budget, with the microphone of a call nobody answered
        // ringing at the other end for as long as it took.
        if let Some(at) = said {
            let gone = !handle
                .ringing()
                .iter()
                .any(|r| r.channel == channel && !r.mine);
            if gone || now.duration_since(at) >= window.settle {
                break;
            }
        }
        let wait = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(50));
        let _ = tokio::time::timeout(wait, handle.changed()).await;
    }

    // The same close as a window's, and for the same reason: the store lock
    // is the session's until its task ends, and the application opening
    // next would be refused as "another client is already using this
    // account".
    let closing = handle.close();
    let gone = Instant::now() + Duration::from_secs(5);
    while !closing.is_finished() && Instant::now() < gone {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    out.steps.push(Step::Closed);
    out.took = started.elapsed();
    out
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
        let quiet = window.quiet.clone();
        let exchange = window.exchange.clone();
        let silenced = move |channel: &[u8; 32]| quiet.silenced(&exchange, channel);
        for notification in notify::compose(&unseen, window.privacy, &silenced) {
            if phone.notify(&notification) {
                out.notified += 1;
            }
        }
        for ring in handle.ringing() {
            // **A muted conversation does not ring either.** Awake,
            // `sigil_chat::announce` puts its rings through the same
            // `Quiet::silenced` before posting one. Asleep this did not --
            // so do-not-disturb, which is the setting somebody turns on
            // precisely to stop a phone ringing, silenced the messages and
            // let the calls through.
            if ring.mine || ring.answered || silenced(&ring.channel) {
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

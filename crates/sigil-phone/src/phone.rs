//! The phone, as the window and the shell see it: something that can show a
//! notification and present a call. Everything platform-shaped is behind
//! this, so the rest of the crate tests on a desktop with [`Fake`].

use std::sync::Mutex;

use sqnr_core::PubKey;

use crate::notify::Notification;

/// A call ringing when the phone looked (SIP-36 through a SIP-45 wake), as
/// the session reports it: within the ring window, not ours, not yet
/// answered. Presented through the platform's call surface; a call the
/// session no longer reports as ringing is a missed one, and the transcript
/// says so when it is next opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ring {
    pub channel: [u8; 32],
    pub seq: u64,
    pub from: PubKey,
    /// What the conversation it rang in is called.
    pub label: String,
    /// A direct message: the caller is the conversation.
    pub direct: bool,
}

/// What the phone can do for the window.
pub trait Phone: Send + Sync {
    /// Show a notification. Returns whether it was shown.
    fn notify(&self, notification: &Notification) -> bool;
    /// Present a call that is ringing. Returns whether it was.
    fn ring(&self, ring: &Ring) -> bool;

    /// Take a ring down: it was declined, answered, cancelled, or the
    /// caller gave up.
    ///
    /// **The window could post one and had no way to withdraw it.** A ring
    /// goes up as an *ongoing* notification, which cannot be swiped away,
    /// and the only thing that took one down was
    /// `sigil_chat::announce::withdraw_gone` — which sweeps the rings the
    /// *running client* posted, from a record it keeps in memory. A ring
    /// posted while the phone was asleep is in no such record, in no such
    /// process. So a call nobody answered left an unswipeable "Incoming
    /// call" on the shade, still offering Answer, for good.
    ///
    /// The same bug `withdraw_gone`'s own comment describes as fixed —
    /// fixed on the path that was awake.
    fn unring(&self, channel: &[u8; 32]);
}

/// A phone that remembers what it was asked to show. For tests.
#[derive(Default)]
pub struct Fake {
    pub notified: Mutex<Vec<Notification>>,
    pub rang: Mutex<Vec<Ring>>,
    pub unrang: Mutex<Vec<[u8; 32]>>,
}

impl Fake {
    pub fn notifications(&self) -> Vec<Notification> {
        self.notified
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn rings(&self) -> Vec<Ring> {
        self.rang.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Which rings it was asked to take down.
    pub fn unrings(&self) -> Vec<[u8; 32]> {
        self.unrang
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Phone for Fake {
    fn notify(&self, notification: &Notification) -> bool {
        self.notified
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(notification.clone());
        true
    }

    fn ring(&self, ring: &Ring) -> bool {
        self.rang
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(ring.clone());
        true
    }

    fn unring(&self, channel: &[u8; 32]) {
        self.unrang
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(*channel);
    }
}

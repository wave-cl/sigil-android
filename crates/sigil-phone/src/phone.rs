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
}

/// A phone that remembers what it was asked to show. For tests.
#[derive(Default)]
pub struct Fake {
    pub notified: Mutex<Vec<Notification>>,
    pub rang: Mutex<Vec<Ring>>,
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
}

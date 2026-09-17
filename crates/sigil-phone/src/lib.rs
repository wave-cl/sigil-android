//! What makes sigil a phone client -- SIP-47, on top of sigil's own crates.
//!
//! A desktop client is a process that runs. A phone client is a process that
//! is *allowed* to run, for seconds at a time, when the platform says so; and
//! what it may do in those seconds, in what order, is the whole design. This
//! crate is that order, written so it can be run and tested on a desktop
//! against a real exchange with nothing of a phone in it:
//!
//! - [`pairing`] -- the two strings that make a phone a device of an account;
//! - [`wake`] -- leaving a SIP-45 endpoint with the exchange, every connect;
//! - [`window`] -- the wake window: connect, register, catch up, **write**,
//!   and only then say anything;
//! - [`notify`] -- what is said, composed from plaintext the phone alone
//!   opened, under the person's setting;
//! - [`phone`] -- the seam the platform is behind, and a fake for tests.
//!
//! The session itself is `sigil_chat::session`, unchanged: the phone
//! reconciles, tops up prekeys, opens envelopes and writes its store with the
//! same code the desktop does, which is what makes "the store is written
//! before anything is shown" a property of this crate's *order* rather than
//! of a second implementation.

pub mod notify;
pub mod pairing;
pub mod phone;
pub mod wake;
pub mod window;

pub use notify::{Notification, Privacy};
pub use phone::{Fake, Phone, Ring};
pub use wake::Registered;
pub use window::{Distributor, Outcome, Step, Window};

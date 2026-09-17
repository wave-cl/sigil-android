//! The Android side: what only compiles for the phone.
//!
//! Three pieces. [`bridge`] holds the JavaVM and the application context
//! and is how Rust calls into the Kotlin glue (`Notifier`, `Vault`, `Files`,
//! `Endpoint`). [`platform`] puts sigil's seams -- `Notify`, the file
//! `Chooser`, the identity `Vault`, `sigil_phone::Phone` -- over that
//! bridge. [`entry`] is `android_main`, the window; [`jni`] is what Kotlin
//! calls: the wake window, an endpoint, a picked file, a pressed
//! notification.

pub mod bridge;
pub mod entry;
pub mod jni;
pub mod platform;

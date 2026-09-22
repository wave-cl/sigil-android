//! sigil on a phone.
//!
//! The window is sigil's own: `sigil_shell::Shell` drawing `sigil_chat`,
//! `sigil_admin` and the one app this crate adds, through eframe's Android
//! backend. What a phone adds is around it -- an identity kept by the key
//! store ([`identity`]), a setting for how much a notification says
//! ([`settings`]), the *Phone* tab ([`phone_app`]), and, on Android only,
//! the activity's entry point, the JNI the wake service and the
//! notifications go through, and the platform behind `sigil_phone`'s seams
//! ([`android`]).
//!
//! Everything outside [`android`] compiles and tests on a desktop, which is
//! where it is tested.

pub mod host;
pub mod identity;
pub mod phone_app;
pub mod settings;

#[cfg(target_os = "android")]
pub mod android;

pub use host::{Host, PhoneReport, Shared};

/// SIP-45: how long the exchange may keep the wake endpoint, from the
/// person's setting on the Phone tab, in seconds. Read off the file each
/// time: it is asked for as an endpoint arrives, on a Java thread that
/// holds no app.
pub fn wake_ttl_secs() -> u32 {
    let at = settings::Settings::path_under(&host::data_dir());
    settings::Settings::load(&at).endpoint_days.clamp(1, 30) * 86_400
}

//! Building the window: the shell, its apps, and the notifier the phone
//! supplies. Shared between the Android entry and a desktop run of the
//! phone build, and testable on the latter.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sigil::app::{App, Notify};
use sigil_platform::{Capability, Support};
use sigil_shell::Shell;

use crate::identity::{self, Vault, Where};
use crate::phone_app::PhoneApp;
use crate::settings::Settings;

/// What the platform side reports about itself, and what the window shows
/// in the Phone tab. Written from wherever the facts arrive -- the
/// distributor's receiver, the wake service, the activity -- and read each
/// pass. One lock, small, never held across anything slow.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhoneReport {
    /// The UnifiedPush endpoint the distributor gave, if any.
    pub endpoint: Option<String>,
    /// Which distributor is delivering: its package, "embedded FCM", or
    /// none.
    pub distributor: Option<String>,
    /// The last wake window's one-line report.
    pub last_window: Option<String>,
    /// Whether the platform lets sigil post notifications right now.
    pub notifications: Option<bool>,
}

pub type Shared = Arc<Mutex<PhoneReport>>;

/// Where sigil keeps its own state: `$XDG_DATA_HOME/sigil`, which the
/// Android entry points inside the app's files directory.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sigil")
}

/// The window, built.
pub struct Host {
    pub shell: Shell,
    pub report: Shared,
}

impl Host {
    /// Build the shell for the phone: the identity opened through the
    /// vault, the settings read, the apps made, the notifier installed.
    ///
    /// `home` is what `HOME` is set to -- the app's files directory on
    /// Android -- and the identity lives under it where sqnr expects one.
    pub fn build(
        home: &Path,
        vault: &dyn Vault,
        notify: Box<dyn Notify>,
        capabilities: Vec<Capability>,
        report: Shared,
    ) -> Result<Host, String> {
        let account = identity::ensure(&Where::under(home), vault)?;
        let settings_at = Settings::path_under(&data_dir());
        let settings = Settings::load(&settings_at);
        let mut capabilities = capabilities;
        capabilities.push(Capability::new(
            "Key store",
            "keeps the passphrase that seals this phone's key",
            if vault.describe().starts_with("no key store") {
                Support::no(vault.describe())
            } else {
                Support::Yes
            },
        ));
        let apps: Vec<Box<dyn App>> = vec![
            Box::new(sigil_chat::ChatApp::new()),
            Box::new(sigil_admin::AdminApp::new()),
            Box::new(PhoneApp::new(
                settings,
                settings_at,
                capabilities,
                report.clone(),
            )),
        ];
        let shell = Shell::new(apps, None)
            .with_notify(notify)
            .with_account(account);
        Ok(Host { shell, report })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Unsealed;

    /// The window builds on a desktop with nothing of a phone in it: an
    /// identity is made and opened, and the shell holds it unlocked.
    #[test]
    fn the_phone_window_builds_headless_with_an_identity_it_made() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::build(
            dir.path(),
            &Unsealed,
            Box::new(sigil::Silent),
            Vec::new(),
            Shared::default(),
        )
        .unwrap();
        assert!(host.shell.accounts().active().is_unlocked());
        assert!(Where::under(dir.path()).identity.exists());
    }
}

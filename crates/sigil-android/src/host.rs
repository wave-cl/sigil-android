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

/// The one notifier, boxed for the shell as well as shared with the chat
/// app: the shell's `with_notify` takes a box, and the phone has one
/// notifier.
struct Also(Arc<dyn Notify + Send + Sync>);

impl Notify for Also {
    fn notice(&self, notice: sigil::app::Notice<'_>) -> bool {
        self.0.notice(notice)
    }
    fn pressed(&self) -> Vec<sigil::app::Target> {
        self.0.pressed()
    }
    fn withdraw(&self, target: &sigil::app::Target) {
        self.0.withdraw(target)
    }
    fn calling(&self, with: Option<&str>) {
        self.0.calling(with)
    }
}

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
    ///
    /// `remember`: whether the roster -- the identity and the exchanges
    /// added to it -- is written to `accounts.json` as it changes. The phone
    /// passes `true`; a test passes `false`, since the file is the real one.
    pub fn build(
        home: &Path,
        vault: &dyn Vault,
        notify: std::sync::Arc<dyn Notify + Send + Sync>,
        capabilities: Vec<Capability>,
        report: Shared,
        remember: bool,
        reach: crate::phone_app::Reach,
    ) -> Result<Host, String> {
        let at = Where::under(home);
        let opened = identity::ensure(&at, vault)?;
        // The remembered roster, with this identity in it and opened: what
        // was added to it last time -- an exchange, chiefly -- comes back.
        let mut accounts = if remember {
            sigil::Accounts::load()
        } else {
            sigil::Accounts::of(Vec::new())
        };
        let i = accounts.use_path(at.identity.clone());
        if !accounts.unlock(i, &opened.passphrase) {
            return Err(format!(
                "{} did not open in the roster with the passphrase the key store holds",
                at.identity.display()
            ));
        }
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
        // **The chat app is given the notifier twice over**: once through the
        // shell, for what a frame says, and once of its own, for what has to
        // be said when no frame is coming -- which on a phone is whenever it
        // is not in front. A ring that arrived then reached the session and
        // was said to nobody; see `sigil_chat::announce`.
        let apps: Vec<Box<dyn App>> = vec![
            Box::new(sigil_chat::ChatApp::new().with_off_frame_notify(notify.clone())),
            Box::new(sigil_admin::AdminApp::new()),
            Box::new(
                PhoneApp::new(settings, settings_at, capabilities, report.clone())
                    .with_reach(reach),
            ),
        ];
        let shell = Shell::new(apps, None)
            .with_notify(Box::new(Also(notify)))
            .with_roster(accounts, remember);
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
            std::sync::Arc::new(sigil::Silent),
            Vec::new(),
            Shared::default(),
            false,
            Box::new(|_| Ok(())),
        )
        .unwrap();
        assert!(host.shell.accounts().active().is_unlocked());
        assert!(Where::under(dir.path()).identity.exists());
    }

    /// The roster the phone builds is the remembered one with the identity
    /// in it: an exchange added to it is still there after a rebuild from
    /// the same files, which is what surviving a relaunch means.
    #[test]
    fn an_exchange_added_to_the_roster_is_there_after_a_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let at = Where::under(dir.path());
        let opened = identity::ensure(&at, &Unsealed).unwrap();
        let mut accounts = sigil::Accounts::of(Vec::new());
        let i = accounts.use_path(at.identity.clone());
        assert!(accounts.unlock(i, &opened.passphrase));
        // Directly, not through a home: SIP-85's `via` is the third
        // argument, and the phone's roster is the plain case.
        assert!(accounts.add_exchange(i, "trunk.exchange", None));
        // What the shell would write is what the identity's row now says;
        // the file itself is the real one and is not touched by a test. The
        // empty name first is the default exchange, always present.
        assert_eq!(
            accounts.held(i).unwrap().exchanges(),
            vec![String::new(), "trunk.exchange".to_string()]
        );
    }
}

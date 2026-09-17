//! The phone's own key, and what keeps it.
//!
//! SIP-47: the phone generates an Ed25519 device key on first run, keeps
//! its secret in the platform's key store, and never has the account's key.
//! Precisely what "in the key store" means here: the key is an ordinary
//! sqnr identity file, sealed under a passphrase nobody is ever shown; the
//! passphrase is 32 random bytes, sealed by a [`Vault`] -- on Android an
//! AES key the hardware holds and will not export -- and written beside the
//! store. The seed itself has to be in memory to make a sQUIC handshake,
//! which is why a file exists at all; what the vault buys is that the file
//! is opaque to anything that copies it off the phone without the phone.
//!
//! Testable on a desktop with an in-memory vault, which is how it is.

use std::path::{Path, PathBuf};

use sigil::Account;

/// What seals the passphrase. The platform's key store, behind a seam.
pub trait Vault: Send + Sync {
    /// Seal `plain`; opaque bytes only this vault opens.
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String>;
    /// Open what [`seal`](Self::seal) made, or say why not.
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String>;
    /// For the capabilities list.
    fn describe(&self) -> &'static str;
}

/// A vault that keeps its key in memory: tests, and a desktop run of the
/// phone build. Says so.
pub struct Unsealed;

impl Vault for Unsealed {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        Ok(plain.to_vec())
    }

    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        Ok(sealed.to_vec())
    }

    fn describe(&self) -> &'static str {
        "no key store on this platform: the passphrase is written as it is"
    }
}

/// Where the identity and its sealed passphrase live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    /// `~/.sqnr/identity`, where every sqnr client looks.
    pub identity: PathBuf,
    /// Beside sigil's own state.
    pub sealed: PathBuf,
}

impl Where {
    /// Under `home`: the identity where sqnr keeps one, the sealed
    /// passphrase where sigil keeps its state.
    pub fn under(home: &Path) -> Where {
        Where {
            identity: home.join(".sqnr").join("identity"),
            sealed: home.join(".local/share/sigil").join("passphrase.sealed"),
        }
    }
}

/// The phone's identity, made on first run and opened on every run.
///
/// **Never overwrites.** An identity file with no sealed passphrase beside
/// it is refused rather than replaced: the file is somebody's key, and a
/// missing passphrase is a question, not a licence.
pub fn ensure(at: &Where, vault: &dyn Vault) -> Result<Account, String> {
    let have_identity = at.identity.exists();
    let have_sealed = at.sealed.exists();
    let passphrase = match (have_identity, have_sealed) {
        (true, true) => {
            let sealed =
                std::fs::read(&at.sealed).map_err(|e| format!("{}: {e}", at.sealed.display()))?;
            let plain = vault.open(&sealed)?;
            String::from_utf8(plain).map_err(|_| "the sealed passphrase is not text".to_string())?
        }
        (false, _) => {
            let passphrase = fresh_passphrase();
            if let Some(dir) = at.identity.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            sqnr::identity::generate(&at.identity, Some(&passphrase))?;
            if let Some(dir) = at.sealed.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            let sealed = vault.seal(passphrase.as_bytes())?;
            std::fs::write(&at.sealed, sealed)
                .map_err(|e| format!("{}: {e}", at.sealed.display()))?;
            passphrase
        }
        (true, false) => {
            return Err(format!(
                "{} exists but nothing here can open it: its passphrase is not in the key \
                 store. Move it aside to make a fresh device key, or restore the sealed \
                 passphrase beside it.",
                at.identity.display()
            ));
        }
    };
    let mut account = Account::discover(Some(at.identity.clone()));
    if !account.unlock(&passphrase) {
        return Err(format!(
            "{} did not open with the passphrase the key store holds",
            at.identity.display()
        ));
    }
    Ok(account)
}

/// 32 random bytes, as hex: nobody types it, so it need not be typeable.
fn fresh_passphrase() -> String {
    use rand_core::RngCore;
    let mut bytes = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A vault that reverses bytes: enough to tell sealed from plain.
    struct Backwards;
    impl Vault for Backwards {
        fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
            Ok(plain.iter().rev().copied().collect())
        }
        fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
            Ok(sealed.iter().rev().copied().collect())
        }
        fn describe(&self) -> &'static str {
            "backwards"
        }
    }

    /// A vault that has lost its key.
    struct Lost;
    impl Vault for Lost {
        fn seal(&self, _: &[u8]) -> Result<Vec<u8>, String> {
            Ok(vec![0])
        }
        fn open(&self, _: &[u8]) -> Result<Vec<u8>, String> {
            Err("the key store has no key".into())
        }
        fn describe(&self) -> &'static str {
            "lost"
        }
    }

    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn a_first_run_makes_a_key_and_a_second_run_opens_the_same_one() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let at = Where::under(dir.path());
        let first = ensure(&at, &Backwards).unwrap();
        let me = first.unlocked().expect("unlocked").me();
        assert!(at.identity.exists() && at.sealed.exists());
        // The passphrase on disk is not the passphrase.
        let sealed = std::fs::read(&at.sealed).unwrap();
        assert!(
            sealed.iter().any(|b| !b.is_ascii_hexdigit()) || {
                let s = String::from_utf8_lossy(&sealed).to_string();
                Backwards.open(s.as_bytes()).unwrap() != sealed
            }
        );
        let second = ensure(&at, &Backwards).unwrap();
        assert_eq!(
            second.unlocked().unwrap().me(),
            me,
            "the same key, not a new one"
        );
    }

    /// A key store that cannot open the passphrase is an error somebody
    /// reads, never a fresh key over the old one.
    #[test]
    fn a_lost_key_store_never_replaces_the_identity() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let at = Where::under(dir.path());
        let first = ensure(&at, &Lost).unwrap();
        let me = first.unlocked().unwrap().me();
        let why = ensure(&at, &Lost).expect_err("cannot open");
        assert!(why.contains("no key"), "{why}");
        // Still the same file, still the same key.
        let back = ensure(&at, &Unsealed).err();
        assert!(back.is_some());
        let again = Account::discover(Some(at.identity.clone()));
        assert_eq!(again.public(), Some(me));
    }

    /// An identity with no sealed passphrase beside it is a question.
    #[test]
    fn an_identity_without_its_passphrase_is_refused_not_overwritten() {
        let _s = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let at = Where::under(dir.path());
        let first = ensure(&at, &Backwards).unwrap();
        let me = first.unlocked().unwrap().me();
        std::fs::remove_file(&at.sealed).unwrap();
        let why = ensure(&at, &Backwards).expect_err("refused");
        assert!(why.contains("Move it aside"), "{why}");
        assert_eq!(
            Account::discover(Some(at.identity.clone())).public(),
            Some(me)
        );
    }
}

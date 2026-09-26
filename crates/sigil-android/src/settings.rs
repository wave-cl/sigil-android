//! What the person chose, kept as a small JSON file beside sigil's other
//! state: how long a wake endpoint lives, and whether to stay reachable
//! without a distributor. Both are the phone's own.
//!
//! **What a notification says is no longer one of them.** SIP-47 requires
//! the phone to offer it and it was kept here, where the wake window could
//! read it with no app around -- and where the running client, which
//! composes notifications of its own, could not. It is a sigil preference
//! now, read from the same directory by both; the field below carries an
//! upgrading phone's choice across and is then cleared.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sigil_phone::Privacy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// **Where this setting used to live.** It is sigil's preference now
    /// (`sigil::prefs::Privacy`), because the wake window was not the only
    /// thing composing notifications and the other path could not see a
    /// value kept here. Read once on the next start and cleared; `None`
    /// afterwards, and on a phone that never had it.
    ///
    /// Carried across rather than dropped, because the direction the
    /// default falls is *more* said on a locked screen: a person who chose
    /// less and was quietly given everything back would not be told.
    #[serde(default)]
    pub privacy: Option<Privacy>,
    /// How long the exchange keeps the wake endpoint, in days. SIP-45 caps
    /// it at 30; a phone that connects daily could ask for less, and one
    /// left in a drawer wants all of it.
    pub endpoint_days: u32,
    /// Keep the process alive with its connection open when sigil is not
    /// in front, so calls and messages arrive without a push distributor.
    /// A foreground service with a quiet notice, and the battery pays; off
    /// unless chosen. Absent from a file written before it existed.
    #[serde(default)]
    pub stay_reachable: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            privacy: None,
            endpoint_days: 30,
            stay_reachable: false,
        }
    }
}

impl Settings {
    pub fn path_under(data: &Path) -> PathBuf {
        data.join("phone.json")
    }

    /// What is on disk, or the default when nothing is or it will not
    /// parse -- a setting that cannot be read is the default, said in the
    /// log, not a phone that will not start.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!("{}: unreadable ({e}); using the defaults", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_absent_or_broken() {
        let dir = tempfile::tempdir().unwrap();
        let path = Settings::path_under(dir.path());
        assert_eq!(Settings::load(&path), Settings::default());
        let mine = Settings {
            privacy: None,
            endpoint_days: 7,
            stay_reachable: true,
        };
        mine.save(&path).unwrap();
        assert_eq!(Settings::load(&path), mine);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        // A file from before `stay_reachable` existed reads as not chosen.
        std::fs::write(&path, br#"{"privacy":"sender_only","endpoint_days":7}"#).unwrap();
        assert!(!Settings::load(&path).stay_reachable);
    }

    /// **A choice this phone already made is carried across, not dropped.**
    ///
    /// What a notification may say lived here, where only the wake window
    /// could read it; it is a sigil preference now, which both the wake
    /// window and the running client read. A file written by the build
    /// before this one spells it exactly as it always did -- and the
    /// default it would otherwise fall back to says *more* on a locked
    /// screen, not less, so reading it is not optional.
    #[test]
    fn an_older_file_still_carries_what_a_notification_may_say() {
        let dir = tempfile::tempdir().unwrap();
        let path = Settings::path_under(dir.path());
        std::fs::write(&path, br#"{"privacy":"fact_only","endpoint_days":7}"#).unwrap();
        assert_eq!(Settings::load(&path).privacy, Some(Privacy::FactOnly));

        // And once carried, gone from here: two stores for one setting is
        // two answers to what a locked screen may show.
        let mut carried = Settings::load(&path);
        assert_eq!(carried.privacy.take(), Some(Privacy::FactOnly));
        carried.save(&path).unwrap();
        assert_eq!(Settings::load(&path).privacy, None);
    }
}

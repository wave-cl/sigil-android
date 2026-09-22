//! What the person chose, kept as a small JSON file beside sigil's other
//! state. The one setting SIP-47 requires the phone to offer is how much a
//! notification says; the rest are the phone's own.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sigil_phone::Privacy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// How much a notification says. SIP-47 leaves the default to the
    /// phone; this one says who and what, since a person who wants less
    /// has a setting and a person who wants more has none.
    pub privacy: PrivacySetting,
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

/// [`Privacy`], with a serde derive it does not carry itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrivacySetting {
    #[default]
    SenderAndText,
    SenderOnly,
    FactOnly,
}

impl From<PrivacySetting> for Privacy {
    fn from(p: PrivacySetting) -> Privacy {
        match p {
            PrivacySetting::SenderAndText => Privacy::SenderAndText,
            PrivacySetting::SenderOnly => Privacy::SenderOnly,
            PrivacySetting::FactOnly => Privacy::FactOnly,
        }
    }
}

impl From<Privacy> for PrivacySetting {
    fn from(p: Privacy) -> PrivacySetting {
        match p {
            Privacy::SenderAndText => PrivacySetting::SenderAndText,
            Privacy::SenderOnly => PrivacySetting::SenderOnly,
            Privacy::FactOnly => PrivacySetting::FactOnly,
        }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            privacy: PrivacySetting::default(),
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
            privacy: PrivacySetting::FactOnly,
            endpoint_days: 7,
            stay_reachable: true,
        };
        mine.save(&path).unwrap();
        assert_eq!(Settings::load(&path), mine);
        assert_eq!(Privacy::from(mine.privacy), Privacy::FactOnly);
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        // A file from before `stay_reachable` existed reads as not chosen.
        std::fs::write(&path, br#"{"privacy":"sender_only","endpoint_days":7}"#).unwrap();
        let older = Settings::load(&path);
        assert_eq!(older.privacy, PrivacySetting::SenderOnly);
        assert!(!older.stay_reachable);
    }
}

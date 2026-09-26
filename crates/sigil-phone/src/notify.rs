//! What the phone says on its lock screen, composed from plaintext it
//! alone opened (SIP-47 §Notifications).
//!
//! Three rules from the SIP, all here so they can be tested without a
//! phone: the person chooses how much a notification says, and the phone
//! MUST offer sender-and-text, sender-only, and the fact of a message; a
//! busy channel is one thing that happened, so at most one notification per
//! channel per window, naming the newest, with a count; and a wake that
//! found nothing new says nothing.
//!
//! Nothing composed here leaves the phone. The platform's notification
//! surface is the only consumer, and what it stores and syncs is the
//! platform's business -- which is why "the fact of a message" exists as a
//! setting, and why under it the channel is not named either.

use std::collections::BTreeMap;

use sigil_chat::session::Arrival;

/// How much a notification says (SIP-47): the phone MUST offer all three,
/// and which is the default is the client's to choose.
///
/// **Defined in `sigil::prefs` rather than here.** It was here, because this
/// crate composes the wake window's notifications and nothing else needed
/// it -- which was the mistake. The running client composes notifications
/// too, in `sigil_chat::announce`, and knew nothing about the setting: a
/// phone told to say only that something had arrived said who and what for
/// as long as its process was alive, which with the reachable service on is
/// most of the time. One type, in the crate both paths already depend on,
/// is what keeps the two answers the same.
pub use sigil::prefs::Privacy;

/// One notification: what it says, and where a press on it leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// The conversation it is about, or `None` for one that names none.
    pub channel: Option<[u8; 32]>,
    pub title: String,
    pub body: String,
    /// How many arrivals it stands for.
    pub count: usize,
    /// The newest arrival's sequence number, for the platform to key on so a
    /// later window replaces rather than stacks.
    pub newest: u64,
}

/// One notification per channel, naming the newest, with a count -- or one
/// notification for everything, under [`Privacy::FactOnly`]. Nothing for
/// nothing.
pub fn compose(arrivals: &[Arrival], privacy: Privacy) -> Vec<Notification> {
    if arrivals.is_empty() {
        return Vec::new();
    }
    if privacy == Privacy::FactOnly {
        let newest = arrivals.iter().map(|a| a.seq).max().unwrap_or(0);
        return vec![Notification {
            channel: None,
            title: "Sigil".to_string(),
            body: match arrivals.len() {
                1 => "A new message".to_string(),
                n => format!("{n} new messages"),
            },
            count: arrivals.len(),
            newest,
        }];
    }
    let mut by_channel: BTreeMap<[u8; 32], Vec<&Arrival>> = BTreeMap::new();
    for a in arrivals {
        by_channel.entry(a.channel).or_default().push(a);
    }
    by_channel
        .into_iter()
        .map(|(channel, mut together)| {
            together.sort_by_key(|a| a.seq);
            let newest = together.last().expect("a channel with an arrival");
            let more = together.len() - 1;
            let title = newest.conversation.clone();
            let mut body = match privacy {
                Privacy::SenderAndText if newest.direct => newest.said.clone(),
                Privacy::SenderAndText => format!("{}: {}", newest.from_label, newest.said),
                Privacy::SenderOnly | Privacy::FactOnly => {
                    format!("{} sent a message", newest.from_label)
                }
            };
            if more > 0 {
                body.push_str(&format!(" · and {more} more"));
            }
            Notification {
                channel: Some(channel),
                title,
                body,
                count: together.len(),
                newest: newest.seq,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqnr_core::PubKey;

    fn arrival(channel: u8, seq: u64, from: &str, said: &str, direct: bool) -> Arrival {
        Arrival {
            channel: [channel; 32],
            seq,
            from: PubKey::new([from.len() as u8; 32]),
            from_label: from.to_string(),
            conversation: if direct {
                from.to_string()
            } else {
                "the group".to_string()
            },
            public: false,
            direct,
            said: said.to_string(),
            in_open: false,
            mentions_me: false,
        }
    }

    /// A wake that found nothing composes nothing (SIP-47).
    #[test]
    fn nothing_new_says_nothing() {
        for privacy in Privacy::ALL {
            assert!(compose(&[], privacy).is_empty(), "{privacy:?}");
        }
    }

    /// One per channel, the newest named, the rest counted.
    #[test]
    fn a_busy_channel_is_one_notification_naming_the_newest() {
        let arrivals = vec![
            arrival(1, 5, "ann", "first", false),
            arrival(1, 7, "bob", "third", false),
            arrival(1, 6, "ann", "second", false),
            arrival(2, 9, "cy", "hello", true),
        ];
        let out = compose(&arrivals, Privacy::SenderAndText);
        assert_eq!(out.len(), 2);
        let group = out.iter().find(|n| n.channel == Some([1; 32])).unwrap();
        assert_eq!(group.title, "the group");
        assert_eq!(group.body, "bob: third · and 2 more");
        assert_eq!(group.count, 3);
        assert_eq!(group.newest, 7);
        let dm = out.iter().find(|n| n.channel == Some([2; 32])).unwrap();
        assert_eq!(dm.title, "cy");
        assert_eq!(dm.body, "hello");
        assert_eq!(dm.count, 1);
    }

    /// Sender-only carries no words; fact-only carries no sender and no
    /// conversation, and is one notification for the lot.
    #[test]
    fn the_quieter_settings_say_less() {
        let arrivals = vec![
            arrival(1, 5, "ann", "secret", false),
            arrival(2, 9, "cy", "secret too", true),
        ];
        let sender = compose(&arrivals, Privacy::SenderOnly);
        assert_eq!(sender.len(), 2);
        for n in &sender {
            assert!(!n.body.contains("secret"), "{}", n.body);
            assert!(n.body.ends_with("sent a message"), "{}", n.body);
        }
        let fact = compose(&arrivals, Privacy::FactOnly);
        assert_eq!(fact.len(), 1);
        assert_eq!(fact[0].channel, None);
        assert_eq!(fact[0].body, "2 new messages");
        assert!(!fact[0].body.contains("ann") && !fact[0].body.contains("cy"));
        assert_eq!(fact[0].newest, 9);
        assert_eq!(
            compose(&arrivals[..1], Privacy::FactOnly)[0].body,
            "A new message"
        );
    }
}

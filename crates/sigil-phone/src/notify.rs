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
pub fn compose(
    arrivals: &[Arrival],
    privacy: Privacy,
    silenced: &dyn Fn(&[u8; 32]) -> bool,
) -> Vec<Notification> {
    // **Muted here too, not only while the phone is awake.** A running
    // client checks `Quiet::silenced` before it says anything
    // (`sigil_chat::announce`); this path did not, so a conversation
    // somebody muted was silent while they were looking at it and woke the
    // phone while they were not -- which is the half that matters, and the
    // opposite of what they asked for. Do-not-disturb is in the same rule.
    //
    // Applied here rather than by the caller because a caller can forget,
    // and because it has to happen before the counting: mute everything and
    // even the quietest setting should say nothing at all.
    let arrivals: Vec<Arrival> = arrivals
        .iter()
        .filter(|a| !silenced(&a.channel))
        .cloned()
        .collect();
    let arrivals = &arrivals[..];
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
            // **A mention says that it is one.** The running client gives a
            // mention its own words (`sigil_chat::mention_said`, "X
            // mentioned you in #room") and treats it as its own kind of
            // event; this said "X sent a message" like anything else. On a
            // phone the wake window is the path that usually speaks, so the
            // one notification somebody most needs to tell apart from the
            // rest was the one that looked like all of them.
            //
            // The newest arrival that mentions them, not the newest arrival
            // — "ann mentioned you" is wrong if ann simply spoke last after
            // somebody else did the mentioning. The count below still counts
            // everything.
            let mention = together.iter().rev().find(|a| a.mentions_me).copied();
            let mut body = match (mention, privacy) {
                (Some(m), Privacy::SenderAndText) => {
                    format!("{} mentioned you: {}", m.from_label, m.said)
                }
                // Who, and that it was a mention, is still "who wrote" --
                // and nothing of what was said.
                (Some(m), Privacy::SenderOnly | Privacy::FactOnly) => {
                    format!("{} mentioned you", m.from_label)
                }
                (None, Privacy::SenderAndText) if newest.direct => newest.said.clone(),
                (None, Privacy::SenderAndText) => {
                    format!("{}: {}", newest.from_label, newest.said)
                }
                (None, Privacy::SenderOnly | Privacy::FactOnly) => {
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

    fn mention(channel: u8, seq: u64, from: &str, said: &str) -> Arrival {
        Arrival {
            mentions_me: true,
            ..arrival(channel, seq, from, said, false)
        }
    }

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

    /// Nothing muted, and not on do-not-disturb: what the tests that came
    /// before the mute was honoured all meant.
    fn loud(_channel: &[u8; 32]) -> bool {
        false
    }

    /// A wake that found nothing composes nothing (SIP-47).
    #[test]
    fn nothing_new_says_nothing() {
        for privacy in Privacy::ALL {
            assert!(compose(&[], privacy, &loud).is_empty(), "{privacy:?}");
        }
    }

    /// **A muted conversation does not wake the phone either.**
    ///
    /// The running client checks `Quiet::silenced` before it says anything;
    /// this path did not, so a conversation somebody muted was silent while
    /// they were looking at it and woke them while they were not. With a
    /// control, because a filter that dropped *everything* would pass every
    /// assertion below.
    #[test]
    fn a_muted_conversation_says_nothing() {
        let arrivals = vec![
            arrival(1, 5, "ann", "in the muted room", false),
            arrival(2, 9, "cy", "in the other one", true),
        ];
        let muted_one = |c: &[u8; 32]| *c == [1u8; 32];

        let out = compose(&arrivals, Privacy::SenderAndText, &muted_one);
        assert_eq!(out.len(), 1, "only the unmuted one: {out:?}");
        assert_eq!(out[0].channel, Some([2; 32]));
        assert!(!out[0].body.contains("muted room"));

        // The control: with nothing muted the same arrivals make two.
        assert_eq!(compose(&arrivals, Privacy::SenderAndText, &loud).len(), 2);
    }

    /// **Silenced before counted.** Under the quietest setting a muted
    /// conversation must not even raise the count -- "3 new messages" for
    /// three messages in the one room somebody muted is the mute failing
    /// quietly rather than loudly.
    #[test]
    fn a_muted_conversation_is_not_counted_either() {
        let arrivals = vec![
            arrival(1, 5, "ann", "one", false),
            arrival(1, 6, "ann", "two", false),
            arrival(2, 9, "cy", "three", true),
        ];
        let muted_one = |c: &[u8; 32]| *c == [1u8; 32];
        let fact = compose(&arrivals, Privacy::FactOnly, &muted_one);
        assert_eq!(fact.len(), 1);
        assert_eq!(fact[0].body, "A new message", "the muted two are not in it");
        assert_eq!(fact[0].count, 1);

        // Everything muted: a wake that found nothing it may speak of says
        // nothing at all, as one that found nothing does.
        for privacy in Privacy::ALL {
            assert!(
                compose(&arrivals, privacy, &|_| true).is_empty(),
                "{privacy:?}"
            );
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
        let out = compose(&arrivals, Privacy::SenderAndText, &loud);
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

    /// **A mention is told apart from any other message.**
    ///
    /// Awake, a mention has its own words and its own kind of event. Asleep
    /// it read as "somebody sent a message", which on a phone is the path
    /// that usually speaks -- so the notification somebody most needs to
    /// pick out of a lock screen was the one that looked like the rest.
    #[test]
    fn a_mention_says_that_it_is_one() {
        let arrivals = vec![
            arrival(1, 5, "ann", "something", false),
            mention(1, 6, "bram", "has anyone seen @me"),
            arrival(1, 7, "ann", "and then this", false),
        ];
        let out = compose(&arrivals, Privacy::SenderAndText, &loud);
        assert_eq!(out.len(), 1);
        // **Bram, not ann.** Ann spoke last; bram did the mentioning, and
        // naming the newest arrival would have credited the wrong person.
        assert!(
            out[0]
                .body
                .starts_with("bram mentioned you: has anyone seen @me"),
            "{}",
            out[0].body
        );
        assert!(out[0].body.ends_with("· and 2 more"), "{}", out[0].body);
        assert_eq!(out[0].count, 3);

        // Under sender-only, that it was a mention is still "who wrote".
        let quiet = compose(&arrivals, Privacy::SenderOnly, &loud);
        assert_eq!(quiet[0].body, "bram mentioned you · and 2 more");
        assert!(!quiet[0].body.contains("anyone seen"));

        // The control: the same conversation without the mention.
        let plain = vec![
            arrival(1, 5, "ann", "something", false),
            arrival(1, 7, "ann", "and then this", false),
        ];
        let out = compose(&plain, Privacy::SenderAndText, &loud);
        assert!(!out[0].body.contains("mentioned"), "{}", out[0].body);
    }

    /// And the quietest setting still names nobody: a mention is a message,
    /// and "somebody mentioned you" is a great deal more than "something
    /// arrived" to anyone reading over a shoulder.
    #[test]
    fn the_fact_of_a_message_does_not_leak_a_mention() {
        let arrivals = vec![mention(1, 6, "bram", "@me are you there")];
        let fact = compose(&arrivals, Privacy::FactOnly, &loud);
        assert_eq!(fact.len(), 1);
        assert_eq!(fact[0].channel, None);
        assert_eq!(fact[0].body, "A new message");
        for named in ["bram", "mentioned", "are you there"] {
            assert!(!fact[0].body.contains(named), "{named}: {}", fact[0].body);
        }
    }

    /// Sender-only carries no words; fact-only carries no sender and no
    /// conversation, and is one notification for the lot.
    #[test]
    fn the_quieter_settings_say_less() {
        let arrivals = vec![
            arrival(1, 5, "ann", "secret", false),
            arrival(2, 9, "cy", "secret too", true),
        ];
        let sender = compose(&arrivals, Privacy::SenderOnly, &loud);
        assert_eq!(sender.len(), 2);
        for n in &sender {
            assert!(!n.body.contains("secret"), "{}", n.body);
            assert!(n.body.ends_with("sent a message"), "{}", n.body);
        }
        let fact = compose(&arrivals, Privacy::FactOnly, &loud);
        assert_eq!(fact.len(), 1);
        assert_eq!(fact[0].channel, None);
        assert_eq!(fact[0].body, "2 new messages");
        assert!(!fact[0].body.contains("ann") && !fact[0].body.contains("cy"));
        assert_eq!(fact[0].newest, 9);
        assert_eq!(
            compose(&arrivals[..1], Privacy::FactOnly, &loud)[0].body,
            "A new message"
        );
    }
}

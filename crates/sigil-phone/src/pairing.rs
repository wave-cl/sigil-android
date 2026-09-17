//! SIP-47's two pairing strings: what the phone shows, and what it is shown.
//!
//! `sqx-device:<key>` is the phone's device key, the one thing it knows
//! before it has been told anything. `sqx-pair:<account>@<domain>[,<domain>]`
//! is where to go once a client holding the account has registered it: the
//! account, and every exchange the account uses. A person MAY show a SIP-38
//! name (`colin@squic.org`) instead, and the phone resolves it to the same
//! account and starts from that one domain.
//!
//! Neither string is a secret -- a device key is public, and an account and
//! a domain are what anybody could look up -- and neither is ever sent to an
//! exchange. They are read by people and phones, so they are forgiving about
//! whitespace and case in the parts where case has no meaning, and strict
//! about the parts where it does: a key is base58, and base58 is
//! case-sensitive.

use sqnr_core::PubKey;

pub const DEVICE_PREFIX: &str = "sqx-device:";
pub const PAIR_PREFIX: &str = "sqx-pair:";
/// SIP-47's cap on either string.
pub const MAX_LEN: usize = 512;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PairingError {
    #[error("longer than {MAX_LEN} bytes, which no pairing string is")]
    TooLong,
    #[error("that is not a device string: it does not start with `{DEVICE_PREFIX}`")]
    NotADevice,
    #[error("that is not a key: {0}")]
    NotAKey(String),
    #[error("that is not a pairing string: neither `{PAIR_PREFIX}…` nor a name@domain")]
    NotAPair,
    #[error("a pairing string names at least one domain, after the `@`")]
    NoDomain,
    #[error("`{0}` is not a domain name")]
    NotADomain(String),
}

/// What the phone shows: its device key, as text and as a QR.
pub fn device_string(device: &PubKey) -> String {
    format!("{DEVICE_PREFIX}{device}")
}

/// What the registering client read off the phone.
pub fn parse_device(shown: &str) -> Result<PubKey, PairingError> {
    if shown.len() > MAX_LEN {
        return Err(PairingError::TooLong);
    }
    let shown = shown.trim();
    let key = shown
        .strip_prefix(DEVICE_PREFIX)
        .ok_or(PairingError::NotADevice)?
        .trim();
    key.parse::<PubKey>()
        .map_err(|e| PairingError::NotAKey(e.to_string()))
}

/// Whom the phone belongs to, and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    /// The account's key, and the domains it uses.
    Key(PubKey),
    /// A SIP-38 name, resolved at the one domain it names.
    Name(String),
}

/// A parsed `sqx-pair:` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub owner: Owner,
    /// Lower-cased, in the order given, with repeats removed. At least one.
    pub domains: Vec<String>,
}

/// What the registering client shows: the account and every exchange it is
/// at. `domains` are as the client knows them; they are lower-cased here,
/// since a domain has no case.
pub fn pair_string(account: &PubKey, domains: &[String]) -> String {
    let mut seen = Vec::new();
    for d in domains {
        let d = d.trim().to_ascii_lowercase();
        if !d.is_empty() && !seen.contains(&d) {
            seen.push(d);
        }
    }
    format!("{PAIR_PREFIX}{account}@{}", seen.join(","))
}

/// What the phone read off the registering client, or what a person typed.
///
/// Accepted: `sqx-pair:<key>@<domain>[,<domain>]*`, the same with a SIP-38
/// name in place of the key, and a bare `name@domain` with no prefix at all
/// -- which is what a person types when they know their handle and nothing
/// else.
pub fn parse_pair(shown: &str) -> Result<Pair, PairingError> {
    if shown.len() > MAX_LEN {
        return Err(PairingError::TooLong);
    }
    let shown = shown.trim();
    let body = shown.strip_prefix(PAIR_PREFIX).unwrap_or(shown).trim();
    let (who, where_) = body.rsplit_once('@').ok_or(PairingError::NotAPair)?;
    let who = who.trim();
    if who.is_empty() {
        return Err(PairingError::NotAPair);
    }
    let mut domains = Vec::new();
    for d in where_.split(',') {
        let d = d.trim().to_ascii_lowercase();
        if d.is_empty() {
            continue;
        }
        if !is_domain(&d) {
            return Err(PairingError::NotADomain(d));
        }
        if !domains.contains(&d) {
            domains.push(d);
        }
    }
    if domains.is_empty() {
        return Err(PairingError::NoDomain);
    }
    // A key parses; anything else is a name. Tried in that order, as
    // sqex_proto::name::classify does, so an explicit key is never
    // reinterpreted as a name that happens to look like one.
    let owner = match who.parse::<PubKey>() {
        Ok(key) => Owner::Key(key),
        Err(_) => {
            if domains.len() != 1 {
                // A name lives at one domain; the others follow from SIP-46
                // once the phone is there. Naming several beside a name is a
                // string nobody meant to write.
                return Err(PairingError::NotAPair);
            }
            let name = who.to_ascii_lowercase();
            if !is_label(&name) {
                return Err(PairingError::NotAPair);
            }
            Owner::Name(format!("{name}@{}", domains[0]))
        }
    };
    Ok(Pair { owner, domains })
}

/// SIP-38's grammar for one label: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`, at most 63.
fn is_label(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// A DNS name: labels joined by dots, at least two of them, at most 253 bytes.
fn is_domain(s: &str) -> bool {
    s.len() <= 253 && s.split('.').count() >= 2 && s.split('.').all(is_label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(b: u8) -> PubKey {
        PubKey::new([b; 32])
    }

    #[test]
    fn a_device_string_round_trips_and_forgives_whitespace() {
        let shown = device_string(&key(7));
        assert!(shown.starts_with("sqx-device:"));
        assert_eq!(parse_device(&shown).unwrap(), key(7));
        assert_eq!(parse_device(&format!("  {shown} \n")).unwrap(), key(7));
    }

    #[test]
    fn a_device_string_is_refused_without_its_prefix_or_with_a_bad_key() {
        assert_eq!(
            parse_device(&key(1).to_string()),
            Err(PairingError::NotADevice)
        );
        assert!(matches!(
            parse_device("sqx-device:not-a-key"),
            Err(PairingError::NotAKey(_))
        ));
        assert_eq!(
            parse_device(&format!("sqx-device:{}", "1".repeat(600))),
            Err(PairingError::TooLong)
        );
    }

    #[test]
    fn a_pair_string_round_trips_with_its_domains_lowered_and_deduplicated() {
        let shown = pair_string(
            &key(3),
            &[
                "Squic.org".into(),
                "trunk.exchange".into(),
                "squic.org".into(),
            ],
        );
        assert_eq!(
            shown,
            format!("sqx-pair:{}@squic.org,trunk.exchange", key(3))
        );
        let pair = parse_pair(&shown).unwrap();
        assert_eq!(pair.owner, Owner::Key(key(3)));
        assert_eq!(pair.domains, vec!["squic.org", "trunk.exchange"]);
    }

    /// A person who knows their handle types that and nothing else.
    #[test]
    fn a_bare_name_at_a_domain_is_a_pair() {
        let pair = parse_pair("Colin@Squic.org").unwrap();
        assert_eq!(pair.owner, Owner::Name("colin@squic.org".into()));
        assert_eq!(pair.domains, vec!["squic.org"]);
        let pair = parse_pair("sqx-pair:colin@squic.org").unwrap();
        assert_eq!(pair.owner, Owner::Name("colin@squic.org".into()));
    }

    #[test]
    fn a_pair_string_needs_a_domain_and_a_real_one() {
        assert_eq!(
            parse_pair(&format!("sqx-pair:{}@", key(1))),
            Err(PairingError::NoDomain)
        );
        assert_eq!(
            parse_pair(&format!("sqx-pair:{}@localhost", key(1))),
            Err(PairingError::NotADomain("localhost".into()))
        );
        assert_eq!(parse_pair("sqx-pair:nothing"), Err(PairingError::NotAPair));
        assert_eq!(parse_pair("@squic.org"), Err(PairingError::NotAPair));
        // A name lives at one domain.
        assert_eq!(
            parse_pair("colin@squic.org,trunk.exchange"),
            Err(PairingError::NotAPair)
        );
    }

    /// A key is tried first, so a key is never read as a name -- and a
    /// wrong key is a name, which SIP-38 will say it cannot resolve, rather
    /// than a mangled key silently pointing at somebody else.
    #[test]
    fn a_key_is_a_key_before_it_is_a_name() {
        let k = key(9).to_string();
        let pair = parse_pair(&format!("{k}@squic.org")).unwrap();
        assert_eq!(pair.owner, Owner::Key(key(9)));
    }
}

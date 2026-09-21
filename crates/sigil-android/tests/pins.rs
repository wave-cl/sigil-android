//! One version of every shared crate in the build.
//!
//! # Why this is a test
//!
//! This workspace builds sigil's crates **by path** and pins sqex **by tag**,
//! and sigil pins sqex by tag too. When the two pins disagree, nothing fails:
//! cargo builds *both* versions and links them side by side. The types are
//! distinct, so it compiles only for as long as nothing passes one across to
//! the other -- and when something does, the error is about a type that looks
//! identical to the one it is being given.
//!
//! It happened twice on 2026-09-21, both times within an hour of sigil moving
//! its pin: sqex-proto, sqex-chat, sqex-discovery and sqexd were each in the
//! build at two versions at once. Nothing said so. `cargo metadata` did, and
//! nobody was asking it.
//!
//! A path dep's pins come from *its* workspace, so every consuming workspace
//! has to be repinned the same day. This is the thing that notices.

use std::collections::BTreeMap;
use std::process::Command;

/// Crates that come from a pinned tag and are shared with sigil's workspace.
///
/// Matched by prefix rather than listed exactly, so a new `sqex-*` crate is
/// covered the day it is added.
const SHARED: &[&str] = &["sqex", "sqnr", "squic"];

#[test]
fn no_shared_crate_is_in_the_build_at_two_versions() {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).expect("metadata is utf-8");

    // Deliberately crude, and deliberately not serde_json: this crate does
    // not otherwise need a JSON parser, and the shape being read is two
    // adjacent fields cargo has emitted the same way for years. If it ever
    // stops, the floor below says so rather than the test passing on an
    // empty answer.
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for chunk in text.split("\"name\":\"").skip(1) {
        let Some((name, rest)) = chunk.split_once('"') else {
            continue;
        };
        if !SHARED.iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let Some(after) = rest.split("\"version\":\"").nth(1) else {
            continue;
        };
        let Some((version, _)) = after.split_once('"') else {
            continue;
        };
        let at = seen.entry(name.to_string()).or_default();
        if !at.iter().any(|v| v == version) {
            at.push(version.to_string());
        }
    }

    // The floor. A parse that found nothing would otherwise report no
    // duplicates and read exactly like a clean build.
    assert!(
        seen.len() >= 5,
        "only {} shared crates were found in the metadata ({:?}): the parse \
         is not reading it, and a scan that finds nothing cannot find a \
         duplicate either",
        seen.len(),
        seen.keys().collect::<Vec<_>>()
    );

    let split: Vec<String> = seen
        .iter()
        .filter(|(_, vs)| vs.len() > 1)
        .map(|(n, vs)| format!("{n} at {}", vs.join(" and ")))
        .collect();
    assert!(
        split.is_empty(),
        "{} shared crate(s) are in this build at more than one version:\n  {}\n\
         sigil's workspace has moved its pin and this one has not. Repin \
         Cargo.toml to match ../sigil/Cargo.toml. Two versions of one crate \
         link fine until something passes a type from one to the other.",
        split.len(),
        split.join("\n  ")
    );
}

//! Every Kotlin class Rust calls is one `Native.init` looked up.
//!
//! `bridge::class` hands out classes from a list resolved at start, because
//! a lookup from the window's thread threw ClassNotFoundException on the
//! first launch. A class called and not listed fails at the call, with the
//! words "a class Rust calls is not in GLUE" -- in a `trouble` at the foot
//! of a pane, or in a log. `ReachService` shipped that way: the Phone tab
//! said the phone was reachable and no service had started. This reads the
//! calls and the list off the source, so the list cannot fall behind.

use std::path::Path;

#[test]
fn every_class_rust_calls_is_in_glue() {
    let android = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/android");
    let bridge = std::fs::read_to_string(android.join("bridge.rs")).expect("bridge.rs");
    let list = bridge
        .split("const GLUE: &[&str] = &[")
        .nth(1)
        .and_then(|rest| rest.split("];").next())
        .expect("GLUE is a literal list in bridge.rs");
    let glue: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
    assert!(glue.len() >= 5, "GLUE read as {glue:?}: the scan lost it");

    // Every `bridge::class("X")` and `call_static_with_context("X", ...)`
    // in the Android arm, whichever file it is in.
    let mut called: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&android).expect("src/android") {
        let path = entry.expect("an entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("a source file");
        for needle in ["bridge::class(\"", "call_static_with_context(\""] {
            for piece in source.split(needle).skip(1) {
                if let Some(name) = piece.split('"').next() {
                    called.push((name.to_string(), path.display().to_string()));
                }
            }
        }
    }
    assert!(
        called.len() >= 5,
        "only {} calls found: the scan is pointed at the wrong place",
        called.len()
    );
    let missing: Vec<String> = called
        .iter()
        .filter(|(name, _)| !glue.contains(&name.as_str()))
        .map(|(name, at)| format!("{name} (called from {at})"))
        .collect();
    assert!(
        missing.is_empty(),
        "class(es) Rust calls and Native.init does not look up -- add to GLUE in \
         bridge.rs:\n  {}",
        missing.join("\n  ")
    );
}

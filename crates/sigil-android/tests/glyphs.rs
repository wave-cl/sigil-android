//! Every character this crate *types* is one the font actually has.
//!
//! # Why a scan and not a list
//!
//! sigil has this check already, as a hand-kept list of the literals somebody
//! remembered to add. It has caught the trap twice -- `✓`, `✓✓` and `↳`
//! shipping as `□`, and `●`/`○` before them -- and it did not catch the third,
//! because the third was in *this* crate, which had no such check, and because
//! a list only covers what was added to it.
//!
//! The third was `("●", theme.accent)` in the Phone tab's capability rows.
//! `●` is not in egui's bundled font, so every capability this phone actually
//! has drew as a tofu box, and the one state somebody opens that pane to
//! confirm read as a rendering fault. Nothing failed: the accessibility tree
//! carries the *string*, which was correct, so every text assertion passed.
//! Only looking at the phone found it.
//!
//! So this reads the crate's own source, takes every non-ASCII character that
//! appears inside a string literal, and asks a real `egui::Context` whether
//! the fonts sigil runs with have it. A literal added tomorrow is covered
//! without anybody adding it anywhere.
//!
//! # What it does not cover
//!
//! It reads characters, so an escape -- `"\u{25CF}"` -- goes straight past
//! it. Checked, not assumed: the control was run both ways, and the escaped
//! form passed while the literal one failed. That is the right trade for now,
//! because nobody writes a dot as an escape; somebody pastes the character
//! in, which is exactly how this shipped three times. If an escape ever does
//! it, the answer is to decode them here rather than to go back to a list.

use std::path::{Path, PathBuf};

/// The source files this scans, so a failure can name one.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The non-ASCII characters inside string literals, with the line each is on.
///
/// Deliberately crude: it drops `//` comments, then takes what is between
/// unescaped double quotes. Comments are dropped because this file, and the
/// ones that record the trap, *discuss* the characters that are missing --
/// and a check that flags its own explanation is a check people switch off.
/// A raw string or a `"` inside a char literal could confuse it; if it ever
/// does, the failure names the file and the line, which is enough to see.
fn typed(source: &str) -> Vec<(usize, char)> {
    let mut found = Vec::new();
    for (n, line) in source.lines().enumerate() {
        let code = match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        };
        let mut in_string = false;
        let mut escaped = false;
        for c in code.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            match c {
                '\\' if in_string => escaped = true,
                '"' => in_string = !in_string,
                _ if in_string && !c.is_ascii() => found.push((n + 1, c)),
                _ => {}
            }
        }
    }
    found
}

#[test]
fn every_character_in_a_string_literal_is_in_the_font() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&root, &mut files);
    assert!(
        files.len() > 3,
        "only {} source files found under {}: the scan is pointed at the wrong \
         place and would pass whatever the code said",
        files.len(),
        root.display()
    );

    let mut checked = 0usize;
    let mut missing: Vec<String> = Vec::new();
    let ctx = egui::Context::default();
    // The pass's output has to be taken and cleared: dropping a
    // `TexturesDelta` with the font atlas in it panics on the way out, which
    // reads as a failure of whatever the test was doing.
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        let id = egui::FontId::proportional(14.0);
        for file in &files {
            let Ok(source) = std::fs::read_to_string(file) else {
                continue;
            };
            for (line, c) in typed(&source) {
                checked += 1;
                if !ui.ctx().fonts_mut(|f| f.has_glyph(&id, c)) {
                    missing.push(format!(
                        "{}:{line}: U+{:04X} {c:?} is drawn and the font does not \
                         have it, so it draws as a tofu box -- paint it instead \
                         (see sigil_ui::dot)",
                        file.display(),
                        c as u32
                    ));
                }
            }
        }
        // The control. This crate's own strings do contain non-ASCII --
        // the em dashes and ellipses in its sentences -- so a scan that
        // examined nothing would be a pass that meant nothing.
        assert!(
            checked > 0,
            "no non-ASCII character was found in any string literal, so this \
             test asked the font nothing"
        );
        // And that the instrument can say no: a character known not to be
        // in the font must be reported as missing.
        assert!(
            !ui.ctx().fonts_mut(|f| f.has_glyph(&id, '●')),
            "U+25CF '●' is in the font now. Good news, and this test's \
             negative control is gone: pick another character that is not."
        );
    });
    out.textures_delta.clear();

    assert!(
        missing.is_empty(),
        "{} character(s) are typed and cannot be drawn:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

//! The *Phone* tab, drawn on a phone.
//!
//! It is the pane most likely to be looked at on the device and the one
//! least likely to be looked at anywhere else: the desktop has no Phone tab,
//! so nothing in sigil's own tests renders it, and until this file it had no
//! test of any kind. Both faults found on the actual phone were in it -- a
//! filled disc that the font does not have, drawn against every capability
//! the phone *had*, and a duplicate Notifications row.
//!
//! Two questions, the same two sigil asks of every chat route, and neither
//! needs a renderer: does anything draw wider than the screen, and is
//! anything drawn where a finger cannot get to it.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use sigil::app::{App, AppContext};
use sigil::navigator::Navigator;
use sigil::{Account, Accounts, theme};
use sigil_android::host::{PhoneReport, Shared};
use sigil_android::phone_app::PhoneApp;
use sigil_android::settings::Settings;
use sigil_platform::{Capability, Support};
use std::sync::{Arc, Mutex};

/// The OnePlus NE2213: 360 x 804 points, 1080 x 2412 at 3x. Not a Pixel's
/// 412, which is the width at which every test passed while the phone
/// overflowed.
const PHONE_WIDTH: f32 = 360.0;
const PHONE_HEIGHT: f32 = 804.0;

/// What the platform side says about itself, as long as it ever gets.
///
/// An endpoint is a URL: one unbroken word, which is the shape wrapping
/// cannot help with, and the one this pane is guaranteed to be given.
/// The state the phone is actually in until somebody installs a distributor:
/// nothing delivering, nothing to show, and -- until now -- nothing to press.
/// It is the longest of the two, because it is the one that has to explain
/// itself.
fn nothing_delivering() -> Shared {
    Arc::new(Mutex::new(PhoneReport {
        endpoint: None,
        distributor: None,
        last_window: None,
        notifications: Some(false),
    }))
}

fn report() -> Shared {
    Arc::new(Mutex::new(PhoneReport {
        endpoint: Some(
            "https://ntfy.example.org/up?instance=01HZXQ8A7K3MN4P5R6S7T8V9W0&auth=bearer".into(),
        ),
        distributor: Some("org.unifiedpush.distributor.nextpush".into()),
        last_window: Some(
            "connected, registered, caught up 3 conversations, wrote 7 messages, said 1".into(),
        ),
        notifications: Some(true),
    }))
}

/// One of each answer, with a refusal as long as a refusal gets.
fn capabilities() -> Vec<Capability> {
    vec![
        Capability::new(
            "Notifications",
            "telling you something was said",
            Support::Yes,
        ),
        Capability::new(
            "Start at login",
            "being here when the phone comes back",
            Support::no(
                "Android has no such thing for an app that is not a launcher; the wake \
                 window is what stands in for it",
            ),
        ),
        Capability::new("Key store", "holding this phone's key", Support::Yes),
        // As the phone reports it with nothing installed.
        Capability::new(
            "Being woken",
            "the exchange wakes this phone for a message or a call (SIP-45)",
            Support::no("no push distributor is installed and no FCM bridge is configured"),
        ),
    ]
}

fn harness(capabilities: Vec<Capability>) -> Harness<'static> {
    harness_reporting(capabilities, report())
}

fn harness_reporting(capabilities: Vec<Capability>, report: Shared) -> Harness<'static> {
    harness_reaching(capabilities, report, Box::new(|_| Ok(())), 1.0).0
}

/// The same tab, with the reader's text size turned up.
fn harness_scaled(capabilities: Vec<Capability>, scale: f32) -> Harness<'static> {
    harness_reaching(capabilities, report(), Box::new(|_| Ok(())), scale).0
}

/// The same, with the platform's "stay reachable" hook supplied, and the
/// settings file's path handed back to be read.
fn harness_reaching(
    capabilities: Vec<Capability>,
    report: Shared,
    reach: sigil_android::phone_app::Reach,
    text_scale: f32,
) -> (Harness<'static>, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let at = dir.path().join("settings.json");
    // Kept for the harness's life: the app writes to it when a radio button
    // is pressed, and a `TempDir` dropped here would take the directory.
    let keep = Box::leak(Box::new(dir));
    let _ = keep;
    let app =
        PhoneApp::new(Settings::default(), at.clone(), capabilities, report).with_reach(reach);
    let app = std::rc::Rc::new(std::cell::RefCell::new(app));
    let mut accounts = Accounts::of(vec![Account::unlocked_for_test([1u8; 32])]);
    let harness = Harness::builder()
        .with_size(egui::vec2(PHONE_WIDTH, PHONE_HEIGHT))
        .build_ui(move |ui| {
            let mut app = app.borrow_mut();
            let ctx = ui.ctx().clone();
            sigil::Form::install(&ctx, sigil::Form::Phone);
            sigil::TextScale::install(&ctx, text_scale);
            theme::install(&ctx, theme::light(), theme::dark());
            ctx.set_theme(egui::Theme::Dark);
            let t = sigil::ColorTheme::current(&ctx);
            let mut nav = Navigator::default();
            let mut app_ctx = AppContext {
                navigator: &mut nav,
                accounts: &mut accounts,
                unfocused: false,
                away: false,
                notify: &sigil::Silent,
                connections: &Default::default(),
            };
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE
                        .fill(t.surface_primary)
                        .inner_margin(egui::Margin::same(sigil::tokens::SPACING_MD as i8)),
                )
                .show(ui, |ui| {
                    let _ = app.render(&mut app_ctx, ui);
                });
        });
    (harness, at)
}

/// Every widget's box, in points, with what it is called.
fn boxes(h: &Harness<'static>) -> Vec<(String, f64, f64, f64)> {
    fn walk(node: egui_kittest::Node<'_>, ppp: f64, out: &mut Vec<(String, f64, f64, f64)>) {
        let n = node.accesskit_node();
        let name = n
            .label()
            .map(|l| l.to_string())
            .or_else(|| n.value().map(|v| v.to_string()))
            .unwrap_or_else(|| format!("{:?}", n.role()));
        // The accesskit node's own box, not `Node::rect`: that one `expect`s
        // a rectangle and the root has none, so it panics.
        if let Some(b) = n.bounding_box() {
            out.push((name, b.x0 / ppp, b.x1 / ppp, b.y1 / ppp));
        }
        for c in node.children() {
            walk(c, ppp, out);
        }
    }
    let mut seen = Vec::new();
    walk(h.root(), h.ctx.pixels_per_point() as f64, &mut seen);
    seen
}

fn to_the_end(h: &mut Harness<'static>) {
    for _ in 0..40 {
        h.hover_at(egui::pos2(PHONE_WIDTH / 2.0, PHONE_HEIGHT / 2.0));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -400.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::default(),
        });
        // Not `run`: a pane with a spinner repaints for ever, and `run`
        // panics when it exceeds its step budget.
        h.run_steps(2);
    }
}

#[test]
fn the_phone_tab_fits_the_phone() {
    let mut h = harness(capabilities());
    h.run();
    h.run();
    let seen = boxes(&h);
    assert!(
        seen.len() > 8,
        "only {} widgets drew, so this proves nothing",
        seen.len()
    );
    let over: Vec<String> = seen
        .iter()
        .filter(|(_, x0, x1, _)| x1 - x0 > 0.0 && (*x1 > PHONE_WIDTH as f64 + 1.0 || *x0 < -1.0))
        .map(|(name, x0, x1, _)| format!("{name:?} at {x0:.0}..{x1:.0}"))
        .collect();
    assert!(
        over.is_empty(),
        "{} widget(s) are drawn outside a {PHONE_WIDTH}-point screen:\n  {}",
        over.len(),
        over.join("\n  ")
    );
}

#[test]
fn the_phone_tab_fits_the_phone_when_the_text_is_turned_up() {
    let mut over: Vec<String> = Vec::new();
    for scale in [1.3f32, 2.0] {
        let mut h = harness_scaled(capabilities(), scale);
        h.run();
        h.run();
        // **What the instrument is pointed at.** This passed first time, and
        // a scale that never reached the style would pass for exactly the
        // same reason `the_phone_tab_fits_the_phone` does.
        let body = h.ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body].size;
        assert!(
            (body - 15.0 * scale).abs() < 0.01,
            "the text was not turned up: body is {body} points, not {} -- so \
             this proves nothing",
            15.0 * scale
        );
        let seen = boxes(&h);
        assert!(seen.len() > 8, "only {} widgets drew", seen.len());
        for (name, x0, x1, _) in &seen {
            if x1 - x0 > 0.0 && (*x1 > PHONE_WIDTH as f64 + 1.0 || *x0 < -1.0) {
                over.push(format!("at {scale}x: {name:?} at {x0:.0}..{x1:.0}"));
            }
        }
    }
    assert!(
        over.is_empty(),
        "{} widget(s) are drawn outside a {PHONE_WIDTH}-point screen when the \
         text is turned up:\n  {}",
        over.len(),
        over.join("\n  ")
    );
}

/// **The key is one line somebody can read out.**
///
/// `sqx-device:<key>` is fifty-five characters of monospace, and on the phone
/// it wrapped after `sqx-` -- the one hyphen in it -- so the key started with
/// a dangling prefix on the line above. Seen on the device, not in any test:
/// the width check passed, because a label that wraps is not a label that
/// overflows. The scheme is a caption now and the key stands alone, and this
/// asks for the key as one widget with nothing cut and nothing joined.
#[test]
fn the_phones_key_is_shown_whole_on_one_line() {
    for scale in [1.0f32, 1.3, 2.0] {
        one_line_key(scale);
    }
}

/// **Asserted as a line, not as a label.** `labels.contains(&key)` was the
/// whole of this check, and accesskit reports a wrapped label by its source
/// string -- so the key split across two lines answered to its own name and
/// this passed. It did: at 1.3 the key was drawn as `…4A4m5t` and
/// `gebLHaRSZ9`, and only the probe that looked for a *piece* of the key saw
/// it. The pieces are what to look for.
fn one_line_key(scale: f32) {
    let mut h = harness_scaled(capabilities(), scale);
    h.run();
    h.run();
    let key = sigil::Account::unlocked_for_test([1u8; 32])
        .unlocked()
        .expect("unlocked")
        .me()
        .to_string();
    let seen = boxes(&h);
    let split: Vec<&String> = seen
        .iter()
        .map(|(n, ..)| n)
        .filter(|n| n.len() < key.len() && !n.is_empty() && key.contains(n.as_str()))
        .collect();
    assert!(
        split.is_empty(),
        "at {scale}x the key is drawn in pieces, so it broke across lines: \
         {split:?}"
    );
    let width = seen
        .iter()
        .find(|(n, ..)| *n == key)
        .map(|(_, x0, x1, _)| x1 - x0)
        .unwrap_or_default();
    assert!(
        width > 0.0 && width <= PHONE_WIDTH as f64,
        "at {scale}x the key is {width:.0} points across a {PHONE_WIDTH}-point \
         screen"
    );
    let labels: Vec<String> = seen.into_iter().map(|(n, ..)| n).collect();
    assert!(
        labels.contains(&key),
        "the key is not drawn as one label of its own: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l == "sqx-device:"),
        "the scheme is not shown as its caption"
    );
    // The whole string is still on screen once, and rightly: the QR names
    // what it encodes, which is what the other device scans.
    assert_eq!(
        labels
            .iter()
            .filter(|l| l.starts_with("sqx-device:") && l.len() > 11)
            .count(),
        1,
        "the whole `sqx-device:` string should be the QR's and nobody else's: {labels:?}"
    );
}

/// **A caption has to sit on the thing it captions.**
///
/// `sqx-device:` is the scheme half of the string the QR carries, split off
/// so that the key beneath it can be read out as one line. It shares its row
/// with the Copy button -- and a row is at least `interact_size` tall, which
/// on a phone is 44 points. So a 14-point caption was centred in a 44-point
/// row and floated a finger's width above the key it belongs to, with the
/// result that the two read as separate facts: a label with a button and
/// nothing after it, then a key from nowhere. Seen on the handset.
#[test]
fn the_scheme_sits_on_the_key_it_captions() {
    fn rect_of(h: &Harness<'static>, want: &str) -> Option<(f32, f32)> {
        fn walk(node: egui_kittest::Node<'_>, want: &str, ppp: f32, out: &mut Option<(f32, f32)>) {
            let n = node.accesskit_node();
            let name = n
                .label()
                .map(|l| l.to_string())
                .or_else(|| n.value().map(|v| v.to_string()));
            if name.as_deref() == Some(want)
                && let Some(b) = n.bounding_box()
            {
                *out = Some((b.y0 as f32 / ppp, b.y1 as f32 / ppp));
            }
            for c in node.children() {
                walk(c, want, ppp, out);
            }
        }
        let mut out = None;
        walk(h.root(), want, h.ctx.pixels_per_point(), &mut out);
        out
    }

    let mut h = harness(capabilities());
    h.run();
    h.run();
    let key = sigil::Account::unlocked_for_test([1u8; 32])
        .unlocked()
        .expect("unlocked")
        .me()
        .to_string();
    let caption = rect_of(&h, "sqx-device:").expect("the scheme caption");
    let below = rect_of(&h, &key).expect("the key");
    let gap = below.0 - caption.1;
    let tall = caption.1 - caption.0;
    assert!(
        below.0 > caption.0,
        "the key is not under its caption: {caption:?} {below:?}"
    );
    assert!(
        gap <= tall,
        "the caption floats {gap} above the key it captions, which is more \
         than its own height ({tall}) -- they read as two facts"
    );
}

#[test]
fn nothing_in_the_phone_tab_is_out_of_reach() {
    let mut h = harness(capabilities());
    h.run();
    h.run();
    let before = boxes(&h)
        .into_iter()
        .map(|(_, _, _, y1)| y1)
        .fold(0.0f64, f64::max);
    // The control: a pane that already fits has nothing to scroll, and then
    // scrolling to the end proves nothing.
    assert!(
        before > PHONE_HEIGHT as f64,
        "the Phone tab drew only {before:.0} points of content on an \
         {PHONE_HEIGHT}-point screen, so the scroll below is not being asked \
         anything. Lengthen the fixture."
    );
    to_the_end(&mut h);
    let deepest = boxes(&h)
        .into_iter()
        .max_by(|a, b| a.3.total_cmp(&b.3))
        .expect("something drew");
    assert!(
        deepest.3 <= PHONE_HEIGHT as f64 + 1.0,
        "{:?} still ends at y={:.0} on an {PHONE_HEIGHT}-point screen after \
         scrolling to the end, so nothing reaches it",
        deepest.0,
        deepest.3
    );
}

/// **The one screen that says the phone cannot be woken offers a way to fix
/// it.**
///
/// Until a distributor is installed, sigil hears nothing while it is not in
/// front -- which is the whole of SIP-45 not working -- and this pane is
/// where a person finds that out. It said "Install a UnifiedPush distributor
/// (ntfy, for one)" and stopped: a name to remember and nothing to press.
///
/// Two links now, because which distributor to run is the person's choice
/// and naming one is not choosing for them. They are real controls now as
/// well: `links` was not in this workspace's eframe features, so every
/// hyperlink in sigil opened nothing at all (`tests/links.rs`).
///
/// And the pane still fits, and is still reachable, in the state that has
/// the most to say -- which is this one, not the one with a distributor.
#[test]
fn with_no_distributor_the_phone_tab_offers_one() {
    let mut h = harness_reporting(capabilities(), nothing_delivering());
    h.run();
    h.run();
    let said = text_of(&h);
    assert!(
        said.contains("No push distributor"),
        "the pane does not say the phone cannot be woken: {said}"
    );
    for what in ["Get ntfy", "Other distributors"] {
        assert!(
            said.contains(what),
            "{what:?} is not offered, so the pane names a problem and no way \
             out of it: {said}"
        );
    }

    let seen = boxes(&h);
    let over: Vec<String> = seen
        .iter()
        .filter(|(_, x0, x1, _)| x1 - x0 > 0.0 && (*x1 > PHONE_WIDTH as f64 + 1.0 || *x0 < -1.0))
        .map(|(name, x0, x1, _)| format!("{name:?} at {x0:.0}..{x1:.0}"))
        .collect();
    assert!(
        over.is_empty(),
        "{} widget(s) are drawn outside a {PHONE_WIDTH}-point screen:\n  {}",
        over.len(),
        over.join("\n  ")
    );

    to_the_end(&mut h);
    let deepest = boxes(&h)
        .into_iter()
        .max_by(|a, b| a.3.total_cmp(&b.3))
        .expect("something drew");
    assert!(
        deepest.3 <= PHONE_HEIGHT as f64 + 1.0,
        "{:?} still ends at y={:.0} after scrolling to the end",
        deepest.0,
        deepest.3
    );
}

/// Everything the pane says, as one string. Both `label` and `value`:
/// accesskit puts an interactive widget's text in one and a plain one's in
/// the other, so reading only labels sees buttons and no prose.
fn text_of(h: &Harness<'static>) -> String {
    let mut found: Vec<String> = Vec::new();
    fn walk(node: egui_kittest::Node<'_>, out: &mut Vec<String>) {
        let n = node.accesskit_node();
        if let Some(l) = n.label() {
            out.push(l.to_string());
        }
        if let Some(v) = n.value() {
            out.push(v.to_string());
        }
        for c in node.children() {
            walk(c, out);
        }
    }
    walk(h.root(), &mut found);
    found.join(" | ")
}

/// With no distributor, the tab offers to stay reachable instead; choosing
/// it is written down and told to the platform, and at launch a phone that
/// chose it before is told again. With a distributor there is nothing to
/// stay awake for, and it is not offered.
#[test]
fn staying_reachable_is_offered_without_a_distributor_and_told_to_the_platform() {
    let told = std::rc::Rc::new(std::cell::RefCell::new(Vec::<bool>::new()));
    let hook = {
        let told = told.clone();
        Box::new(move |on: bool| {
            told.borrow_mut().push(on);
            Ok(())
        })
    };
    let (mut h, at) = harness_reaching(capabilities(), nothing_delivering(), hook, 1.0);
    h.run();
    h.run();
    assert!(told.borrow().is_empty(), "nothing chosen, nothing told");
    // On screen before it is pressed: a press on a widget scrolled out of
    // view lands on nothing.
    to_the_end(&mut h);
    let choice = h.get_by_label("Stay reachable without one");
    choice.click();
    h.run();
    h.run();
    assert_eq!(*told.borrow(), vec![true], "the platform was not told");
    assert!(
        Settings::load(&at).stay_reachable,
        "the choice was not written down"
    );
    assert!(
        text_of(&h).contains("Running with the screen off"),
        "the pane does not say what it is doing: {}",
        text_of(&h)
    );
    // And the warning that nothing arrives while sigil is not in front is
    // withdrawn, since it is no longer true.
    assert!(
        !text_of(&h).contains("hears nothing while it is not in front"),
        "the warning stands while the phone is reachable: {}",
        text_of(&h)
    );
    assert!(text_of(&h).contains("stays running instead"));
    // And the "Being woken" row below reads with the choice: kept awake,
    // not the platform's word that nothing wakes it.
    assert!(
        text_of(&h).contains("kept awake instead"),
        "the capability row still says the phone cannot be reached: {}",
        text_of(&h)
    );

    // At launch, a phone that chose it is told again, through with_reach.
    let told = std::rc::Rc::new(std::cell::RefCell::new(Vec::<bool>::new()));
    let hook = {
        let told = told.clone();
        Box::new(move |on: bool| {
            told.borrow_mut().push(on);
            Ok(())
        })
    };
    let chosen = Settings {
        stay_reachable: true,
        ..Settings::default()
    };
    let _app =
        PhoneApp::new(chosen, at.clone(), capabilities(), nothing_delivering()).with_reach(hook);
    assert_eq!(*told.borrow(), vec![true], "not applied at launch");

    // With a distributor, not offered.
    let mut h = harness_reporting(capabilities(), report());
    h.run();
    h.run();
    assert!(
        h.query_by_label("Stay reachable without one").is_none(),
        "offered where there is a distributor: {}",
        text_of(&h)
    );
}

/// Choosing how long the exchange keeps the endpoint offers the endpoint
/// again with the new span, for the sessions to register on their next
/// pass. Without an endpoint there is nothing to offer.
#[test]
fn choosing_the_endpoints_span_offers_it_again_to_the_sessions() {
    let _ = sigil::wake::take();
    let mut h = harness_reporting(capabilities(), report());
    h.run();
    h.run();
    to_the_end(&mut h);
    h.get_by_label("a week").click();
    h.run();
    let offered = sigil::wake::take().expect("the endpoint was offered again");
    assert_eq!(offered.ttl_secs, 7 * 86_400);
    assert!(
        offered
            .url
            .as_deref()
            .is_some_and(|u| u.starts_with("https://ntfy.example.org/up")),
        "{offered:?}"
    );
    assert!(sigil::wake::take().is_none(), "offered twice");

    // No distributor, no endpoint: choosing a span offers nothing.
    let mut h = harness_reporting(capabilities(), nothing_delivering());
    h.run();
    h.run();
    to_the_end(&mut h);
    h.get_by_label("a day").click();
    h.run();
    assert!(
        sigil::wake::take().is_none(),
        "an endpoint was offered with none held"
    );
}

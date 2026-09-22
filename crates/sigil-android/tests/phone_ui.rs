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
use egui_kittest::kittest::NodeT;
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
    ]
}

fn harness(capabilities: Vec<Capability>) -> Harness<'static> {
    harness_reporting(capabilities, report())
}

fn harness_reporting(capabilities: Vec<Capability>, report: Shared) -> Harness<'static> {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let at = dir.path().join("settings.json");
    // Kept for the harness's life: the app writes to it when a radio button
    // is pressed, and a `TempDir` dropped here would take the directory.
    let keep = Box::leak(Box::new(dir));
    let _ = keep;
    let app = PhoneApp::new(Settings::default(), at, capabilities, report);
    let app = std::rc::Rc::new(std::cell::RefCell::new(app));
    let mut accounts = Accounts::of(vec![Account::unlocked_for_test([1u8; 32])]);
    Harness::builder()
        .with_size(egui::vec2(PHONE_WIDTH, PHONE_HEIGHT))
        .build_ui(move |ui| {
            let mut app = app.borrow_mut();
            let ctx = ui.ctx().clone();
            sigil::Form::install(&ctx, sigil::Form::Phone);
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
        })
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

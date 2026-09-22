//! The *Phone* tab: what this phone can do, where its wake goes, how much a
//! notification says, and the key another device pairs it by.
//!
//! The same shape as the desktop's Desktop tab and for the same reason: it
//! is the answer to "why did that not happen", and nothing here is silently
//! inert. Pairing's other half -- where the phone goes once it has been
//! registered -- is in Chat's Devices pane, beside the desktop's half, so
//! both ends of SIP-47 are in one place on every kind of device.

use std::path::PathBuf;

use sigil::app::{App, AppContext, AppResponse};
use sigil::{ColorTheme, tokens};
use sigil_phone::Privacy;
use sigil_platform::Capability;

use crate::host::Shared;
use crate::settings::{PrivacySetting, Settings};

pub struct PhoneApp {
    settings: Settings,
    settings_at: PathBuf,
    capabilities: Vec<Capability>,
    report: Shared,
    trouble: Option<String>,
}

impl PhoneApp {
    pub fn new(
        settings: Settings,
        settings_at: PathBuf,
        capabilities: Vec<Capability>,
        report: Shared,
    ) -> PhoneApp {
        PhoneApp {
            settings,
            settings_at,
            capabilities,
            report,
            trouble: None,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    fn save(&mut self) {
        self.trouble = self.settings.save(&self.settings_at).err();
    }
}

impl App for PhoneApp {
    fn title(&self) -> &str {
        "Phone"
    }

    fn render(&mut self, ctx: &mut AppContext<'_>, ui: &mut egui::Ui) -> AppResponse {
        let theme = ColorTheme::current(ui.ctx());
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("This phone");
            ui.add_space(tokens::SPACING_SM);

            // SIP-47 §Pairing, step 1: the key, as text and as a QR.
            match ctx.account().unlocked() {
                Some(unlocked) => {
                    let shown = sigil_phone::pairing::device_string(&unlocked.me());
                    ui.colored_label(
                        theme.text_secondary,
                        "This phone's key. Show it to a device that holds your account -- \
                         its Chat › Devices pane reads it -- and it will register this \
                         phone and show you where to go.",
                    );
                    ui.add_space(tokens::SPACING_SM);
                    sigil_ui::qr(ui, &shown, 200.0);
                    // **The scheme on its own line.** `sqx-device:<key>` is
                    // fifty-five characters of monospace, and on the phone
                    // it broke after `sqx-` -- the one hyphen in it -- so the
                    // key somebody is meant to read out started with a
                    // dangling prefix on the line above. The key alone fits
                    // the width; the scheme is what the QR and Copy carry,
                    // and here it is a caption.
                    let (scheme, key) = shown.split_once(':').unwrap_or(("", &shown));
                    if !scheme.is_empty() {
                        ui.colored_label(
                            theme.text_muted,
                            egui::RichText::new(format!("{scheme}:")).small(),
                        );
                    }
                    ui.add(
                        egui::Label::new(egui::RichText::new(key).monospace().small())
                            .selectable(true),
                    );
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(shown);
                    }
                }
                None => {
                    ui.colored_label(
                        theme.text_muted,
                        "The key store has not opened this phone's key.",
                    );
                }
            }

            ui.add_space(tokens::SPACING_XL);
            ui.heading("Notifications say");
            ui.colored_label(
                theme.text_secondary,
                "Composed here, from words only this phone can read, and shown to the \
                 platform. The quietest setting keeps the platform as ignorant as the \
                 push service, which is handed four bytes.",
            );
            let mut privacy = self.settings.privacy;
            for choice in Privacy::ALL {
                let setting = PrivacySetting::from(choice);
                ui.radio_value(&mut privacy, setting, choice.describe());
            }
            if privacy != self.settings.privacy {
                self.settings.privacy = privacy;
                self.save();
            }

            ui.add_space(tokens::SPACING_XL);
            ui.heading("Being woken");
            let report = self.report.lock().map(|r| r.clone()).unwrap_or_default();
            match (&report.distributor, &report.endpoint) {
                (Some(d), Some(e)) => {
                    ui.label(format!("Distributor: {d}"));
                    ui.colored_label(
                        theme.text_muted,
                        egui::RichText::new(format!("endpoint {}", brief(e))).small(),
                    );
                }
                (Some(d), None) => {
                    ui.label(format!("Distributor: {d}, no endpoint yet"));
                }
                (None, _) => {
                    ui.colored_label(
                        theme.destructive,
                        "No push distributor. Until there is one, sigil hears nothing \
                         while it is not in front: a message waits until the app is \
                         opened.",
                    );
                    ui.colored_label(
                        theme.text_secondary,
                        "A distributor is a separate app, chosen by you, that holds the \
                         one connection every app on this phone is woken through. sigil \
                         never sees the push service's account, and the push service \
                         never sees more than four bytes.",
                    );
                    // **A way to do it, not only a name to remember.** This
                    // said "Install a UnifiedPush distributor (ntfy, for
                    // one)" and stopped there: the one screen that reports
                    // the phone cannot be woken offered nothing to press. A
                    // link is what the person needs, and a link is a control
                    // now -- `links` was not in this workspace's eframe
                    // features, so every one of them opened nothing.
                    ui.horizontal_wrapped(|ui| {
                        ui.hyperlink_to("Get ntfy", NTFY);
                        ui.add_space(tokens::SPACING_SM);
                        ui.hyperlink_to("Other distributors", UNIFIEDPUSH);
                    });
                }
            }
            if let Some(last) = &report.last_window {
                ui.colored_label(
                    theme.text_muted,
                    egui::RichText::new(format!("last wake: {last}")).small(),
                );
            }
            // **The label above, not beside.** On the phone this row was
            // twenty-one characters of label, then a slider, then its value
            // box, and the slider got what was left: about ninety points of
            // travel for a range of thirty, which is a control you cannot
            // land on a number with a finger. Above it, the slider has the
            // width of the pane.
            let mut days = self.settings.endpoint_days;
            let wide = ui.available_width() >= tokens::NARROW_WIDTH;
            // **And the track itself has to be told.** Moving the label off
            // the row gave the slider room and it did not take it: egui sizes
            // a slider from `spacing.slider_width`, a fixed 100 points, so it
            // stayed the same stub with more space beside it. The width is
            // the pane less the value box that sits after it.
            let mut set = |ui: &mut egui::Ui| {
                let room = ui.available_width() - VALUE_BOX;
                ui.spacing_mut().slider_width = room.max(120.0);
                ui.add(
                    egui::Slider::new(&mut days, 1..=30)
                        .suffix(" days")
                        .clamping(egui::SliderClamping::Always),
                )
                .changed()
            };
            let changed = if wide {
                ui.horizontal(|ui| {
                    ui.label("Keep the endpoint for");
                    set(ui)
                })
                .inner
            } else {
                ui.colored_label(
                    theme.text_secondary,
                    egui::RichText::new("Keep the endpoint for").small(),
                );
                set(ui)
            };
            if changed {
                self.settings.endpoint_days = days;
                self.save();
            }

            ui.add_space(tokens::SPACING_XL);
            ui.heading("What this phone can do");
            for c in &self.capabilities {
                ui.horizontal(|ui| {
                    // **Painted, not written.** These were `●` and `○`, and
                    // `●` is not in egui's bundled font: every capability
                    // this phone actually *has* drew as a tofu box, so the
                    // one state somebody opens this pane to confirm read as
                    // a rendering fault. `sigil_ui::dot` exists for exactly
                    // this -- its own comment records `●`/`○` doing it once
                    // before -- and it carries the word, which a shape
                    // cannot.
                    let yes = c.support.is_yes();
                    sigil_ui::dot(
                        ui,
                        yes,
                        theme.accent,
                        theme.text_muted,
                        if yes { "yes" } else { "no" },
                    );
                    ui.add_space(tokens::SPACING_XS);
                    ui.vertical(|ui| {
                        ui.label(c.name);
                        ui.colored_label(theme.text_secondary, egui::RichText::new(c.what).small());
                        if let Some(why) = c.support.reason() {
                            ui.colored_label(theme.text_muted, egui::RichText::new(why).small());
                        }
                    });
                });
            }
            if let Some(trouble) = &self.trouble {
                ui.add_space(tokens::SPACING_SM);
                ui.colored_label(theme.destructive, trouble);
            }
        });
        AppResponse::default()
    }
}

/// The distributor most people will want, on the store this phone has.
///
/// A Play link rather than a package name: `market://` needs Play and
/// F-Droid users have none, and an `https://` play link is handled by Play
/// where it is installed and by a browser where it is not.
const NTFY: &str = "https://play.google.com/store/apps/details?id=io.heckel.ntfy";

/// And the list of the others, because which distributor to run is the
/// person's choice and naming one is not the same as choosing for them.
const UNIFIEDPUSH: &str = "https://unifiedpush.org/users/distributors/";

/// Room for the number beside a slider: "30 days" in a box, with its
/// padding. Measured once by eye against the phone rather than derived,
/// because egui gives no way to ask what a `DragValue` will be before it is
/// drawn; erring large only costs the track a few points.
const VALUE_BOX: f32 = 90.0;

/// The desktop's capability rows and the phone's own, as one list.
///
/// # Why anything has to be dropped
///
/// The two lists overlap on Notifications, and they disagree by
/// construction: the desktop's row reports *unavailable* on Android and
/// gives as its reason "the phone posts its own notifications" -- which is a
/// sentence pointing at the row below it. So the pane showed the same name
/// twice, adjacent, one hollow and one filled, and somebody opening it to
/// find out whether notifications work read the wrong one first.
///
/// A row of the phone's own always wins, by name, because the phone is the
/// thing being asked about. Kept here rather than in `android/entry.rs`
/// because that module is `cfg(target_os = "android")` and nothing in it can
/// be tested on the machine this is written on.
pub fn merged(platform: Vec<Capability>, phone: Vec<Capability>) -> Vec<Capability> {
    let named: Vec<&'static str> = phone.iter().map(|c| c.name).collect();
    let mut out: Vec<Capability> = platform
        .into_iter()
        .filter(|c| !named.contains(&c.name))
        .collect();
    out.extend(phone);
    out
}

/// An endpoint, shortened for a row: its host, and the tail of its path.
fn brief(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let tail: String = path
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if path.is_empty() {
        host.to_string()
    } else {
        format!("{host}/…{tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_platform::Support;

    /// The pane does not list one capability twice under one name.
    ///
    /// It did: the desktop's Notifications row said "unavailable -- the
    /// phone posts its own notifications", which is a sentence about the row
    /// underneath it, and the two sat together with opposite marks.
    #[test]
    fn a_capability_the_phone_answers_for_itself_is_not_listed_twice() {
        let desktop = vec![
            Capability::new(
                "Notifications",
                "the desktop's answer",
                Support::no("not here"),
            ),
            Capability::new("Tray icon", "a tray", Support::no("no tray on a phone")),
        ];
        let phone = vec![
            Capability::new("Notifications", "the phone's answer", Support::Yes),
            Capability::new("Being woken", "SIP-45", Support::no("no distributor")),
        ];
        let rows = merged(desktop, phone);
        let names: Vec<&str> = rows.iter().map(|c| c.name).collect();
        assert_eq!(
            names.iter().filter(|n| **n == "Notifications").count(),
            1,
            "Notifications is listed twice: {names:?}"
        );
        assert!(
            rows.iter()
                .any(|c| c.name == "Notifications" && c.support.is_yes()),
            "the row that survived is the desktop's, which is the one that \
             does not know: {names:?}"
        );
        assert!(
            names.contains(&"Tray icon") && names.contains(&"Being woken"),
            "a row only one list has must still be there: {names:?}"
        );
    }

    #[test]
    fn an_endpoint_is_shown_as_its_host_and_a_tail() {
        assert_eq!(
            brief("https://ntfy.sh/up/abcdefghijklmnop"),
            "ntfy.sh/…ijklmnop"
        );
        assert_eq!(brief("https://ntfy.sh"), "ntfy.sh");
    }
}

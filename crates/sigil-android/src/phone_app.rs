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

/// What keeps the process alive with its connection open, or lets it go:
/// the platform's foreground service on the phone, nothing on a desktop
/// where this is built and tested.
pub type Reach = Box<dyn Fn(bool) -> Result<(), String>>;

pub struct PhoneApp {
    settings: Settings,
    settings_at: PathBuf,
    capabilities: Vec<Capability>,
    report: Shared,
    trouble: Option<String>,
    reach: Reach,
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
            reach: Box::new(|_| Ok(())),
        }
    }

    /// The platform's way of staying reachable, and the setting applied
    /// through it now: a phone that chose this before is reachable from
    /// launch, not from the next visit to the tab.
    pub fn with_reach(mut self, reach: Reach) -> PhoneApp {
        self.reach = reach;
        if self.settings.stay_reachable
            && let Err(why) = (self.reach)(true)
        {
            self.trouble = Some(why);
        }
        self
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

    /// The handset, not the default globe: this pane is about *this*
    /// device -- what the platform lets it do, and how it is reached when
    /// it is not in front.
    fn icon(&self) -> sigil::Icon {
        sigil::Icon::Device
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
                    // Small, under its heading: what a card explains is
                    // worth reading once, and at body size three lines of
                    // it push the thing it explains off a phone's screen.
                    ui.colored_label(
                        theme.text_secondary,
                        egui::RichText::new(
                            "This phone's key. Show it to a device that holds your account — \
                             its Chat › Devices pane reads it — and it will register this \
                             phone and show you where to go.",
                        )
                        .small(),
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
                    // The caption and the way to take the key share a row:
                    // the caption is short, and Copy under the key -- a row
                    // of its own, a row's worth of space away -- read as
                    // belonging to whatever came next. **Never beside the
                    // key itself**: forty-four monospace characters and a
                    // button are wider than the pane, and a row wider than
                    // the pane re-lays every row after it.
                    ui.horizontal(|ui| {
                        if !scheme.is_empty() {
                            ui.colored_label(
                                theme.text_muted,
                                egui::RichText::new(format!("{scheme}:")).small(),
                            );
                        }
                        if sigil_ui::icon_button_named(ui, sigil_ui::Icon::Copy, "Copy").clicked() {
                            ui.ctx().copy_text(shown.clone());
                        }
                    });
                    ui.add(
                        egui::Label::new(egui::RichText::new(key).monospace().small())
                            .selectable(true),
                    );
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
                egui::RichText::new(
                    "Composed here, from words only this phone can read, and shown to the \
                     platform. The quietest setting keeps the platform as ignorant as the \
                     push service, which is handed four bytes.",
                )
                .small(),
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
                    // Red only while it is true: with the phone staying
                    // reachable there is no distributor and nothing waits.
                    if self.settings.stay_reachable {
                        ui.colored_label(
                            theme.text_secondary,
                            "No push distributor. sigil stays running instead, so nothing \
                             waits for the app to be opened.",
                        );
                    } else {
                        ui.colored_label(
                            theme.destructive,
                            "No push distributor. Until there is one, sigil hears nothing \
                             while it is not in front: a message waits until the app is \
                             opened.",
                        );
                    }
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
                    // **Or stay awake.** What a messenger does on a phone
                    // without push: the process kept alive with its
                    // connection open, a quiet notice on the shade saying
                    // so, and the battery paying for it. The person's
                    // choice, and off unless chosen. Only offered where it
                    // is the alternative -- with a distributor there is
                    // nothing to stay awake for.
                    ui.add_space(tokens::SPACING_SM);
                    let mut stay = self.settings.stay_reachable;
                    if ui
                        .checkbox(&mut stay, "Stay reachable without one")
                        .on_hover_text(
                            "sigil keeps running with the screen off and its connection \
                             open, and says so on the shade. Calls and messages arrive \
                             as they would on a desktop. It costs battery.",
                        )
                        .changed()
                    {
                        self.settings.stay_reachable = stay;
                        self.save();
                        if let Err(why) = (self.reach)(stay) {
                            self.trouble = Some(why);
                        }
                    }
                    ui.colored_label(
                        theme.text_muted,
                        egui::RichText::new(if stay {
                            "Running with the screen off; a quiet notice on the shade says \
                             so. It costs battery."
                        } else {
                            "sigil keeps running with the screen off, and calls and \
                             messages arrive as they would on a desktop. It costs battery."
                        })
                        .small(),
                    );
                }
            }
            if let Some(last) = &report.last_window {
                ui.colored_label(
                    theme.text_muted,
                    egui::RichText::new(format!("last wake: {last}")).small(),
                );
            }
            // **Three choices, not a slider.** This was a 1..=30 slider with
            // a value box, and on the phone its handle was a pill the size
            // of a button on a track nobody could land a number on; the
            // number itself is not one anybody chooses to the day. A day, a
            // week or a month is the whole of the decision -- a phone that
            // connects daily, one that does not, one left in a drawer -- so
            // those are the choices, as words, and a value set some other
            // way is shown as a fourth so it is never silently lost.
            let mut days = self.settings.endpoint_days;
            ui.colored_label(
                theme.text_secondary,
                egui::RichText::new("Keep the endpoint for").small(),
            );
            let mut choices: Vec<(String, u32)> = vec![
                ("a day".into(), 1),
                ("a week".into(), 7),
                ("a month".into(), 30),
            ];
            if !choices.iter().any(|(_, n)| *n == days) {
                choices.push((format!("{days} days"), days));
            }
            let changed = ui
                .horizontal_wrapped(|ui| {
                    let mut changed = false;
                    for (word, n) in &choices {
                        if ui.selectable_label(days == *n, word).clicked() && days != *n {
                            days = *n;
                            changed = true;
                        }
                    }
                    changed
                })
                .inner;
            if changed {
                self.settings.endpoint_days = days;
                self.save();
                // SIP-45: the exchange holds the endpoint for the old span
                // until told the new one -- offered again to the sessions,
                // which register it on their next pass.
                if let Some(url) = &report.endpoint {
                    sigil::wake::offer(Some(url.clone()), days.clamp(1, 30) * 86_400);
                }
            }

            ui.add_space(tokens::SPACING_XL);
            ui.heading("What this phone can do");
            for c in &self.capabilities {
                // **A row that reads with the choice above it.** "Being
                // woken" is the platform's answer at start -- no distributor
                // -- and with the phone staying reachable the outcome it
                // describes is had another way. The row says which; it does
                // not claim a wake that never happens.
                let kept_awake =
                    c.name == "Being woken" && !c.support.is_yes() && self.settings.stay_reachable;
                ui.horizontal(|ui| {
                    // **Painted, not written.** These were `●` and `○`, and
                    // `●` is not in egui's bundled font: every capability
                    // this phone actually *has* drew as a tofu box, so the
                    // one state somebody opens this pane to confirm read as
                    // a rendering fault. `sigil_ui::dot` exists for exactly
                    // this -- its own comment records `●`/`○` doing it once
                    // before -- and it carries the word, which a shape
                    // cannot.
                    let yes = c.support.is_yes() || kept_awake;
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
                        if kept_awake {
                            ui.colored_label(
                                theme.text_muted,
                                egui::RichText::new(
                                    "not woken: kept awake instead, by your choice above",
                                )
                                .small(),
                            );
                        } else if let Some(why) = c.support.reason() {
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

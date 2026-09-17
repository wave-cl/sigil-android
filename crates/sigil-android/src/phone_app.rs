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
                    ui.add(
                        egui::Label::new(egui::RichText::new(&shown).monospace().small())
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
                        "No push distributor. Install a UnifiedPush distributor (ntfy, for \
                         one), or build with the FCM bridge configured; until then sigil \
                         hears nothing while it is not in front.",
                    );
                }
            }
            if let Some(last) = &report.last_window {
                ui.colored_label(
                    theme.text_muted,
                    egui::RichText::new(format!("last wake: {last}")).small(),
                );
            }
            ui.horizontal(|ui| {
                ui.label("Keep the endpoint for");
                let mut days = self.settings.endpoint_days;
                if ui
                    .add(egui::Slider::new(&mut days, 1..=30).suffix(" days"))
                    .changed()
                {
                    self.settings.endpoint_days = days;
                    self.save();
                }
            });

            ui.add_space(tokens::SPACING_XL);
            ui.heading("What this phone can do");
            for c in &self.capabilities {
                ui.horizontal(|ui| {
                    let (mark, colour) = if c.support.is_yes() {
                        ("●", theme.accent)
                    } else {
                        ("○", theme.text_muted)
                    };
                    ui.colored_label(colour, mark);
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

    #[test]
    fn an_endpoint_is_shown_as_its_host_and_a_tail() {
        assert_eq!(
            brief("https://ntfy.sh/up/abcdefghijklmnop"),
            "ntfy.sh/…ijklmnop"
        );
        assert_eq!(brief("https://ntfy.sh"), "ntfy.sh");
    }
}

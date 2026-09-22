//! `android_main`: the window.
//!
//! What a desktop's `main` does, in the order a phone needs it: the
//! environment pointed at the app's own directory before anything reads
//! `HOME`, logging to logcat, a tokio runtime entered for the life of the
//! process, the platform seams installed, the identity opened through the
//! key store, and eframe handed the activity.

use eframe::NativeOptions;
use sigil_platform::{Capability, Support};
use sigil_shell::Shell;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use super::platform::{AndroidChooser, AndroidNotifier, AndroidVault};
use crate::host::{Host, PhoneReport, Shared};

/// The host. Only this implements `eframe::App`; the shell is what it draws.
struct Sigil {
    shell: Shell,
}

impl eframe::App for Sigil {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let unfocused = !ctx.input(|i| i.viewport().focused.unwrap_or(true));
        self.shell.update_all(ctx, unfocused);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // What the system draws over the surface, as `Insets.kt` last said,
        // in points: the shell keeps everything clear of it, and lifts the
        // composer above the keyboard.
        let ppp = ui.ctx().pixels_per_point().max(0.1);
        let [top, bottom, left, right] = super::platform::insets_px();
        self.shell.set_insets(sigil::Insets {
            top: top as f32 / ppp,
            bottom: bottom as f32 / ppp,
            left: left as f32 / ppp,
            right: right as f32 / ppp,
        });
        self.shell.ui(ui);
    }
}

/// Point `HOME` and the XDG directories inside the app's files directory,
/// so `~/.sqnr`, `~/.sqex` and `~/.local/share/sigil` land where only this
/// app can read them. Done once, before anything asks.
pub fn point_home(files_dir: &std::path::Path) {
    if std::env::var_os("HOME").is_none() {
        // SAFETY: called before any other thread exists, from the entry or
        // the first JNI call into this library.
        unsafe {
            std::env::set_var("HOME", files_dir);
            std::env::set_var("XDG_DATA_HOME", files_dir.join(".local/share"));
            std::env::set_var("XDG_CONFIG_HOME", files_dir.join(".config"));
        }
    }
}

pub fn install_logging() {
    let _ = tracing_subscriber::registry()
        .with(super::logcat::Logcat::new("sigil"))
        .with(tracing_subscriber::EnvFilter::new(
            "sigil=info,sigil_android=info,sigil_phone=info,sigil_net=info,sigil_chat=info,\
             sigil_voice=info,sqex_voice=info,sqex_chat=info",
        ))
        .try_init();
    // A panic on a phone goes to stderr, which nobody reads, and then the
    // process aborts with only a signal in the log. Say what it was, where
    // the rest of the log is, before it does.
    std::panic::set_hook(Box::new(|info| {
        let where_ = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        tracing::error!("panicked at {where_}: {info}");
    }));
}

/// What both lists call the same thing, so the duplicate can be found by
/// name rather than by position.
const NOTIFICATIONS: &str = "Notifications";

/// The capability list the Phone tab shows: the desktop's rows, every one
/// of them absent here with its reason, and the phone's own.
///
/// **Except where the two are the same question.** The desktop's list has a
/// Notifications row, which on Android reports *unavailable* and gives as its
/// reason "the phone posts its own notifications" -- pointing at the row
/// below it, which reports the real answer. So the pane showed
/// "Notifications" twice, adjacent, one hollow and one filled, and somebody
/// opening it to find out whether notifications work read the wrong one
/// first. The platform's row is dropped and the phone's is the answer.
pub fn capabilities(report: &PhoneReport) -> Vec<Capability> {
    let platform = sigil_platform::Platform::new();
    let mut rows: Vec<Capability> = Vec::new();
    rows.push(Capability::new(
        NOTIFICATIONS,
        "says what arrived while sigil was not in front, and rings",
        match report.notifications {
            Some(true) => Support::Yes,
            Some(false) => Support::no("turned off for sigil in the phone's settings"),
            None => Support::no("not asked yet"),
        },
    ));
    rows.push(Capability::new(
        "Being woken",
        "the exchange wakes this phone for a message or a call (SIP-45)",
        match (&report.distributor, &report.endpoint) {
            (Some(_), Some(_)) => Support::Yes,
            (Some(d), None) => Support::no(format!("{d} has given no endpoint yet")),
            (None, _) => {
                Support::no("no push distributor is installed and no FCM bridge is configured")
            }
        },
    ));
    // Through `merged`, so the rule about an overlapping name is the one
    // with a test on it: this module is `cfg(target_os = "android")` and
    // nothing in it can be run on the machine it is written on.
    crate::phone_app::merged(platform.capabilities(), rows)
}

#[unsafe(no_mangle)]
pub fn android_main(app: android_activity::AndroidApp) {
    let files_dir = app
        .internal_data_path()
        .expect("an Android app has a files directory");
    point_home(&files_dir);
    install_logging();
    tracing::info!(
        "sigil-android {} starting in {}",
        env!("CARGO_PKG_VERSION"),
        files_dir.display()
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build the tokio runtime");
    let _guard = runtime.enter();

    sigil_chat::files::install(Box::new(AndroidChooser));

    let report: Shared = Shared::default();
    {
        let mut r = report.lock().unwrap();
        r.endpoint = super::platform::stored_endpoint();
        r.distributor = r.endpoint.as_ref().map(|_| "UnifiedPush".to_string());
        r.notifications = Some(super::platform::notifications_enabled());
    }
    let capabilities = capabilities(&report.lock().unwrap());
    for c in &capabilities {
        match c.support.reason() {
            None => tracing::info!("{}: available", c.name),
            Some(why) => tracing::warn!("{}: unavailable — {why}", c.name),
        }
    }

    let host = match Host::build(
        &files_dir,
        &AndroidVault,
        std::sync::Arc::new(AndroidNotifier::new()),
        capabilities,
        report,
        // The roster is written as it changes: an exchange added here is
        // there at the next launch, and after the process is killed.
        true,
        Box::new(super::platform::set_reach),
    ) {
        Ok(host) => host,
        Err(why) => {
            tracing::error!("sigil cannot start: {why}");
            return;
        }
    };

    let options = NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };
    let shell = host.shell;
    if let Err(e) = eframe::run_native(
        "sigil",
        options,
        Box::new(move |cc| {
            // A phone, said before the theme is installed, which is what
            // gives it the touch scale; and a way for the insets to ask for
            // a repaint when the keyboard comes and goes.
            sigil::Form::install(&cc.egui_ctx, sigil::Form::Phone);
            super::platform::repaint_with({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            });
            sigil::theme::install(&cc.egui_ctx, sigil::theme::light(), sigil::theme::dark());
            sigil_ui::install_loaders(&cc.egui_ctx);
            Ok(Box::new(Sigil { shell }))
        }),
    ) {
        tracing::error!("the window ended: {e}");
    }
}

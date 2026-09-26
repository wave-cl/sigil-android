//! What Kotlin calls. Every function here is named for `Native.kt`; the
//! signatures are the JNI encoding of the declarations there.

use std::ffi::c_void;
use std::time::Duration;

use jni::objects::{JClass, JObject, JObjectArray, JString};
use jni::sys::{JNI_VERSION_1_6, jint, jstring};
use jni::{JNIEnv, JavaVM};
use sigil::app::Target;
use sigil_net::Dial;
use sigil_phone::{Distributor, Window};
use sqnr_core::PubKey;

use super::bridge;
use super::platform::{self, AndroidPhone, AndroidVault};
use crate::identity::{self, Where};
use crate::settings::Settings;

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
    bridge::install(vm);
    JNI_VERSION_1_6
}

/// `Native.init(Context)`: the application context, from `SigilApp`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_init(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
) {
    if let Err(why) = bridge::set_context(&mut env, context) {
        tracing::error!("no application context: {why}");
    }
}

/// `Native.wake(filesDir, endpoint, budgetSecs)`: SIP-47's window, for every
/// exchange the identity holds, one after another, on this thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_wake(
    mut env: JNIEnv,
    _class: JClass,
    files_dir: JString,
    endpoint: JString,
    budget_secs: jint,
) -> jstring {
    let files_dir = std::path::PathBuf::from(bridge::string_from(&mut env, &files_dir));
    let endpoint = if endpoint.is_null() {
        None
    } else {
        Some(bridge::string_from(&mut env, &endpoint)).filter(|s| !s.is_empty())
    };
    super::entry::point_home(&files_dir);
    super::entry::install_logging();
    let report = wake(&files_dir, endpoint, budget_secs.max(5) as u64);
    tracing::info!("{report}");
    env.new_string(&report)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn wake(home: &std::path::Path, endpoint: Option<String>, budget_secs: u64) -> String {
    let account = match identity::ensure(&Where::under(home), &AndroidVault) {
        Ok(opened) => opened.account,
        Err(why) => return format!("no identity to wake: {why}"),
    };
    let Some(unlocked) = account.unlocked() else {
        return "the identity did not unlock".to_string();
    };
    let me = unlocked.me();
    let settings = Settings::load(&Settings::path_under(&crate::host::data_dir()));
    let prefs = sigil::Prefs::load();
    let quiet = sigil::Quiet::load();
    let accounts = sigil::Accounts::load();
    let exchanges: Vec<String> = accounts
        .all()
        .find(|h| h.account().public() == Some(me))
        .map(|h| h.exchanges())
        .unwrap_or_else(|| vec![String::new()]);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => return format!("no runtime: {e}"),
    };
    let per_exchange = Duration::from_secs(budget_secs / exchanges.len().max(1) as u64);
    // The same layers the window's chat sessions dial by: an added exchange
    // is named, the default one is whatever the identity's own SIP-38 handle
    // and `~/.sqnr/config` resolve to.
    let config = sqnr::config::Config::load();
    let mut reports = Vec::new();
    for exchange in exchanges {
        let layers = if exchange.is_empty() {
            sigil_net::discovery::layers(
                sigil_net::discovery::nothing_explicit(),
                &config,
                Some(unlocked.path()),
            )
        } else {
            vec![sigil_net::Layer {
                server: Some(exchange.clone()),
                ..Default::default()
            }]
        };
        let dial = Dial::Discover(layers);
        let mut window = Window::new(dial, unlocked.signer());
        window.budget = per_exchange;
        // **The same value the running client composes by.** Loaded from
        // sigil's preferences in this same directory rather than from the
        // phone's own file: two stores for one setting is two answers to
        // "what may a locked screen say", and the one this window used to
        // read was the one the client ignored.
        window.privacy = prefs.privacy;
        // **And what they muted.** Read from the same directory, keyed by
        // the exchange as the roster names it -- the empty string for the
        // default one, which is how the client keys a mute there too.
        window.quiet = quiet.clone();
        window.exchange = exchange.clone();
        window.distributor = endpoint.as_ref().map(|url| Distributor {
            url: url.clone(),
            ttl_secs: settings.endpoint_days.clamp(1, 30) * 24 * 60 * 60,
        });
        let phone = AndroidPhone {
            identity: me.to_string(),
            exchange: exchange.clone(),
        };
        let out = runtime.block_on(sigil_phone::window::run(window, &phone));
        reports.push(format!(
            "{}: connected={} registered={:?} new={} said={} rang={} took={:.1}s{}",
            if exchange.is_empty() {
                "default exchange"
            } else {
                &exchange
            },
            out.connected,
            out.registered,
            out.arrivals,
            out.notified,
            out.rang,
            out.took.as_secs_f32(),
            out.trouble
                .map(|t| format!(" trouble={t}"))
                .unwrap_or_default()
        ));
    }
    reports.join("; ")
}

/// `Native.endpoint(url)`: the distributor gave, changed, or withdrew it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_endpoint(
    mut env: JNIEnv,
    _class: JClass,
    url: JString,
) {
    let url = if url.is_null() {
        None
    } else {
        Some(bridge::string_from(&mut env, &url)).filter(|s| !s.is_empty())
    };
    platform::store_endpoint(url.as_deref());
    // And to the sessions: registered on their next pass, or forgotten --
    // an endpoint the distributor took back is one the exchange would go
    // on posting to until its ttl.
    sigil::wake::offer(url.clone(), crate::wake_ttl_secs());
    tracing::info!(
        present = url.is_some(),
        "wake endpoint updated by the distributor"
    );
}

/// `Native.picked(paths)`: the storage framework answered a pick.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_picked(
    mut env: JNIEnv,
    _class: JClass,
    paths: JObjectArray,
) {
    if paths.is_null() {
        platform::picked(None);
        return;
    }
    let n = env.get_array_length(&paths).unwrap_or(0);
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        if let Ok(obj) = env.get_object_array_element(&paths, i) {
            let s = JString::from(obj);
            out.push(std::path::PathBuf::from(bridge::string_from(&mut env, &s)));
        }
    }
    platform::picked(Some(out));
}

/// `Native.decline(filesDir, exchange, channelHex, seq, budgetSecs)`: refuse
/// a ring, with nothing drawn.
///
/// The ring notification's other button. Answer opens the application,
/// because a call is a screen and that is what Answer asks for; Decline
/// must not, so this runs headless like a wake window and the application
/// is never started. Returns a line for the log.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_decline(
    mut env: JNIEnv,
    _class: JClass,
    files_dir: JString,
    exchange: JString,
    channel: JString,
    seq: jni::sys::jlong,
    budget_secs: jint,
) -> jstring {
    let files_dir = std::path::PathBuf::from(bridge::string_from(&mut env, &files_dir));
    let exchange = bridge::string_from(&mut env, &exchange);
    let channel = bridge::string_from(&mut env, &channel);
    super::entry::point_home(&files_dir);
    super::entry::install_logging();
    // Negative is "not known": the running client's notification carries a
    // `Target`, which has no `seq` in it, and the decline finds the ring.
    let seq = (seq >= 0).then_some(seq as u64);
    let report = decline(
        &files_dir,
        &exchange,
        &channel,
        seq,
        budget_secs.max(3) as u64,
    );
    tracing::info!("{report}");
    env.new_string(&report)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn decline(
    home: &std::path::Path,
    exchange: &str,
    channel_hex: &str,
    seq: Option<u64>,
    budget_secs: u64,
) -> String {
    let Some(channel) = channel_from_hex(channel_hex) else {
        return format!("not a conversation: {channel_hex:?}");
    };
    let account = match identity::ensure(&Where::under(home), &AndroidVault) {
        Ok(opened) => opened.account,
        Err(why) => return format!("no identity to decline with: {why}"),
    };
    let Some(unlocked) = account.unlocked() else {
        return "the identity did not unlock".to_string();
    };
    // The same dial the window uses: a named exchange as itself, the
    // default one through whatever the identity's own handle and
    // `~/.sqnr/config` resolve to.
    let config = sqnr::config::Config::load();
    let layers = if exchange.is_empty() {
        sigil_net::discovery::layers(
            sigil_net::discovery::nothing_explicit(),
            &config,
            Some(unlocked.path()),
        )
    } else {
        vec![sigil_net::Layer {
            server: Some(exchange.to_string()),
            ..Default::default()
        }]
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => return format!("no runtime: {e}"),
    };
    let mut window = Window::new(Dial::Discover(layers), unlocked.signer());
    window.budget = Duration::from_secs(budget_secs);
    window.exchange = exchange.to_string();
    let out = runtime.block_on(sigil_phone::window::decline(window, channel, seq));
    format!(
        "declined={} connected={} took={:?}{}",
        out.declined,
        out.connected,
        out.took,
        out.trouble
            .map(|t| format!(" trouble={t}"))
            .unwrap_or_default()
    )
}

/// A conversation as a notification spells it: sixty-four hex characters.
///
/// One reader, because two would be two chances to disagree about what a
/// channel is -- and `Notifier` writes this string for every kind of
/// notification it posts, so everything coming back the other way has to
/// read it the same.
fn channel_from_hex(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

/// `Native.pressed(identity, exchange, channelHex, answer)`: a notification
/// led here. `answer` is Answer on a ring, which opens the conversation
/// **and** answers the call; an ordinary press only opens it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_pressed(
    mut env: JNIEnv,
    _class: JClass,
    identity: JString,
    exchange: JString,
    channel: JString,
    answer: jni::sys::jboolean,
) {
    let identity = bridge::string_from(&mut env, &identity);
    let exchange = bridge::string_from(&mut env, &exchange);
    let channel = bridge::string_from(&mut env, &channel);
    let Ok(identity) = identity.parse::<PubKey>() else {
        return;
    };
    let Some(bytes) = channel_from_hex(&channel) else {
        return;
    };
    platform::pressed(Target {
        identity,
        exchange,
        channel: bytes,
        answer: answer != 0,
    });
}

/// `Native.link(url)`: a `sigil://` link, **offered**.
///
/// Queued for the interface, which asks about it and waits: a link is a thing
/// somebody else wrote and put where you would press it, and the most
/// dangerous one is the least dramatic -- `sigil://room/<secret>` names a
/// room whose membership is holding the secret, and nobody can be removed
/// from one. Nothing here acts; see `sigil::deeplink`.
///
/// It was logged and dropped, which is not the same thing at all: a link
/// tapped on this phone opened sigil and then did nothing, with the reason
/// visible only in logcat.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_link(
    mut env: JNIEnv,
    _class: JClass,
    url: JString,
) {
    let url = bridge::string_from(&mut env, &url);
    match sigil_platform::deeplink::offer(&url) {
        Ok(link) => tracing::info!(
            "offered a link, awaiting confirmation: {}",
            sigil_platform::deeplink::confirmation(&link)
        ),
        Err(why) => tracing::warn!("refused a link: {why}"),
    }
}

/// `Native.theme(dark)`: the phone's light or dark, at start and on change.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_theme(
    _env: JNIEnv,
    _class: JClass,
    dark: jni::sys::jboolean,
) {
    platform::set_theme(dark != 0);
    tracing::info!(dark = dark != 0, "the phone's theme");
}

/// `Native.insets(top, bottom, left, right)`: what the system draws over the
/// surface, in pixels, whenever it changes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_insets(
    _env: JNIEnv,
    _class: JClass,
    top: jint,
    bottom: jint,
    left: jint,
    right: jint,
) {
    platform::set_insets([top, bottom, left, right]);
}

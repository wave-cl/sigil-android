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
        window.privacy = settings.privacy.into();
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

/// `Native.pressed(identity, exchange, channelHex)`: a notification led here.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_pressed(
    mut env: JNIEnv,
    _class: JClass,
    identity: JString,
    exchange: JString,
    channel: JString,
) {
    let identity = bridge::string_from(&mut env, &identity);
    let exchange = bridge::string_from(&mut env, &exchange);
    let channel = bridge::string_from(&mut env, &channel);
    let Ok(identity) = identity.parse::<PubKey>() else {
        return;
    };
    let mut bytes = [0u8; 32];
    if channel.len() != 64 {
        return;
    }
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&channel[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    platform::pressed(Target {
        identity,
        exchange,
        channel: bytes,
    });
}

/// `Native.link(url)`: a `sigil://` link, offered. Logged for now; the
/// shell's confirmation for links is the desktop's next step and this one's.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_squic_sigil_Native_link(
    mut env: JNIEnv,
    _class: JClass,
    url: JString,
) {
    let url = bridge::string_from(&mut env, &url);
    match sigil_platform::deeplink::parse(&url) {
        Ok(link) => tracing::info!(
            "offered a link, awaiting confirmation: {}",
            sigil_platform::deeplink::confirmation(&link)
        ),
        Err(why) => tracing::warn!("refused a link: {why}"),
    }
}

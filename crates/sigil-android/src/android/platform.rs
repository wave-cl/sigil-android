//! sigil's seams, over the bridge: notifications, the key store, file
//! choosing, the endpoint, and the phone the wake window talks to.

use std::sync::Mutex;

use jni::objects::JValue;
use sigil::app::{Notice, Notify, Sound, Target};
use sigil_chat::files::{Answer, Chooser, Pick};
use sigil_phone::{Notification, Phone, Ring};

use super::bridge::{self, call_static_with_context, jstring, with_env};
use crate::identity::Vault;

const CONTEXT_STRING_5_INT_BOOL: &str = "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;IZ)V";
const CONTEXT_STRING_4: &str = "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V";

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Post one message notification through `Notifier.message`.
pub fn post_message(
    identity: &str,
    exchange: &str,
    channel: Option<&[u8; 32]>,
    title: &str,
    body: &str,
    count: usize,
    sound: bool,
) -> Result<(), String> {
    with_env(|env, context| {
        let class = bridge::class("Notifier")?;
        let identity = jstring(env, identity)?;
        let exchange = jstring(env, exchange)?;
        let channel_s = channel.map(hex).unwrap_or_default();
        let channel = if channel.is_some() {
            Some(jstring(env, &channel_s)?)
        } else {
            None
        };
        let title = jstring(env, title)?;
        let body = jstring(env, body)?;
        let null = jni::objects::JObject::null();
        let channel_arg = match &channel {
            Some(c) => JValue::Object(c),
            None => JValue::Object(&null),
        };
        env.call_static_method(
            class,
            "message",
            CONTEXT_STRING_5_INT_BOOL,
            &[
                JValue::Object(context),
                JValue::Object(&identity),
                JValue::Object(&exchange),
                channel_arg,
                JValue::Object(&title),
                JValue::Object(&body),
                JValue::Int(count.min(i32::MAX as usize) as i32),
                JValue::Bool(u8::from(sound)),
            ],
        )?;
        Ok(())
    })
}

/// Present a ring through `Notifier.ring`.
pub fn post_ring(
    identity: &str,
    exchange: &str,
    channel: &[u8; 32],
    from: &str,
) -> Result<(), String> {
    let channel = hex(channel);
    with_env(|env, context| {
        let class = bridge::class("Notifier")?;
        let identity = jstring(env, identity)?;
        let exchange = jstring(env, exchange)?;
        let channel = jstring(env, &channel)?;
        let from = jstring(env, from)?;
        env.call_static_method(
            class,
            "ring",
            CONTEXT_STRING_4,
            &[
                JValue::Object(context),
                JValue::Object(&identity),
                JValue::Object(&exchange),
                JValue::Object(&channel),
                JValue::Object(&from),
            ],
        )?;
        Ok(())
    })
}

/// Whether the platform lets sigil post notifications right now.
pub fn notifications_enabled() -> bool {
    with_env(|env, context| {
        let class = bridge::class("Notifier")?;
        env.call_static_method(
            class,
            "enabled",
            "(Landroid/content/Context;)Z",
            &[JValue::Object(context)],
        )?
        .z()
    })
    .unwrap_or(false)
}

/// The window's notifier: sigil's `Notify`, over the platform's surface.
/// A press comes back through `Native.pressed` into [`pressed`].
pub struct AndroidNotifier {
    pressed: Mutex<Vec<Target>>,
}

static PRESSED: Mutex<Vec<Target>> = Mutex::new(Vec::new());

/// Called from the JNI export when the activity is opened by a press.
pub fn pressed(target: Target) {
    if let Ok(mut p) = PRESSED.lock() {
        p.push(target);
    }
}

impl AndroidNotifier {
    pub fn new() -> AndroidNotifier {
        AndroidNotifier {
            pressed: Mutex::new(Vec::new()),
        }
    }
}

impl Default for AndroidNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notify for AndroidNotifier {
    fn notice(&self, notice: Notice<'_>) -> bool {
        let (identity, exchange, channel) = match &notice.target {
            Some(t) => (t.identity.to_string(), t.exchange.clone(), Some(t.channel)),
            None => (String::new(), String::new(), None),
        };
        let result = match (notice.sound, channel) {
            (Sound::Ring, Some(channel)) => {
                post_ring(&identity, &exchange, &channel, notice.summary)
            }
            _ => post_message(
                &identity,
                &exchange,
                channel.as_ref(),
                notice.summary,
                notice.body,
                1,
                notice.sound != Sound::None,
            ),
        };
        match result {
            Ok(()) => true,
            Err(why) => {
                tracing::warn!("could not post a notification: {why}");
                false
            }
        }
    }

    fn pressed(&self) -> Vec<Target> {
        let mut out = self
            .pressed
            .lock()
            .map(|mut p| std::mem::take(&mut *p))
            .unwrap_or_default();
        if let Ok(mut global) = PRESSED.lock() {
            out.append(&mut global);
        }
        out
    }
}

/// The wake window's phone: notifications composed by `sigil_phone`,
/// posted for one identity at one exchange.
pub struct AndroidPhone {
    pub identity: String,
    pub exchange: String,
}

impl Phone for AndroidPhone {
    fn notify(&self, n: &Notification) -> bool {
        post_message(
            &self.identity,
            &self.exchange,
            n.channel.as_ref(),
            &n.title,
            &n.body,
            n.count,
            true,
        )
        .map_err(|why| tracing::warn!("could not post: {why}"))
        .is_ok()
    }

    fn ring(&self, r: &Ring) -> bool {
        post_ring(&self.identity, &self.exchange, &r.channel, &r.label)
            .map_err(|why| tracing::warn!("could not ring: {why}"))
            .is_ok()
    }
}

/// The Android key store, through `Vault.kt`.
pub struct AndroidVault;

impl Vault for AndroidVault {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        bridge::call_vault("seal", plain)?.ok_or_else(|| "the key store sealed nothing".to_string())
    }

    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        bridge::call_vault("open", sealed)?
            .ok_or_else(|| "the key store would not open the passphrase".to_string())
    }

    fn describe(&self) -> &'static str {
        "the Android key store seals the passphrase under a hardware-held key"
    }
}

/// File choosing through the storage access framework. One pick at a time:
/// the activity result comes back through `Native.picked` into [`picked`].
pub struct AndroidChooser;

static PICKING: Mutex<Option<Answer>> = Mutex::new(None);

pub fn picked(paths: Option<Vec<std::path::PathBuf>>) {
    if let Ok(mut slot) = PICKING.lock()
        && let Some(answer) = slot.take()
    {
        answer.give(paths);
    }
}

impl Chooser for AndroidChooser {
    fn pick_files(&self) -> Pick {
        let (pick, answer) = Pick::pending();
        if let Ok(mut slot) = PICKING.lock() {
            if let Some(previous) = slot.take() {
                previous.give(None);
            }
            *slot = Some(answer);
        }
        // `Files.pick(Activity)`: the activity is the NativeActivity the
        // window runs in, which android-activity's glue makes available.
        let started = with_env(|env, _| {
            let ctx = ndk_context::android_context();
            // SAFETY: the pointer is the activity object android-activity
            // handed the glue, valid for the life of the activity.
            let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
            let class = bridge::class("Files")?;
            env.call_static_method(
                class,
                "pick",
                "(Landroid/app/Activity;)V",
                &[JValue::Object(&activity)],
            )?;
            Ok(())
        });
        if let Err(why) = started {
            tracing::warn!("could not open the file chooser: {why}");
            picked(None);
        }
        pick
    }

    fn save_file(&self, name: &str) -> Pick {
        let target = with_env(|env, _| {
            let ctx = ndk_context::android_context();
            // SAFETY: as above.
            let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };
            let class = bridge::class("Files")?;
            let name = jstring(env, name)?;
            let out = env.call_static_method(
                class,
                "saveTarget",
                "(Landroid/app/Activity;Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&activity), JValue::Object(&name)],
            )?;
            let s = jni::objects::JString::from(out.l()?);
            Ok(bridge::string_from(env, &s))
        });
        match target {
            Ok(path) if !path.is_empty() => Pick::answered(Some(vec![path.into()])),
            Ok(_) => Pick::answered(None),
            Err(why) => {
                tracing::warn!("nowhere to save: {why}");
                Pick::answered(None)
            }
        }
    }
}

/// The endpoint Kotlin holds, through `Endpoint.get`.
pub fn stored_endpoint() -> Option<String> {
    with_env(|env, context| {
        let class = bridge::class("Endpoint")?;
        let out = env.call_static_method(
            class,
            "get",
            "(Landroid/content/Context;)Ljava/lang/String;",
            &[JValue::Object(context)],
        )?;
        let obj = out.l()?;
        if obj.is_null() {
            return Ok(None);
        }
        let s = jni::objects::JString::from(obj);
        Ok(Some(bridge::string_from(env, &s)))
    })
    .unwrap_or_else(|why| {
        tracing::warn!("could not read the endpoint: {why}");
        None
    })
}

pub fn store_endpoint(url: Option<&str>) {
    let result = with_env(|env, context| {
        let class = bridge::class("Endpoint")?;
        let null = jni::objects::JObject::null();
        let s = match url {
            Some(u) => Some(jstring(env, u)?),
            None => None,
        };
        let arg = match &s {
            Some(s) => JValue::Object(s),
            None => JValue::Object(&null),
        };
        env.call_static_method(
            class,
            "set",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(context), arg],
        )?;
        Ok(())
    });
    if let Err(why) = result {
        tracing::warn!("could not store the endpoint: {why}");
    }
    let _ = call_static_with_context;
}

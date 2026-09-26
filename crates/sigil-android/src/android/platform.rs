//! sigil's seams, over the bridge: notifications, the key store, file
//! choosing, the endpoint, and the phone the wake window talks to.

use std::sync::Mutex;

use jni::objects::JValue;
use sigil::app::{CallPress, Notice, Notify, Sound, Target};
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

/// `CallService.begin(context, with, identity)`: a foreground service for the
/// call.
///
/// The identity goes with the label because the notice the service stands
/// behind carries a way back into the call and a way to end it, and a press
/// on either has to say which call it meant -- this phone can hold one per
/// identity.
pub fn begin_call(with: &str, identity: &str) -> Result<(), String> {
    with_env(|env, context| {
        let class = bridge::class("CallService")?;
        let with = jstring(env, with)?;
        let identity = jstring(env, identity)?;
        env.call_static_method(
            class,
            "begin",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(context),
                JValue::Object(&with),
                JValue::Object(&identity),
            ],
        )?;
        Ok(())
    })
}

/// `Audio.setSpeaker(context, on)`: put the call on the loudspeaker, or back
/// on the earpiece.
///
/// **Returns where the sound actually goes**, not what was asked for. A tablet
/// has no earpiece, a headset takes precedence over both, and the platform can
/// refuse outright -- so a control drawn from the request rather than from the
/// answer would tell somebody their call was private when it was not.
///
/// `with_env` rather than `call_static_with_context`: that helper discards the
/// return value, and here the return value is the whole point.
pub fn set_speaker(on: bool) -> Result<bool, String> {
    with_env(|env, context| {
        let class = bridge::class("Audio")?;
        let got = env
            .call_static_method(
                class,
                "setSpeaker",
                "(Landroid/content/Context;Z)Z",
                &[JValue::Object(context), JValue::Bool(u8::from(on))],
            )?
            .z()?;
        Ok(got)
    })
}

/// `CallService.end(context)`: the call is over, let the process go.
pub fn end_call() -> Result<(), String> {
    bridge::call_static_with_context("CallService", "end", "(Landroid/content/Context;)V", &[])
}

/// `ReachService.begin(context)` / `end(context)`: keep the process alive
/// with its connection open, or let it go. The person's choice on the
/// Phone tab, and what a phone with no push distributor has instead of
/// being woken.
pub fn set_reach(on: bool) -> Result<(), String> {
    let method = if on { "begin" } else { "end" };
    bridge::call_static_with_context("ReachService", method, "(Landroid/content/Context;)V", &[])
}

/// Present a ring through `Notifier.ring`.
/// `Notifier.endRing(context, channelHex)`: take a ring off the shade.
pub fn end_ring(channel: &[u8; 32]) -> Result<(), String> {
    let channel = hex(channel);
    with_env(|env, context| {
        let class = bridge::class("Notifier")?;
        let channel = jstring(env, &channel)?;
        env.call_static_method(
            class,
            "endRing",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(context), JValue::Object(&channel)],
        )?;
        Ok(())
    })
}

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

/// Presses on the notice a live call stands behind.
///
/// A second queue rather than a `Target` with a flag on it: these arrive from
/// a broadcast receiver with no activity and no conversation in hand, and one
/// of them -- Hang up -- deliberately does not bring the window up at all.
static CALL_PRESSES: Mutex<Vec<CallPress>> = Mutex::new(Vec::new());

/// Called from the JNI exports behind the call notice's controls.
pub fn call_pressed(press: CallPress) {
    if let Ok(mut p) = CALL_PRESSES.lock() {
        p.push(press);
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
            // **The body, not the summary.** The summary of a ring is the
            // constant "Incoming call"; the body is what names the caller --
            // the conversation and who it is from. Posting the summary as
            // the ring's title threw the caller away and put a fixed phrase
            // where their name belongs, which is why a ring on this phone
            // never said who was calling.
            (Sound::Ring, Some(channel)) => post_ring(&identity, &exchange, &channel, notice.body),
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

    /// `Notifier.endRing`: the call that had never been made from anywhere.
    fn withdraw(&self, target: &Target) {
        if let Err(why) = end_ring(&target.channel) {
            tracing::warn!("could not take a ring down: {why}");
        }
    }

    /// Start and stop `CallService`, which is what keeps this process alive
    /// while a call is up.
    ///
    /// **Android stops an app that is not in front**, microphone or no
    /// microphone, unless a foreground service says otherwise -- so without
    /// this a call ends when the screen does. The service was written for
    /// exactly this and nothing had ever started it, because `Notify` had no
    /// call-began hook for it to be started from.
    ///
    /// Called on change, never on the clock: the caller compares a call being
    /// up against what it last said, so this does not start a foreground
    /// service sixty times a second.
    fn calling(&self, live: Option<sigil::InCall<'_>>) {
        let result = match live {
            Some(live) => begin_call(live.with, &live.identity.to_string()),
            None => end_call(),
        };
        if let Err(why) = result {
            // Logged, not silent: a call that the platform does not know
            // about is a call the system may stop, and the only sign would
            // be a call ending for no reason anybody can see.
            tracing::warn!("could not tell the platform about a call: {why}");
        }
    }

    /// A phone has both, and the person chooses between them here rather
    /// than anywhere else.
    fn routable(&self) -> bool {
        true
    }

    fn route(&self, speaker: bool) -> bool {
        match set_speaker(speaker) {
            Ok(got) => got,
            Err(why) => {
                // Said rather than swallowed, and the *old* route returned:
                // a control that drew what it asked for would tell somebody
                // their call had moved to the earpiece when it had not.
                tracing::warn!("could not move the call's sound: {why}");
                !speaker
            }
        }
    }

    fn call_presses(&self) -> Vec<CallPress> {
        CALL_PRESSES
            .lock()
            .map(|mut p| std::mem::take(&mut *p))
            .unwrap_or_default()
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

    fn unring(&self, channel: &[u8; 32]) {
        // Quietly: this is swept over every conversation on every wake, and
        // cancelling a notification that is not there is the ordinary case
        // rather than a fault. Only a failure to *reach* the platform is
        // worth a line, and `end_ring` says which.
        if let Err(why) = end_ring(channel) {
            tracing::debug!("could not take a ring down: {why}");
        }
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
        // `Files.pick()`. **No activity crosses this boundary.** It used to:
        // the raw `jobject` from `ndk_context` was handed to Kotlin, and
        // pressing the paperclip segfaulted the process inside `Files.pick`
        // -- a SIGSEGV rather than an exception, because a bad reference is
        // not a null one and nothing on either side could check it. Kotlin
        // tracks its own window now (`Host`), which is where an activity's
        // lifetime is actually known.
        let started = with_env(|env, _| {
            let class = bridge::class("Files")?;
            env.call_static_method(class, "pick", "()V", &[])?;
            Ok(())
        });
        if let Err(why) = started {
            tracing::warn!("could not open the file chooser: {why}");
            picked(None);
        }
        pick
    }

    fn save_file(&self, name: &str) -> Pick {
        // The application context, the same one every other glue call uses.
        // Saving needs a Context and never needed an activity, so the raw
        // pointer that crashed `pick` was never load-bearing here either.
        let target = with_env(|env, context| {
            let class = bridge::class("Files")?;
            let name = jstring(env, name)?;
            let out = env.call_static_method(
                class,
                "saveTarget",
                "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(context), JValue::Object(&name)],
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

/// What the system draws over the surface, in pixels, as `Insets.kt` last
/// reported it: top, bottom, left, right. Read every pass by the window and
/// turned into points there.
static INSETS: Mutex<[i32; 4]> = Mutex::new([0; 4]);
/// How to ask the window to look again when the insets change -- the
/// keyboard rising is an event nothing else would repaint for.
static REPAINT: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

pub fn set_insets(px: [i32; 4]) {
    if let Ok(mut i) = INSETS.lock() {
        *i = px;
    }
    if let Some(repaint) = REPAINT.get() {
        repaint();
    }
}

pub fn insets_px() -> [i32; 4] {
    INSETS.lock().map(|i| *i).unwrap_or([0; 4])
}

pub fn repaint_with(f: impl Fn() + Send + Sync + 'static) {
    let _ = REPAINT.set(Box::new(f));
}

/// The phone's theme, as the system last said: `Some(true)` dark, taken
/// once by the frame that applies it. winit reports no theme on Android,
/// so egui's "follow the system" fell to dark on every phone.
static THEME: Mutex<Option<bool>> = Mutex::new(None);

pub fn set_theme(dark: bool) {
    if let Ok(mut t) = THEME.lock() {
        *t = Some(dark);
    }
    if let Some(repaint) = REPAINT.get() {
        repaint();
    }
}

pub fn take_theme() -> Option<bool> {
    THEME.lock().ok().and_then(|mut t| t.take())
}

//! The way into Kotlin: a JavaVM stashed at load, the application context
//! stashed at start, and a few typed calls over them.
//!
//! Every call attaches the current thread, which is what makes this usable
//! from the window's tokio threads and from the wake service's thread
//! alike. Errors are strings: a JNI failure here is a bug in the glue, and
//! the caller's business is to say so and carry on.

use std::collections::HashMap;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};

static VM: OnceLock<JavaVM> = OnceLock::new();
static CONTEXT: OnceLock<GlobalRef> = OnceLock::new();
/// The glue's classes, found once from a Java thread. **A thread attached
/// from native code looks classes up through the system class loader, which
/// has never heard of this app's**: `FindClass("org/squic/sigil/Vault")`
/// from the window's thread threw ClassNotFoundException on the first
/// launch. `Native.init` runs on the main thread with the app's loader, so
/// the classes are found there and kept as global references.
static CLASSES: OnceLock<HashMap<&'static str, GlobalRef>> = OnceLock::new();

pub const PACKAGE: &str = "org/squic/sigil";

/// Every class Rust calls into. Add here before calling a new one.
const GLUE: &[&str] = &["Vault", "Notifier", "Files", "Endpoint", "CallService"];

/// Called from `JNI_OnLoad`, which runs when either the activity or the
/// wake service loads the library. Once per process.
pub fn install(vm: JavaVM) {
    let _ = VM.set(vm);
}

/// The application context and the glue's classes, from `Native.init`,
/// which runs on a Java thread. Once per process.
pub fn set_context(env: &mut JNIEnv, context: JObject) -> Result<(), String> {
    let global = env.new_global_ref(context).map_err(|e| e.to_string())?;
    let _ = CONTEXT.set(global);
    let mut classes = HashMap::new();
    for name in GLUE {
        let class = env
            .find_class(format!("{PACKAGE}/{name}"))
            .map_err(|e| format!("{name}: {e}"))?;
        let global = env.new_global_ref(class).map_err(|e| e.to_string())?;
        classes.insert(*name, global);
    }
    let _ = CLASSES.set(classes);
    Ok(())
}

/// One of the glue's classes, by its short name, as a class usable from any
/// attached thread.
pub fn class<'a>(name: &str) -> jni::errors::Result<JClass<'a>> {
    let classes = CLASSES.get().ok_or(jni::errors::Error::NullPtr(
        "no classes: Native.init was not called",
    ))?;
    let global = classes.get(name).ok_or(jni::errors::Error::NullPtr(
        "a class Rust calls is not in GLUE",
    ))?;
    // SAFETY: the global reference lives for the process; the class handle
    // made from it is only borrowed for one call and never deleted.
    Ok(unsafe { JClass::from_raw(global.as_obj().as_raw()) })
}

pub fn has_context() -> bool {
    CONTEXT.get().is_some()
}

/// Run `f` with an attached environment and the application context.
pub fn with_env<R>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> jni::errors::Result<R>,
) -> Result<R, String> {
    let vm = VM
        .get()
        .ok_or("the JavaVM was never installed (no JNI_OnLoad)")?;
    let context = CONTEXT
        .get()
        .ok_or("no application context: Native.init was not called")?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let result = f(&mut env, context.as_obj());
    // A pending Java exception would poison the next call on this thread.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
        return Err("a Java exception was thrown in the glue; see logcat".into());
    }
    result.map_err(|e| e.to_string())
}

/// A static void method on one of the glue's classes, taking the context
/// first.
pub fn call_static_with_context(
    class: &str,
    name: &str,
    sig: &str,
    args: &[JValue],
) -> Result<(), String> {
    with_env(|env, context| {
        let class = self::class(class)?;
        let mut all: Vec<JValue> = Vec::with_capacity(args.len() + 1);
        all.push(JValue::Object(context));
        all.extend(args.iter().cloned());
        env.call_static_method(class, name, sig, &all)?;
        Ok(())
    })
}

pub fn jstring<'a>(env: &mut JNIEnv<'a>, s: &str) -> jni::errors::Result<JString<'a>> {
    env.new_string(s)
}

pub fn string_from(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|j| j.into()).unwrap_or_default()
}

/// `Vault.seal` / `Vault.open`: bytes in, bytes out, or null.
pub fn call_vault(name: &str, input: &[u8]) -> Result<Option<Vec<u8>>, String> {
    with_env(|env, _| {
        let class = class("Vault")?;
        let bytes = env.byte_array_from_slice(input)?;
        let out = env.call_static_method(class, name, "([B)[B", &[JValue::Object(&bytes)])?;
        let out = out.l()?;
        if out.is_null() {
            return Ok(None);
        }
        let array = JByteArray::from(out);
        Ok(Some(env.convert_byte_array(&array)?))
    })
}

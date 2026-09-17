//! tracing to logcat, readably.
//!
//! `tracing-android` writes an event's fields straight after one another --
//! `caught up in one round tripnamed=1whole=1` -- and offers no way to
//! change that. This is the sixty lines it would take to say the same thing
//! with spaces: the level as logcat's priority, the target, the message,
//! then every field as `name=value`, which is how the desktop's log reads.

use std::ffi::{CString, c_int};

use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

pub struct Logcat {
    tag: CString,
}

impl Logcat {
    pub fn new(tag: &str) -> Logcat {
        Logcat {
            tag: CString::new(tag).expect("a tag with no NUL"),
        }
    }
}

#[derive(Default)]
struct Fields {
    message: String,
    rest: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.rest
                .push_str(&format!(" {}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.rest.push_str(&format!(" {}={}", field.name(), value));
        }
    }
}

impl<S: Subscriber> Layer<S> for Logcat {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let priority = match *meta.level() {
            Level::ERROR => android_log_sys::LogPriority::ERROR,
            Level::WARN => android_log_sys::LogPriority::WARN,
            Level::INFO => android_log_sys::LogPriority::INFO,
            Level::DEBUG => android_log_sys::LogPriority::DEBUG,
            Level::TRACE => android_log_sys::LogPriority::VERBOSE,
        };
        let mut fields = Fields::default();
        event.record(&mut fields);
        let line = format!("{}: {}{}", meta.target(), fields.message, fields.rest);
        // A NUL in a message would truncate it; replace rather than drop.
        let line = CString::new(line.replace('\0', "?")).unwrap_or_default();
        // SAFETY: both pointers are to NUL-terminated strings that live for
        // the call.
        unsafe {
            android_log_sys::__android_log_write(
                priority as c_int,
                self.tag.as_ptr(),
                line.as_ptr(),
            );
        }
    }
}

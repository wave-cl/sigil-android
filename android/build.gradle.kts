// The Android half of sigil-android: one application module whose only
// code of its own is the glue -- an activity that hosts the Rust window, a
// receiver for the wake, a service for a call, and the notifications.
// Everything that decides anything is in Rust; see ../README.md.
plugins {
    id("com.android.application") version "8.7.3" apply false
    id("org.jetbrains.kotlin.android") version "2.1.0" apply false
}

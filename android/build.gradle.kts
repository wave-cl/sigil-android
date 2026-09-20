// The Android half of sigil-android: one application module whose only
// code of its own is the glue -- an activity that hosts the Rust window, a
// receiver for the wake, a service for a call, and the notifications.
// Everything that decides anything is in Rust; see ../README.md.
plugins {
    // 8.9 is the first plugin that knows compile SDK 36; it wants Gradle
    // 8.11.1, which is what ci.yml generates the wrapper with.
    id("com.android.application") version "8.9.3" apply false
    id("org.jetbrains.kotlin.android") version "2.3.0" apply false
}

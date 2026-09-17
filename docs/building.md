# Building

## The host half

```
cargo test --workspace
```

That is CI's `host` job. It compiles sigil's crates by path from `../sigil`,
sqex's from `../sqex` through the `[patch]` in `Cargo.toml`, and sqnr from
its tag -- and runs every test in `sigil-phone` and `sigil-android`,
including the wake window against an in-process exchange. Nothing of Android
is compiled; `crates/sigil-android/src/android` is `cfg(target_os =
"android")`.

Sharing sigil's target directory saves the first build most of an hour:

```
CARGO_TARGET_DIR=../sigil/target cargo test --workspace
```

## The phone

Needs: the Android SDK with platform 35 and build-tools 35, the NDK (r26 or
newer), `cargo-ndk`, the `aarch64-linux-android` Rust target, cmake (the
Opus codec builds itself with it), and a JDK 17 for Gradle.

```
rustup target add aarch64-linux-android
cargo install cargo-ndk
export ANDROID_HOME=…/Android/sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/<version>
scripts/build-apk            # debug APK at android/app/build/outputs/apk/debug/
scripts/build-apk --release
```

On a Mac with Homebrew, the whole toolchain is four installs and one
`sdkmanager` run (which asks you to accept Google's licences):

```
brew install openjdk@17 gradle
brew install --cask android-commandlinetools
export JAVA_HOME=$(brew --prefix)/opt/openjdk@17
export ANDROID_HOME=$(brew --prefix)/share/android-commandlinetools
$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --licenses
$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --install \
    "ndk;27.2.12479018" "platforms;android-35" "build-tools;35.0.0"
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
cargo install cargo-ndk
(cd android && gradle wrapper --gradle-version 8.11.1)   # as CI does
scripts/build-apk --release
adb install -r android/app/build/outputs/apk/release/app-release.apk
```

Homebrew's own Gradle is newer than the Android plugin supports, which is
why the wrapper is generated first; `scripts/build-apk` prefers `./gradlew`
when it exists. The first release build compiles every crate for
`aarch64-linux-android` and takes minutes; after that only what changed.

The launcher icon is the mark of a key, drawn by `scripts/launcher-icon`
into `android/app/src/main/res/mipmap-*/ic_launcher.png` and committed: by
default the all-ones key (thirty-two bytes of 0x01), so the icon is a
mark like every other in the app and nobody's in particular; `scripts/launcher-icon <KEY>` draws some
identity's instead. It also writes the adaptive icon (`mipmap-anydpi-v26/ic_launcher.xml`
over `ic_launcher_layer.png`), whose layer is the mark at two thirds of
a 108dp square: a launcher masks the middle two thirds, a circle on most
phones, so the mark fills the whole of the icon's circle rather than
sitting shrunk inside it. It is a binary (`launcher_icon`) and not a
build step, so a build never needs the host half of the workspace.

`scripts/build-apk` runs `cargo ndk` for the ABI in `gradle.properties`
(`sigil.abi`, `arm64-v8a` by default), copies `libsigil_android.so` into
`android/app/src/main/jniLibs/<abi>/`, and then runs Gradle. Gradle does not
drive cargo: a build with a stale library is a build somebody forgot the
first step of, and the script refuses to run Gradle if the library is
missing.

### What the Android build depends on that is not tagged

Two upstream changes matter only on Android and are not in a tag yet:

- **sqnr** links its YubiKey backend (PC/SC) on every target but Android.
  Without it, the link fails on `libpcsclite`.
- **sqex-discovery** falls back to public validating resolvers when the
  system has no resolver configuration at all, which Android does not.

`scripts/build-apk` passes `--config` patches pointing sqnr at `../sqnr`
(sqex is already patched in `Cargo.toml` for SIP-52). CI checks out sibling
trees at `SQNR_REF` / `SQEX_REF` / `SIGIL_REF` for the same reason. Once the
tags exist -- sqnr, then sqex, then sigil, in that order -- the patches go
and the pins in `Cargo.toml` move.

### Known risks the first real build will surface

- `openh264` (C++ through `cc`) and `opus` (cmake) under the NDK toolchain;
- `sigil-update`'s `ureq`, compiled through `sigil-shell`'s library and never
  run on the phone;
- winit's soft-keyboard handling in egui;
- `cpal`'s AAudio backend under a foreground service.

None of these is in this repository's code; all of them are why the CI job
exists.

## CI

`.github/workflows/ci.yml`: `host` runs the tests; `android` cross-builds
the library and assembles a debug APK on `ubuntu-latest`, which has the SDK
and NDK preinstalled. Both jobs check out the sibling repositories the
`[patch]` and path dependencies name, at the refs given as workflow inputs
(defaulting to `main`), so the job is honest about which trees it built.

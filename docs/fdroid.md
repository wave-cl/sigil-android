# F-Droid

Three ways onto F-Droid, in increasing order of what they cost and what
they are worth. They are a sequence rather than alternatives: the first is
built here and needs nobody's permission, the last takes weeks and puts the
app in front of everyone who has F-Droid installed.

None of them needs a developer account, a fee, a hosted privacy policy, or
the twelve-testers-for-fourteen-days rule that stands between a personal
Google account and production. That is why a test version goes here first.

## What the app satisfies

Checked against the released `sigil-android-v0.1.11.apk`, not against the
build script's intentions:

- **No proprietary code.** There is no `com.google.firebase.*` and no
  `com.google.android.gms.*` in any dex.
  `org.unifiedpush.android:embedded-fcm-distributor` ships no Firebase
  itself -- its `FirebaseReceiver` only *names*
  `org.unifiedpush.android.foss_embedded_fcm_distributor.fcm.FirebaseReceiver`
  as a reflective string, and that class is not in the APK. The
  Google-namespaced code present is protobuf, gson and Tink, all
  Apache-2.0. No anti-feature flag is needed for a non-free dependency.
- **Release-signed, not debuggable.** `apksigner` reports one signer,
  `CN=Sigil upload key, O=squic.org`; `aapt2 dump badging` reports neither
  `application-debuggable` nor `testOnly`. Both are hard requirements for
  the prebuilt routes.
- **A monotonic versionCode** derived from the workspace version
  (0.1.11 -> 1011), which is what every repository orders updates by.
- **A licence file.** `LICENSE` (MIT), which is what F-Droid identifies a
  licence from -- `Cargo.toml`'s `license =` is not read by anything here.
- **Listing metadata** in `fastlane/metadata/android/en-US/`: title, short
  and full descriptions, the 512x512 icon, the feature graphic, three phone
  screenshots and a changelog per versionCode. All three routes read this
  one directory.

One limitation, worth stating rather than discovering: the APK carries
**arm64-v8a only**. That is every phone sold in the last several years and
not much else -- no armeabi-v7a, no x86_64, so no emulators and no older
hardware. `scripts/build-apk` takes the other ABIs through `sigil.abi`, at
33 MB of native library each.

## Route A: our own repository

An F-Droid repository is a directory of APKs and a signed index. Anyone who
adds its URL to their F-Droid client gets the app and its updates, with no
review and no queue. Ours is published at

```
https://wave-cl.github.io/sigil-android/fdroid/repo
```

and `fdroid/index.html` is the page that tells somebody how to add it.

**The APKs are not built for it and are not in the git tree.** They are the
artefacts of the GitHub releases, downloaded as they were signed, so what
F-Droid installs is byte-for-byte what a release attached. The repository
key signs only the *index*; the APK keeps the upload key's signature, which
is why an install from F-Droid and an install from a release upgrade over
each other instead of colliding.

Three pieces:

- **`scripts/fdroid-key`** makes the repository key once, into
  `~/.sigil-fdroid-key.p12` with a random passphrase in
  `~/.sigil-fdroid-key.pass` beside it (both 0600). **A repository's key
  cannot be rotated** -- clients pin it when they add the repository, and a
  new key is a new repository everybody must add again. Back it up with the
  upload key.
- **`scripts/fdroid-repo OUT APK...`** assembles and signs the index, and
  then asserts what it produced: that the newest version is in it, that
  every version carries a changelog, that the licence is MIT and that the
  screenshots arrived. A run that quietly produced an empty listing would
  otherwise look exactly like one that worked.
- **`.github/workflows/fdroid.yml`** runs it on every published release
  (or by hand, taking the most recent N), and deploys to Pages. It needs
  two secrets:

  | Secret | What |
  |---|---|
  | `SIGIL_FDROID_KEYSTORE_B64` | `base64 -i ~/.sigil-fdroid-key.p12` |
  | `SIGIL_FDROID_KEYSTORE_PASSWORD` | the contents of `~/.sigil-fdroid-key.pass` |

`fdroid` itself comes from `scripts/fdroid-venv`, which pip-installs
fdroidserver into `~/.cache/`. Homebrew's bottle of it wants to build GCC
from source on macOS, which takes hours; pip takes a minute.

Two things about fdroidserver that cost time and are not in its docs:

- `repo_icon` is checked as a path **relative to the working directory**,
  not under `repo/icons/` as its own warning message claims.
- A per-version changelog is attached only to a version listed in the app's
  `Builds:`. Nothing populates that in a binary repository, so every
  changelog is **silently dropped** unless the versions are written into it
  -- which `scripts/fdroid-repo` does, from the APK filenames.

Route A's weakness is discovery: nobody finds the app, they are sent to it.

## Route B: IzzyOnDroid

A third-party repository that many F-Droid users already have added, which
takes **prebuilt APKs from GitHub releases** -- it does not build from
source, so none of Route C's toolchain problems apply. Its bar is the
fastlane metadata, a FOSS licence, a release-signed APK that is not
debuggable, and a monotonic versionCode: everything in *What the app
satisfies* above.

Submission is an "App Submission" issue on the IzzyOnDroid tracker giving
the app name, the licence, the source URL and the summary. After that each
GitHub release is picked up automatically.

This is the cheapest route to an audience, and the usual stepping stone to
Route C.

## Route C: f-droid.org proper

The main repository builds every app **from source on its own buildserver**
and signs the result with F-Droid's key -- which is why an app there is
trusted, and why this is the hard one. Two specific obstacles:

1. **Gradle does not drive cargo here.** `scripts/build-apk` runs
   `cargo ndk` and *then* Gradle, deliberately. F-Droid runs Gradle itself,
   so the metadata needs a `prebuild:` that installs the pinned Rust
   toolchain (`rust-toolchain.toml`), adds the Android target, installs
   `cargo-ndk` and fetches dependencies -- the build proper runs
   `--locked --offline` with no network.
2. **Three sibling git dependencies.** sigil, sqex and sqnr are pinned to
   tags rather than paths, which is what makes this possible at all;
   `cargo fetch --locked` in the prebuild has to pull them too.

An app in Route C is signed by F-Droid, not by us, so it is a *different*
signature from Routes A and B: a user cannot upgrade across, they must
uninstall and reinstall. Worth knowing before pointing anyone at both.

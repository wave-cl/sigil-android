# sigil-android

[sigil](https://github.com/wave-cl/sigil) on a phone: the same voice calls and
end-to-end encrypted chat over sQUIC, as a **device** of an account whose key
lives somewhere else, on a platform that lets an application run only when it
says so.

It is sigil's own crates -- `sigil-chat`, `sigil-admin`, the `sigil-shell`
library -- drawn by eframe's Android backend, so what the desktop can do the
phone can do, by construction rather than by porting. What a phone adds is
around them, and is what this repository is:

- **The wake window** (`crates/sigil-phone`, [SIP-47](https://github.com/wave-cl/sips/blob/main/sip-0047.md)).
  A phone cannot hold the exchange's event stream open. It leaves a push
  endpoint with the exchange ([SIP-45](https://github.com/wave-cl/sips/blob/main/sip-0045.md)),
  is woken with four content-free bytes, and in the seconds it is allowed
  connects, registers, catches up, **writes its store**, and only then says
  anything -- because opening a message spends a one-time prekey, and a phone
  suspended between showing and writing would hold a notification for a
  message nothing can open again.
- **Catching up in one round trip** ([SIP-52](https://github.com/wave-cl/sips/blob/main/sip-0052.md)),
  built into sqex and sigil as part of this work: every channel's new
  entries and the keys that open them, in one request, so the window is a
  handshake and a round trip rather than a round trip per conversation.
- **Pairing.** The phone shows `sqx-device:<key>`; a device holding the
  account registers it and shows `sqx-pair:<account>@<domains>`; the phone
  goes there, finds itself in the account's device list with the credential
  that was presented, and only then treats the account as its own. Two
  public strings, no secret in either. See [docs/pairing.md](docs/pairing.md).
- **Notifications composed on the phone**, from plaintext only it opened,
  under a setting of how much they say -- who and what, who only, or only
  that something arrived. Nothing composed leaves the phone. See
  [docs/wake.md](docs/wake.md).
- **The key store.** The phone's key is an ordinary sqnr identity file, sealed
  under a passphrase nobody is shown, which the Android key store seals
  under a hardware-held key.

## Status

**Early, and not yet built for a phone.** Everything that decides anything
lives in `sigil-phone` and the host half of `sigil-android`, and is tested on
a desktop against a real in-process exchange: the wake window end to end
(woken by the exchange, writes before it says, says once, counts, the quiet
setting, a ring), pairing strings, notification composition, the identity
and its vault. The Android-only glue -- `android_main`, the JNI, the Kotlin
-- is written and has not been compiled: this machine has no Android SDK.
The CI workflow builds it on a runner that has one. See
[docs/building.md](docs/building.md) and [docs/design.md](docs/design.md)
for the parity matrix and what is and is not proven.

## Layout

```
crates/sigil-phone     SIP-47 as code, tested against a real exchange on a desktop
crates/sigil-android   the host: window builder, Phone tab, identity, settings;
                       android/ is the entry point, JNI and platform seams
android/               the Gradle project and the Kotlin glue
docs/                  design, wake, pairing, building
scripts/               build-apk, check
```

## Building

The host half: `cargo test --workspace` on a desktop, exactly as CI runs it.
The phone: `scripts/build-apk`, which needs the Android NDK and
`cargo-ndk`; [docs/building.md](docs/building.md) says how, and what the
build depends on that is not tagged yet.

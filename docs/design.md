# Design

## The one decision

sigil's crates run on the phone. eframe 0.36 has an Android backend (winit's
`android-native-activity`), egui draws to a surface, and `sigil-shell`'s
library is a shell over a list of `sigil::App`s that knows nothing about
where it is drawn. So the chat app, the admin console, the transcript, the
call views, the devices pane, verified contacts, succession, multi-home
channels -- all of it -- is the desktop's code, and parity is a property of
the build rather than a list of screens to catch up on.

What that costs: egui's soft-keyboard behaviour on Android is winit's, and
winit's is basic (it shows the keyboard on focus; IME composition is not
handled); a phone-width layout is what `sigil-chat` already does for a narrow
window, which was designed for a desktop window narrowed and may want more.
Both are known and neither is a reason to write a second client.

## What a phone adds

| Need | Where | Status |
|---|---|---|
| Be woken when asleep (SIP-45) | `WakeReceiver.kt` → `WakeService.kt` → `Native.wake` → `sigil_phone::window` | window proven on a desktop; JNI unbuilt |
| The window's order: connect, register, catch up, write, *then* say (SIP-47) | `sigil_phone::window::run`, `Outcome::steps` | proven against a real exchange |
| Catch up in one round trip (SIP-52) | `sqexd::catchup`, `Chat::catchup`, `sigil_chat::session::catch_up` | proven; byte-equal to `fetch`/`key/get` |
| Notifications from plaintext, under a setting | `sigil_phone::notify`, `Notifier.kt` | composition proven; posting unbuilt |
| A ring on a locked screen | `Notifier.ring` with a full-screen intent | unbuilt |
| Pairing, both halves | desktop: `sqx-pair:` in Chat › Devices; phone: `Cmd::ClaimAccount`, `Chat::claim_listed` | compiled; claim untested end to end |
| The device key in the key store | `identity::ensure` + `Vault.kt` | vault seam proven with fakes; Keystore unbuilt |
| A distributor the person chooses | UnifiedPush connector; embedded FCM distributor as fallback | wired in Gradle; unbuilt |
| A call with the microphone open, visibly | `CallService.kt` | **written, not started by anything**: the shell exposes no call-began hook yet |
| Files to attach, from the storage framework | `sigil_chat::files::Chooser` → `Files.kt` | seam proven on desktop; Android arm unbuilt |
| Saving a file | app's Downloads directory, no dialog | v1 |
| `sigil://` links | `MainActivity` → `Native.link` | logged, not yet confirmed and acted on |
| Self-update | none: the store or the APK | by design |
| Tray, global shortcuts, start at login, launcher badge | `sigil-platform` Android arms | report absent, with why |

## Presence, and what the phone cannot say yet

A phone beats (SIP-4) as its device key, and nobody reads that: a contact
is read by account. [SIP-50](https://github.com/wave-cl/sips/blob/main/sip-0050.md)
answers an account's read from its devices and adds a bit saying a wake
would land; until an exchange implements it, a person whose only client is
this phone reads as absent to everybody. Likewise
[SIP-51](https://github.com/wave-cl/sips/blob/main/sip-0051.md): a desktop
handing this phone history (SIP-42) needs both open at once until the
exchange wakes the phone for a sibling's open. Both are Draft and
unimplemented; both are what the phone needs next from the exchange.

## What is proven and what is not

Proven, by tests on a desktop against an in-process `sqexd` with a loopback
distributor standing in for the push service:

- the exchange wakes the endpoint the phone left, with `wake`;
- a window with nothing new says nothing; one with a message says it once,
  after the store holds it, and the next window does not say it again;
- three messages are one notification naming the newest with a count; the
  quiet setting names neither sender nor conversation;
- a call rings;
- `catchup` is byte-for-byte what `fetch` and `key/get` answer; a stranger's
  and a nonexistent channel are one answer; a small budget defers rather than
  describes; an anonymous caller is refused;
- the identity is made once and opened again; a lost key store refuses
  rather than replaces.

Not proven, and said so:

- anything on an actual phone: the JNI, the Kotlin, the Keystore, the
  soft keyboard, audio through AAudio, the FCM bridge;
- the pairing claim end to end (it is compiled; the test that registers a
  phone from a desktop session and claims it is the next one to write);
- a wake window's cost on a cellular radio, as opposed to on loopback.

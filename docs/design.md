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

- on an actual phone, audio through AAudio and the FCM bridge. (The JNI,
  the Kotlin, the Keystore, the insets and the soft keyboard were seen
  working on a OnePlus NE2213 on Android 16: the app connects to
  trunk.exchange, the app bar starts under the status bar, the composer rises
  above the keyboard, a long press opens a message's menu.)
- the pairing claim end to end (it is compiled; the test that registers a
  phone from a desktop session and claims it is the next one to write);
- a wake window's cost on a cellular radio, as opposed to on loopback.

## Layout on a phone

The desktop is left as it is. One value tells sigil what it is running in:
`sigil::Form { Desktop, Phone }`, set once by the host and read from the egui
context; the desktop never sets it. Layout stays width-driven -- a narrow
desktop window collapses to one pane the same way a phone does, from
`sigil::layout` -- and `Form` decides only what a phone *is* that a narrow
window is not:

- the system's bars and keyboard lie over the surface, so the shell keeps
  everything clear of `Insets` the host supplies (`Insets.kt` listens for
  the platform's answer and hands it to Rust in pixels; the keyboard is a
  bottom inset, which is what lifts the composer above it);
- the title strip is an app bar, a finger tall, under the status bar. Its
  head is the app's (`App::head_ui`): over the list, the identity's mark
  with its presence dot and then "Sigil"; over a conversation, Back and
  the conversation's name, with the conversation's More in the corner --
  one bar, where the desktop's conversation bar would have been a second
  one under it;
- there is no rail: the screen is the app's, and the other apps (the
  console, the phone's own settings) are a menu on the app bar's title
  (the product's name over the home app, the app's name elsewhere), which
  a phone that only chats never opens;
- every icon button's target is a finger's (`Form::button_size`), with the
  theme's touch scale: taller controls, more slop around a target, a
  floating scroll bar, body text a size up;
- dialogs and fields are bounded by the width there is, which also fixes a
  narrow desktop window.

Touch is not a form. Whether a finger has been seen is egui's own
`has_touch_screen()`, and it turns on what hover cannot do: a tap on a
message reveals its actions and a tap elsewhere puts them away; a press
held still opens a menu of them where the finger is; the picture viewer
pans by drag and zooms by pinch. A desktop with a touchscreen gets all of
it.

A narrow pane also folds the conversation bar: everything but Back, the
name and the identity goes behind one More button, the call first. Six
controls beside the identity were wider than the row, and a right-to-left
row that overflows pushes Back off the left edge and drags the transcript
after it. The same fold serves a desktop window pulled in.

Every row of the transcript has to fit the pane, not only look as if it
does: egui grows a ui to whatever is drawn in it, so one row 35 points too
wide -- a file's name beside its Save button, which a `horizontal` never
wraps -- laid out every message after it for a pane that wide. A wrapped
message then ran off the right edge and one's own were right-aligned to
an edge past the screen. The phone is 360 points across (1080 pixels at
3×); the test harnesses are sized from it, not from a Pixel's 412, which
is the width at which every test passed while the phone overflowed.

The phone's Back button closes whatever menu is open; with none open it
steps back through the shell's history, then asks the app for a step
(`App::back`: the chat closes an open conversation for the list), and
failing that is Escape, which closes a viewer or a dialog. winit hands
it to egui as BrowserBack, which nothing had looked at. In a bubble the
time is always the furthest right, with a receipt, "edited" or where
the message came from to its left; and on a phone a row inside a bubble
is as tall as its line, not the finger-tall control the theme makes
every other row, so a name sits close to its words. The same holds in the chat list, where
the name and the last words are also centred on the mark beside them
(egui lays out in one pass, so their height is remembered from the last
pass and the padding above them is half of what is left). The identity's mark in the app
bar is a size down from the desktop's, and the words the desktop shows
beside it about a link being down or retrying are not shown on the
phone: the dot on the mark says it, and the session reconnects itself.

Known limits: the soft keyboard delivers key events only (no IME
composition, so CJK and swipe typing do not commit -- a NativeActivity
limit winit does not bridge); the QR is drawn, not scanned.

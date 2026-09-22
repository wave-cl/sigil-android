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

This table said "unbuilt" against four things the phone visibly does, for
weeks after they started working. Each row now says what was seen and where,
so a row nobody has checked reads differently from one somebody has.

| Need | Where | Status |
|---|---|---|
| Be woken when asleep (SIP-45) | `WakeReceiver.kt` → `WakeService.kt` → `Native.wake` → `sigil_phone::window` | window proven on a desktop; the JNI is built and exported; **no wake has ever arrived**, for want of a distributor (below) |
| The window's order: connect, register, catch up, write, *then* say (SIP-47) | `sigil_phone::window::run`, `Outcome::steps` | proven against a real exchange |
| Catch up in one round trip (SIP-52) | `sqexd::catchup`, `Chat::catchup`, `sigil_chat::session::catch_up` | proven; byte-equal to `fetch`/`key/get` |
| Notifications from plaintext, under a setting | `sigil_phone::notify`, `Notifier.kt` | **posting works on the device** (2026-09-19); the Phone tab reports the permission as granted |
| A ring on a locked screen | `Notifier.ring` with a full-screen intent | **works on the device** (2026-09-19): it names the caller, Answer answers, and it is withdrawn when the call goes |
| Pairing, both halves | desktop: `sqx-pair:` in Chat › Devices, `Cmd::LinkDevice` (writes the credential **and registers the phone**, `Chat::register_device`, sqex 0.104.2); phone: `Cmd::ClaimAccount`, `Chat::claim_listed` | **both halves work, end to end in a test** (2026-09-22): the phone, told only where to go, finds itself listed and takes the account. Not yet seen on the phone |
| The device key in the key store | `identity::ensure` + `Vault.kt` | **works on the device**: the app opens its own identity, and the Phone tab reports the key store as present |
| A distributor the person chooses | UnifiedPush connector; embedded FCM distributor as fallback | **still nothing installed** -- but the Phone tab now says what a distributor is and offers two links to get one, where before it named ntfy in prose and stopped. The embedded bridge has no gateway, so it no longer offers itself as one |
| A call with the microphone open, visibly | `CallService.kt`, `Notify::calling` | **wired**: sigil's `Notify` gained a call-began hook, the chat app says so on change, and the Android arm starts and stops the service. Tested on a desktop through `update`; **not yet seen on the phone** |
| A call to or from somebody at another exchange (SIP-39) | `sigil_net::spawn_cross_call` / `spawn_cross_answer`, the chat's ring card and its notification | **answered on the phone** (2026-09-22): rung from squic.org by `sqex-voice call a@trunk.exchange`, the card over the list and the shade both offered Answer, and each connected -- `SessionUp` on a bridged id, media both ways, 0.0% loss. The first live ring found three faults the tests had not: the shade's Answer opened a conversation that did not exist and answered nothing; the card's buttons were painted over the key at 360 points; and a ring over the list drew nothing at all. All three fixed and each has a test now. **Placing one is the conversation's call button**: somebody at another exchange is reached the SIP-60 way (*New conversation*, `name@domain`), and a call placed in that conversation rings their session at their exchange and connects on the connections both sessions hold — `a_call_inside_a_conversation_across_exchanges_connects` in sigil, against two federated exchanges. No shell loads a Calls app, so the bridge-by-name field is the command line's |
| Succession (SIP-44): a will, guardians, a vouch, the claim | Chat › Devices › *If you lose your key*, over `Chat`'s succession methods (sqex 0.104.3) | **built**, both ways proven through the pane's commands against a real exchange; the phone is the device most likely to be the one that is lost, and it had no terminal for this. **The pane was seen on the phone** (2026-09-22, the 04:12 build): all four sections, every field's action a mark at its right. Seeing it is also what found that Devices was reachable only from inside a conversation; it is on the identity menu now. Nothing has been signed from the phone yet |
| Files to attach, from the storage framework | `sigil_chat::files::Chooser` → `Files.kt` | **works on the device** (2026-09-19), once the picker was started from the main thread |
| Saving a file | app's Downloads directory, no dialog | v1 |
| Links open (hyperlinks anywhere in sigil) | eframe's `links` feature; `webbrowser` via `ndk-context` | **seen on the device** (2026-09-22): *Get ntfy* on the Phone tab opened Play |
| `sigil://` links | `MainActivity` → `Native.link` → `sigil_platform::deeplink::offer` → the shell's question | **offered and acted on**: the shell asks (`deeplink::confirmation`), and on yes the Calls app calls or joins the room and Chat opens the contact. Nothing happens on no, or on pressing away from it |
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
- the pairing claim on the actual phone. It was unprovable for a reason
  that is worth keeping: the exchange took `/device/register` from "the
  delegate itself, or an already-registered device of the same account",
  and the client had one method, `register_self`, which is the first kind.
  Nothing anywhere posted the second, so a phone that showed its key and was
  named by a desktop was never in the account's list, and its claim could
  only succeed for a device that had already registered -- which by then did
  not need to claim. `Chat::register_device` (sqex 0.104.2) is the second
  kind, `LinkDevice` calls it after writing the credential, and
  `a_device_named_by_its_sibling_claims_the_account_by_name_alone` in
  sigil's `chat_session` is the whole of it, with the refusal as its control.
  Scanning the QR on the desktop is still a paste: the QR is drawn, not read;
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

**One bar, and the app bar is it.** The shell already drew a pushed view's
name there, from `App::nav_title` -- Devices, Members, Public channels,
Channel settings -- and each of those four drew its own Back and its own
heading again in the pane under it. The name was on the screen twice, under
two back arrows, on the four panes with the least room to spare. The bar
carries the name and the way back now, as it already did for an open
conversation, and a view's one action goes to the corner beside where the
conversation's More is (`App::chrome_ui` is handed the history entry so a
view can tell it is the one being drawn). The desktop keeps its own heads:
its panes are columns with no bar over them.

Nothing failed while that was wrong, and the reason is worth keeping: the
chat tests' phone harness composed the app bar *its own way* -- the app's
head, then the product's name -- and never asked `nav_title`. Every phone
snapshot showed "Sigil" over a pane whose real bar says "Devices". A harness
that builds the thing under test differently from the shell proves things
about a screen nobody will ever see.

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

That rule is a test now rather than a habit, in four places: the five chat
routes, the operator console, the Calls pane, and `sigil-ui`'s own list
widgets. Each draws into a 360-point pane and asserts what the ui came out
as -- which is the fault itself, in one number, and needs no renderer, so it
runs in an ordinary `cargo test` rather than in the snapshot job somebody
runs before a release. The failure names the route and the number: "Members
draws 422 points wide in a 360-point pane" is actionable and a red snapshot
is not.

It was written after a day of finding the same bug by eye. Members carried
five buttons per member, some 200 points over, so they were painted across
the member above and the reports below began off the left edge. The console
and the Calls pane asked for boxes 420 and 260 points wide, and the prose
around them then wrapped to the overflowed width and was clipped mid-word by
the pane. The roster's line about the path -- "2.1% lost, 180 ms of buffer,
concealing 3 frames in 100" -- sat at the end of a `horizontal` and took a
row to 572.

**Run it on data as long as data really gets.** The fixtures were "Ada" and
"notes.txt": nothing in them is longer than a phone, so every pane passed
while a display name, a camera's filename or a URL would have torn it apart.
Lengthening the fixture found two more immediately, both of the same shape:
`truncate` shrinks a label to what is available *at the moment it is added*,
so a name given the whole row takes the whole row and whatever follows it
goes past the edge. The fix each time is to lay out the fixed part first and
give the variable part the remainder -- measured, not guessed.

Two faults were found only on the phone, and neither had a failing test
until the device said what shape it was. A tap on the app bar revealed a
message's action strip, because the strip's hit test ran `Rect::contains` on
the bubble's layout rect and a message scrolled behind the bar still has
one. And every capability this phone *has* drew as a tofu box, because the
filled disc it was marked with is not in egui's bundled font: the one state
somebody opens that pane to confirm read as a rendering fault.

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

The search box and the composer are the whole width on a phone, a little
taller, with their one control inside the box at the right -- the
magnifier, the paperclip -- rather than beside it; there is no Send
button, because the keyboard's own key sends. The launcher icon is the
mark of the all-ones key (see building.md).

Calls were checked on the phone against a desktop: the microphone and
notification permissions are runtime permissions, asked for at the
start (nothing asked, so no call could open the microphone and no
notification was ever posted); the file picker is started on the main
thread (from Rust's own thread it crashed inside the framework); a Rust
panic is written to logcat before the abort; and every call event is
logged, since a call that ended silently had nothing to read.

A dialog cannot scroll -- egui 0.36 makes a `ScrollArea` inside a `Modal`
dismiss the dialog on any press inside it, with every `auto_shrink` and inside
a sensing scope of its own -- so a dialog that does not fit is simply gone,
and a `Modal` being centred means it goes off the top as much as the bottom.
Every one of the six is measured against both edges, upright and sideways.
Verify was the one that did not fit sideways, at 482 points on a 360-point
screen; it is given a wider box on a short screen and its code and key go
beside the words rather than under them.

**A link is a control, which it was not.** `eframe`'s `links` feature is in
its defaults and both workspaces build it with `default-features = false`, so
`egui-winit` compiled `open_url_in_browser` to a log line: every hyperlink in
sigil opened nothing, on the phone and on the desktop, in silence. There is no
failing render and no error -- a link that does nothing looks exactly like one
nobody pressed. `tests/links.rs` in each workspace asks `cargo tree -i
webbrowser`, which walks the *resolved* graph; the first version asked `cargo
metadata`, which lists the lock file and passed with the feature taken back
out.

Known limits: the soft keyboard delivers key events only (no IME
composition, so CJK and swipe typing do not commit -- a NativeActivity
limit winit does not bridge); the QR is drawn, not scanned.

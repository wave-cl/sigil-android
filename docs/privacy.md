# Sigil for Android: privacy

*Effective 2026-09-18. This page is what Google Play links to as the
application's privacy policy.*

Sigil is a client. It runs on your phone and talks to an **exchange**: a
server you or somebody else operates, which your account is on. Nothing in
Sigil talks to the people who publish it.

## What stays on the phone

- **Your device's key.** Made on the phone the first time Sigil runs, and
  never leaves it. It is stored as an encrypted file whose passphrase is
  held by the Android key store, in hardware where the phone has it.
- **Your messages, files and settings**, in a database in the
  application's private storage. Android's backup is turned off for it, so
  none of this is copied to a Google account.
- **Notifications** are composed on the phone from messages only it can
  decrypt, under your setting of how much they say. The text never leaves
  the phone.

Uninstalling Sigil deletes all of it.

## What goes to the exchange

Everything Sigil sends, it sends to the exchange your account is on, over an
encrypted transport (sQUIC). The exchange sees:

- **Which device connected, and when**: your device's public key, the
  account it belongs to, and your network address while connected.
- **Where to wake you**: the push endpoint you registered, so it can send
  the word `wake` when something arrives for you. That is the whole of what
  it sends by push.
- **Who talks to whom**: which conversations your account is in, and when
  entries are posted to them.
- **The messages themselves, encrypted end to end.** The exchange stores and
  forwards them and cannot read them; only the devices in the conversation
  hold the keys. Voice calls are likewise encrypted between the devices
  and are not stored.

What the exchange keeps and for how long is the operator's decision, not
Sigil's. If you use an exchange we operate, its own terms apply; if you run
your own, you are the operator.

## Push notifications

To be woken while asleep, Sigil registers with a **UnifiedPush
distributor** of your choosing, installed separately (ntfy, for one). The
distributor learns *when* your exchange has something for you, and nothing
of what. Sigil does not use Google's push service unless a build says so on
this page.

## Permissions

- **Microphone**: voice calls. Asked when Sigil starts, used only during a
  call, and the ongoing-call notification shows when it is open.
- **Notifications**: messages and incoming calls.
- **Full-screen notifications**: an incoming call over the lock screen.
- **Internet**: the exchange.

Sigil does not read your contacts, your location, your camera, or anything
else on the phone.

## What we collect

Nothing. Sigil has no analytics, no crash reporting, no advertising, and no
account with us. We do not receive, and cannot produce, any data about your
use of it.

## Changes and contact

Changes to this page are made in the repository that publishes it:
https://github.com/wave-cl/sigil-android/blob/main/docs/privacy.md. Questions
to <!-- the developer account's contact address -->.

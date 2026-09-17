# Being woken, and what is said

## The path

1. The exchange has an event for this device and the device holds no stream.
   It POSTs `wake` -- four bytes, nothing else -- to the endpoint the device
   registered (SIP-45), at most once per 30 s while the device stays away,
   at once for a ring.
2. The endpoint belongs to a UnifiedPush distributor the person installed
   (ntfy, for one), or, with none installed, to the application's own
   embedded FCM distributor, whose endpoint is at a rewrite proxy the
   publisher runs. Either delivers to `WakeReceiver`.
3. `WakeReceiver` starts `WakeService`, a short foreground service. The
   platform shows the quiet notice ("Checking what arrived") while it runs.
4. `WakeService` calls `Native.wake(filesDir, endpoint, budget)`, which runs
   `sigil_phone::window::run` for each exchange the identity holds, one
   after another, each with a share of the budget.
5. The window: connect and subscribe; register the endpoint again; the
   session tops up prekeys and catches up (SIP-52) and writes the store; wait
   until nothing new has arrived for a moment; compose notifications from
   what the session reports as new to this device; post them; close the
   stream; return. `Outcome::steps` is the order, and the test asserts it.

## What is said

`sigil_phone::notify::compose`, under the person's setting:

| Setting | Title | Body |
|---|---|---|
| who wrote, and what they said | the conversation | `alice: the words · and 2 more` (a direct message: the words alone) |
| who wrote, and nothing of what | the conversation | `alice sent a message · and 2 more` |
| only that something arrived | `Sigil` | `3 new messages`, one notification for everything, naming no conversation |

One notification per conversation per window, keyed on the conversation so
a later window replaces rather than stacks. A window that finds nothing new
posts nothing. A ring is posted through the calls channel with a
full-screen intent, so it reaches a locked screen; answering opens the
window on the conversation, where the call is answered exactly as on a
desktop.

What the phone considers *new* is what its store did not hold when the
window opened -- `ChatHandle::unseen`, added to sigil-chat for this. The
desktop's `arrivals` is "what arrived while I watched", which for a phone
woken from sleep is nothing.

## The distributor bridge

On a phone with no UnifiedPush distributor, the embedded FCM distributor
needs a gateway: an HTTPS service that takes the exchange's POST at
`https://<gateway>/wpfcm?t=<token>`, forwards it to Firebase Cloud
Messaging as a **high-priority data message** carrying the body, and answers
404 for a token FCM no longer knows so the exchange forgets it (SIP-45).
This is UnifiedPush's own gateway shape and their reference gateway serves
it; SIP-47 §The distributor says what such a bridge may and may not do. Set
its address as the Gradle property `sigil.wakeProxy` (in
`android/gradle.properties` or `-Psigil.wakeProxy=…`) and put the Firebase
project's `google-services.json` in `android/app/`. With neither, no
embedded distributor is offered and the Phone tab says one is needed.

SIP-45's wake carries no VAPID authorisation, so the gateway must accept a
bare POST; the distributor is configured with an empty VAPID key.

What the bridge learns is what SIP-45 lets any distributor learn: *when*,
per token. It is handed nothing to forward but the word `wake`.

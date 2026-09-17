# Pairing a phone

SIP-47 §Pairing, as the two clients do it.

1. **On the phone**, the *Phone* tab shows `sqx-device:<key>` as text and as
   a QR. That is the phone's device key, made on first run and kept by the
   key store. It knows nothing else yet.
2. **On a device that holds the account** -- a desktop sigil -- open Chat ›
   Devices, put the phone's key in *Its key* (the `sqx-device:` prefix is
   accepted), and press *Write credential*. sigil registers itself first if
   it never had, issues a 90-day credential naming the phone, and registers
   it at the exchange. Below the credential it now shows
   `sqx-pair:<account>@<domain>[,<domain>]` with a QR: the account, and every
   exchange this identity holds a session at.
3. **On the phone**, Chat › Devices › *Where the other device sent you*: scan
   or type the `sqx-pair:` string (or a `name@domain` you know) and press *Go
   there*. The phone adds each domain as an exchange, opens a session at each,
   and asks each to list the account's devices. It must find itself there,
   with the credential the desktop presented; then it records the account as
   its own, publishes prekeys under its key, and collects whatever its
   siblings have already sealed to it.
4. History follows over SIP-42 when both devices are open at once -- which,
   until an exchange implements SIP-51, means keeping the phone in front
   while the desktop has an open outstanding toward it.

Neither string is a secret. A device key shown to the wrong client gets a
credential from the wrong account, which the phone refuses at step 3 because
it is not in *its* account's list. A pairing string shown to the wrong phone
names an account and a domain, both public, and that phone is in no list.

**Unfinished:** step 3 is compiled and not yet tested end to end; a test
that registers a phone from a desktop session and claims it is the next one
to write. The QR is drawn but not scanned -- the phone has no camera view
yet; the string is typed or pasted.

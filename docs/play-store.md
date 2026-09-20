# Google Play

What it takes to put sigil-android on the Play Store, in the order it
happens. The build side is done in this repository; the console side is a
person's, and the last section is what that person has to type.

## What the build does

- **Target SDK 36.** Play takes nothing older than Android 16 for a new app
  or an update since 31 August 2026 (`compileSdk`/`targetSdk` in
  `android/app/build.gradle.kts`; the platform in `ci.yml`'s sdkmanager
  line moves with them).
- **A bundle, not an APK.** `scripts/build-apk --bundle` produces
  `android/app/build/outputs/bundle/release/app-release.aab`, which is the
  only thing Play accepts for a new app. The APK from `--release` is for a
  phone over adb.
- **The upload key.** Play app signing holds the key an APK reaches a phone
  under; ours only signs uploads. `scripts/upload-key` makes it (OpenSSL,
  PKCS12; this machine has no Java) and `build.gradle.kts` reads it from
  four environment variables, all or none:

  ```
  SIGIL_UPLOAD_KEYSTORE            path to the .p12
  SIGIL_UPLOAD_KEYSTORE_PASSWORD
  SIGIL_UPLOAD_KEY_ALIAS           sigil-upload
  SIGIL_UPLOAD_KEY_PASSWORD        the same as the keystore's
  ```

  Without them a release build is signed with the committed debug key --
  fine for adb, refused by Play -- and `--bundle` refuses to run at all.
- **One version.** `versionName` is `[workspace.package] version` in
  `Cargo.toml`; `versionCode` is derived from it (`0.1.0` → 1000, `1.2.3`
  → 1002003). Play refuses an upload whose code does not exceed the last,
  so a release that forgot to bump fails at upload rather than shipping
  under the old number. Bump the version, tag `vX.Y.Z`, done.
- **`.github/workflows/release.yml`** runs on a `v*` tag (refusing one that
  is not the workspace's version), builds the bundle and the APK signed
  with the upload key from the secrets, attaches both to a GitHub release
  with the sibling trees' commits in the notes, and -- when
  `PLAY_SERVICE_ACCOUNT_JSON` is set -- uploads the bundle to the
  `internal` track (or the track given to a manual run).
- **Foreground services.** Every `FOREGROUND_SERVICE_<TYPE>` permission in
  the manifest is a form in the console, with a video. The wake window is a
  `shortService` (Android 14's type for a bounded run: three minutes, no
  permission, nothing to declare), leaving `phoneCall|microphone` for
  `CallService`, which a calling application justifies in a sentence.
- **Listing graphics.** `scripts/launcher-icon` also writes
  `android/play/icon-512.png` and `android/play/feature-1024x500.png`, the
  two Play requires. Screenshots are taken from a phone (below).

## Secrets

In the repository's GitHub settings (`gh secret set NAME < file` works):

| Secret | What |
|---|---|
| `SIGIL_UPLOAD_KEYSTORE_B64` | `base64 -i ~/.sigil-upload-key.p12` |
| `SIGIL_UPLOAD_KEYSTORE_PASSWORD` | as chosen at `scripts/upload-key` |
| `SIGIL_UPLOAD_KEY_ALIAS` | `sigil-upload` |
| `SIGIL_UPLOAD_KEY_PASSWORD` | the same as the keystore's |
| `PLAY_SERVICE_ACCOUNT_JSON` | optional: the key of a service account the console has been told about (Users and permissions → invite the account's e-mail with release rights) |

Keep the `.p12` backed up outside the repository. Losing it is not fatal --
Play app signing lets an upload key be reset through the console -- but it
is a support ticket.

## The console, in order

1. **A developer account.** A personal account made after 13 November 2023
   must run a *closed test* with 12 testers opted in for 14 continuous days
   before it may apply for production. An organisation account is exempt.
   This is the longest pole: start the closed test first and do the listing
   while it runs.
2. **Create the app**: name `Sigil`, default language, "App", free. The
   package is `org.squic.sigil` and cannot change afterwards.
3. **Upload the first bundle by hand** (Testing → Internal testing → Create
   release), which enrols the app in Play app signing with our key as the
   upload key. After this the workflow can upload.
4. **Set up the app** (the dashboard's list), with the answers in the next
   section: privacy policy, app access, ads, content rating, target
   audience, news app, COVID, data safety, government app, financial
   features, health, the foreground-service and full-screen-intent
   declarations.
5. **Store listing**: the text below, `android/play/icon-512.png`,
   `android/play/feature-1024x500.png`, and at least two phone screenshots
   (9:16, 1080×2412 from the OnePlus is fine):

   ```
   adb exec-out screencap -p > android/play/screenshot-1.png
   ```

6. **Closed testing** (the 12 testers) or, for an organisation account,
   straight to production. Play reviews a first submission in days, not
   hours.

## The answers

Drafts. Read them against the code before typing them in; the data-safety
form in particular is a legal statement.

**Privacy policy URL**: [docs/privacy.md](privacy.md) has to be *hosted*
at a stable URL -- GitHub Pages on this repository
(`https://wave-cl.github.io/sigil-android/privacy`) or a page on
squic.org.

**App access**: "All functionality is available without special access"
is not true -- a reviewer needs an account on an exchange. Provide
instructions: an exchange to use, and that a first device makes an account
in the app itself (no credentials to hand over), or a test account's
`sqx-pair:` string.

**Ads**: none. **News app**: no. **Government app**: no. **Financial
features**: none. **Health**: none.

**Target audience**: 18 and over (there is nothing for children here, and
anything younger brings the Families policy).

**Content rating** (IARC questionnaire): a communication application;
users interact and exchange content; no moderation by the developer (the
content is end-to-end encrypted). Expect a Teen or Mature rating.

**User-generated content.** Play's UGC policy requires an in-app way to
block a user (there is one: Devices/contacts → block) and a way to *report*
objectionable content. Sigil has no report action: nobody but the
recipient can read a message, so there is nobody to report it to but the
exchange's operator, who cannot read it either. Say so in the review notes;
if the reviewer insists, a "report" that copies the sender's key and the
operator's contact address is the honest minimum.

**Foreground service** (`phoneCall`, `microphone`): "Sigil makes voice
calls. While a call is in progress the microphone is open and the call must
continue with the screen off or another application in front; the
foreground service shows the ongoing-call notification with Hang up." The
video: start a call, lock the screen, show the notification, hang up.

**Full-screen intent**: "Incoming calls. A ring is shown over the lock
screen so it can be answered or declined." (Calling applications are the
case Play lists for this permission.)

**Data safety.** Play asks what leaves the phone, to whom, and why. Sigil
sends nothing to the developer. Everything it sends goes to the *exchange*
the account is on, which the person chose and which may be anyone's, over
sQUIC (encrypted in transit) and, for content, end-to-end encrypted so the
exchange cannot read it.

| Data type | Collected? | Shared? | Purpose | Notes |
|---|---|---|---|---|
| Messages (in-app messages) | yes, to the exchange | no | app functionality | end-to-end encrypted; the exchange stores ciphertext |
| Audio (voice) | yes, during a call | no | app functionality | encrypted; not stored |
| Photos/videos, files | yes, when sent | no | app functionality | end-to-end encrypted |
| Device or other IDs | yes: the device's public key | no | app functionality, account management | generated on the phone; identifies this device to the account |
| Contacts (address book) | no | | | the app never reads the phone's contacts |
| Location | no | | | |
| Crash logs, diagnostics, analytics | no | | | logcat only; nothing is sent |

Encrypted in transit: yes. Deletion: a person can remove the device from
the account (the device registry, SIP-22) and
delete conversations; there is no account with the developer to request
deletion from. The data is not "processed ephemerally" in Play's sense
(messages are stored on the phone), except call audio, which is.

If the default exchange is one *you* operate (`ex.squic.org`), Play treats
you as the recipient of the data above; say so in the policy and here.

**Store listing text**

Short description (80 characters):

> Calls and end-to-end encrypted chat over sQUIC, as a device of your account.

Full description:

> Sigil is a phone client for the sQUIC exchange: voice calls and
> end-to-end encrypted messaging with the people, groups and channels
> of an account whose key lives on your own machine.
>
> A phone is a device of the account, not the account. Pair it from a
> desktop Sigil in two strings -- no password, no phone number, no e-mail
> -- and the phone holds a key of its own, sealed by the Android key store.
> Remove the device and it is gone.
>
> - Voice calls, one to one and in rooms, that ring on a locked screen
> - Direct messages, groups and public channels; replies, reactions,
>   edits, files and images
> - Notifications composed on the phone from what only it can read, under
>   a setting of how much they say
> - Woken by the exchange through a push distributor of your choosing
>   (UnifiedPush: ntfy and others); no Google account is required
> - Nothing sent to the developer: no analytics, no crash reports, no
>   contacts read
>
> You need an account on an exchange -- run your own, or use one that is
> open -- and a desktop Sigil to pair from.

**Category**: Communication. **Tags**: messaging, calls, encryption.
**Contact e-mail**: the one on the developer account; **website**:
https://github.com/wave-cl/sigil-android.

## What a Play user does not get

Play users have no UnifiedPush distributor unless they install one (ntfy
is on Play), and the embedded FCM distributor needs a Firebase project and
a gateway that are not configured (`sigil.wakeProxy` is empty and there is
no `google-services.json`). Until one or the other exists, a Play install
gets messages when it is opened and is never woken. The Phone tab says a
distributor is needed; the listing above says which. Setting the bridge up
is `docs/wake.md`'s last section.

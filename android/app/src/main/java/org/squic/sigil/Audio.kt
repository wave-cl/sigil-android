package org.squic.sigil

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.util.Log

/**
 * The audio session a call runs in.
 *
 * **Without this a call is not a call, as far as Android is concerned.**
 * `MODE_NORMAL` is the mode for playing music, and in it the platform does not
 * put the voice-communication effects on the microphone -- the echo canceller,
 * the automatic gain, the noise suppressor. `sqex-voice` has a noise gate and
 * **no echo canceller of its own**, so the platform's is the only one in the
 * stack: without `MODE_IN_COMMUNICATION` a call on a loudspeaker hears itself.
 *
 * It is also what makes the rest of the phone behave as it does on every other
 * call: music pauses instead of playing under it, the proximity sensor blanks
 * the screen against an ear, the volume keys move the call's own level, and the
 * sound comes out of the **earpiece** rather than the loudspeaker.
 *
 * Held for exactly as long as [CallService] runs, and given back when it stops,
 * because a mode nobody restores is a phone that stays wrong after the call.
 */
object Audio {
    private const val TAG = "sigil"

    /** Held so it can be abandoned; `null` when the call is not holding focus. */
    private var focus: AudioFocusRequest? = null

    /**
     * The mode before the call, restored when it ends.
     *
     * **Not `MODE_NORMAL`.** Restoring a constant rather than what was there
     * puts the phone in the wrong state whenever something else already had it
     * somewhere else -- a real call in the dialer, another voice app -- and the
     * symptom of that is somebody else's call going quiet when ours ends.
     */
    private var wasMode: Int? = null

    private fun manager(ctx: Context): AudioManager =
        ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    /** Take the call's audio session: the communication mode, then focus. */
    @JvmStatic
    fun beginCall(ctx: Context) {
        val am = manager(ctx)
        try {
            // Only the first time: `onStartCommand` can be called again for a
            // service that is already running, and the second call would
            // remember `MODE_IN_COMMUNICATION` as the mode to go back to.
            if (wasMode == null) wasMode = am.mode
            am.mode = AudioManager.MODE_IN_COMMUNICATION
            val attrs = AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build()
            // Exclusive: a call is not something to duck music under, it is
            // something to stop music for.
            val want = AudioFocusRequest
                .Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_EXCLUSIVE)
                .setAudioAttributes(attrs)
                .build()
            focus = want
            val granted = am.requestAudioFocus(want)
            // **Read back what was achieved, not what was asked.** Neither of
            // the two lines above reports a refusal by throwing: `mode` is a
            // request the platform may decline, which is why `dumpsys audio`
            // prints a requested mode and an actual one separately, and
            // `requestAudioFocus` returns its answer rather than raising it.
            // This handset ignores focus requests from apps it does not
            // consider foreground -- fifteen of them in its own log -- and the
            // only symptom of that would be music playing under a call, with
            // nothing anywhere saying why.
            Log.i(TAG, "audio session taken: ${focusWord(granted)}, ${state(am)}")
            if (granted != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) {
                Log.w(TAG, "the call has no audio focus: music will play under it")
            }
            if (am.mode != AudioManager.MODE_IN_COMMUNICATION) {
                Log.w(
                    TAG,
                    "the call is not in the communication mode: no echo canceller, " +
                        "no automatic gain, no noise suppressor"
                )
            }
        } catch (e: Exception) {
            // A call without the session is worse than a call, and better than
            // no call: say so and carry on.
            Log.w(TAG, "could not take the call's audio session", e)
        }
    }

    /** Give it all back, in the order it was taken. */
    @JvmStatic
    fun endCall(ctx: Context) {
        val am = manager(ctx)
        try {
            focus?.let { am.abandonAudioFocusRequest(it) }
            focus = null
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                am.clearCommunicationDevice()
            } else {
                @Suppress("DEPRECATION")
                am.isSpeakerphoneOn = false
            }
            wasMode?.let { am.mode = it }
            wasMode = null
            // The other half of the readout: a mode that was not given back is
            // a phone that stays wrong after the call, and it is silent.
            Log.i(TAG, "audio session given back: ${state(am)}")
        } catch (e: Exception) {
            Log.w(TAG, "could not give the call's audio session back", e)
        }
    }

    /**
     * Put the call on the loudspeaker, or back on the earpiece.
     *
     * **Returns where the sound actually goes**, which is not always what was
     * asked: a tablet has no earpiece, a headset takes precedence over both,
     * and `setCommunicationDevice` can simply refuse. A control that drew what
     * it asked for rather than what happened would lie.
     *
     * Ordering matters: this is ignored on many devices unless the mode is
     * already `MODE_IN_COMMUNICATION`, so [beginCall] comes first.
     */
    @JvmStatic
    fun setSpeaker(ctx: Context, on: Boolean): Boolean {
        val am = manager(ctx)
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                if (on) {
                    val speaker = am.availableCommunicationDevices
                        .firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER }
                    if (speaker != null) {
                        am.setCommunicationDevice(speaker)
                    } else {
                        Log.w(TAG, "this device offers no loudspeaker to move the call to")
                    }
                } else {
                    // **"Not the loudspeaker" is a release, not a choice of
                    // earpiece.** Clearing lets the platform pick, and in
                    // `MODE_IN_COMMUNICATION` it picks the earpiece on a bare
                    // phone and the headset when one is plugged in or paired --
                    // which is what somebody wearing a headset means by this
                    // button. Naming `TYPE_BUILTIN_EARPIECE` outright, as this
                    // did, takes the call off their headset and holds it
                    // against the phone; the comment here already said a
                    // headset takes precedence, and the code did the opposite.
                    am.clearCommunicationDevice()
                    // Insist only if letting go left it on the loudspeaker,
                    // which some devices do by keeping the last route.
                    if (speakerOn(ctx)) {
                        am.availableCommunicationDevices
                            .firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
                            ?.let { am.setCommunicationDevice(it) }
                    }
                }
            } else {
                @Suppress("DEPRECATION")
                am.isSpeakerphoneOn = on
            }
        } catch (e: Exception) {
            Log.w(TAG, "could not move the call's sound", e)
        }
        val got = speakerOn(ctx)
        if (got != on) {
            Log.i(TAG, "asked for the ${if (on) "loudspeaker" else "earpiece"}; ${state(am)}")
        }
        return got
    }

    /** Where the call's sound is coming out now. */
    @JvmStatic
    fun speakerOn(ctx: Context): Boolean {
        val am = manager(ctx)
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                am.communicationDevice?.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER
            } else {
                @Suppress("DEPRECATION")
                am.isSpeakerphoneOn
            }
        } catch (e: Exception) {
            Log.w(TAG, "could not read where the call's sound is going", e)
            false
        }
    }

    /**
     * The mode and the route as `dumpsys audio` would show them, for one line
     * in the log at each end of a call.
     *
     * Spelt the same way the platform's own dump does -- the mode by name, the
     * route by device type -- so a reading taken here and a reading taken with
     * `adb shell dumpsys audio` can be put side by side.
     */
    private fun state(am: AudioManager): String {
        val route = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            am.communicationDevice?.let { deviceWord(it.type) } ?: "none"
        } else {
            @Suppress("DEPRECATION")
            if (am.isSpeakerphoneOn) "speaker" else "platform's choice"
        }
        return "mode=${modeWord(am.mode)} route=$route"
    }

    private fun modeWord(mode: Int): String = when (mode) {
        AudioManager.MODE_NORMAL -> "NORMAL"
        AudioManager.MODE_RINGTONE -> "RINGTONE"
        AudioManager.MODE_IN_CALL -> "IN_CALL"
        AudioManager.MODE_IN_COMMUNICATION -> "IN_COMMUNICATION"
        else -> "mode $mode"
    }

    private fun focusWord(granted: Int): String = when (granted) {
        AudioManager.AUDIOFOCUS_REQUEST_GRANTED -> "focus granted"
        AudioManager.AUDIOFOCUS_REQUEST_FAILED -> "focus refused"
        AudioManager.AUDIOFOCUS_REQUEST_DELAYED -> "focus delayed"
        else -> "focus answered $granted"
    }

    private fun deviceWord(type: Int): String = when (type) {
        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "earpiece"
        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "speaker"
        AudioDeviceInfo.TYPE_WIRED_HEADSET -> "wired headset"
        AudioDeviceInfo.TYPE_WIRED_HEADPHONES -> "wired headphones"
        AudioDeviceInfo.TYPE_USB_HEADSET -> "usb headset"
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO -> "bluetooth"
        AudioDeviceInfo.TYPE_BLE_HEADSET -> "bluetooth le"
        AudioDeviceInfo.TYPE_HEARING_AID -> "hearing aid"
        else -> "device type $type"
    }
}

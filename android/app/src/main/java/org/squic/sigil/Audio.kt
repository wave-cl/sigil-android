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
            am.requestAudioFocus(want)
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
                val want = if (on) {
                    AudioDeviceInfo.TYPE_BUILTIN_SPEAKER
                } else {
                    AudioDeviceInfo.TYPE_BUILTIN_EARPIECE
                }
                val device = am.availableCommunicationDevices.firstOrNull { it.type == want }
                when {
                    device != null -> am.setCommunicationDevice(device)
                    // **No earpiece is not a failure.** On a tablet, or with a
                    // headset in the way, "earpiece" means "stop forcing the
                    // loudspeaker and let the platform choose" -- which is what
                    // clearing does. Asking for a speaker that is not there is
                    // a different thing and has nothing to fall back to.
                    !on -> am.clearCommunicationDevice()
                    else -> Log.w(TAG, "this device offers no loudspeaker to move the call to")
                }
            } else {
                @Suppress("DEPRECATION")
                am.isSpeakerphoneOn = on
            }
        } catch (e: Exception) {
            Log.w(TAG, "could not move the call's sound", e)
        }
        return speakerOn(ctx)
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
}

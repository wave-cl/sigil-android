package org.squic.sigil

import android.content.Context
import android.media.AudioManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The audio session, on the handset, with nothing listening in the room.
 *
 * # WARNING: running this wipes the app from the device
 *
 * `connectedAndroidTest` installs a debug build, runs, and then **uninstalls
 * the app under test** -- which takes its private data with it: the
 * identity, the store, every conversation. I learned that by doing it to a
 * phone holding a real account, whose key existed nowhere else and which had
 * no SIP-44 successor. It was not recoverable.
 *
 * So: **run this only against a handset you are willing to re-provision**,
 * and never against one carrying an account that matters. The debug build is
 * signed differently from the release one anyway, so the install replaces
 * whatever was there regardless of the uninstall.
 *
 * `scripts/audio-test` is the way in: it refuses while the app is installed
 * and says what would be lost, because a warning in a comment did not stop
 * this happening once and would not stop it again.
 *
 *     scripts/audio-test             # refuses if the handset holds an account
 *     scripts/audio-test --wipe-ok   # proceed, having read the above
 *
 * # Why this is an instrumented test and not a unit one
 *
 * Mode, focus and routing are the platform's answers. A desktop test can only
 * ask a mock, and a mock of an OEM's audio policy is a picture of what I
 * imagined rather than of the phone — this one is a OnePlus, whose audio
 * stack is not the emulator's. So it runs on the device.
 *
 * # Why it needed no person, when the call it is about did
 *
 * A connected call opens the microphone, which is why the call itself is not
 * something to arrange while nobody is in the room. [Audio.beginCall] opens
 * none: it sets the mode and asks for focus, and the media engine's step —
 * the one that reaches a microphone — is separate and is not called here. So
 * the half of the call that had never been verified on hardware can be, and
 * repeatably, without anybody present.
 *
 * Reaching `CallService` over adb was the first idea and is correctly
 * refused: it is `exported="false"`, and opening it up to make it testable
 * would trade a real hole for a test.
 */
@RunWith(AndroidJUnit4::class)
class AudioTest {
    private val ctx: Context
        get() = InstrumentationRegistry.getInstrumentation().targetContext

    private val manager: AudioManager
        get() = ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager

    /**
     * **Without `MODE_IN_COMMUNICATION` a call has no echo canceller.**
     * Android applies AEC, AGC and noise suppression in that mode and not
     * outside it, and `sqex-voice` has a gate but no canceller of its own —
     * the platform's is the only one in the stack. This is the single
     * assertion the whole audio session exists for.
     */
    @Test
    fun a_call_takes_the_session_and_gives_it_back() {
        val before = manager.mode
        try {
            Audio.beginCall(ctx)
            assertEquals(
                "a call did not put the phone in communication mode, so the platform's " +
                    "echo canceller is not in the path and a loudspeaker call hears itself",
                AudioManager.MODE_IN_COMMUNICATION,
                manager.mode,
            )
        } finally {
            Audio.endCall(ctx)
        }
        // Restored, not merely set to MODE_NORMAL: whatever was going on
        // before the call was somebody else's, and a call that ends by
        // flattening it breaks the thing it interrupted.
        assertEquals(
            "the mode was not given back after the call",
            before,
            manager.mode,
        )
    }

    /**
     * **And it reports what it achieved, not what it was asked.**
     *
     * Routing is ignored on many devices unless the mode is already
     * `MODE_IN_COMMUNICATION`, so the order here is load-bearing and is the
     * order `CallService` uses. `setSpeaker` returns what `speakerOn` reads
     * afterwards, so a device that refuses a route says so rather than
     * leaving the button lying.
     */
    @Test
    fun the_route_moves_and_says_where_it_landed() {
        try {
            Audio.beginCall(ctx)

            val loud = Audio.setSpeaker(ctx, true)
            assertTrue("asked for the loudspeaker and did not get it", loud)
            assertTrue("setSpeaker said yes and speakerOn disagrees", Audio.speakerOn(ctx))

            val quiet = Audio.setSpeaker(ctx, false)
            assertFalse("asked for the earpiece and stayed on the loudspeaker", quiet)
            assertFalse("setSpeaker said no and speakerOn disagrees", Audio.speakerOn(ctx))
        } finally {
            Audio.endCall(ctx)
        }
    }
}

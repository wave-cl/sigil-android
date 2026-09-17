package org.squic.sigil

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The platform's key store, as SIP-47 asks: the phone's device secret never
 * leaves it. Precisely: the sqnr identity file is sealed under a passphrase
 * the phone never shows anybody, and that passphrase is sealed here, under
 * an AES key the hardware holds and will not export. Rust asks this to seal
 * once and to open on every start; the identity's seed itself is needed in
 * memory for the sQUIC handshake, which is why the file exists at all.
 */
object Vault {
    private const val ALIAS = "sigil-passphrase"
    private const val STORE = "AndroidKeyStore"

    fun ensureKey() {
        val ks = KeyStore.getInstance(STORE).apply { load(null) }
        if (ks.containsAlias(ALIAS)) return
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, STORE)
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        gen.generateKey()
    }

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance(STORE).apply { load(null) }
        return ks.getKey(ALIAS, null) as SecretKey
    }

    /** Seal `plain`; the result is `iv || ciphertext`. Called from Rust. */
    @JvmStatic
    fun seal(plain: ByteArray): ByteArray {
        ensureKey()
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        return cipher.iv + cipher.doFinal(plain)
    }

    /** Open what [seal] made, or null if it will not open. Called from Rust. */
    @JvmStatic
    fun open(sealed: ByteArray): ByteArray? {
        if (sealed.size < 13) return null
        return try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed.copyOfRange(0, 12)))
            cipher.doFinal(sealed.copyOfRange(12, sealed.size))
        } catch (e: Exception) {
            null
        }
    }
}

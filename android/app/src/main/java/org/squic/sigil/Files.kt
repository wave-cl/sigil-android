package org.squic.sigil

import android.app.Activity
import android.content.Intent
import android.net.Uri
import java.io.File

/**
 * Choosing files to attach: the storage access framework, which hands back
 * content URIs rather than paths. Each is copied into the app's cache under
 * its display name and the paths are handed to Rust, which stages them the
 * way the desktop stages a dialog's answer. Saving goes to the app's own
 * Downloads directory without a dialog, for now; the Files app shows it.
 */
object Files {
    private const val PICK = 0x5161

    /**
     * Called from Rust, with no activity passed: see [Host]. Handing a raw
     * `jobject` across for this segfaulted the process.
     */
    @JvmStatic
    fun pick() {
        val activity = Host.get()
        if (activity == null) {
            // No window: nothing can be picked, and the waiting Rust side
            // must be released or the composer stays stuck.
            Native.picked(null)
            return
        }
        // Called from Rust's own thread. Starting an activity belongs on the
        // main thread; from another it crashed inside the framework.
        activity.runOnUiThread {
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = "*/*"
                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            }
            activity.startActivityForResult(intent, PICK)
        }
    }

    /** Where a file arriving as `name` is written. Called from Rust. */
    @JvmStatic
    fun saveTarget(ctx: android.content.Context, name: String): String {
        val dir = ctx.getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS) ?: ctx.filesDir
        dir.mkdirs()
        return File(dir, if (name.isBlank()) "attachment" else name).absolutePath
    }

    fun onResult(activity: Activity, requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != PICK) return
        if (resultCode != Activity.RESULT_OK || data == null) {
            Native.picked(null)
            return
        }
        val uris = mutableListOf<Uri>()
        data.clipData?.let { clip -> for (i in 0 until clip.itemCount) uris += clip.getItemAt(i).uri }
        data.data?.let { uris += it }
        val paths = uris.take(4).mapNotNull { copyIn(activity, it) }
        Native.picked(paths.toTypedArray())
    }

    private fun copyIn(activity: Activity, uri: Uri): String? {
        val name = activity.contentResolver.query(uri, null, null, null, null)?.use { c ->
            val i = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
            if (c.moveToFirst() && i >= 0) c.getString(i) else null
        } ?: "attachment"
        val out = File(activity.cacheDir, "attach").apply { mkdirs() }.let { File(it, name) }
        return try {
            activity.contentResolver.openInputStream(uri)?.use { input ->
                out.outputStream().use { input.copyTo(it) }
            } ?: return null
            out.absolutePath
        } catch (e: Exception) {
            null
        }
    }
}

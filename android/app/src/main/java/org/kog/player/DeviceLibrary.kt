package org.kog.player

import android.content.Context
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import java.io.File
import java.security.MessageDigest

/** Android document permissions and file copying only. Once staged, the Rust
 * library performs discovery, format filtering, archive expansion and tags. */
internal class DeviceLibrary(private val context: Context) {
    val root = File(context.filesDir, "Kog Imports").apply { mkdirs() }
    private fun identity(uri: Uri) = MessageDigest.getInstance("SHA-256")
        .digest(uri.toString().toByteArray()).take(12).joinToString("") { "%02x".format(it) }

    private fun name(document: DocumentFile): String {
        val name = document.name ?: error("The selected file has no name")
        require(name != "." && name != ".." && '/' !in name && '\u0000' !in name) { "Invalid document name" }
        return name
    }
    fun stageFolder(folder: DocumentFile): File {
        val directory = File(root, identity(folder.uri)).apply { mkdirs() }
        copy(folder, directory)
        return directory
    }
    fun stageFile(uri: Uri, accessibleRoot: DocumentFile?): File {
        val document = DocumentFile.fromSingleUri(context, uri) ?: error("Cannot access selected file")
        // A tree grant permits staging the companion banks beside the song.
        // Standalone document grants can expose only that selected document.
        fun parent(folder: DocumentFile): DocumentFile? {
            val children = folder.listFiles()
            if (children.any { it.uri == uri }) return folder
            for (child in children) if (child.isDirectory) parent(child)?.let { return it }
            return null
        }
        val folder = accessibleRoot?.let(::parent)
        if (folder != null) return File(stageFolder(folder), name(document))
        val directory = File(root, identity(uri)).apply { mkdirs() }
        return File(directory, name(document)).also { copy(document, it) }
    }
    private fun copy(source: DocumentFile, destination: File) {
        if (source.isDirectory) {
            destination.mkdirs()
            for (child in source.listFiles()) copy(child, File(destination, name(child)))
        } else if (source.isFile) {
            val modified = source.lastModified()
            if (modified > 0 && destination.isFile && destination.length() == source.length() && destination.lastModified() == modified) return
            destination.parentFile?.mkdirs()
            val temporary = File.createTempFile("kog-import-", ".part", destination.parentFile)
            try {
                context.contentResolver.openInputStream(source.uri)?.use { input ->
                    temporary.outputStream().use(input::copyTo)
                } ?: error("Cannot read ${source.name}")
                check(temporary.renameTo(destination)) { "Cannot import ${source.name}" }
                if (modified > 0) destination.setLastModified(modified)
            } finally { temporary.delete() }
        }
    }
}

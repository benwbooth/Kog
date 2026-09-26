package org.kog.player

import android.content.Context
import android.net.Uri
import androidx.media3.common.C
import androidx.media3.datasource.BaseDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DataSpec
import androidx.media3.datasource.TransferListener
import androidx.media3.common.util.UnstableApi
import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/** Kog's Rust decoder library; absent on ABIs where a native build has not been packaged. */
internal object NativeAudio {
    private val loadResult = runCatching { System.loadLibrary("kog_android_audio") }
    val available = loadResult.isSuccess
    val loadError: String? = loadResult.exceptionOrNull()?.message

    external fun nativeSetHelperDirectory(path: String): Boolean
    external fun nativeOpen(path: String, subsong: Int, midiEngine: String,
        soundfontPath: String, sc55RomPath: String, mt32RomPath: String): Long
    external fun nativeDurationMs(handle: Long): Long
    external fun nativeRead(handle: Long, output: ByteArray, offset: Int, length: Int): Int
    external fun nativeSeek(handle: Long, positionMs: Long): Boolean
    external fun nativeClose(handle: Long)

    fun configure(context: Context) {
        if (available) nativeSetHelperDirectory(context.applicationInfo.nativeLibraryDir)
    }

    fun useFor(track: Track): Boolean {
        if (!track.isDevice) return false
        val extension = track.name.substringAfterLast('.', "").lowercase()
        return extension !in setOf("mp3", "mp2", "aac", "m4a", "m4b", "mp4", "flac", "wav",
            "wave", "ogg", "oga", "opus", "webm", "mka", "mkv")
    }

    fun uri(track: Track): Uri = Uri.Builder().scheme("kog-native").authority("device")
        .appendQueryParameter("source", track.path)
        .appendQueryParameter("name", track.name)
        .appendQueryParameter("fragment", track.fragment)
        .build()
}

/** Supplies a WAV view over Kog's 48 kHz stereo PCM stream to Media3. */
@UnstableApi
internal class NativePcmDataSource(private val context: Context) : BaseDataSource(false) {
    private var sourceUri: Uri? = null
    private var handle = 0L
    private var header = ByteArray(0)
    private var position = 0L
    private var pendingByte = -1
    private var totalBytes = C.LENGTH_UNSET.toLong()

    override fun open(dataSpec: DataSpec): Long {
        transferInitializing(dataSpec)
        if (!NativeAudio.available) throw IOException(
            "Kog's native decoder could not load: ${NativeAudio.loadError.orEmpty()}")
        val uri = dataSpec.uri
        val source = Uri.parse(uri.getQueryParameter("source") ?: throw IOException("Missing device file"))
        val name = uri.getQueryParameter("name").orEmpty()
        val file = cacheFile(source, name)
        val fragment = uri.getQueryParameter("fragment")?.toIntOrNull() ?: -1
        val preferences = context.getSharedPreferences("kog", Context.MODE_PRIVATE)
        val opened = NativeAudio.nativeOpen(file.absolutePath, fragment,
            preferences.getString("local_midi_engine", "opl3windows") ?: "opl3windows",
            preferences.getString("midi_soundfont", "").orEmpty(),
            preferences.getString("midi_sc55_roms", "").orEmpty(),
            preferences.getString("midi_mt32_roms", "").orEmpty())
        if (opened == 0L) throw IOException("Kog could not open $name")
        handle = opened
        sourceUri = uri
        val duration = NativeAudio.nativeDurationMs(handle)
        val frames = if (duration > 0) ((duration.toDouble() * 48_000.0) / 1000.0).toLong() else -1L
        val pcmBytes = if (frames >= 0) frames * 4 else -1L
        header = wavHeader(pcmBytes)
        totalBytes = if (pcmBytes >= 0) header.size + pcmBytes else C.LENGTH_UNSET.toLong()
        position = dataSpec.position
        pendingByte = -1
        if (position >= header.size) {
            val milliseconds = ((position - header.size) / 4) * 1000 / 48_000
            NativeAudio.nativeSeek(handle, milliseconds)
        }
        transferStarted(dataSpec)
        return if (totalBytes >= 0) (totalBytes - position).coerceAtLeast(0) else C.LENGTH_UNSET.toLong()
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (length == 0) return 0
        if (position < header.size) {
            val count = minOf(length, header.size - position.toInt())
            header.copyInto(buffer, offset, position.toInt(), position.toInt() + count)
            position += count
            bytesTransferred(count)
            return count
        }
        if (totalBytes >= 0 && position >= totalBytes) return C.RESULT_END_OF_INPUT
        if (pendingByte >= 0) {
            buffer[offset] = pendingByte.toByte()
            pendingByte = -1
            position++
            bytesTransferred(1)
            return 1
        }
        if (length == 1) {
            val pair = ByteArray(2)
            val count = NativeAudio.nativeRead(handle, pair, 0, pair.size)
            if (count <= 0) return count
            buffer[offset] = pair[0]
            if (count > 1) pendingByte = pair[1].toInt() and 0xff
            position++
            bytesTransferred(1)
            return 1
        }
        val count = NativeAudio.nativeRead(handle, buffer, offset,
            if (totalBytes >= 0) minOf(length.toLong(), totalBytes - position).toInt() else length)
        if (count > 0) {
            position += count
            bytesTransferred(count)
        }
        return count
    }

    override fun getUri(): Uri? = sourceUri

    override fun close() {
        if (handle != 0L) {
            NativeAudio.nativeClose(handle)
            handle = 0
            transferEnded()
        }
        sourceUri = null
        pendingByte = -1
    }

    private fun cacheFile(uri: Uri, name: String): File {
        val document = androidx.documentfile.provider.DocumentFile.fromSingleUri(context, uri)
        val identity = "$uri:${document?.length()}:${document?.lastModified()}"
        val digest = MessageDigest.getInstance("SHA-256").digest(identity.toByteArray())
            .take(12).joinToString("") { "%02x".format(it) }
        val filename = name.takeLast(100).replace(Regex("[^A-Za-z0-9._-]"), "_")
        val directory = File(context.cacheDir, "kog-native-audio").apply { mkdirs() }
        val file = File(directory, "${digest}_$filename")
        if (!file.isFile) {
            val temporary = File.createTempFile("kog-", ".part", directory)
            try {
                context.contentResolver.openInputStream(uri)?.use { input ->
                    temporary.outputStream().use(input::copyTo)
                } ?: throw IOException("Cannot read $name")
                if (!temporary.renameTo(file)) throw IOException("Cannot cache $name")
            } finally {
                temporary.delete()
            }
        }
        return file
    }

    private fun wavHeader(pcmBytes: Long): ByteArray {
        val bytes = if (pcmBytes < 0) 0x7fff_ffff else pcmBytes.coerceAtMost(0xffff_ff00L)
        val riff = (bytes + 36).coerceAtMost(0xffff_ffffL)
        return ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN).apply {
            put("RIFF".toByteArray()); putInt(riff.toInt()); put("WAVE".toByteArray())
            put("fmt ".toByteArray()); putInt(16); putShort(1); putShort(2)
            putInt(48_000); putInt(48_000 * 4); putShort(4); putShort(16)
            put("data".toByteArray()); putInt(bytes.toInt())
        }.array()
    }
}

/** Routes Kog PCM and server HTTP traffic without altering Android's file/content handling. */
@UnstableApi
internal class KogBaseDataSource(private val context: Context, private val http: DataSource) : DataSource {
    private val listeners = mutableListOf<TransferListener>()
    private var delegate: DataSource? = null

    override fun addTransferListener(transferListener: TransferListener) {
        listeners.add(transferListener)
    }

    override fun open(dataSpec: DataSpec): Long {
        val selected = if (dataSpec.uri.scheme == "kog-native") NativePcmDataSource(context) else http
        listeners.forEach(selected::addTransferListener)
        delegate = selected
        return selected.open(dataSpec)
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int =
        delegate?.read(buffer, offset, length) ?: C.RESULT_END_OF_INPUT

    override fun getUri(): Uri? = delegate?.uri

    override fun getResponseHeaders(): Map<String, List<String>> = delegate?.responseHeaders.orEmpty()

    override fun close() {
        delegate?.close()
        delegate = null
    }
}

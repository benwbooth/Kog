package org.kog.player

import android.net.Uri
import android.os.Bundle
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import org.json.JSONObject

/** Presentation and platform transport of a shared library locator. */
internal fun Track.mediaItem(api: KogApi): MediaItem {
    val extras = Bundle().apply { putString("kog_track", saved().toString()) }
    val metadata = MediaMetadata.Builder().setTitle(label).setArtist(artist).setAlbumTitle(album)
        .setTrackNumber(trackNumber).setDiscNumber(discNumber).setExtras(extras)
    api.art(this)?.let { metadata.setArtworkUri(Uri.parse(it)) }
    val builder = MediaItem.Builder().setMediaId(key).setMediaMetadata(metadata.build())
    if (NativeAudio.useFor(this)) builder.setUri(NativeAudio.uri(this)).setMimeType(MimeTypes.AUDIO_WAV)
    else builder.setUri(api.stream(this))
    return builder.build()
}

internal fun MediaItem.kogTrack(): Track {
    val stored = mediaMetadata.extras?.getString("kog_track")
    require(stored != null) { "Missing Kog track locator" }
    return Track.parse(JSONObject(stored))
}

package org.kog.player

import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.datasource.DataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import java.util.Base64

/** Owns playback while the UI is closed and exposes Bluetooth/system media controls. */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val http = DataSource.Factory {
            val api = KogApi(this)
            val headers = when {
                api.token.isNotBlank() -> mapOf("Authorization" to "Bearer ${api.token}")
                api.username.isNotBlank() -> mapOf("Authorization" to "Basic " +
                    Base64.getEncoder().encodeToString("${api.username}:${api.password}".toByteArray()))
                else -> emptyMap()
            }
            DefaultHttpDataSource.Factory().setDefaultRequestProperties(headers).createDataSource()
        }
        val data = DefaultDataSource.Factory(this, http)
        val player = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(data))
            .build()
        session = MediaSession.Builder(this, player).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}

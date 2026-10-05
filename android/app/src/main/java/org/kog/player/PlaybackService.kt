package org.kog.player

import android.os.Bundle
import android.content.Context
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import org.json.JSONObject
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
    companion object {
        const val POLICY_COMMAND = "org.kog.player.POLICY"
        internal fun createOutput(context: Context): ExoPlayer {
            NativeAudio.configure(context)
            val http = DataSource.Factory {
                val api = KogApi(context)
                val headers = when {
                    api.token.isNotBlank() -> mapOf("Authorization" to "Bearer ${api.token}")
                    api.username.isNotBlank() -> mapOf("Authorization" to "Basic " +
                        Base64.getEncoder().encodeToString("${api.username}:${api.password}".toByteArray()))
                    else -> emptyMap()
                }
                DefaultHttpDataSource.Factory().setDefaultRequestProperties(headers).createDataSource()
            }
            val data = DefaultDataSource.Factory(context, DataSource.Factory {
                KogBaseDataSource(context, http.createDataSource())
            })
            return ExoPlayer.Builder(context)
                .setMediaSourceFactory(DefaultMediaSourceFactory(data))
                .build()
        }
    }

    override fun onCreate() {
        super.onCreate()
        val player = createOutput(this)
        val policyPlayer = PolicyPlayer(this, player) { snapshot ->
            session?.setSessionExtras(Bundle().apply { putString("kog_policy", snapshot.toString()) })
        }
        session = MediaSession.Builder(this, policyPlayer).setCallback(object : MediaSession.Callback {
            override fun onConnect(session: MediaSession, controller: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
                return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                    .setAvailableSessionCommands(MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                        .add(SessionCommand(POLICY_COMMAND, Bundle.EMPTY)).build()).build()
            }
            override fun onCustomCommand(session: MediaSession, controller: MediaSession.ControllerInfo,
                customCommand: SessionCommand, args: Bundle): ListenableFuture<SessionResult> {
                if (customCommand.customAction != POLICY_COMMAND) return super.onCustomCommand(session, controller, customCommand, args)
                return try {
                    val snapshot = policyPlayer.dispatch(JSONObject(args.getString("command") ?: "{}"))
                    Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS,
                        Bundle().apply { putString("kog_policy", snapshot.toString()) }))
                } catch (error: Exception) {
                    Futures.immediateFuture(SessionResult(SessionResult.RESULT_ERROR_BAD_VALUE,
                        Bundle().apply { putString("error", error.message) }))
                }
            }
        }).build()
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

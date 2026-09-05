import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtWebEngine
import QtWebChannel
import org.kog.native 1.0

ApplicationWindow {
    id: root
    required property var app
    required property var mainWindow
    property var skin: ({})
    property string rendererStatus: qsTr("Loading modern skin…")
    property int sentRevision: -1
    property int requestCount: 0
    property double requestWindow: 0
    property bool pageStarted: false
    Component.onCompleted: pageStarted = true
    signal openGallery()
    signal openEqualizer()
    signal openVisualizer()
    title: qsTr("Kog — %1 (modern skin)").arg(skin.title || "Modern")
    width: 1000
    height: 720
    minimumWidth: 500
    minimumHeight: 350
    onClosing: function(close) {
        if (!mainWindow.applicationQuitRequested) {
            close.accepted = false
            hide()
            mainWindow.showFromTray()
        }
    }
    onSkinChanged: {
        sentRevision = -1
        rendererStatus = qsTr("Loading modern skin…")
        bridge.skinUrl = "kogskin://current/skin.wal?v=" + Date.now()
        if (pageStarted) Qt.callLater(function() { web.reload() })
    }
    function updateState(force) {
        if (!app) return
        if (force || sentRevision !== app.playlist_revision) {
            const snapshot = JSON.parse(app.skin_state(true))
            // Keep rows in a separate persistent property: WebChannel batches
            // notifications and may coalesce a full snapshot with the next tick.
            bridge.tracksJson = JSON.stringify(snapshot.tracks || [])
            delete snapshot.tracks
            bridge.stateJson = JSON.stringify(snapshot)
        } else bridge.stateJson = app.skin_state(false)
        sentRevision = app.playlist_revision
    }
    function number(value) { return typeof value === "number" && isFinite(value) }
    function row(value) { return number(value) && Math.floor(value) === value && value >= 0 && value < app.playlist_count }
    function rows(value) { return Array.isArray(value) && value.length <= app.playlist_count && value.every(row) }
    function command(name, payload) {
        if (!visible || typeof name !== "string" || typeof payload !== "string" || payload.length > 131072) return
        const now = Date.now()
        if (now - requestWindow > 1000) { requestWindow = now; requestCount = 0 }
        if (++requestCount > 60) return
        let data
        try { data = JSON.parse(payload) } catch (_) { return }
        switch (name) {
        case "ready": rendererStatus = qsTr("Experimental modern skin — some skin features may be unsupported"); updateState(true); break
        case "error": rendererStatus = qsTr("Skin error: %1").arg(String(data).slice(0, 300)); break
        case "play": if (app.playback_state !== "playing") app.play_pause(); break
        case "pause": if (app.playback_state === "playing") app.play_pause(); break
        case "stop": app.stop(); break
        case "next": app.next(); break
        case "previous": app.previous(); break
        case "seek": if (number(data)) app.seek(Math.max(0, Math.min(data, app.duration_seconds))); break
        case "volume": if (number(data)) app.set_volume_level(Math.max(0, Math.min(1, data))); break
        case "playIndex": if (row(data)) app.activate_playlist_index(data); break
        case "remove": if (rows(data)) app.remove_tracks(data.join(",")); break
        case "swap":
            if (data && row(data.first) && row(data.second) && data.first !== data.second) {
                const first = Math.min(data.first, data.second)
                const last = Math.max(data.first, data.second)
                app.move_tracks(String(first), last + 1)
                app.move_tracks(String(last - 1), first)
            }
            break
        case "move":
            if (data && rows(data.indices) && number(data.target) && Math.floor(data.target) === data.target && data.target >= 0 && data.target <= app.playlist_count) {
                const remaining = []
                for (let i = 0; i < app.playlist_count; ++i) if (data.indices.indexOf(i) < 0) remaining.push(i)
                app.move_tracks(data.indices.join(","), data.target < remaining.length ? remaining[data.target] : app.playlist_count)
            }
            break
        case "clear": app.clear_playlist(); break
        case "openFiles": app.open_audio_files(); break
        case "savePlaylist": app.save_playlist(); break
        case "restore": hide(); mainWindow.showFromTray(); break
        case "shuffle": if (["off", "all", "albums"].indexOf(data) >= 0) app.select_shuffle_mode(data); break
        case "repeat": if (["off", "playlist", "track"].indexOf(data) >= 0) app.select_repeat_mode(data); break
        case "eqBand":
            if (data && number(data.index) && Math.floor(data.index) === data.index && data.index >= 0 && data.index < 10 && number(data.gain))
                app.update_skin_equalizer_band(data.index, Math.max(-20, Math.min(20, data.gain)))
            break
        case "eqPreamp": if (number(data)) app.update_equalizer_preamp(Math.max(-20, Math.min(20, data))); break
        case "eqEnabled": if (typeof data === "boolean") app.update_equalizer_enabled(data); break
        case "openEqualizer": openEqualizer(); break
        case "openVisualizer": openVisualizer(); break
        case "openGallery": openGallery(); break
        }
    }
    // This is the entire web-facing API. AppController itself is never published.
    WebChannel {
        id: channel
        registeredObjects: [bridge]
    }
    QtObject {
        id: bridge
        WebChannel.id: "kog"
        property string stateJson: "{}"
        property string tracksJson: "[]"
        property string skinUrl: "kogskin://current/skin.wal"
        function request(command, payloadJson) { root.command(command, payloadJson) }
    }
    Timer { interval: 100; running: root.visible; repeat: true; onTriggered: root.updateState(false) }
    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        WebEngineView {
            id: web
            objectName: "modernWebView"
            Layout.fillWidth: true
            Layout.fillHeight: true
            url: "qrc:/kog/modern/index.html"
            webChannel: channel
            profile: ModernSkinProfile { skinPath: root.skin.archivePath || "" }
            settings.localContentCanAccessRemoteUrls: false
            settings.localContentCanAccessFileUrls: false
            settings.javascriptCanOpenWindows: false
            settings.javascriptCanAccessClipboard: false
            settings.pluginsEnabled: false
            settings.fullScreenSupportEnabled: false
            settings.screenCaptureEnabled: false
            settings.webGLEnabled: false
            onNavigationRequested: function(request) {
                if (request.url.toString() !== "qrc:/kog/modern/index.html") request.action = WebEngineNavigationRequest.IgnoreRequest
            }
            onLoadingChanged: function(info) {
                if (info.status === WebEngineView.LoadFailedStatus) root.rendererStatus = qsTr("Renderer failed: %1").arg(info.errorString)
            }
            onRenderProcessTerminated: root.rendererStatus = qsTr("The modern skin renderer stopped. Reopen the skin to retry.")
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 6
            Label { text: root.rendererStatus; textFormat: Text.PlainText; elide: Text.ElideRight; Layout.fillWidth: true }
            ToolButton { text: qsTr("Skins…"); onClicked: root.openGallery() }
            ToolButton { text: qsTr("Kog"); onClicked: { root.hide(); root.mainWindow.showFromTray() } }
        }
    }
}

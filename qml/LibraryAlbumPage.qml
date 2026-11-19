pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

// An album from the whole library, opened by clicking an album's name or
// cover wherever a song shows it (the transport, a row's album): a big
// cover, "Album", the title, artist · year · N songs and length, a play
// button, and every song of the album (from the library index, not just
// the list it was clicked in) laid out as the rows view lays them out.
// It shows once its songs are found, never half-built; Back, Escape or
// Backspace close it.
FocusScope {
    id: page

    required property var app
    required property var theme
    required property var searchModel
    property var rowLines: ["title", "artist", "album", "format", "length", "path"]

    // The album shown, and the one being looked up.
    property var album: null
    property int ticket: 0
    readonly property bool shown: album !== null
    visible: shown

    readonly property bool darkTheme: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.66,
        theme.base.g + (theme.text.g - theme.base.g) * 0.66,
        theme.base.b + (theme.text.b - theme.base.b) * 0.66, 1)

    function open(albumName, albumArtist) {
        if (!albumName || String(albumName).trim().length === 0)
            return
        ticket = app.request_album(String(albumName), String(albumArtist || ""))
        poll.restart()
    }
    function close() {
        ticket = 0
        poll.stop()
        album = null
    }
    Timer {
        id: poll
        interval: 40
        repeat: true
        onTriggered: {
            if (page.ticket === 0) {
                stop()
                return
            }
            const json = String(page.app.album_result(page.ticket))
            if (json.length === 0)
                return
            stop()
            page.ticket = 0
            try {
                const found = JSON.parse(json)
                if (found.songs && found.songs.length > 0) {
                    page.album = found
                    songList.positionViewAtBeginning()
                    songList.forceActiveFocus()
                }
            } catch (_) {}
        }
    }

    readonly property var songs: album ? album.songs : []
    readonly property string entriesJson: JSON.stringify(songs.map(song => song.entry))
    function lengthLabel(ms) {
        const seconds = Math.round(ms / 1000)
        if (seconds < 3600)
            return seconds < 60 ? qsTr("%1 sec").arg(seconds)
                : qsTr("%1 min %2 sec").arg(Math.floor(seconds / 60)).arg(seconds % 60)
        return qsTr("%1 hr %2 min").arg(Math.floor(seconds / 3600)).arg(Math.floor(seconds / 60) % 60)
    }
    function clock(ms) {
        const seconds = Math.round(ms / 1000)
        const minutes = Math.floor(seconds / 60)
        const rest = String(seconds % 60).padStart(2, "0")
        return minutes >= 60
            ? Math.floor(minutes / 60) + ":" + String(minutes % 60).padStart(2, "0") + ":" + rest
            : minutes + ":" + rest
    }
    // What the player calls the song while it plays (`current_file`): a
    // song of a multi-song file (NSF, NSFE, GBS...) as `#N`, counting from
    // one, after the file, so each of its songs is told apart.
    function fileOf(song) {
        const entry = song.entry || {}
        if (entry.kind === "archive")
            return entry.path + " :: " + entry.entry
        return String(entry.path || "")
    }
    function source(song) {
        const fragment = String((song.entry || {}).fragment || "")
        return /^[0-9]+$/.test(fragment)
            ? fileOf(song) + "#" + (Number(fragment) + 1)
            : fileOf(song)
    }
    function playingRow() {
        const playing = String(page.app.current_file || "")
        if (playing.length === 0 || page.app.playback_state === "stopped")
            return -1
        for (let row = 0; row < songs.length; ++row) {
            if (playing === source(songs[row]))
                return row
        }
        // A whole file listed once plays as its first song, `#1`.
        for (let row = 0; row < songs.length; ++row) {
            if (source(songs[row]) === fileOf(songs[row])
                    && playing.startsWith(fileOf(songs[row]) + "#"))
                return row
        }
        return -1
    }
    readonly property int playingIndex: {
        page.app.current_file
        page.app.playback_state
        return shown ? playingRow() : -1
    }
    readonly property bool albumPlaying: playingIndex >= 0 && app.playback_state === "playing"

    // The rows' data, as PlaylistCardRow reads a list's.
    QtObject {
        id: rows
        readonly property int playlist_revision: page.songs.length
        readonly property int current_index: -1
        readonly property string current_file: page.app.current_file
        readonly property string playback_state: page.app.playback_state
        readonly property real audio_level_low: page.app.audio_level_low
        readonly property real audio_level_low_mid: page.app.audio_level_low_mid
        readonly property real audio_level_mid: page.app.audio_level_mid
        readonly property real audio_level_high_mid: page.app.audio_level_high_mid
        readonly property real audio_level_high: page.app.audio_level_high
        readonly property real audio_level_air: page.app.audio_level_air
        readonly property int desktop_art_revision: page.app.desktop_art_revision
        function track_value_at(row, column) {
            const song = page.songs[row]
            if (!song)
                return ""
            const entry = song.entry || {}
            const path = String(entry.kind === "archive" ? entry.path + "/" + entry.entry : entry.path || "")
            switch (column) {
            case "title": return song.title || path.slice(path.lastIndexOf("/") + 1)
            case "artist": return song.artist || ""
            case "album": return song.album || ""
            case "albumartist": return song.albumArtist || ""
            case "date": return song.year ? String(song.year) : ""
            case "genre": return song.genre || ""
            case "length": return song.durationMs > 0 ? page.clock(song.durationMs) : ""
            case "codec": return song.system || ""
            case "path": return path
            case "filename": return path.slice(path.lastIndexOf("/") + 1)
            case "source": return page.source(song)
            default: return ""
            }
        }
        function track_number_at(row) { return "" }
        function track_missing_at(row) { return false }
        function track_known_image_at(row, tile) { return "?" }
        function track_cover_at(row) {
            const song = page.songs[row]
            if (!song)
                return ""
            const entry = song.entry || {}
            return page.app.mix_cover(entry.kind || "local", entry.path || "", entry.entry || "",
                song.album || "", song.artist || "")
        }
    }

    Rectangle {
        anchors.fill: parent
        color: page.theme.base
    }

    ListView {
        id: songList
        anchors.fill: parent
        clip: true
        focus: true
        model: page.songs.length
        boundsBehavior: Flickable.StopAtBounds
        maximumFlickVelocity: 12000
        flickDeceleration: 2200
        reuseItems: true
        readonly property real verticalScrollGutter: albumBar.visible ? albumBar.implicitWidth + 4 : 0
        property int cursor: -1

        Keys.onEscapePressed: event => { page.close(); event.accepted = true }
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Backspace || event.key === Qt.Key_Back) {
                page.close()
                event.accepted = true
            }
        }
        Keys.onUpPressed: event => { cursor = Math.max(0, cursor - 1); positionViewAtIndex(cursor, ListView.Contain); event.accepted = true }
        Keys.onDownPressed: event => { cursor = Math.min(count - 1, cursor + 1); positionViewAtIndex(cursor, ListView.Contain); event.accepted = true }
        Keys.onReturnPressed: event => { if (cursor >= 0) page.app.play_entries(page.entriesJson, cursor); event.accepted = true }

        onContentYChanged: albumBar.flash()
        ScrollBar.vertical: FadingScrollBar {
            id: albumBar
            edgeInset: 10
            onPressedChanged: if (pressed) albumWheel.stop()
        }
        KineticWheelHandler {
            id: albumWheel
            view: songList
        }

        header: Item {
            id: head
            width: songList.width - songList.verticalScrollGutter
            height: Math.max(cover.height, details.implicitHeight) + 72
            readonly property var first: page.songs.length > 0 ? page.songs[0] : null
            readonly property string coverUrl: {
                page.app.desktop_art_revision
                const song = head.first
                if (!song)
                    return ""
                const entry = song.entry || {}
                return String(page.app.mix_cover(entry.kind || "local", entry.path || "",
                    entry.entry || "", song.album || "", song.artist || ""))
            }

            AbstractButton {
                id: back
                x: 12
                y: 10
                width: 32
                height: 32
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                Accessible.name: qsTr("Back")
                onClicked: page.close()
                background: Rectangle {
                    radius: 16
                    color: Qt.rgba(page.theme.text.r, page.theme.text.g, page.theme.text.b,
                        back.pressed ? 0.16 : (back.hovered ? 0.1 : 0.05))
                }
                contentItem: Item {
                    Image {
                        anchors.centerIn: parent
                        width: 16
                        height: 16
                        rotation: -90
                        source: Qt.resolvedUrl("icons/go-up" + (page.darkTheme ? "-light" : "") + ".svg")
                        sourceSize.width: 32
                        sourceSize.height: 32
                    }
                }
                ToolTip.visible: hovered
                ToolTip.delay: 650
                ToolTip.text: qsTr("Back (Esc)")
            }

            Item {
                id: cover
                x: 24
                y: 52
                width: Math.min(220, Math.max(120, head.width * 0.28))
                height: width
                Rectangle {
                    anchors.fill: parent
                    radius: 10
                    visible: !art.visible
                    color: Qt.rgba(page.theme.text.r, page.theme.text.g, page.theme.text.b, 0.07)
                }
                Image {
                    id: art
                    anchors.fill: parent
                    source: head.coverUrl
                    visible: status === Image.Ready
                    asynchronous: false
                    smooth: true
                    mipmap: true
                    fillMode: Image.PreserveAspectFit
                    verticalAlignment: Image.AlignBottom
                }
            }

            Column {
                id: details
                anchors.left: cover.right
                anchors.leftMargin: 24
                anchors.right: parent.right
                anchors.rightMargin: 24
                anchors.bottom: cover.bottom
                spacing: 6

                Label {
                    text: qsTr("Album")
                    font.pixelSize: 12
                    font.bold: true
                    color: page.theme.text
                }
                Label {
                    width: parent.width
                    text: page.album ? page.album.album : ""
                    font.pixelSize: Math.round(Math.max(22, Math.min(44, head.width / 18)))
                    font.bold: true
                    color: page.theme.text
                    elide: Text.ElideRight
                    maximumLineCount: 2
                    wrapMode: Text.Wrap
                }
                Label {
                    width: parent.width
                    text: {
                        const album = page.album
                        if (!album)
                            return ""
                        const count = page.songs.length
                        return [album.artist || "", album.year ? String(album.year) : "",
                                count === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(count)]
                            .filter(part => part.length > 0).join("  ·  ")
                            + (album.lengthMs > 0 ? ", " + page.lengthLabel(album.lengthMs) : "")
                    }
                    font.pixelSize: 13
                    color: page.mutedText
                    elide: Text.ElideRight
                }
                Item {
                    width: 1
                    height: 8
                }
                // Plays the album from its first song; pauses and resumes
                // while one of its songs plays.
                AbstractButton {
                    id: play
                    width: 56
                    height: 56
                    hoverEnabled: true
                    focusPolicy: Qt.NoFocus
                    Accessible.name: page.albumPlaying ? qsTr("Pause") : qsTr("Play album")
                    onClicked: {
                        if (page.playingIndex >= 0)
                            page.app.play_pause()
                        else
                            page.app.play_entries(page.entriesJson, 0)
                    }
                    scale: pressed ? 0.96 : (hovered ? 1.05 : 1)
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    background: Rectangle {
                        radius: 28
                        color: page.theme.accent
                    }
                    contentItem: Item {
                        ControlsImpl.IconImage {
                            anchors.centerIn: parent
                            anchors.horizontalCenterOffset: page.albumPlaying ? 0 : 2
                            width: 22
                            height: 22
                            sourceSize.width: 22
                            sourceSize.height: 22
                            name: page.albumPlaying ? "media-playback-pause" : "media-playback-start"
                            color: "white"
                        }
                    }
                }
            }
        }

        delegate: PlaylistCardRow {
            required property int index
            width: songList.width - songList.verticalScrollGutter
            app: rows
            theme: page.theme
            searchModel: page.searchModel
            lines: page.rowLines
            rowIndex: index
            selected: songList.cursor === index
            fallbackCover: songList.headerItem ? songList.headerItem.coverUrl : ""
            onPressed: (row, modifiers, button) => {
                songList.forceActiveFocus()
                songList.cursor = row
            }
            onActivated: row => page.app.play_entries(page.entriesJson, row)
        }
    }

    // The mouse's back button closes it too.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.BackButton
        onClicked: page.close()
    }
}

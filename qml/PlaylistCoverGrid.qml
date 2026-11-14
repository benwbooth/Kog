pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

// Songs as cover tiles: the album art on top, the title and artist under
// it. Reads the same row data as PlaylistRow (track_value_at,
// track_tile_at, ...) and reports presses and activations the same way, so
// the play queue and the playlist tabs drive it like their lists.
GridView {
    id: grid

    required property var app
    required property var theme
    required property var searchModel
    property string searchQuery: ""
    // Whether a row is selected: a function, so the host's own selection
    // (read inside it) keeps each tile's binding up to date.
    property var selectionTest: function(row) { return false }

    // The song count. Assigning a new integer model resets the view to its
    // top, so songs arriving keep the scroll position, as the lists do.
    property int rowCount: 0
    model: 0
    function syncRowCount() {
        if (model === rowCount)
            return
        const previousY = contentY
        model = rowCount
        forceLayout()
        contentY = Math.max(-topMargin, Math.min(previousY,
            contentHeight + bottomMargin - height))
    }
    onRowCountChanged: syncRowCount()
    Component.onCompleted: syncRowCount()

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal openVisualizer()
    // Arrow keys, by rows of songs: -1/+1 sideways, ∓columns up and down.
    signal moveRequested(int delta, int modifiers)

    readonly property bool darkTiles: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.66,
        theme.base.g + (theme.text.g - theme.base.g) * 0.66,
        theme.base.b + (theme.text.b - theme.base.b) * 0.66, 1)

    readonly property real verticalScrollGutter: gridScrollBar.visible
        ? gridScrollBar.implicitWidth + 4 : 0
    readonly property real availableWidth: Math.max(0, width - leftMargin
        - rightMargin - verticalScrollGutter)
    // Tiles near 180 points wide, stretched to fill the row exactly.
    readonly property int columns: Math.max(1, Math.floor(availableWidth / 180))
    readonly property real coverSize: Math.max(48, Math.floor(cellWidth) - 20)

    leftMargin: 6
    rightMargin: 6
    topMargin: 6
    bottomMargin: 6
    cellWidth: Math.floor(availableWidth / columns)
    cellHeight: coverSize + 56
    clip: true
    reuseItems: true
    cacheBuffer: Math.min(height, 800)
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    maximumFlickVelocity: 12000
    flickDeceleration: 2200
    keyNavigationEnabled: false
    highlightMoveDuration: 0
    focus: true

    Keys.onLeftPressed: event => { grid.moveRequested(-1, event.modifiers); event.accepted = true }
    Keys.onRightPressed: event => { grid.moveRequested(1, event.modifiers); event.accepted = true }
    Keys.onUpPressed: event => { grid.moveRequested(-grid.columns, event.modifiers); event.accepted = true }
    Keys.onDownPressed: event => { grid.moveRequested(grid.columns, event.modifiers); event.accepted = true }

    onDraggingChanged: if (dragging) gridWheel.stop()
    onContentYChanged: gridScrollBar.flash()
    ScrollBar.vertical: FadingScrollBar {
        id: gridScrollBar
        edgeInset: 10
        onPressedChanged: if (pressed) gridWheel.stop()
    }
    KineticWheelHandler {
        id: gridWheel
        view: grid
    }

    delegate: Item {
        id: tile

        required property int index
        readonly property int revision: grid.app.playlist_revision
        readonly property bool selected: grid.selectionTest(index)
        property bool hovered: false

        width: grid.cellWidth
        height: grid.cellHeight

        function value(column) {
            revision
            return String(grid.app.track_value_at(index, column))
        }
        readonly property string trackPath: value("path")
        readonly property string titleText: value("title")
        readonly property string artistText: {
            const artist = value("artist")
            return artist.length > 0 ? artist : value("albumartist")
        }
        readonly property bool isCurrentTrack: {
            revision
            return grid.app.current_index >= 0
                && Number(grid.app.track_number_at(index)) === grid.app.current_index + 1
        }
        readonly property bool isPlaying: isCurrentTrack
            && grid.app.playback_state === "playing"
        readonly property bool isActiveTrack: isCurrentTrack
            && grid.app.playback_state !== "stopped"
        readonly property bool isMissing: {
            revision
            return grid.app.track_missing_at(index)
        }
        readonly property string formatIcon: {
            if (trackPath.length === 0)
                return "kog-format-audio"
            const name = String(grid.searchModel.icon_name(trackPath))
            return name.startsWith("kog-format-") ? name : "kog-format-audio"
        }

        // Asked for only once the tile has stayed on screen a moment, so a
        // fast scroll passes tiles by; looked up off the UI thread, arriving
        // through desktop_art_revision.
        property bool coverWanted: false
        onTrackPathChanged: {
            coverWanted = false
            if (trackPath.length > 0)
                coverSettle.restart()
            else
                coverSettle.stop()
        }
        Component.onCompleted: if (trackPath.length > 0) coverSettle.restart()
        GridView.onPooled: {
            hovered = false
            coverWanted = false
            coverSettle.stop()
            pointer.hoverX = -1
        }
        GridView.onReused: if (trackPath.length > 0) coverSettle.restart()
        Timer {
            id: coverSettle
            interval: 150
            onTriggered: tile.coverWanted = true
        }
        readonly property string coverUrl: {
            if (!coverWanted)
                return ""
            revision
            grid.app.desktop_art_revision
            return String(grid.app.track_tile_at(index))
        }

        // The selection and hover wash, behind the whole tile.
        Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            radius: 10
            color: tile.selected
                ? Qt.rgba(grid.theme.highlight.r, grid.theme.highlight.g,
                    grid.theme.highlight.b, 0.32)
                : (tile.hovered ? grid.theme.button : "transparent")
        }

        Item {
            id: art
            x: Math.round((tile.width - width) / 2)
            y: 10
            width: grid.coverSize
            height: grid.coverSize

            // No cover (yet): the format's icon on a quiet square.
            Rectangle {
                anchors.fill: parent
                radius: 7
                visible: !cover.visible
                color: Qt.rgba(grid.theme.text.r, grid.theme.text.g, grid.theme.text.b,
                    grid.darkTiles ? 0.07 : 0.06)

                Image {
                    anchors.centerIn: parent
                    width: Math.round(parent.width * 0.36)
                    height: width
                    source: Qt.resolvedUrl("icons/" + tile.formatIcon
                        + (grid.darkTiles ? "-light" : "") + ".svg")
                    sourceSize.width: 128
                    sourceSize.height: 128
                    fillMode: Image.PreserveAspectFit
                    opacity: 0.55
                }
            }
            // The tile image already has its corners rounded.
            Image {
                id: cover
                anchors.fill: parent
                source: tile.coverUrl
                visible: status === Image.Ready
                asynchronous: true
                cache: true
                smooth: true
                mipmap: true
                fillMode: Image.PreserveAspectFit
            }
            // The selected tile's cover gets the accent ring.
            Rectangle {
                anchors.fill: parent
                anchors.margins: -3
                radius: 10
                color: "transparent"
                border.width: 2
                border.color: grid.theme.highlight
                visible: tile.selected
            }
            // The playing song: its state and meter in a dark pill on the art.
            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 6
                width: tile.isPlaying ? 50 : 26
                height: 26
                radius: 13
                visible: tile.isActiveTrack
                color: Qt.rgba(0, 0, 0, 0.62)

                ControlsImpl.IconImage {
                    x: 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: 11
                    height: 11
                    sourceSize.width: 11
                    sourceSize.height: 11
                    name: tile.isPlaying ? "media-playback-start" : "media-playback-pause"
                    color: "white"
                }
                Loader {
                    anchors.right: parent.right
                    anchors.rightMargin: 4
                    anchors.verticalCenter: parent.verticalCenter
                    width: 22
                    height: 20
                    active: tile.isPlaying
                    sourceComponent: WaveformMeter {
                        app: grid.app
                        dark: true
                    }
                }
            }
        }

        SearchHighlightLabel {
            id: titleLabel
            x: art.x
            y: art.y + art.height + 7
            width: art.width
            sourceText: tile.titleText
            query: grid.searchQuery
            searchModel: grid.searchModel
            wholeQuery: true
            font.pixelSize: 13
            font.bold: true
            color: tile.isMissing ? grid.theme.placeholderText
                : (tile.isActiveTrack ? grid.theme.accent : grid.theme.text)
        }
        SearchHighlightLabel {
            id: artistLabel
            x: art.x
            y: titleLabel.y + titleLabel.height + 1
            width: art.width
            sourceText: tile.artistText
            query: grid.searchQuery
            searchModel: grid.searchModel
            wholeQuery: true
            font.pixelSize: 12
            color: tile.isMissing ? grid.theme.placeholderText : grid.mutedText
        }

        // The full text of a line, only when the line cuts it off.
        readonly property string hoverTip: {
            if (pointer.hoverX < 0)
                return ""
            for (const line of [titleLabel, artistLabel]) {
                if (pointer.hoverY >= line.y && pointer.hoverY < line.y + line.height
                        && pointer.hoverX >= line.x && pointer.hoverX < line.x + line.width)
                    return line.elided ? line.sourceText : ""
            }
            return ""
        }

        MouseArea {
            id: pointer
            property real hoverX: -1
            property real hoverY: -1
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            hoverEnabled: true
            onEntered: tile.hovered = true
            onExited: {
                tile.hovered = false
                hoverX = -1
            }
            onPositionChanged: mouse => {
                hoverX = mouse.x
                hoverY = mouse.y
            }
            onClicked: mouse => {
                grid.forceActiveFocus()
                grid.pressed(tile.index, mouse.modifiers, mouse.button)
            }
            onDoubleClicked: mouse => {
                if (mouse.button !== Qt.LeftButton)
                    return
                if (tile.isPlaying && mouse.x >= art.x + art.width - 60
                        && mouse.y >= art.y + art.height - 36
                        && mouse.y < art.y + art.height) {
                    grid.openVisualizer()
                    return
                }
                grid.activated(tile.index)
            }
        }

        ToolTip {
            x: Math.round(Math.max(0, Math.min(pointer.hoverX - width / 2, tile.width - width)))
            y: Math.round(pointer.hoverY - height - 10)
            delay: 650
            visible: tile.hovered && tile.hoverTip.length > 0
            text: tile.hoverTip
        }
    }
}

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

// Songs as cover tiles, one per album: the songs sharing an album tag and
// album artist (or artist) make one tile, with the album's name, its artist
// and how many songs it has; a song with no album tag is a tile of its own.
// Clicking an album opens its page (a big cover, its details, a play button
// and its songs as rows); Back, Escape or Backspace returns to the grid where
// it was. Reads the same row data as PlaylistRow (track_value_at,
// track_tile_at, ...) plus the host's tiles (groupSource), and reports
// presses and activations by row the same way, so the play queue and the
// playlist tabs drive it like their lists.
FocusScope {
    id: browser

    required property var app
    required property var theme
    required property var searchModel
    property string searchQuery: ""
    // Whether a row is selected: a function, so the host's own selection
    // (read inside it) keeps each tile's binding up to date.
    property var selectionTest: function(row) { return false }
    // The song count; the tiles are regrouped whenever it or the rows change.
    property int rowCount: 0
    // The tiles for the rows shown, as JSON from Rust:
    // [{album, artist, year, length, single, rows, numbers}].
    property var groupSource: function() { return "[]" }
    // What the album page's rows show, as the rows view's lines.
    property var rowLines: [["title"], ["artist"], ["album", "format", "length"], ["path"]]

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal openVisualizer()

    readonly property bool darkTiles: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.66,
        theme.base.g + (theme.text.g - theme.base.g) * 0.66,
        theme.base.b + (theme.text.b - theme.base.b) * 0.66, 1)

    // The tiles, the tile each row is in, and the tile each song number
    // (what track_number_at gives) is in, for finding the playing song.
    property var groups: []
    property var rowTile: []
    property var numberTile: ({})
    readonly property int revision: app.playlist_revision
    onRevisionChanged: regroup.restart()
    onRowCountChanged: regroup.restart()
    Component.onCompleted: refreshGroups()
    Timer {
        id: regroup
        interval: 120
        onTriggered: browser.refreshGroups()
    }
    function refreshGroups() {
        regroup.stop()
        let parsed = []
        if (rowCount > 0) {
            try {
                parsed = JSON.parse(groupSource())
            } catch (_) {
                parsed = []
            }
        }
        const tiles = []
        const numbers = {}
        for (let tile = 0; tile < parsed.length; ++tile) {
            for (const row of parsed[tile].rows)
                tiles[row] = tile
            for (const number of parsed[tile].numbers)
                numbers[number] = tile
        }
        groups = parsed
        rowTile = tiles
        numberTile = numbers
        grid.syncTileCount()
        // The open album follows its songs as the tiles move.
        if (openKey.length > 0) {
            const found = groups.findIndex(group => !group.single && groupKey(group) === openKey)
            if (found < 0)
                closeAlbum()
            else
                openTile = found
        }
    }
    function groupKey(group) {
        return group.album.toLowerCase() + "\u0001" + group.artist.toLowerCase()
    }
    function tileSelected(tile) {
        const group = groups[tile]
        if (!group)
            return false
        const rows = group.rows
        for (let i = 0; i < rows.length && i < 500; ++i) {
            if (selectionTest(rows[i]))
                return true
        }
        return false
    }
    // The playing song's tile.
    readonly property int playingTile: {
        const tile = numberTile[app.current_index + 1]
        return app.current_index >= 0 && tile !== undefined ? tile : -1
    }

    // Shows a row's tile, as the lists show a row.
    function positionViewAtIndex(row, mode) {
        const tile = rowTile[row]
        if (tile !== undefined && openTile < 0)
            grid.positionViewAtIndex(tile, mode)
    }

    // The album page: its tile, followed by album and artist as tiles move.
    property int openTile: -1
    property string openKey: ""
    readonly property var openGroup: openTile >= 0 && openTile < groups.length
        ? groups[openTile] : null
    property real gridScrollY: 0
    property int albumCursor: -1
    function openAlbum(tile) {
        const group = groups[tile]
        if (!group || group.single)
            return
        gridScrollY = grid.contentY
        grid.currentTile = tile
        openKey = groupKey(group)
        openTile = tile
        albumCursor = -1
        albumList.positionViewAtBeginning()
        albumList.forceActiveFocus()
    }
    function closeAlbum() {
        if (openKey.length === 0)
            return
        openKey = ""
        openTile = -1
        Qt.callLater(() => {
            grid.contentY = Math.max(-grid.topMargin, Math.min(gridScrollY,
                grid.contentHeight + grid.bottomMargin - grid.height))
            grid.forceActiveFocus()
        })
    }
    function playTile(tile) {
        const group = groups[tile]
        if (group && group.rows.length > 0)
            activated(group.rows[0])
    }

    GridView {
        id: grid

        anchors.fill: parent
        visible: browser.openGroup === null
        focus: visible

        // The tile count. Assigning a new integer model resets the view to
        // its top, so tiles arriving keep the scroll position, as the lists
        // do.
        model: 0
        function syncTileCount() {
            const count = browser.groups.length
            if (model === count)
                return
            const previousY = contentY
            model = count
            forceLayout()
            contentY = Math.max(-topMargin, Math.min(previousY,
                contentHeight + bottomMargin - height))
        }

        // The tile the arrow keys move from: the last one clicked or moved to.
        property int currentTile: -1
        function moveTile(delta) {
            const count = browser.groups.length
            if (count === 0)
                return
            const tile = Math.max(0, Math.min(count - 1, currentTile < 0 ? 0 : currentTile + delta))
            currentTile = tile
            browser.pressed(browser.groups[tile].rows[0], Qt.NoModifier, Qt.LeftButton)
            positionViewAtIndex(tile, GridView.Contain)
        }
        // Return opens the album; a lone song is left to the host to play.
        function openCurrent() {
            const group = browser.groups[currentTile]
            if (!group || group.single)
                return false
            browser.openAlbum(currentTile)
            return true
        }

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
        cellHeight: coverSize + 72
        clip: true
        reuseItems: true
        cacheBuffer: Math.min(height, 800)
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        maximumFlickVelocity: 12000
        flickDeceleration: 2200
        keyNavigationEnabled: false
        highlightMoveDuration: 0

        Keys.onLeftPressed: event => { grid.moveTile(-1); event.accepted = true }
        Keys.onRightPressed: event => { grid.moveTile(1); event.accepted = true }
        Keys.onUpPressed: event => { grid.moveTile(-grid.columns); event.accepted = true }
        Keys.onDownPressed: event => { grid.moveTile(grid.columns); event.accepted = true }
        Keys.onReturnPressed: event => event.accepted = grid.openCurrent()
        Keys.onEnterPressed: event => event.accepted = grid.openCurrent()

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
            readonly property int revision: browser.revision
            readonly property var group: browser.groups[index] || null
            readonly property bool single: !group || group.single
            // The tile's first song: its cover and, for a lone song, its title
            // stand for the tile.
            readonly property int row: group && group.rows.length > 0 ? group.rows[0] : -1
            readonly property int songCount: group ? group.rows.length : 0
            readonly property bool selected: {
                revision
                return browser.tileSelected(index)
            }
            property bool hovered: false

            width: grid.cellWidth
            height: grid.cellHeight

            function value(column) {
                revision
                return row >= 0 ? String(browser.app.track_value_at(row, column)) : ""
            }
            readonly property string trackPath: value("path")
            readonly property string titleText: single ? value("title") : group.album
            readonly property string artistText: {
                if (!single)
                    return group.artist
                const artist = value("artist")
                return artist.length > 0 ? artist : value("albumartist")
            }
            readonly property string countText: single ? ""
                : (songCount === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(songCount))
            readonly property bool isCurrentTrack: browser.playingTile === index
            readonly property bool isPlaying: isCurrentTrack
                && browser.app.playback_state === "playing"
            readonly property bool isActiveTrack: isCurrentTrack
                && browser.app.playback_state !== "stopped"
            readonly property bool isMissing: {
                revision
                return single && row >= 0 && browser.app.track_missing_at(row)
            }
            readonly property string formatIcon: {
                if (trackPath.length === 0)
                    return "kog-format-audio"
                const name = String(browser.searchModel.icon_name(trackPath))
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
                browser.app.desktop_art_revision
                return row >= 0 ? String(browser.app.track_tile_at(row)) : ""
            }

            // The selection and hover wash, behind the whole tile.
            Rectangle {
                anchors.fill: parent
                anchors.margins: 3
                radius: 10
                color: tile.selected
                    ? Qt.rgba(browser.theme.highlight.r, browser.theme.highlight.g,
                        browser.theme.highlight.b, 0.32)
                    : (tile.hovered ? browser.theme.button : "transparent")
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
                    color: Qt.rgba(browser.theme.text.r, browser.theme.text.g, browser.theme.text.b,
                        browser.darkTiles ? 0.07 : 0.06)

                    Image {
                        anchors.centerIn: parent
                        width: Math.round(parent.width * 0.36)
                        height: width
                        source: Qt.resolvedUrl("icons/" + tile.formatIcon
                            + (browser.darkTiles ? "-light" : "") + ".svg")
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
                    border.color: browser.theme.highlight
                    visible: tile.selected
                }
                // Plays the album (or the song), Spotify style: shown while the
                // tile is pointed at.
                Rectangle {
                    id: playButton
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 8
                    width: 40
                    height: 40
                    radius: 20
                    visible: !tile.isActiveTrack && tile.row >= 0
                    opacity: tile.hovered ? 1 : 0
                    scale: pointer.overPlay ? 1.06 : 1
                    color: browser.theme.accent
                    Behavior on opacity { NumberAnimation { duration: 120 } }
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    ControlsImpl.IconImage {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: 1
                        width: 16
                        height: 16
                        sourceSize.width: 16
                        sourceSize.height: 16
                        name: "media-playback-start"
                        color: "white"
                    }
                }
                // The playing song: its state and meter in a dark pill on the art.
                Rectangle {
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 6
                    width: tile.isPlaying ? 62 : 30
                    height: 30
                    radius: 15
                    visible: tile.isActiveTrack
                    color: Qt.rgba(0, 0, 0, 0.66)

                    ControlsImpl.IconImage {
                        x: 10
                        anchors.verticalCenter: parent.verticalCenter
                        width: 11
                        height: 11
                        sourceSize.width: 11
                        sourceSize.height: 11
                        name: tile.isPlaying ? "media-playback-start" : "media-playback-pause"
                        color: "white"
                    }
                    // White bars, as tall as the pill allows, so the meter reads
                    // on any cover.
                    Loader {
                        anchors.right: parent.right
                        anchors.rightMargin: 6
                        anchors.verticalCenter: parent.verticalCenter
                        width: 28
                        height: 22
                        active: tile.isPlaying
                        sourceComponent: WaveformMeter {
                            app: browser.app
                            dark: true
                            highlighted: true
                            highlightColor: "white"
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
                query: browser.searchQuery
                searchModel: browser.searchModel
                wholeQuery: true
                font.pixelSize: 13
                font.bold: true
                color: tile.isMissing ? browser.theme.placeholderText
                    : (tile.isActiveTrack ? browser.theme.accent : browser.theme.text)
            }
            SearchHighlightLabel {
                id: artistLabel
                x: art.x
                y: titleLabel.y + titleLabel.height + 1
                width: art.width
                sourceText: tile.artistText
                query: browser.searchQuery
                searchModel: browser.searchModel
                wholeQuery: true
                font.pixelSize: 12
                color: tile.isMissing ? browser.theme.placeholderText : browser.mutedText
            }
            // An album's song count.
            Label {
                x: art.x
                y: tile.artistText.length > 0 ? artistLabel.y + artistLabel.height + 1
                    : artistLabel.y
                width: art.width
                visible: !tile.single
                text: tile.countText
                font.pixelSize: 11
                color: browser.mutedText
                opacity: 0.85
                elide: Text.ElideRight
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
                readonly property bool overPlay: playButton.visible && hoverX >= 0
                    && hoverX >= art.x + art.width - 48 && hoverX < art.x + art.width
                    && hoverY >= art.y + art.height - 48 && hoverY < art.y + art.height
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
                // A click selects the tile's first song; on an album it also
                // opens the album's page, and on the play button it plays.
                onClicked: mouse => {
                    grid.forceActiveFocus()
                    grid.currentTile = tile.index
                    if (tile.row < 0)
                        return
                    if (mouse.button === Qt.LeftButton && overPlay) {
                        browser.playTile(tile.index)
                        return
                    }
                    browser.pressed(tile.row, tile.single ? mouse.modifiers : Qt.NoModifier, mouse.button)
                    if (mouse.button === Qt.LeftButton && !tile.single
                            && !(mouse.modifiers & (Qt.ControlModifier | Qt.ShiftModifier | Qt.MetaModifier)))
                        browser.openAlbum(tile.index)
                }
                onDoubleClicked: mouse => {
                    if (mouse.button !== Qt.LeftButton)
                        return
                    if (tile.isPlaying && mouse.x >= art.x + art.width - 60
                            && mouse.y >= art.y + art.height - 36
                            && mouse.y < art.y + art.height) {
                        browser.openVisualizer()
                        return
                    }
                    if (tile.single && tile.row >= 0 && !overPlay)
                        browser.activated(tile.row)
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

    // The album page: a big cover beside "Album", the title, the artist,
    // year, song count and length, a big play button, then the songs as the
    // rows view shows them.
    ListView {
        id: albumList

        anchors.fill: parent
        visible: browser.openGroup !== null
        focus: visible
        clip: true
        model: browser.openGroup ? browser.openGroup.rows.length : 0
        boundsBehavior: Flickable.StopAtBounds
        maximumFlickVelocity: 12000
        flickDeceleration: 2200
        reuseItems: true
        cacheBuffer: Math.min(height, 800)

        readonly property var group: browser.openGroup
        readonly property int firstRow: group && group.rows.length > 0 ? group.rows[0] : -1
        readonly property real verticalScrollGutter: albumScrollBar.visible
            ? albumScrollBar.implicitWidth + 4 : 0

        function moveCursor(delta) {
            const rows = group ? group.rows : []
            if (rows.length === 0)
                return
            browser.albumCursor = Math.max(0, Math.min(rows.length - 1,
                browser.albumCursor < 0 ? 0 : browser.albumCursor + delta))
            browser.pressed(rows[browser.albumCursor], Qt.NoModifier, Qt.LeftButton)
            positionViewAtIndex(browser.albumCursor, ListView.Contain)
        }

        Keys.onEscapePressed: event => { browser.closeAlbum(); event.accepted = true }
        Keys.onBackPressed: event => { browser.closeAlbum(); event.accepted = true }
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Backspace) {
                browser.closeAlbum()
                event.accepted = true
            }
        }
        Keys.onUpPressed: event => { moveCursor(-1); event.accepted = true }
        Keys.onDownPressed: event => { moveCursor(1); event.accepted = true }
        Keys.onReturnPressed: event => {
            if (group && browser.albumCursor >= 0)
                browser.activated(group.rows[browser.albumCursor])
            event.accepted = true
        }
        Keys.onEnterPressed: event => {
            if (group && browser.albumCursor >= 0)
                browser.activated(group.rows[browser.albumCursor])
            event.accepted = true
        }

        onContentYChanged: albumScrollBar.flash()
        ScrollBar.vertical: FadingScrollBar {
            id: albumScrollBar
            edgeInset: 10
            onPressedChanged: if (pressed) albumWheel.stop()
        }
        KineticWheelHandler {
            id: albumWheel
            view: albumList
        }

        header: Item {
            id: albumHeader
            width: albumList.width - albumList.verticalScrollGutter
            height: Math.max(albumCover.height, details.implicitHeight) + 72

            readonly property string coverUrl: {
                browser.revision
                browser.app.desktop_art_revision
                return albumList.firstRow >= 0
                    ? String(browser.app.track_tile_at(albumList.firstRow)) : ""
            }

            // Back to the grid, where it was.
            AbstractButton {
                id: backButton
                x: 12
                y: 10
                width: 32
                height: 32
                hoverEnabled: true
                focusPolicy: Qt.NoFocus
                Accessible.name: qsTr("Back")
                onClicked: browser.closeAlbum()
                background: Rectangle {
                    radius: 16
                    color: Qt.rgba(browser.theme.text.r, browser.theme.text.g, browser.theme.text.b,
                        backButton.pressed ? 0.16 : (backButton.hovered ? 0.1 : 0.05))
                }
                contentItem: Item {
                    Image {
                        anchors.centerIn: parent
                        width: 16
                        height: 16
                        rotation: -90
                        source: Qt.resolvedUrl("icons/go-up" + (browser.darkTiles ? "-light" : "") + ".svg")
                        sourceSize.width: 32
                        sourceSize.height: 32
                    }
                }
                ToolTip.visible: hovered
                ToolTip.delay: 650
                ToolTip.text: qsTr("Back (Esc)")
            }

            Item {
                id: albumCover
                x: 24
                y: 52
                width: Math.min(220, Math.max(120, albumHeader.width * 0.28))
                height: width

                Rectangle {
                    anchors.fill: parent
                    radius: 10
                    visible: !albumArt.visible
                    color: Qt.rgba(browser.theme.text.r, browser.theme.text.g, browser.theme.text.b,
                        browser.darkTiles ? 0.07 : 0.06)
                    Image {
                        anchors.centerIn: parent
                        width: Math.round(parent.width * 0.32)
                        height: width
                        source: Qt.resolvedUrl("icons/kog-format-audio"
                            + (browser.darkTiles ? "-light" : "") + ".svg")
                        sourceSize.width: 128
                        sourceSize.height: 128
                        opacity: 0.55
                    }
                }
                Image {
                    id: albumArt
                    anchors.fill: parent
                    source: albumHeader.coverUrl
                    visible: status === Image.Ready
                    asynchronous: true
                    smooth: true
                    mipmap: true
                    fillMode: Image.PreserveAspectFit
                }
            }

            Column {
                id: details
                anchors.left: albumCover.right
                anchors.leftMargin: 24
                anchors.right: parent.right
                anchors.rightMargin: 24
                anchors.bottom: albumCover.bottom
                spacing: 6

                Label {
                    text: qsTr("Album")
                    font.pixelSize: 12
                    font.bold: true
                    color: browser.theme.text
                }
                Label {
                    width: parent.width
                    text: albumList.group ? albumList.group.album : ""
                    font.pixelSize: Math.max(22, Math.min(44, albumHeader.width / 18))
                    font.bold: true
                    color: browser.theme.text
                    elide: Text.ElideRight
                    maximumLineCount: 2
                    wrapMode: Text.Wrap
                }
                Label {
                    width: parent.width
                    text: {
                        const group = albumList.group
                        if (!group)
                            return ""
                        const count = group.rows.length
                        return [group.artist, group.year,
                                count === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(count)]
                            .filter(part => part.length > 0).join("  ·  ")
                            + (group.length.length > 0 ? ", " + group.length : "")
                    }
                    font.pixelSize: 13
                    color: browser.mutedText
                    elide: Text.ElideRight
                }
                Item {
                    width: 1
                    height: 8
                }
                // Plays the album from its first song.
                AbstractButton {
                    id: bigPlay
                    width: 56
                    height: 56
                    hoverEnabled: true
                    focusPolicy: Qt.NoFocus
                    Accessible.name: qsTr("Play album")
                    onClicked: if (albumList.firstRow >= 0) browser.activated(albumList.firstRow)
                    scale: pressed ? 0.96 : (hovered ? 1.05 : 1)
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    background: Rectangle {
                        radius: 28
                        color: browser.theme.accent
                    }
                    contentItem: Item {
                        ControlsImpl.IconImage {
                            anchors.centerIn: parent
                            anchors.horizontalCenterOffset: 2
                            width: 22
                            height: 22
                            sourceSize.width: 22
                            sourceSize.height: 22
                            name: "media-playback-start"
                            color: "white"
                        }
                    }
                }
            }
        }

        delegate: PlaylistCardRow {
            required property int index
            width: albumList.width - albumList.verticalScrollGutter
            app: browser.app
            theme: browser.theme
            searchModel: browser.searchModel
            searchQuery: browser.searchQuery
            lines: browser.rowLines
            rowIndex: albumList.group && index < albumList.group.rows.length
                ? albumList.group.rows[index] : -1
            selected: {
                browser.revision
                return rowIndex >= 0 && browser.selectionTest(rowIndex)
            }
            onOpenVisualizer: browser.openVisualizer()
            onPressed: (row, modifiers, button) => {
                albumList.forceActiveFocus()
                browser.albumCursor = index
                browser.pressed(row, modifiers, button)
            }
            onActivated: row => browser.activated(row)
        }
    }

    // The mouse's back button returns from an album, too.
    MouseArea {
        anchors.fill: parent
        visible: browser.openGroup !== null
        acceptedButtons: Qt.BackButton
        onClicked: browser.closeAlbum()
    }
}

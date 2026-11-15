pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

// Songs as cover tiles, one per album: the songs sharing an album tag and
// album artist (or artist) make one tile, with the album's name, its artist
// and how many songs it has; a song with no album tag is a tile of its own.
// Clicking an album opens its page (a big cover, its details, a play button
// and its songs as rows); Back, Escape or Backspace returns to the grid where
// it was. With flow set, the albums show instead as a cover flow over the
// chosen album's songs. Reads the same row data as PlaylistRow (track_value_at,
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
    // [{key, album, artist, year, length, single, rows, numbers}].
    property var groupSource: function() { return "[]" }
    // Show the albums as a cover flow, with the chosen album's songs below,
    // instead of a grid.
    property bool flow: false
    // What the album page's rows show, as the rows view's lines.
    property var rowLines: ["title", "artist", "album", "format", "length", "path"]

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal openVisualizer()

    readonly property bool darkTiles: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.66,
        theme.base.g + (theme.text.g - theme.base.g) * 0.66,
        theme.base.b + (theme.text.b - theme.base.b) * 0.66, 1)

    // A cover for an album (or song) without art: its title in bold and
    // its artist under it, on a quiet tint picked from the title, so tiles
    // tell apart at a glance without a generic icon.
    function tintFor(text) {
        let hash = 0
        for (let i = 0; i < text.length; ++i)
            hash = (hash * 31 + text.charCodeAt(i)) | 0
        const hue = ((hash % 360) + 360) % 360 / 360
        return darkTiles ? Qt.hsla(hue, 0.28, 0.2, 1) : Qt.hsla(hue, 0.32, 0.86, 1)
    }
    component TextCover: Rectangle {
        id: textCover
        property string title: ""
        property string subtitle: ""
        readonly property real size: Math.min(width, height)
        radius: 7
        color: browser.tintFor(title + "\u0001" + subtitle)
        clip: true
        Column {
            x: Math.round(textCover.size * 0.09)
            y: Math.round(textCover.size * 0.09)
            width: textCover.width - 2 * x
            spacing: Math.round(textCover.size * 0.03)
            Label {
                width: parent.width
                text: textCover.title
                font.pixelSize: Math.max(9, Math.round(textCover.size * 0.11))
                font.bold: true
                lineHeight: 0.95
                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                maximumLineCount: 4
                elide: Text.ElideRight
                color: browser.theme.text
            }
            Label {
                width: parent.width
                visible: text.length > 0
                text: textCover.subtitle
                font.pixelSize: Math.max(8, Math.round(textCover.size * 0.08))
                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                maximumLineCount: 2
                elide: Text.ElideRight
                color: browser.mutedText
            }
        }
    }

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
    // Albums are grouped only once every song's tags are in, so the grid
    // never shows songs apart that later merge into their album: until then
    // it keeps the tiles it had (none for a list just opened) and says how
    // many songs are left to read.
    property int tagsPending: 0
    // What the rows belong to (a playlist tab's key): another list starts
    // without the last one's tiles.
    property string groupScope: ""
    onGroupScopeChanged: {
        groups = []
        rowTile = []
        numberTile = ({})
        grid.syncTileCount()
        regroup.restart()
    }
    function refreshGroups() {
        regroup.stop()
        tagsPending = rowCount > 0 && app.tags_pending ? Number(app.tags_pending()) : 0
        if (tagsPending > 0) {
            // Ask again shortly: tags arrive in batches as files are read.
            regroup.restart()
            return
        }
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
        return group.key
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
    // The playing song's tile: by its place when the list knows it, else
    // by what it is (see PlaylistCardRow.isCurrentTrack).
    readonly property int playingTile: {
        if (app.current_index >= 0) {
            const tile = numberTile[app.current_index + 1]
            return tile !== undefined ? tile : -1
        }
        const playing = String(app.current_file || "")
        if (playing.length === 0 || app.playback_state === "stopped")
            return -1
        revision
        for (let row = 0; row < rowTile.length; ++row) {
            if (String(app.track_value_at(row, "source")) === playing)
                return rowTile[row] === undefined ? -1 : rowTile[row]
        }
        return -1
    }

    // Following the playing song: when it changes, the grid scrolls its
    // album into view (leaving an open album page alone) and the cover
    // flow springs to its album and shows it in the songs below. Someone
    // browsing is not pulled away mid-gesture: the follow waits until they
    // have been idle a few seconds.
    property real lastBrowse: 0
    property bool followPending: true
    function noteBrowse() {
        lastBrowse = Date.now()
    }
    Connections {
        target: browser.app
        function onCurrent_indexChanged() {
            browser.followPending = true
            Qt.callLater(browser.followPlaying)
        }
    }
    onPlayingTileChanged: if (followPending) Qt.callLater(followPlaying)
    onVisibleChanged: if (visible) {
        followPending = true
        lastBrowse = 0
        Qt.callLater(followPlaying)
    }
    onFlowChanged: if (flow) {
        followPending = true
        lastBrowse = 0
        Qt.callLater(followPlaying)
    }
    Timer {
        id: followIdle
        interval: 500
        onTriggered: browser.followPlaying()
    }
    function followPlaying() {
        if (!followPending || !visible)
            return
        const tile = playingTile
        if (tile < 0)
            return
        const idle = Date.now() - lastBrowse
        if (idle < 3000 || flowPane.dragging || grid.moving) {
            followIdle.interval = Math.max(200, 3000 - idle)
            followIdle.restart()
            return
        }
        followPending = false
        if (flow) {
            flowPane.showTile(tile)
        } else if (openTile < 0) {
            grid.positionViewAtIndex(tile, GridView.Contain)
        }
    }

    // Shows a row's tile, as the lists show a row.
    function positionViewAtIndex(row, mode) {
        const tile = rowTile[row]
        if (tile === undefined)
            return
        if (flow)
            flowPane.moveTo(tile)
        else if (openTile < 0)
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
    // An album's play button: pauses or resumes while the playing song is
    // one of its songs, and plays it from its first song otherwise.
    function tileActive(tile) {
        return tile >= 0 && tile === playingTile && app.playback_state !== "stopped"
    }
    function tilePlaying(tile) {
        return tile >= 0 && tile === playingTile && app.playback_state === "playing"
    }
    function togglePlayTile(tile) {
        if (tileActive(tile))
            app.play_pause()
        else
            playTile(tile)
    }

    GridView {
        id: grid

        anchors.fill: parent
        visible: !browser.flow && browser.openGroup === null
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
        onMovingChanged: if (moving) browser.noteBrowse()
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
            // A cover already known shows on the tile's first frame, with
            // no wait; only unknown ones wait for the tile to settle.
            readonly property string knownCover: {
                revision
                browser.app.desktop_art_revision
                return row >= 0 && browser.app.track_known_image_at
                    ? String(browser.app.track_known_image_at(row, true)) : "?"
            }
            readonly property bool coverKnown: knownCover !== "?"
            readonly property string coverUrl: {
                if (coverKnown)
                    return knownCover
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

                // No cover: the album's (or song's) name and artist.
                TextCover {
                    anchors.fill: parent
                    visible: !cover.visible
                    title: tile.titleText
                    subtitle: tile.artistText
                }
                // The tile image already has its corners rounded.
                Image {
                    id: cover
                    anchors.fill: parent
                    source: tile.coverUrl
                    visible: status === Image.Ready
                    asynchronous: !tile.coverKnown
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
                    visible: tile.row >= 0
                    opacity: tile.hovered ? 1 : 0
                    scale: pointer.overPlay ? 1.06 : 1
                    color: browser.theme.accent
                    Behavior on opacity { NumberAnimation { duration: 120 } }
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    ControlsImpl.IconImage {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: tile.isPlaying ? 0 : 1
                        width: 16
                        height: 16
                        sourceSize.width: 16
                        sourceSize.height: 16
                        name: tile.isPlaying ? "media-playback-pause" : "media-playback-start"
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
                    // The play button takes its place while pointed at.
                    visible: tile.isActiveTrack && !tile.hovered
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
                        browser.togglePlayTile(tile.index)
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
        visible: !browser.flow && browser.openGroup !== null
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
        // The album's cover, which its rows show until they have their own.
        readonly property string coverUrl: {
            browser.revision
            browser.app.desktop_art_revision
            if (firstRow < 0)
                return ""
            const known = browser.app.track_known_image_at
                ? String(browser.app.track_known_image_at(firstRow, true)) : "?"
            return known !== "?" ? known : String(browser.app.track_tile_at(firstRow))
        }
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

            readonly property string coverUrl: albumList.coverUrl

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

                TextCover {
                    anchors.fill: parent
                    radius: 10
                    visible: !albumArt.visible
                    title: albumList.group ? albumList.group.album : ""
                    subtitle: albumList.group ? albumList.group.artist : ""
                }
                Image {
                    id: albumArt
                    anchors.fill: parent
                    source: albumHeader.coverUrl
                    visible: status === Image.Ready
                    asynchronous: false
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
                    readonly property bool albumPlaying: browser.tilePlaying(browser.openTile)
                    Accessible.name: albumPlaying ? qsTr("Pause") : qsTr("Play album")
                    onClicked: browser.togglePlayTile(browser.openTile)
                    scale: pressed ? 0.96 : (hovered ? 1.05 : 1)
                    Behavior on scale { NumberAnimation { duration: 90 } }
                    background: Rectangle {
                        radius: 28
                        color: browser.theme.accent
                    }
                    contentItem: Item {
                        ControlsImpl.IconImage {
                            anchors.centerIn: parent
                            anchors.horizontalCenterOffset: bigPlay.albumPlaying ? 0 : 2
                            width: 22
                            height: 22
                            sourceSize.width: 22
                            sourceSize.height: 22
                            name: bigPlay.albumPlaying ? "media-playback-pause" : "media-playback-start"
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
            fallbackCover: albumList.coverUrl
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

    // The cover flow: the albums as covers in a row, the centre one facing
    // forward and its neighbours turned and receding, each over its
    // reflection. Arrows, the wheel and dragging move through them on a
    // spring; clicking an album lists its songs in the pane below, under a
    // splitter that can be dragged.
    Item {
        id: flowPane

        anchors.fill: parent
        visible: browser.flow
        focus: visible

        // The flow's share of the pane's height, kept with the preferences.
        property real split: {
            const saved = browser.app.load_ui_setting
                ? Number(browser.app.load_ui_setting("coverFlowSplit", "0.5")) : 0.5
            return saved > 0.15 && saved < 0.9 ? saved : 0.5
        }
        function saveSplit() {
            if (browser.app.save_ui_setting)
                browser.app.save_ui_setting("coverFlowSplit", String(split))
        }

        // The album the arrows move to, and the shown position, which
        // springs after it (or follows the pointer while dragging).
        property int target: 0
        property real position: 0
        property bool dragging: false
        readonly property int count: browser.groups.length
        readonly property int centre: Math.max(0, Math.min(count - 1, Math.round(position)))
        onTargetChanged: if (!dragging) position = target
        Behavior on position {
            enabled: !flowPane.dragging
            SpringAnimation {
                spring: 3.2
                damping: 0.32
                mass: 1
                epsilon: 0.0005
            }
        }
        function moveTo(tile) {
            target = Math.max(0, Math.min(count - 1, tile))
        }
        // Centres the album and lists its songs, revealing the playing one.
        function showTile(tile) {
            const group = browser.groups[tile]
            if (!group)
                return
            moveTo(tile)
            chosenKey = group.single ? "row:" + group.rows[0] : group.key
            const at = group.numbers.indexOf(browser.app.current_index + 1)
            Qt.callLater(() => {
                if (at >= 0)
                    flowSongs.positionViewAtIndex(at, ListView.Contain)
            })
        }
        onCountChanged: if (target >= count) moveTo(count - 1)

        // The album listed below, by its key so it stays put as tiles move;
        // until one is clicked, the first.
        property string chosenKey: ""
        readonly property var chosenGroup: {
            const groups = browser.groups
            if (chosenKey.length > 0) {
                const found = groups.find(group => (group.single ? "row:" + group.rows[0] : group.key) === chosenKey)
                if (found)
                    return found
            }
            return groups.length > 0 ? groups[0] : null
        }
        function choose(tile) {
            const group = browser.groups[tile]
            if (!group)
                return
            moveTo(tile)
            chosenKey = group.single ? "row:" + group.rows[0] : group.key
            browser.albumCursor = -1
            if (group.rows.length > 0)
                browser.pressed(group.rows[0], Qt.NoModifier, Qt.LeftButton)
            flowSongs.positionViewAtBeginning()
        }

        Keys.onLeftPressed: event => { browser.noteBrowse(); moveTo(target - 1); event.accepted = true }
        Keys.onRightPressed: event => { browser.noteBrowse(); moveTo(target + 1); event.accepted = true }
        Keys.onPressed: event => {
            browser.noteBrowse()
            if (event.key === Qt.Key_Home) {
                moveTo(0)
                event.accepted = true
            } else if (event.key === Qt.Key_End) {
                moveTo(count - 1)
                event.accepted = true
            } else if (event.key === Qt.Key_PageUp) {
                moveTo(target - 5)
                event.accepted = true
            } else if (event.key === Qt.Key_PageDown) {
                moveTo(target + 5)
                event.accepted = true
            }
        }
        Keys.onReturnPressed: event => { choose(target); event.accepted = true }
        Keys.onEnterPressed: event => { choose(target); event.accepted = true }
        Keys.onDownPressed: event => { flowSongs.forceActiveFocus(); event.accepted = true }

        Item {
            id: coverFlow

            width: parent.width
            height: Math.round(Math.max(150, Math.min(parent.height - 90,
                parent.height * flowPane.split)))
            clip: true

            // The covers' size and spacing: the centre one fills most of the
            // height above its reflection, its neighbours tuck in behind.
            readonly property real coverSize: Math.max(60, Math.min((height - 46) / 1.3, width * 0.36))
            readonly property real centreGap: coverSize * 0.72
            readonly property real sideGap: coverSize * 0.26
            readonly property int side: Math.max(2, Math.min(9,
                Math.ceil((width / 2 - centreGap) / sideGap) + 1))
            readonly property int slots: side * 2 + 1
            readonly property real coverTop: 12

            // Slot k shows the tile within reach of the centre whose index
            // leaves k modulo the slot count, so moving one album changes
            // only the slot that wraps around.
            Repeater {
                model: coverFlow.slots

                Item {
                    id: slot

                    required property int index
                    readonly property int first: Math.round(flowPane.position) - coverFlow.side
                    readonly property int tile: first
                        + (((index - first) % coverFlow.slots) + coverFlow.slots) % coverFlow.slots
                    readonly property var group: tile >= 0 && tile < flowPane.count
                        ? browser.groups[tile] : null
                    readonly property real offset: tile - flowPane.position
                    readonly property real distance: Math.abs(offset)
                    readonly property real turn: Math.max(-1, Math.min(1, offset))
                    readonly property string coverUrl: {
                        if (!group)
                            return ""
                        browser.revision
                        browser.app.desktop_art_revision
                        const known = browser.app.track_known_image_at
                            ? String(browser.app.track_known_image_at(group.rows[0], true)) : "?"
                        return known !== "?" ? known : String(browser.app.track_tile_at(group.rows[0]))
                    }
                    readonly property bool coverKnown: {
                        if (!group)
                            return false
                        browser.app.desktop_art_revision
                        return !!browser.app.track_known_image_at
                            && String(browser.app.track_known_image_at(group.rows[0], true)) !== "?"
                    }
                    readonly property string titleText: {
                        if (!group)
                            return ""
                        if (!group.single)
                            return group.album
                        browser.revision
                        return String(browser.app.track_value_at(group.rows[0], "title"))
                    }

                    visible: group !== null && distance <= coverFlow.side
                    width: coverFlow.coverSize
                    height: coverFlow.coverSize * 1.3
                    x: coverFlow.width / 2 - width / 2 + (distance < 1
                        ? offset * coverFlow.centreGap
                        : Math.sign(offset) * (coverFlow.centreGap + (distance - 1) * coverFlow.sideGap))
                    y: coverFlow.coverTop
                    z: 1000 - Math.round(distance * 10)
                    scale: distance < 1 ? 1 - 0.18 * distance : 0.82
                    transform: Rotation {
                        origin.x: slot.width / 2
                        origin.y: coverFlow.coverSize / 2
                        axis { x: 0; y: 1; z: 0 }
                        angle: -58 * slot.turn
                    }

                    Item {
                        id: coverBox
                        width: coverFlow.coverSize
                        height: width

                        TextCover {
                            id: blank
                            width: coverFlow.coverSize
                            height: width
                            radius: 6
                            visible: !art.visible
                            title: slot.titleText
                            subtitle: slot.group ? slot.group.artist : ""
                        }
                        Image {
                            id: art
                            width: coverFlow.coverSize
                            height: width
                            source: slot.coverUrl
                            visible: status === Image.Ready
                            asynchronous: !slot.coverKnown
                            cache: true
                            smooth: true
                            mipmap: true
                            fillMode: Image.PreserveAspectFit
                        }
                    }
                    // The centre album's play button, shown while the flow is
                    // pointed at or the album is playing; pauses and resumes
                    // the album playing.
                    Rectangle {
                        x: coverFlow.coverSize - width - 10
                        y: coverFlow.coverSize - height - 10
                        width: 44
                        height: 44
                        radius: 22
                        visible: slot.distance < 0.5
                            && (flowPointer.containsMouse || browser.tileActive(slot.tile))
                        color: browser.theme.accent
                        ControlsImpl.IconImage {
                            anchors.centerIn: parent
                            anchors.horizontalCenterOffset: browser.tilePlaying(slot.tile) ? 0 : 1
                            width: 18
                            height: 18
                            sourceSize.width: 18
                            sourceSize.height: 18
                            name: browser.tilePlaying(slot.tile) ? "media-playback-pause" : "media-playback-start"
                            color: "white"
                        }
                    }
                    // The reflection: the cover (or its blank) upside down,
                    // fading out.
                    Item {
                        y: coverFlow.coverSize + 2
                        width: coverFlow.coverSize
                        height: coverFlow.coverSize * 0.3 - 2
                        clip: true
                        opacity: 0.32
                        ShaderEffectSource {
                            width: coverFlow.coverSize
                            height: width
                            sourceItem: coverBox
                            smooth: true
                            transform: Scale {
                                origin.y: coverFlow.coverSize / 2
                                yScale: -1
                            }
                        }
                        Rectangle {
                            anchors.fill: parent
                            gradient: Gradient {
                                GradientStop {
                                    position: 0
                                    color: Qt.rgba(browser.theme.base.r, browser.theme.base.g,
                                        browser.theme.base.b, 0.2)
                                }
                                GradientStop { position: 0.85; color: browser.theme.base }
                            }
                        }
                    }
                }
            }

            // The centre album's name and artist under its reflection.
            Column {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 6
                width: Math.min(parent.width - 24, coverFlow.coverSize * 2.4)
                z: 2000
                readonly property var group: browser.groups[flowPane.centre] || null

                Label {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: {
                        const group = parent.group
                        if (!group)
                            return ""
                        if (!group.single)
                            return group.album
                        browser.revision
                        return String(browser.app.track_value_at(group.rows[0], "title"))
                    }
                    font.pixelSize: 14
                    font.bold: true
                    color: browser.theme.text
                    elide: Text.ElideRight
                }
                Label {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: {
                        const group = parent.group
                        if (!group)
                            return ""
                        const count = group.rows.length
                        return [group.artist, group.single ? ""
                                : (count === 1 ? qsTr("1 song") : qsTr("%1 songs").arg(count))]
                            .filter(part => part.length > 0).join("  ·  ")
                    }
                    font.pixelSize: 12
                    color: browser.mutedText
                    elide: Text.ElideRight
                }
            }

            // Wheel, drag and click: the wheel steps an album per notch (or
            // per 60 pixels of a touchpad's swipe), a drag follows the
            // pointer and coasts to the nearest album, a click brings an
            // album to the centre and lists its songs, and a double click on
            // the centre plays it.
            MouseArea {
                id: flowPointer
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton
                hoverEnabled: true
                z: 3000
                property real wheelRest: 0
                property real pressX: 0
                property real pressPosition: 0
                property real lastX: 0
                property real lastTime: 0
                property real velocity: 0
                property bool moved: false

                onWheel: wheel => {
                    browser.noteBrowse()
                    const pixels = wheel.pixelDelta.x !== 0 || wheel.pixelDelta.y !== 0
                    const delta = Math.abs(wheel.angleDelta.x) > Math.abs(wheel.angleDelta.y)
                        ? -wheel.angleDelta.x : -wheel.angleDelta.y
                    const pixelDelta = Math.abs(wheel.pixelDelta.x) > Math.abs(wheel.pixelDelta.y)
                        ? -wheel.pixelDelta.x : -wheel.pixelDelta.y
                    wheelRest += pixels ? pixelDelta / 60 : delta / 120
                    const steps = wheelRest > 0 ? Math.floor(wheelRest) : Math.ceil(wheelRest)
                    if (steps !== 0) {
                        wheelRest -= steps
                        flowPane.moveTo(flowPane.target + steps)
                    }
                    wheel.accepted = true
                }
                onPressed: mouse => {
                    browser.noteBrowse()
                    flowPane.forceActiveFocus()
                    pressX = mouse.x
                    lastX = mouse.x
                    lastTime = Date.now()
                    velocity = 0
                    pressPosition = flowPane.position
                    moved = false
                }
                onPositionChanged: mouse => {
                    if (!pressed)
                        return
                    if (!moved && Math.abs(mouse.x - pressX) < 6)
                        return
                    if (!moved) {
                        moved = true
                        flowPane.dragging = true
                    }
                    const now = Date.now()
                    const elapsed = Math.max(1, now - lastTime)
                    velocity = 0.7 * velocity + 0.3 * (mouse.x - lastX) / elapsed
                    lastX = mouse.x
                    lastTime = now
                    flowPane.position = Math.max(-0.4, Math.min(flowPane.count - 0.6,
                        pressPosition - (mouse.x - pressX) / coverFlow.sideGap / 1.6))
                }
                onReleased: mouse => {
                    if (!moved)
                        return
                    // Coast on: the release speed carries a few albums on.
                    const coast = -velocity * 260 / coverFlow.sideGap / 1.6
                    flowPane.dragging = false
                    const landing = Math.max(0, Math.min(flowPane.count - 1,
                        Math.round(flowPane.position + coast)))
                    if (landing === flowPane.target)
                        flowPane.position = landing
                    flowPane.target = landing
                }
                onCanceled: {
                    flowPane.dragging = false
                    flowPane.position = flowPane.target
                }
                function tileAt(x, y) {
                    // The nearest covers are on top: try them first.
                    let best = -1
                    let bestDistance = 1e9
                    for (let i = 0; i < coverFlow.children.length; ++i) {
                        const item = coverFlow.children[i]
                        if (item.tile === undefined || !item.visible)
                            continue
                        const half = coverFlow.coverSize * item.scale
                            * (item.distance < 1 ? 1 - 0.5 * item.distance : 0.5) / 2
                        const centreX = item.x + item.width / 2
                        if (Math.abs(x - centreX) <= half && y >= coverFlow.coverTop
                                && y <= coverFlow.coverTop + coverFlow.coverSize
                                && item.distance < bestDistance) {
                            best = item.tile
                            bestDistance = item.distance
                        }
                    }
                    return best
                }
                // Whether a point is on the centre cover's play button.
                function onPlayButton(x, y) {
                    const centreX = coverFlow.width / 2 + (flowPane.centre - flowPane.position) * coverFlow.centreGap
                    const right = centreX + coverFlow.coverSize / 2 - 10
                    const bottom = coverFlow.coverTop + coverFlow.coverSize - 10
                    return x >= right - 44 && x <= right && y >= bottom - 44 && y <= bottom
                }
                onClicked: mouse => {
                    if (moved)
                        return
                    if (onPlayButton(mouse.x, mouse.y)) {
                        browser.togglePlayTile(flowPane.centre)
                        return
                    }
                    const tile = tileAt(mouse.x, mouse.y)
                    if (tile >= 0)
                        flowPane.choose(tile)
                }
                onDoubleClicked: mouse => {
                    const tile = tileAt(mouse.x, mouse.y)
                    if (tile >= 0 && tile === flowPane.centre)
                        browser.togglePlayTile(tile)
                }
            }
        }

        // The splitter between the flow and the songs.
        Rectangle {
            id: flowSplitter
            y: coverFlow.height
            width: parent.width
            height: 7
            color: splitDrag.containsMouse || splitDrag.pressed
                ? Qt.rgba(browser.theme.highlight.r, browser.theme.highlight.g, browser.theme.highlight.b, 0.35)
                : "transparent"
            Rectangle {
                anchors.centerIn: parent
                width: 44
                height: 3
                radius: 1.5
                color: browser.mutedText
                opacity: 0.6
            }
            MouseArea {
                id: splitDrag
                anchors.fill: parent
                anchors.topMargin: -3
                anchors.bottomMargin: -3
                hoverEnabled: true
                cursorShape: Qt.SplitVCursor
                onPositionChanged: mouse => {
                    if (!pressed)
                        return
                    const y = mapToItem(flowPane, mouse.x, mouse.y).y
                    flowPane.split = Math.max(0.2, Math.min(0.85, y / Math.max(1, flowPane.height)))
                }
                onReleased: flowPane.saveSplit()
            }
        }

        // The chosen album's songs, as the rows view shows them.
        ListView {
            id: flowSongs

            anchors.top: flowSplitter.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            clip: true
            model: flowPane.chosenGroup ? flowPane.chosenGroup.rows.length : 0
            boundsBehavior: Flickable.StopAtBounds
            maximumFlickVelocity: 12000
            flickDeceleration: 2200
            reuseItems: true
            cacheBuffer: Math.min(height, 800)

            readonly property var group: flowPane.chosenGroup
            // The album's cover, which its rows show until they have their own.
            readonly property string coverUrl: {
                browser.revision
                browser.app.desktop_art_revision
                if (!group || group.single)
                    return ""
                const known = browser.app.track_known_image_at
                    ? String(browser.app.track_known_image_at(group.rows[0], true)) : "?"
                return known !== "?" ? known : String(browser.app.track_tile_at(group.rows[0]))
            }
            readonly property real verticalScrollGutter: flowSongsBar.visible
                ? flowSongsBar.implicitWidth + 4 : 0

            function moveCursor(delta) {
                const rows = group ? group.rows : []
                if (rows.length === 0)
                    return
                browser.albumCursor = Math.max(0, Math.min(rows.length - 1,
                    browser.albumCursor < 0 ? 0 : browser.albumCursor + delta))
                browser.pressed(rows[browser.albumCursor], Qt.NoModifier, Qt.LeftButton)
                positionViewAtIndex(browser.albumCursor, ListView.Contain)
            }
            Keys.onUpPressed: event => {
                if (browser.albumCursor <= 0)
                    flowPane.forceActiveFocus()
                else
                    moveCursor(-1)
                event.accepted = true
            }
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

            onContentYChanged: flowSongsBar.flash()
            ScrollBar.vertical: FadingScrollBar {
                id: flowSongsBar
                edgeInset: 10
                onPressedChanged: if (pressed) flowSongsWheel.stop()
            }
            KineticWheelHandler {
                id: flowSongsWheel
                view: flowSongs
            }

            delegate: PlaylistCardRow {
                required property int index
                width: flowSongs.width - flowSongs.verticalScrollGutter
                app: browser.app
                theme: browser.theme
                searchModel: browser.searchModel
                searchQuery: browser.searchQuery
                lines: browser.rowLines
                rowIndex: flowSongs.group && index < flowSongs.group.rows.length
                    ? flowSongs.group.rows[index] : -1
                fallbackCover: flowSongs.coverUrl
                selected: {
                    browser.revision
                    return rowIndex >= 0 && browser.selectionTest(rowIndex)
                }
                onOpenVisualizer: browser.openVisualizer()
                onPressed: (row, modifiers, button) => {
                    flowSongs.forceActiveFocus()
                    browser.albumCursor = index
                    browser.pressed(row, modifiers, button)
                }
                onActivated: row => browser.activated(row)
            }
        }
    }

    // While a list's tags are still being read and no tiles show yet.
    Label {
        anchors.centerIn: parent
        visible: browser.tagsPending > 0 && browser.groups.length === 0
        text: qsTr("Reading tags… %1 songs to go").arg(browser.tagsPending)
        color: browser.mutedText
        font.pixelSize: 13
    }

    // The mouse's back button returns from an album, too.
    MouseArea {
        anchors.fill: parent
        visible: browser.openGroup !== null
        acceptedButtons: Qt.BackButton
        onClicked: browser.closeAlbum()
    }
}

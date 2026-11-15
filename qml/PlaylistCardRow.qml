pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl

// A song laid out like a message in a mail list: the album cover on the
// left, then up to four lines of chosen fields: by default the title in
// bold, the artist, a quieter line with the album, format and length, and
// the full path in small text. Takes the same data, selection and drag signals
// as PlaylistRow, so a list can swap one for the other.
Item {
    id: root

    required property var app
    required property int rowIndex
    required property var theme
    required property var searchModel
    property string searchQuery: ""
    property bool selected: false
    property bool hovered: false
    property int revision: app.playlist_revision

    readonly property bool isCurrentTrack: {
        revision
        return app.current_index >= 0
            && Number(app.track_number_at(rowIndex)) === app.current_index + 1
    }
    readonly property bool isPlaying: isCurrentTrack
        && app.playback_state === "playing"
    readonly property bool isActiveTrack: isCurrentTrack
        && app.playback_state !== "stopped"
    readonly property bool isMissing: {
        revision
        return app.track_missing_at(rowIndex)
    }
    readonly property bool darkRows: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.66,
        theme.base.g + (theme.text.g - theme.base.g) * 0.66,
        theme.base.b + (theme.text.b - theme.base.b) * 0.66, 1)
    readonly property color textColor: selected ? theme.highlightedText
        : (isMissing ? theme.placeholderText : theme.text)
    readonly property color quietColor: selected
        ? Qt.rgba(theme.highlightedText.r, theme.highlightedText.g,
            theme.highlightedText.b, 0.78)
        : mutedText

    function value(column) {
        revision
        return String(app.track_value_at(rowIndex, column))
    }
    readonly property string trackPath: value("path")
    // The fields to show, as a flat list; the row places each by its role:
    // the title (always) in bold, then the artist (falling back to the
    // album artist, and the album artist beside it when it is shown and
    // differs), then a quiet line of album, year, track number, genre,
    // format, length and play count, then the path (or the filename) in
    // small text. A line with nothing to show is left out, and every line
    // wraps rather than cutting its text off.
    property var lines: ["title", "artist", "album", "format", "length", "path"]
    function shows(field) {
        return field === "title" || (Array.isArray(lines) && lines.indexOf(field) >= 0)
    }
    readonly property string formatText: {
        const codec = value("codec")
        if (codec.length > 0)
            return codec
        const name = trackPath.slice(trackPath.lastIndexOf("/") + 1)
        const dot = name.lastIndexOf(".")
        return dot > 0 ? name.slice(dot + 1).toUpperCase() : ""
    }
    readonly property var lineTexts: {
        revision
        const artist = value("artist")
        const albumArtist = value("albumartist")
        const people = []
        if (shows("artist"))
            people.push(artist.length > 0 ? artist : albumArtist)
        if (shows("albumartist") && albumArtist.length > 0 && people.indexOf(albumArtist) < 0)
            people.push(albumArtist)
        const track = value("track")
        const details = [
            shows("album") ? value("album") : "",
            shows("date") ? value("date") : "",
            shows("track") && track.length > 0 ? "#" + track : "",
            shows("genre") ? value("genre") : "",
            shows("format") ? formatText : "",
            shows("length") ? value("length") : "",
            shows("playcount") ? value("playcount") : ""
        ]
        // The path holds the filename: it shows alone when both are on.
        const file = shows("path") ? trackPath : (shows("filename") ? value("filename") : "")
        return [
            value("title"),
            people.filter(part => part.length > 0).join("  ·  "),
            details.filter(part => part.length > 0).join("  ·  "),
            file
        ]
    }
    readonly property bool starred: value("star").length > 0

    signal pressed(int rowIndex, int modifiers, int button)
    signal activated(int rowIndex)
    signal openVisualizer()
    signal dragStarted(int rowIndex)
    signal dragMoved(real viewX, real viewY)
    signal dragFinished(real viewX, real viewY)
    signal dragCanceled()

    // As tall as the wrapped lines, and no shorter than the cover.
    implicitHeight: Math.max(58, Math.ceil(copy.implicitHeight) + 14)
    height: implicitHeight

    ListView.onPooled: {
        root.hovered = false
        pointer.manualDragging = false
        pointer.suppressNextClick = false
        pointer.hoverX = -1
        pointer.hoverY = -1
        coverWanted = false
    }
    ListView.onReused: if (trackPath.length > 0) coverSettle.restart()

    Rectangle {
        anchors.fill: parent
        anchors.leftMargin: 6
        anchors.rightMargin: 6
        anchors.topMargin: 2
        anchors.bottomMargin: 2
        radius: 6
        color: root.selected
            ? root.theme.highlight
            : (root.hovered
                ? root.theme.button
                : (root.rowIndex % 2 ? root.theme.alternateBase : "transparent"))
    }

    // The format icon until the cover has loaded, and for songs without
    // one. A row asks for its cover only once it has stayed on screen a
    // moment, so a fast scroll passes rows by; the lookup runs off the UI
    // thread and the answer arrives through desktop_art_revision.
    readonly property string formatIcon: {
        if (trackPath.length === 0)
            return "kog-format-audio"
        const name = String(searchModel.icon_name(trackPath))
        return name.startsWith("kog-format-") ? name : "kog-format-audio"
    }
    property bool coverWanted: false
    onTrackPathChanged: {
        coverWanted = false
        if (trackPath.length > 0)
            coverSettle.restart()
        else
            coverSettle.stop()
    }
    Component.onCompleted: if (trackPath.length > 0) coverSettle.restart()
    Timer {
        id: coverSettle
        interval: 150
        onTriggered: root.coverWanted = true
    }
    // A cover already known shows on the row's first frame, with no wait;
    // only unknown ones wait to be looked up. An album's page hands its
    // rows the album's cover to show meanwhile.
    property string fallbackCover: ""
    readonly property string knownCover: {
        revision
        app.desktop_art_revision
        return app.track_known_image_at ? String(app.track_known_image_at(rowIndex, false)) : "?"
    }
    readonly property bool coverKnown: knownCover !== "?"
    readonly property string coverUrl: {
        if (coverKnown)
            return knownCover.length > 0 ? knownCover : fallbackCover
        if (!coverWanted)
            return fallbackCover
        revision
        app.desktop_art_revision
        const url = String(app.track_cover_at(rowIndex))
        return url.length > 0 ? url : fallbackCover
    }

    Item {
        id: art
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 42
        height: 42

        Rectangle {
            anchors.fill: parent
            radius: 6
            visible: !cover.visible
            color: root.selected
                ? Qt.rgba(root.theme.highlightedText.r, root.theme.highlightedText.g,
                    root.theme.highlightedText.b, 0.14)
                : Qt.rgba(root.theme.text.r, root.theme.text.g, root.theme.text.b,
                    root.darkRows ? 0.07 : 0.06)

            Image {
                anchors.centerIn: parent
                width: 22
                height: 22
                source: Qt.resolvedUrl("icons/" + root.formatIcon
                    + (root.selected || root.darkRows ? "-light" : "") + ".svg")
                sourceSize.width: 44
                sourceSize.height: 44
                fillMode: Image.PreserveAspectFit
                opacity: 0.8
            }
        }
        // The small cover already has its corners rounded.
        Image {
            id: cover
            anchors.fill: parent
            source: root.coverUrl
            visible: status === Image.Ready
            // A known cover loads at once, so the row never shows its
            // stand-in first.
            asynchronous: !root.coverKnown
            cache: true
            smooth: true
            mipmap: true
            fillMode: Image.PreserveAspectFit
        }
    }

    Item {
        id: trailing
        anchors.right: parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        // Room for the playing icon and meter on every row, so the text never
        // reflows when a song starts or stops.
        width: 62
        height: 24

        Item {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: 36
            height: 22
            visible: root.isActiveTrack

            ControlsImpl.IconImage {
                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: 12
                height: 12
                sourceSize.width: 12
                sourceSize.height: 12
                name: root.isPlaying ? "media-playback-start" : "media-playback-pause"
                color: root.selected ? root.theme.highlightedText : root.theme.accent
            }
            Loader {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                width: 22
                height: 22
                active: root.isPlaying
                sourceComponent: WaveformMeter {
                    app: root.app
                    highlighted: root.selected
                    highlightColor: root.theme.highlightedText
                    dark: root.darkRows
                }
            }
        }

        Image {
            id: star
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 16
            height: 16
            source: Qt.resolvedUrl(root.starred
                ? "icons/star-filled.svg" : "icons/star-outline.svg")
            // Empty stars wait out of sight until the row is pointed at.
            opacity: root.starred ? 1 : (pointer.overStar ? 0.9 : (root.hovered ? 0.45 : 0))
            fillMode: Image.PreserveAspectFit
            mipmap: true
            Accessible.name: root.starred ? qsTr("Unstar") : qsTr("Star")
            Accessible.role: Accessible.Button
        }
    }

    Column {
        id: copy
        anchors.left: art.right
        anchors.leftMargin: 12
        anchors.right: trailing.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1

        SearchHighlightLabel {
            id: titleLabel
            visible: root.lineTexts[0].length > 0
            width: copy.width
            sourceText: root.lineTexts[0]
            wrap: true
            query: root.searchQuery
            searchModel: root.searchModel
            wholeQuery: true
            font.pixelSize: 13
            font.bold: true
            // The playing song's title takes the accent, as in Earshot.
            color: root.isActiveTrack && !root.selected ? root.theme.accent : root.textColor
        }
        SearchHighlightLabel {
            id: artistLabel
            visible: root.lineTexts[1].length > 0
            width: copy.width
            sourceText: root.lineTexts[1]
            wrap: true
            query: root.searchQuery
            searchModel: root.searchModel
            wholeQuery: true
            font.pixelSize: 12
            color: root.textColor
        }
        SearchHighlightLabel {
            id: detailLabel
            visible: root.lineTexts[2].length > 0
            width: copy.width
            sourceText: root.lineTexts[2]
            wrap: true
            query: root.searchQuery
            searchModel: root.searchModel
            wholeQuery: true
            font.pixelSize: 11
            color: root.quietColor
        }
        // The path (or the filename), small and quiet.
        SearchHighlightLabel {
            id: pathLabel
            visible: root.lineTexts[3].length > 0
            width: copy.width
            sourceText: root.lineTexts[3]
            wrap: true
            query: root.searchQuery
            searchModel: root.searchModel
            wholeQuery: true
            font.pixelSize: 10
            color: root.quietColor
            opacity: root.selected ? 1 : 0.85
        }
    }

    // The full text of a line, only when the line cuts it off (lines wrap,
    // so only while a search highlight is laid out).
    function lineTip(x, y) {
        if (x < 0)
            return ""
        for (const line of [titleLabel, artistLabel, detailLabel, pathLabel]) {
            if (!line.visible)
                continue
            const point = line.mapFromItem(root, x, y)
            if (point.x >= 0 && point.x < line.width && point.y >= 0
                    && point.y < line.height)
                return line.elided ? line.sourceText : ""
        }
        return ""
    }
    readonly property string hoverTip: {
        revision
        return lineTip(pointer.hoverX, pointer.hoverY)
    }

    MouseArea {
        id: pointer
        property real pressX: 0
        property real pressY: 0
        property bool manualDragging: false
        property bool suppressNextClick: false
        property real hoverX: -1
        property real hoverY: -1
        readonly property bool overStar: hoverX >= 0
            && hoverX >= trailing.x + star.x - 6
            && hoverX <= trailing.x + star.x + star.width + 6

        function starHit(x) {
            return x >= trailing.x + star.x - 6
                && x <= trailing.x + star.x + star.width + 6
        }
        function meterHit(x) {
            return root.isPlaying && x >= trailing.x && x < trailing.x + 40
        }

        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        hoverEnabled: true
        preventStealing: true
        scrollGestureEnabled: false
        cursorShape: overStar ? Qt.PointingHandCursor : Qt.ArrowCursor
        onEntered: root.hovered = true
        onExited: {
            root.hovered = false
            hoverX = -1
            hoverY = -1
        }
        onPressed: mouse => {
            pressX = mouse.x
            pressY = mouse.y
            manualDragging = false
            suppressNextClick = false
        }
        onPositionChanged: mouse => {
            hoverX = mouse.x
            hoverY = mouse.y
            if ((mouse.buttons & Qt.LeftButton) === 0)
                return
            if (!manualDragging
                    && (Math.abs(mouse.x - pressX)
                        >= Application.styleHints.startDragDistance
                        || Math.abs(mouse.y - pressY)
                        >= Application.styleHints.startDragDistance)) {
                manualDragging = true
                root.dragStarted(root.rowIndex)
            }
            if (!manualDragging)
                return
            const point = root.mapToItem(root.ListView.view, mouse.x, mouse.y)
            root.dragMoved(point.x, point.y)
        }
        onClicked: mouse => {
            if (suppressNextClick) {
                suppressNextClick = false
                return
            }
            if (mouse.button === Qt.LeftButton && meterHit(mouse.x)) {
                root.openVisualizer()
                return
            }
            if (mouse.button === Qt.LeftButton && starHit(mouse.x)) {
                root.app.toggle_stars(String(root.rowIndex))
                return
            }
            root.pressed(root.rowIndex, mouse.modifiers, mouse.button)
        }
        onDoubleClicked: mouse => {
            if (meterHit(mouse.x)) {
                root.openVisualizer()
                return
            }
            if (starHit(mouse.x))
                return
            root.activated(root.rowIndex)
        }
        onReleased: mouse => {
            if (manualDragging) {
                const point = root.mapToItem(root.ListView.view, mouse.x, mouse.y)
                root.dragFinished(point.x, point.y)
                suppressNextClick = true
                mouse.accepted = true
            }
            manualDragging = false
        }
        onCanceled: {
            manualDragging = false
            suppressNextClick = false
            root.dragCanceled()
        }
    }

    ToolTip {
        id: lineToolTip
        x: Math.round(Math.max(0, Math.min(pointer.hoverX - width / 2, root.width - width)))
        y: Math.round(pointer.hoverY - height - 10)
        delay: 650
        visible: root.hovered && !pointer.manualDragging && root.hoverTip.length > 0
        text: root.hoverTip
    }
}

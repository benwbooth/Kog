pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Effects

// The pinned Home tab: mixes made for the listener from their plays, stars
// and the library index (Daily Mixes, Discover Weekly, unplayed albums and
// more), one row of cards per section. Clicking a card opens the mix as a
// read-only playlist tab; its play button plays it now. The mixes are made
// in Rust once a day; this view only shows what it is given.
Item {
    id: home

    required property var app
    required property var theme

    property var homeData: null
    property var status: ({})
    property int seenGeneration: -1

    readonly property bool dark: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5
    readonly property color mutedText: Qt.rgba(
        theme.base.r + (theme.text.r - theme.base.r) * 0.62,
        theme.base.g + (theme.text.g - theme.base.g) * 0.62,
        theme.base.b + (theme.text.b - theme.base.b) * 0.62, 1)
    readonly property color accent: theme.accent !== undefined ? theme.accent : theme.highlight
    readonly property var sections: homeData && homeData.sections ? homeData.sections : []

    function reload() {
        const generation = app.discover_generation()
        if (generation === seenGeneration)
            return
        seenGeneration = generation
        try {
            const parsed = JSON.parse(app.discover_json())
            status = parsed.status || {}
            homeData = parsed.home || null
        } catch (_) {
        }
    }
    Component.onCompleted: reload()
    onVisibleChanged: if (visible) reload()
    Timer {
        interval: 2000
        repeat: true
        running: home.visible
        onTriggered: home.reload()
    }

    property bool refreshing: false
    function number(value) {
        return Number(value || 0).toLocaleString(Qt.locale(), "f", 0)
    }
    function progressText() {
        const s = status
        let text = qsTr("%1 songs read, %2 measured").arg(number(s.songs)).arg(number(s.analysed))
        if ((s.embedded || 0) > 0)
            text += qsTr(", %1 heard by the sound model").arg(number(s.embedded))
        text += (s.folders_left || 0) > 0
            ? qsTr(". Still reading your library; mixes get better as it goes.")
            : qsTr(". Your whole library has been read.")
        if (refreshing)
            text = qsTr("Making new mixes… ") + text
        return text
    }
    // The progress and the refresh, every second while Home shows.
    Timer {
        interval: 1000
        repeat: true
        running: home.visible
        triggeredOnStart: true
        onTriggered: {
            try {
                const parsed = JSON.parse(home.app.discover_status_json())
                home.status = parsed.status || {}
                if (!parsed.refreshing && home.refreshing)
                    home.reload()
                home.refreshing = !!parsed.refreshing
            } catch (_) {
            }
        }
    }

    function greeting() {
        const hour = new Date().getHours()
        if (hour < 5) return qsTr("Good night")
        if (hour < 12) return qsTr("Good morning")
        if (hour < 18) return qsTr("Good afternoon")
        return qsTr("Good evening")
    }

    // A quiet tint picked from the title, for a cover without art.
    function tintFor(text) {
        let hash = 0
        for (let i = 0; i < text.length; ++i)
            hash = (hash * 31 + text.charCodeAt(i)) | 0
        const hue = ((hash % 360) + 360) % 360 / 360
        return dark ? Qt.hsla(hue, 0.32, 0.24, 1) : Qt.hsla(hue, 0.36, 0.82, 1)
    }

    // Up to four songs from different albums, for a card's cover mosaic.
    function coverSongs(mix) {
        const picked = []
        const albums = {}
        for (const song of mix.songs || []) {
            const key = (song.album || song.title || "").toLowerCase()
            if (albums[key])
                continue
            albums[key] = true
            picked.push(song)
            if (picked.length === 4)
                break
        }
        return picked
    }

    Flickable {
        id: page
        anchors.fill: parent
        clip: true
        contentWidth: width
        contentHeight: column.implicitHeight + 32
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: FadingScrollBar { id: pageBar }
        onContentYChanged: pageBar.flash()
        KineticWheelHandler { view: page }

        Column {
            id: column
            x: 24
            y: 20
            width: page.width - 48
            spacing: 26

            Item {
                width: parent.width
                height: titleLabel.implicitHeight
                Label {
                    id: titleLabel
                    text: home.greeting()
                    font.pixelSize: 26
                    font.bold: true
                    color: home.theme.text
                }
                ToolButton {
                    id: refreshButton
                    anchors.right: parent.right
                    anchors.verticalCenter: titleLabel.verticalCenter
                    visible: home.sections.length > 0
                    // Turns while new mixes are made, finishing its turn
                    // when they arrive.
                    RotationAnimator on rotation {
                        running: home.refreshing
                        from: 0
                        to: 360
                        duration: 900
                        loops: Animation.Infinite
                        alwaysRunToEnd: true
                    }
                    icon.name: "view-refresh"
                    display: AbstractButton.IconOnly
                    flat: true
                    Accessible.name: qsTr("Make new mixes")
                    ToolTip.visible: hovered
                    ToolTip.delay: 600
                    ToolTip.text: Accessible.name
                    onClicked: {
                        home.refreshing = true
                        home.app.discover_refresh()
                    }
                }
            }

            // How far the background reading has got.
            Label {
                visible: home.sections.length > 0 && (home.status.songs || 0) > 0
                width: parent.width
                wrapMode: Text.WordWrap
                color: home.mutedText
                font.pixelSize: 12
                text: home.progressText()
            }

            // Before there is anything to show: what is happening, plainly.
            Label {
                visible: home.sections.length === 0
                width: Math.min(parent.width, 520)
                wrapMode: Text.WordWrap
                color: home.mutedText
                font.pixelSize: 15
                text: (home.status.songs || 0) > 0
                    ? qsTr("Kog is reading your library in the background (%1 songs so far). Your mixes appear here as soon as it has enough to go on; listening to music helps it learn what you like.")
                        .arg(Number(home.status.songs).toLocaleString(Qt.locale(), "f", 0))
                    : qsTr("Your mixes appear here once Kog has read some of your library. Choose a music folder in Preferences if you have not yet; listening to music helps it learn what you like.")
            }

            Repeater {
                model: home.sections
                delegate: Column {
                    id: section
                    required property var modelData
                    width: column.width
                    spacing: 12

                    Item {
                        width: parent.width
                        height: sectionTitle.implicitHeight
                        Label {
                            id: sectionTitle
                            text: section.modelData.title
                            font.pixelSize: 19
                            font.bold: true
                            color: home.theme.text
                        }
                        // Page through a row wider than the view.
                        Row {
                            anchors.right: parent.right
                            anchors.verticalCenter: sectionTitle.verticalCenter
                            visible: row.contentWidth > row.width + 1
                            ToolButton {
                                icon.name: "go-previous"
                                display: AbstractButton.IconOnly
                                flat: true
                                enabled: row.contentX > 0.5
                                Accessible.name: qsTr("Scroll %1 back").arg(section.modelData.title)
                                onClicked: row.page(-1)
                            }
                            ToolButton {
                                icon.name: "go-next"
                                display: AbstractButton.IconOnly
                                flat: true
                                enabled: row.contentX < row.contentWidth - row.width - 0.5
                                Accessible.name: qsTr("Scroll %1 on").arg(section.modelData.title)
                                onClicked: row.page(1)
                            }
                        }
                    }

                    ListView {
                        id: row
                        width: parent.width
                        height: 250
                        orientation: ListView.Horizontal
                        spacing: 14
                        clip: true
                        // Vertical wheel turns scroll the page; sideways
                        // scrolling, the arrows and the scroll bar move the row.
                        interactive: false
                        boundsBehavior: Flickable.StopAtBounds
                        model: section.modelData.mixes
                        ScrollBar.horizontal: FadingScrollBar { id: rowBar }
                        onContentXChanged: rowBar.flash()
                        KineticWheelHandler { view: row; orientation: Qt.Horizontal }
                        function page(direction) {
                            const step = Math.max(186, Math.floor(width / 186) * 186)
                            const target = Math.max(0, Math.min(contentWidth - width,
                                contentX + direction * step))
                            pager.to = target
                            pager.restart()
                        }
                        NumberAnimation {
                            id: pager
                            target: row
                            property: "contentX"
                            duration: 320
                            easing.type: Easing.OutCubic
                        }
                        delegate: MixCard {
                            required property var modelData
                            mix: modelData
                        }
                    }
                }
            }
        }
    }

    component MixCard: Item {
        id: card
        property var mix: ({})
        readonly property var covers: home.coverSongs(mix)
        readonly property bool single: mix.kind === "album" || covers.length < 4
        // Covers loaded so far: the title card hides once one shows.
        property int artReady: 0
        width: 172
        height: 250

        HoverHandler {
            id: hover
            cursorShape: Qt.PointingHandCursor
        }
        TapHandler {
            onTapped: home.app.open_mix(card.mix.id, card.mix.title)
        }

        Rectangle {
            anchors.fill: parent
            anchors.margins: -6
            radius: 10
            color: hover.hovered ? Qt.rgba(home.theme.text.r, home.theme.text.g, home.theme.text.b, 0.07)
                : "transparent"
            Behavior on color { ColorAnimation { duration: 140 } }
        }

        Item {
            id: cover
            width: 172
            height: 172

            // Covers show whole on a quiet backdrop; a card without any
            // shows its title on a tint instead, never both.
            Rectangle {
                anchors.fill: parent
                radius: 8
                color: card.artReady > 0
                    ? Qt.rgba(home.theme.text.r, home.theme.text.g, home.theme.text.b, 0.06)
                    : home.tintFor(card.mix.title || "")
                Behavior on color { ColorAnimation { duration: 180 } }
                Label {
                    x: 14
                    y: 14
                    width: parent.width - 28
                    height: parent.height - 28
                    opacity: card.artReady > 0 ? 0 : 1
                    Behavior on opacity { NumberAnimation { duration: 180 } }
                    text: card.mix.title || ""
                    font.pixelSize: 17
                    font.bold: true
                    fontSizeMode: Text.Fit
                    minimumPixelSize: 11
                    wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                    maximumLineCount: 5
                    elide: Text.ElideRight
                    clip: true
                    color: home.theme.text
                }
            }
            Item {
                id: art
                anchors.fill: parent
                // Rounded corners: the covers are masked by the cover's shape.
                layer.enabled: true
                layer.smooth: true
                layer.effect: MultiEffect {
                    maskEnabled: true
                    maskSource: coverMask
                    maskThresholdMin: 0.5
                    maskSpreadAtMin: 1.0
                }
                Repeater {
                    model: card.single ? card.covers.slice(0, 1) : card.covers
                    delegate: Image {
                        id: tile
                        required property var modelData
                        required property int index
                        readonly property real side: card.single ? 172 : 86
                        x: card.single ? 0 : (index % 2) * 86
                        y: card.single ? 0 : Math.floor(index / 2) * 86
                        width: side
                        height: side
                        sourceSize.width: 256
                        sourceSize.height: 256
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                        mipmap: true
                        opacity: status === Image.Ready ? 1 : 0
                        property bool counted: false
                        onStatusChanged: {
                            const ready = status === Image.Ready
                            if (ready !== counted) {
                                card.artReady += ready ? 1 : -1
                                counted = ready
                            }
                        }
                        Component.onDestruction: if (counted) card.artReady -= 1
                        Behavior on opacity { NumberAnimation { duration: 180 } }
                        source: {
                            home.app.desktop_art_revision
                            return String(home.app.mix_cover(modelData.kind || "local",
                                modelData.path || "", modelData.entry || "",
                                modelData.album || "", modelData.artist || ""))
                        }
                    }
                }
            }
            Item {
                id: coverMask
                width: cover.width
                height: cover.height
                visible: false
                layer.enabled: true
                layer.smooth: true
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: "black"
                }
            }

            // The play button, rising into place while the card is pointed at.
            Rectangle {
                id: playButton
                width: 44
                height: 44
                radius: 22
                x: cover.width - width - 10
                y: cover.height - height - (hover.hovered ? 10 : 2)
                opacity: hover.hovered ? 1 : 0
                color: playArea.pressed ? Qt.darker(home.accent, 1.15) : home.accent
                Behavior on opacity { NumberAnimation { duration: 150; easing.type: Easing.OutCubic } }
                Behavior on y { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
                ControlsImpl.IconImage {
                    anchors.centerIn: parent
                    anchors.horizontalCenterOffset: 1
                    width: 18
                    height: 18
                    sourceSize.width: 18
                    sourceSize.height: 18
                    name: "media-playback-start"
                    color: "white"
                }
                MouseArea {
                    id: playArea
                    anchors.fill: parent
                    enabled: playButton.opacity > 0.5
                    cursorShape: Qt.PointingHandCursor
                    onClicked: home.app.play_mix(card.mix.id, 0)
                }
                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Play %1").arg(card.mix.title || "")
            }
        }

        Label {
            id: cardTitle
            y: cover.height + 10
            width: card.width
            text: card.mix.title || ""
            font.pixelSize: 14
            font.bold: true
            elide: Text.ElideRight
            color: home.theme.text
        }
        Label {
            anchors.top: cardTitle.bottom
            anchors.topMargin: 3
            width: card.width
            text: card.mix.subtitle || ""
            font.pixelSize: 12
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
            clip: true
            maximumLineCount: 2
            elide: Text.ElideRight
            color: home.mutedText
        }
        ToolTip.visible: hover.hovered && (card.mix.subtitle || "").length > 60
        ToolTip.delay: 900
        ToolTip.text: card.mix.subtitle || ""
    }
}

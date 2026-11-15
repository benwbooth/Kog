import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// A small dot by the version text that glows while Kog works in the
// background (reading the library, measuring songs, the sound model,
// making mixes), in this app or in a Kog server sharing the library.
// Clicking it opens a panel saying what each job is doing and how far it
// has got.
Item {
    id: activity

    required property var app
    required property var colors

    property var status: ({})
    property bool refreshing: false
    property real now: Date.now()

    readonly property bool crawling: (status.crawl_at || 0) > now - 20000
    readonly property bool listening: (status.embed_at || 0) > now - 20000
    readonly property bool busy: crawling || listening || refreshing

    implicitWidth: 18
    implicitHeight: 18

    function number(value) {
        return Number(value || 0).toLocaleString(Qt.locale(), "f", 0)
    }

    function summary() {
        if (!busy)
            return qsTr("Background work: idle")
        const jobs = []
        if (refreshing)
            jobs.push(qsTr("making new mixes"))
        if (crawling)
            jobs.push(qsTr("reading your library"))
        if (listening)
            jobs.push(qsTr("running the sound model"))
        return qsTr("Background work: %1").arg(jobs.join(", "))
    }

    component Job: ColumnLayout {
        property string title
        property bool active
        property string doing
        property string detail
        Layout.fillWidth: true
        spacing: 2
        RowLayout {
            spacing: 6
            Rectangle {
                width: 7; height: 7; radius: 3.5
                color: parent.parent.active ? activity.colors.accent : activity.colors.placeholderText
            }
            Label { text: parent.parent.title; font.bold: true }
        }
        Label {
            Layout.fillWidth: true
            text: parent.active ? parent.doing : qsTr("Idle")
            elide: Text.ElideMiddle
            color: activity.colors.text
        }
        Label {
            Layout.fillWidth: true
            visible: text.length > 0
            text: parent.detail
            wrapMode: Text.WordWrap
            font.pixelSize: 11
            color: activity.colors.placeholderText
        }
    }

    Timer {
        interval: 2000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: {
            activity.now = Date.now()
            try {
                const parsed = JSON.parse(activity.app.discover_status_json())
                activity.status = parsed.status || {}
                activity.refreshing = !!parsed.refreshing
            } catch (_) {
            }
        }
    }

    Rectangle {
        id: dot
        anchors.centerIn: parent
        width: 8
        height: 8
        radius: 4
        color: activity.busy ? activity.colors.accent : activity.colors.placeholderText
        opacity: activity.busy ? 1 : 0.45
        Behavior on color { ColorAnimation { duration: 250 } }
        SequentialAnimation on scale {
            running: activity.busy
            loops: Animation.Infinite
            alwaysRunToEnd: true
            NumberAnimation { from: 1; to: 1.45; duration: 700; easing.type: Easing.InOutSine }
            NumberAnimation { from: 1.45; to: 1; duration: 700; easing.type: Easing.InOutSine }
        }
    }

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
    TapHandler { onTapped: panel.opened ? panel.close() : panel.open() }
    ToolTip.visible: hover.hovered && !panel.opened
    ToolTip.delay: 500
    ToolTip.text: summary()
    Accessible.role: Accessible.Button
    Accessible.name: summary()

    Popup {
        id: panel
        y: -implicitHeight - 8
        x: activity.width - width
        width: 380
        padding: 14
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                text: qsTr("Background work")
                font.bold: true
                font.pixelSize: 14
            }

            Job {
                title: qsTr("Library index")
                active: activity.crawling
                doing: activity.status.crawl_activity || ""
                detail: qsTr("%1 songs read, %2 measured. ").arg(activity.number(activity.status.songs))
                        .arg(activity.number(activity.status.analysed))
                    + ((activity.status.folders_left || 0) > 0
                        ? qsTr("%1 folders still to read.").arg(activity.number(activity.status.folders_left))
                        : qsTr("The whole library has been read."))
            }
            Job {
                title: qsTr("Sound model")
                active: activity.listening
                doing: activity.status.embed_activity || ""
                detail: (activity.status.embedding
                        ? activity.status.embedding.charAt(0).toUpperCase() + activity.status.embedding.slice(1) + ". " : "")
                    + qsTr("%1 songs heard.").arg(activity.number(activity.status.embedded))
            }
            Job {
                title: qsTr("Home mixes")
                active: activity.refreshing
                doing: qsTr("Making new mixes")
                detail: qsTr("Made once a day, and when you refresh Home.")
            }
        }
    }
}

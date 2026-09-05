import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.settings

ApplicationWindow {
    id: root
    required property var app
    required property var mainWindow
    property var skin: ({assets: {}})
    property int scaleFactor: 2
    property bool toolbarVisible: false
    property bool playlistVisible: true
    property string settingsFile: ""
    Settings {
        category: "ClassicPlayer"
        fileName: root.settingsFile
        property alias toolbarVisible: root.toolbarVisible
        property alias playlistVisible: root.playlistVisible
    }
    readonly property var assets: skin.assets || ({})
    readonly property var textColors: (skin.textColors || "#000000,#71f5b0").split(",")
    title: qsTr("Kog Classic — ") + (skin.title || "")
    width: 275 * scaleFactor
    height: (116 + (playlistVisible ? 232 : 0)) * scaleFactor + (toolbarVisible ? 44 : 0)
    minimumWidth: 275 * scaleFactor
    maximumWidth: 275 * scaleFactor
    minimumHeight: (116 + (playlistVisible ? 232 : 0)) * scaleFactor + (toolbarVisible ? 44 : 0)
    maximumHeight: minimumHeight
    signal openGallery()
    signal openEqualizer()
    signal openVisualizer()
    onClosing: event => {
        if (!mainWindow.applicationQuitRequested) {
            event.accepted = false
            root.hide()
            mainWindow.showFromTray()
        }
    }
    function restoreQueue() { root.hide(); mainWindow.showFromTray() }
    Menu {
        id: skinMenu
        MenuItem { text: qsTr("Show playlist"); checkable: true; checked: root.playlistVisible; onTriggered: root.playlistVisible = !root.playlistVisible }
        MenuItem { objectName: "classicToolbarToggle"; text: qsTr("Show Kog toolbar"); checkable: true; checked: root.toolbarVisible; onTriggered: root.toolbarVisible = !root.toolbarVisible }
        MenuSeparator {}
        MenuItem { text: qsTr("Skins…"); icon.name: "preferences-desktop-theme"; onTriggered: root.openGallery() }
        MenuItem { text: qsTr("Equalizer…"); icon.name: "preferences-desktop-sound"; onTriggered: root.openEqualizer() }
        MenuItem { text: qsTr("Visualizer…"); icon.name: "audio-equalizer"; onTriggered: root.openVisualizer() }
        MenuItem { text: qsTr("Return to Kog"); icon.name: "view-restore"; onTriggered: root.restoreQueue() }
    }
    function buttonAction(index) {
        if (index === 0) app.previous()
        else if (index === 1 && app.playback_state !== "playing") app.play_pause()
        else if (index === 2 && app.playback_state === "playing") app.play_pause()
        else if (index === 3) app.stop()
        else if (index === 4) app.next()
    }
    Item {
        width: 275; height: 116
        scale: root.scaleFactor
        transformOrigin: Item.TopLeft
        SkinSprite { source: root.assets.main || ""; width: 275; height: 116 }
        SkinSprite { source: root.assets.titlebar || ""; width: 275; height: 14; sheetX: 27; sheetY: root.active ? 0 : 15; visible: source.toString().length > 0 }
        MouseArea { x: 16; y: 0; width: 228; height: 14; onPressed: root.startSystemMove(); onDoubleClicked: root.scaleFactor = root.scaleFactor === 2 ? 3 : 2 }
        SkinSprite {
            x: 264; y: 3; width: 9; height: 9; source: root.assets.titlebar || ""; sheetX: 18
            MouseArea { anchors.fill: parent; onClicked: root.restoreQueue() }
        }
        SkinSprite {
            x: 244; y: 3; width: 9; height: 9; source: root.assets.titlebar || ""; sheetX: 9
            MouseArea { anchors.fill: parent; onClicked: root.showMinimized() }
        }
        SkinSprite {
            x: 6; y: 3; width: 9; height: 9; source: root.assets.titlebar || ""
            MouseArea { anchors.fill: parent; onClicked: skinMenu.popup() }
        }
        Rectangle { x: 111; y: 23; width: 153; height: 11; color: root.textColors[0] }
        Text {
            x: 112; y: 24; width: 151; height: 10
            text: root.app.now_title; textFormat: Text.PlainText
            color: root.textColors[1]; font.pixelSize: 8; font.family: "monospace"
            elide: Text.ElideRight
            MouseArea { anchors.fill: parent; hoverEnabled: true; ToolTip.visible: containsMouse; ToolTip.text: root.app.now_title }
        }
        Text {
            x: 38; y: 24; width: 62; height: 18
            text: Math.floor(root.app.position_seconds / 60) + ":" + String(Math.floor(root.app.position_seconds % 60)).padStart(2, "0")
            color: "#71f5b0"; font.pixelSize: 15; font.family: "monospace"
            visible: !root.assets.numbers
        }
        Repeater {
            model: root.assets.numbers ? 4 : 0
            SkinSprite {
                required property int index
                x: [48, 60, 78, 90][index]; y: 26; width: 9; height: 13
                source: root.assets.numbers || ""
                sheetX: Number((String(Math.floor(root.app.position_seconds / 60) % 100).padStart(2, "0")
                    + String(Math.floor(root.app.position_seconds % 60)).padStart(2, "0"))[index]) * 9
            }
        }
        SkinSprite {
            x: 24; y: 28; width: 9; height: 9; source: root.assets.playpaus || ""
            sheetX: root.app.playback_state === "playing" ? 0 : root.app.playback_state === "paused" ? 9 : 18
        }
        AudioVisualization {
            x: 24; y: 43; width: 76; height: 17
            app: root.app; active: root.visible && root.visibility !== Window.Minimized
            MouseArea { anchors.fill: parent; onClicked: root.openVisualizer() }
        }
        Repeater {
            model: 5
            SkinSprite {
                id: transport
                required property int index
                objectName: "classicTransport" + index
                x: 16 + index * 23; y: 88; width: 23; height: 18
                source: root.assets.cbuttons || ""; sheetX: index * 23; sheetY: click.pressed ? 18 : 0
                MouseArea { id: click; anchors.fill: parent; onClicked: root.buttonAction(transport.index); hoverEnabled: true }
                ToolTip.visible: click.containsMouse
                ToolTip.text: [qsTr("Previous"), qsTr("Play"), qsTr("Pause"), qsTr("Stop"), qsTr("Next")][index]
            }
        }
        SkinSprite {
            x: 136; y: 89; width: 22; height: 16; source: root.assets.cbuttons || ""; sheetX: 114
            sheetY: eject.pressed ? 16 : 0
            MouseArea { id: eject; anchors.fill: parent; onClicked: root.app.open_audio_files(); hoverEnabled: true }
            ToolTip.visible: eject.containsMouse; ToolTip.text: qsTr("Add files")
        }
        Item {
            x: 16; y: 72; width: 248; height: 10
            SkinSprite { source: root.assets.posbar || ""; width: 248; height: 10 }
            Rectangle { anchors.fill: parent; color: "#23363c"; visible: !root.assets.posbar }
            SkinSprite { source: root.assets.posbar || ""; sheetX: 248; x: (parent.width - width) * Math.min(1, root.app.position_seconds / Math.max(1, root.app.duration_seconds)); width: 29; height: 10 }
            Rectangle { width: 5; height: 10; x: (parent.width - width) * root.app.position_seconds / Math.max(1, root.app.duration_seconds); color: "#71f5b0"; visible: !root.assets.posbar }
            MouseArea { anchors.fill: parent; enabled: root.app.duration_seconds > 0; onClicked: event => root.app.seek(Math.max(0, Math.min(1, event.x / width)) * root.app.duration_seconds) }
        }
        Item {
            x: 107; y: 57; width: 68; height: 13
            SkinSprite { source: root.assets.volume || ""; width: 68; height: 13; sheetY: Math.round(root.app.volume * 27) * 15 }
            Rectangle { anchors.fill: parent; visible: !root.assets.volume; color: "#23363c" }
            SkinSprite { source: root.assets.volume || ""; x: root.app.volume * 51; sheetX: 15; sheetY: 422; width: 14; height: 11 }
            MouseArea {
                anchors.fill: parent
                function adjust(x) { root.app.set_volume_level(Math.max(0, Math.min(1, x / width))) }
                onPressed: event => adjust(event.x)
                onPositionChanged: event => { if (pressed) adjust(event.x) }
                hoverEnabled: true
                ToolTip.visible: containsMouse; ToolTip.text: qsTr("Volume: %1%").arg(Math.round(root.app.volume * 100))
            }
        }
        // The playlist uses the skin's PLEDIT artwork; EQ opens Kog's equalizer.
        SkinSprite {
            x: 219; y: 58; width: 23; height: 12; source: root.assets.shufrep || ""; sheetY: 61
            MouseArea { anchors.fill: parent; onClicked: root.openEqualizer() }
        }
        SkinSprite {
            x: 242; y: 58; width: 23; height: 12; source: root.assets.shufrep || ""; sheetX: 23; sheetY: 61
            MouseArea { anchors.fill: parent; onClicked: root.playlistVisible = !root.playlistVisible }
        }
        SkinSprite {
            x: 164; y: 89; width: 47; height: 15; source: root.assets.shufrep || ""; sheetX: 28
            sheetY: root.app.shuffle_mode !== "off" ? 30 : 0
            MouseArea { anchors.fill: parent; onClicked: root.app.select_shuffle_mode(root.app.shuffle_mode === "off" ? "all" : "off") }
        }
        SkinSprite {
            x: 210; y: 89; width: 28; height: 15; source: root.assets.shufrep || ""
            sheetY: root.app.repeat_mode !== "off" ? 30 : 0
            MouseArea { anchors.fill: parent; onClicked: root.app.select_repeat_mode(root.app.repeat_mode === "off" ? "playlist" : "off") }
        }
    }
    ClassicPlaylist {
        x: 0; y: 116 * root.scaleFactor
        width: 275; height: 232
        scale: root.scaleFactor
        transformOrigin: Item.TopLeft
        visible: root.playlistVisible
        app: root.app
        skin: root.skin
        onCloseRequested: root.playlistVisible = false
    }
    MouseArea {
        width: parent.width
        height: 116 * root.scaleFactor
        acceptedButtons: Qt.RightButton
        onClicked: skinMenu.popup()
    }
    footer: ToolBar {
        objectName: "classicToolbar"
        visible: root.toolbarVisible
        height: visible ? 44 : 0
        RowLayout {
            anchors.fill: parent
            ToolButton { text: qsTr("Queue"); icon.name: "view-list-details"; display: root.scaleFactor === 1 ? AbstractButton.IconOnly : AbstractButton.TextOnly; onClicked: root.playlistVisible = !root.playlistVisible }
            ToolButton { text: qsTr("Skins"); icon.name: "preferences-desktop-theme"; display: root.scaleFactor === 1 ? AbstractButton.IconOnly : AbstractButton.TextOnly; onClicked: root.openGallery() }
            ToolButton { text: qsTr("Visualize"); icon.name: "audio-equalizer"; display: root.scaleFactor === 1 ? AbstractButton.IconOnly : AbstractButton.TextOnly; onClicked: root.openVisualizer() }
            Item { Layout.fillWidth: true }
            ComboBox { model: ["1×", "2×", "3×"]; Layout.preferredWidth: 68; currentIndex: root.scaleFactor - 1; onActivated: root.scaleFactor = currentIndex + 1 }
        }
    }
}

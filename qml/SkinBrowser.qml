import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: root
    required property var library
    signal openClassic()
    title: qsTr("Kog — Winamp skins")
    width: 880
    height: 670
    minimumWidth: 600
    minimumHeight: 480
    property int page: 1
    property bool searched: false
    readonly property bool installedTab: tabs.currentIndex === 1
    readonly property var items: JSON.parse(installedTab ? library.installed_json : library.catalog_json)
    function search() {
        root.searched = true
        library.search(query.text, page)
    }
    onVisibleChanged: if (visible && !searched) search()
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            ColumnLayout {
                Layout.fillWidth: true
                Label { text: qsTr("Winamp skins"); font.pixelSize: 23; font.bold: true }
                Label { text: qsTr("Classic and modern artwork. Kog playback."); opacity: 0.7 }
            }
            Button { text: qsTr("Import .wsz / .wal / .zip…"); icon.name: "document-open"; enabled: !root.library.busy; onClicked: root.library.import_file() }
        }
        Label {
            Layout.fillWidth: true
            text: qsTr("Classic skins include a skinned playlist. Modern .wal support is experimental; native plugins and some MAKI features are unsupported.")
            wrapMode: Text.WordWrap
            opacity: 0.75
        }
        RowLayout {
            TabBar {
                id: tabs
                TabButton { text: qsTr("Internet Archive") }
                TabButton { text: qsTr("Installed") }
            }
            Item { Layout.fillWidth: true }
            ComboBox {
                visible: !root.installedTab
                model: [qsTr("Classic"), qsTr("Modern (experimental)")]
                enabled: !root.library.busy
                onActivated: { root.library.modern = currentIndex === 1; root.page = 1; root.search() }
            }
            TextField {
                id: query
                visible: !root.installedTab
                placeholderText: qsTr("Search skins…")
                Layout.preferredWidth: 200
                maximumLength: 120
                enabled: !root.library.busy
                onAccepted: { root.page = 1; root.search() }
            }
            Button { text: qsTr("Search"); visible: !root.installedTab; enabled: !root.library.busy; onClicked: { root.page = 1; root.search() } }
        }
        GridView {
            id: grid
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: root.items
            readonly property int columns: Math.max(2, Math.floor((width - 18) / 260))
            cellWidth: (width - 18) / columns
            cellHeight: 230
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Item {
                id: card
                required property var modelData
                required property int index
                width: grid.cellWidth
                height: grid.cellHeight
                Rectangle {
                    anchors.fill: parent
                    anchors.rightMargin: 10
                    anchors.bottomMargin: 10
                    color: palette.base
                    border.color: palette.mid
                    radius: 7
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 8
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 115
                            color: "#13191e"
                            radius: 4
                            Image {
                                id: preview
                                anchors.fill: parent
                                anchors.margins: 4
                                asynchronous: true
                                source: root.installedTab ? (card.modelData.assets ? card.modelData.assets.main || "" : "") : "https://archive.org/services/img/" + card.modelData.id
                                sourceSize.width: 550
                                sourceSize.height: 232
                                fillMode: Image.PreserveAspectFit
                                smooth: false
                            }
                            Label { anchors.centerIn: parent; color: "#a6b9c3"; text: preview.status === Image.Loading ? qsTr("Loading preview…") : qsTr("Preview unavailable"); visible: preview.status !== Image.Ready }
                        }
                        Label {
                            text: card.modelData.title.replace(/^Winamp Skin: /, "")
                            textFormat: Text.PlainText
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                            font.bold: true
                            HoverHandler { id: titleHover }
                            ToolTip.visible: titleHover.hovered
                            ToolTip.text: card.modelData.title
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Button {
                                text: root.installedTab ? qsTr("Use skin") : qsTr("Install")
                                enabled: !root.library.busy
                                onClicked: {
                                    if (root.installedTab) { root.library.apply(card.index); root.openClassic() }
                                    else root.library.install(card.modelData.id)
                                }
                            }
                            Item { Layout.fillWidth: true }
                            ToolButton {
                                text: qsTr("Source ↗")
                                visible: !root.installedTab || !!card.modelData.source
                                onClicked: Qt.openUrlExternally(root.installedTab ? card.modelData.source : "https://archive.org/details/" + card.modelData.id)
                            }
                        }
                    }
                }
            }
            Label {
                anchors.centerIn: parent
                visible: root.items.length === 0 && !root.library.busy
                text: root.installedTab ? qsTr("No installed skins yet. Import one or browse Internet Archive.") : qsTr("No skins found. Try another search.")
                width: parent.width - 60
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                opacity: 0.7
            }
        }
        RowLayout {
            Layout.fillWidth: true
            BusyIndicator { running: root.library.busy; visible: running; Layout.preferredWidth: 24; Layout.preferredHeight: 24 }
            Label { text: root.library.status; textFormat: Text.PlainText; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Button { text: qsTr("Previous"); visible: !root.installedTab; enabled: root.page > 1 && !root.library.busy; onClicked: { root.page--; root.search() } }
            Label { text: root.page; visible: !root.installedTab }
            Button { text: qsTr("Next"); visible: !root.installedTab; enabled: root.page * 24 < root.library.total && !root.library.busy; onClicked: { root.page++; root.search() } }
        }
        RowLayout {
            Layout.fillWidth: true
            Label {
                text: qsTr("Downloads come from Internet Archive. Skin rights belong to their creators; see each source page.")
                wrapMode: Text.WordWrap
                opacity: 0.65
                Layout.fillWidth: true
                font.pixelSize: 11
            }
            Button {
                text: qsTr("Open player")
                enabled: { const skin = JSON.parse(root.library.active_json); return !!skin.assets || !!skin.archivePath }
                onClicked: root.openClassic()
            }
        }
    }
}

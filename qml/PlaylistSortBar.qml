pragma ComponentBehavior: Bound

import QtQml
import QtQuick
import QtQuick.Controls

// Stands in for the column header when songs show as rows or cover tiles:
// a "Sort by" menu over the same columns, and the direction beside it.
// Choosing the sorted column again flips its direction, as a header click
// does.
Item {
    id: root

    required property var theme
    property string sortColumn: "index"
    property bool sortAscending: true
    property string orderLabel: qsTr("Queue Order")

    signal sortRequested(string column)

    readonly property var sortColumns: [
        { id: "index", label: root.orderLabel },
        { id: "title", label: qsTr("Title") },
        { id: "artist", label: qsTr("Artist") },
        { id: "album", label: qsTr("Album") },
        { id: "albumartist", label: qsTr("Album Artist") },
        { id: "length", label: qsTr("Length") },
        { id: "date", label: qsTr("Year") },
        { id: "track", label: qsTr("Track") },
        { id: "genre", label: qsTr("Genre") },
        { id: "playcount", label: qsTr("Play Count") },
        { id: "filename", label: qsTr("Filename") },
        { id: "path", label: qsTr("Path") }
    ]
    readonly property string sortLabel: {
        const found = sortColumns.find(column => column.id === sortColumn)
        return found ? found.label : sortColumn
    }
    readonly property bool darkTheme: 0.2126 * theme.base.r + 0.7152 * theme.base.g
        + 0.0722 * theme.base.b < 0.5

    implicitHeight: 34

    component SortButton: AbstractButton {
        id: button
        property string label: ""
        implicitHeight: 24
        implicitWidth: buttonLabel.implicitWidth + 16
        hoverEnabled: true
        background: Rectangle {
            radius: 6
            color: Qt.rgba(root.theme.buttonText.r, root.theme.buttonText.g,
                root.theme.buttonText.b,
                button.down ? 0.14 : (button.hovered ? 0.08 : 0))
            Behavior on color { ColorAnimation { duration: 120 } }
        }
        contentItem: Label {
            id: buttonLabel
            text: button.label
            font.pixelSize: 11
            font.bold: true
            color: root.theme.buttonText
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        Label {
            anchors.verticalCenter: parent.verticalCenter
            rightPadding: 4
            text: qsTr("Sort by")
            font.pixelSize: 11
            color: root.theme.buttonText
            opacity: 0.65
        }

        SortButton {
            id: columnButton
            anchors.verticalCenter: parent.verticalCenter
            label: root.sortLabel + "  ▾"
            Accessible.name: qsTr("Sort by %1").arg(root.sortLabel)
            onClicked: sortMenu.popup(columnButton, 0, columnButton.height)
        }
        SortButton {
            anchors.verticalCenter: parent.verticalCenter
            label: root.sortAscending ? "▲" : "▼"
            Accessible.name: root.sortAscending ? qsTr("Ascending") : qsTr("Descending")
            onClicked: root.sortRequested(root.sortColumn)
        }
    }

    Menu {
        id: sortMenu
        Instantiator {
            model: root.sortColumns
            delegate: MenuItem {
                required property var modelData
                text: modelData.label
                checkable: true
                checked: root.sortColumn === modelData.id
                // Choosing the sorted column from the menu leaves it as it
                // is; the arrow beside the menu flips it.
                onTriggered: {
                    if (root.sortColumn !== modelData.id)
                        root.sortRequested(modelData.id)
                    checked = Qt.binding(() => root.sortColumn === modelData.id)
                }
            }
            onObjectAdded: (index, object) => sortMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => sortMenu.removeItem(object)
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: 8
        anchors.rightMargin: 8
        height: 1
        color: root.theme.mid
        opacity: 0.6
    }
}

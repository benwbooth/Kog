pragma ComponentBehavior: Bound

import QtQuick

Rectangle {
    id: root

    required property var theme
    property string savedWidths: ""
    property string sortColumn: "index"
    property bool sortAscending: true

    property real numberWidth: 54
    property real ratingWidth: 78
    property real titleWidth: 220
    property real artistWidth: 190
    property real albumWidth: 220
    property real lengthWidth: 70
    property real yearWidth: 58
    property real genreWidth: 120
    property real trackWidth: 54
    property bool layoutReady: false
    property bool adjustingWidths: false

    signal sortRequested(string column)
    signal columnWidthsChanged(string widths)

    implicitHeight: 30
    color: theme.window
    border.color: theme.mid

    function minimumWidthAt(index) {
        return [38, 48, 80, 70, 70, 48, 42, 60, 38][index]
    }

    function widthAt(index) {
        switch (index) {
        case 0: return numberWidth
        case 1: return ratingWidth
        case 2: return titleWidth
        case 3: return artistWidth
        case 4: return albumWidth
        case 5: return lengthWidth
        case 6: return yearWidth
        case 7: return genreWidth
        case 8: return trackWidth
        default: return 0
        }
    }

    function setWidthAt(index, value) {
        const bounded = Math.max(minimumWidthAt(index), value)
        switch (index) {
        case 0: numberWidth = bounded; break
        case 1: ratingWidth = bounded; break
        case 2: titleWidth = bounded; break
        case 3: artistWidth = bounded; break
        case 4: albumWidth = bounded; break
        case 5: lengthWidth = bounded; break
        case 6: yearWidth = bounded; break
        case 7: genreWidth = bounded; break
        case 8: trackWidth = bounded; break
        }
    }

    function fitFlexibleColumns() {
        if (adjustingWidths || width <= 0)
            return
        const flexible = [2, 3, 4, 7]
        const fixedWidth = numberWidth + ratingWidth + lengthWidth
            + yearWidth + trackWidth
        let minimumFlexibleWidth = 0
        let existingExtra = 0
        for (const index of flexible) {
            minimumFlexibleWidth += minimumWidthAt(index)
            existingExtra += Math.max(0, widthAt(index) - minimumWidthAt(index))
        }
        const targetFlexibleWidth = Math.max(minimumFlexibleWidth, width - fixedWidth)
        const distributable = targetFlexibleWidth - minimumFlexibleWidth
        const fallbackWeights = [0.28, 0.23, 0.28, 0.21]

        adjustingWidths = true
        for (let offset = 0; offset < flexible.length; ++offset) {
            const index = flexible[offset]
            const weight = existingExtra > 0
                ? Math.max(0, widthAt(index) - minimumWidthAt(index)) / existingExtra
                : fallbackWeights[offset]
            setWidthAt(index, minimumWidthAt(index) + distributable * weight)
        }
        adjustingWidths = false
    }

    function restoreWidths() {
        const values = savedWidths.split(",").map(value => Number(value.trim()))
        let valid = values.length === 9
        for (let index = 0; valid && index < values.length; ++index)
            valid = Number.isFinite(values[index]) && values[index] >= minimumWidthAt(index)
        if (valid) {
            adjustingWidths = true
            for (let index = 0; index < values.length; ++index)
                setWidthAt(index, values[index])
            adjustingWidths = false
        }
        fitFlexibleColumns()
    }

    function resizeBoundary(index, delta) {
        if (index < 0 || index >= 8)
            return
        const leftWidth = widthAt(index)
        const rightWidth = widthAt(index + 1)
        const boundedDelta = Math.max(minimumWidthAt(index) - leftWidth,
            Math.min(delta, rightWidth - minimumWidthAt(index + 1)))
        adjustingWidths = true
        setWidthAt(index, leftWidth + boundedDelta)
        setWidthAt(index + 1, rightWidth - boundedDelta)
        adjustingWidths = false
    }

    function encodedWidths() {
        const values = []
        for (let index = 0; index < 9; ++index)
            values.push(widthAt(index).toFixed(2))
        return values.join(",")
    }

    Component.onCompleted: {
        layoutReady = true
        restoreWidths()
    }
    onSavedWidthsChanged: if (layoutReady) restoreWidths()
    onWidthChanged: if (layoutReady) fitFlexibleColumns()

    component HeaderCell: Rectangle {
        id: cell

        required property string label
        required property string identifier
        required property int columnIndex
        property int alignment: Text.AlignLeft

        height: root.height
        color: headerHover.hovered ? root.theme.button : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: qsTr("Sort by %1").arg(label)

        Rectangle {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 1
            height: parent.height * 0.56
            color: root.theme.mid
        }

        Text {
            anchors.fill: parent
            anchors.leftMargin: 7
            anchors.rightMargin: 7
            text: cell.label + (root.sortColumn === cell.identifier
                ? (root.sortAscending ? "  ▲" : "  ▼")
                : "")
            color: root.theme.buttonText
            font.pixelSize: 11
            font.bold: root.sortColumn === cell.identifier
            horizontalAlignment: cell.alignment
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        HoverHandler { id: headerHover }

        MouseArea {
            anchors.fill: parent
            anchors.rightMargin: cell.columnIndex < 8 ? 5 : 0
            acceptedButtons: Qt.LeftButton
            cursorShape: Qt.PointingHandCursor
            onClicked: root.sortRequested(cell.identifier)
        }

        MouseArea {
            id: separatorDrag

            property real previousX: 0

            visible: cell.columnIndex < 8
            z: 2
            width: 9
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.horizontalCenter: parent.right
            acceptedButtons: Qt.LeftButton
            cursorShape: Qt.SplitHCursor
            preventStealing: true
            onPressed: mouse => {
                previousX = mapToItem(root, mouse.x, mouse.y).x
                mouse.accepted = true
            }
            onPositionChanged: mouse => {
                if (!pressed)
                    return
                const currentX = mapToItem(root, mouse.x, mouse.y).x
                root.resizeBoundary(cell.columnIndex, currentX - previousX)
                previousX = currentX
            }
            onReleased: root.columnWidthsChanged(root.encodedWidths())
        }
    }

    Row {
        anchors.fill: parent

        HeaderCell {
            width: root.numberWidth
            label: "#"
            identifier: "index"
            columnIndex: 0
            alignment: Text.AlignHCenter
        }
        HeaderCell {
            width: root.ratingWidth
            label: qsTr("Rating")
            identifier: "rating"
            columnIndex: 1
        }
        HeaderCell {
            width: root.titleWidth
            label: qsTr("Title")
            identifier: "title"
            columnIndex: 2
        }
        HeaderCell {
            width: root.artistWidth
            label: qsTr("Artist")
            identifier: "artist"
            columnIndex: 3
        }
        HeaderCell {
            width: root.albumWidth
            label: qsTr("Album")
            identifier: "album"
            columnIndex: 4
        }
        HeaderCell {
            width: root.lengthWidth
            label: qsTr("Length")
            identifier: "length"
            columnIndex: 5
            alignment: Text.AlignRight
        }
        HeaderCell {
            width: root.yearWidth
            label: qsTr("Year")
            identifier: "year"
            columnIndex: 6
            alignment: Text.AlignRight
        }
        HeaderCell {
            width: root.genreWidth
            label: qsTr("Genre")
            identifier: "genre"
            columnIndex: 7
        }
        HeaderCell {
            width: root.trackWidth
            label: "№"
            identifier: "track"
            columnIndex: 8
            alignment: Text.AlignHCenter
        }
    }
}

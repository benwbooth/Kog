import QtQuick

// Resize handles along a frameless window's edges and corners (shared by
// Ben's Qt Quick apps; see ChromeButtons): the compositor resizes
// (startSystemResize), with the matching cursors; none while maximized or
// full screen. Put it on top of everything:
//     ChromeResizeEdges { window: win; anchors.fill: parent; z: 1000 }
Item {
    id: re
    required property var window
    // How far in from the edge a press grabs (corners: twice that).
    property int grip: 6
    visible: window && window.visibility !== Window.Maximized && window.visibility !== Window.FullScreen

    component Edge: MouseArea {
        property int edges
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        cursorShape: {
            const l = edges & Qt.LeftEdge, r = edges & Qt.RightEdge, t = edges & Qt.TopEdge, b = edges & Qt.BottomEdge
            if ((l && t) || (r && b)) return Qt.SizeFDiagCursor
            if ((r && t) || (l && b)) return Qt.SizeBDiagCursor
            return l || r ? Qt.SizeHorCursor : Qt.SizeVerCursor
        }
        onPressed: re.window.startSystemResize(edges)
    }
    Edge { edges: Qt.LeftEdge;   x: 0; y: re.grip * 2; width: re.grip; height: parent.height - re.grip * 4 }
    Edge { edges: Qt.RightEdge;  x: parent.width - re.grip; y: re.grip * 2; width: re.grip; height: parent.height - re.grip * 4 }
    Edge { edges: Qt.TopEdge;    x: re.grip * 2; y: 0; width: parent.width - re.grip * 4; height: re.grip }
    Edge { edges: Qt.BottomEdge; x: re.grip * 2; y: parent.height - re.grip; width: parent.width - re.grip * 4; height: re.grip }
    Edge { edges: Qt.LeftEdge | Qt.TopEdge;     x: 0; y: 0; width: re.grip * 2; height: re.grip * 2 }
    Edge { edges: Qt.RightEdge | Qt.TopEdge;    x: parent.width - re.grip * 2; y: 0; width: re.grip * 2; height: re.grip * 2 }
    Edge { edges: Qt.LeftEdge | Qt.BottomEdge;  x: 0; y: parent.height - re.grip * 2; width: re.grip * 2; height: re.grip * 2 }
    Edge { edges: Qt.RightEdge | Qt.BottomEdge; x: parent.width - re.grip * 2; y: parent.height - re.grip * 2; width: re.grip * 2; height: re.grip * 2 }
}

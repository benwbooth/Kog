import QtQuick

// Handle the press natively, while the compositor's pointer serial is current.
// A WebChannel round trip is not needed to start the window-manager gesture.
MouseArea {
    id: root
    required property var targetWindow
    objectName: "skinResizeGrip"
    width: 14
    height: 14
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    z: 100
    acceptedButtons: Qt.LeftButton
    cursorShape: Qt.SizeFDiagCursor
    signal resizeRequested(int edges)
    onPressed: resizeRequested(Qt.RightEdge | Qt.BottomEdge)
    onResizeRequested: edges => targetWindow.startSystemResize(edges)
}

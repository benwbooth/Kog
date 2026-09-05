import QtQuick

Image {
    property int sheetX: 0
    property int sheetY: 0
    smooth: false
    sourceClipRect: Qt.rect(sheetX, sheetY, width, height)
}

import QtQuick

Item {
    id: root

    property url source: ""
    property string text: ""
    property bool automaticScroll: true
    property int scrollOffset: 0
    property int scrollThreshold: 146
    property int scrollStep: 5
    property int scrollInterval: 200

    readonly property int glyphWidth: 5
    readonly property int glyphHeight: 6
    readonly property string scrollSeparator: "  ***  "
    readonly property bool bitmapAvailable: atlas.status === Image.Ready
        && atlas.sourceSize.width >= 150 && atlas.sourceSize.height >= 12
    readonly property bool scrolling: text.length * glyphWidth > scrollThreshold
    readonly property int cycleWidth: scrolling ? (text.length + scrollSeparator.length) * glyphWidth : 0
    readonly property string displayText: scrolling ? text + scrollSeparator + text : text

    width: 154
    height: glyphHeight
    clip: true

    Image { id: atlas; source: root.source; visible: false }
    Image {
        // Winamp fills the remainder of a short title with TEXT.BMP column 4.
        anchors.fill: parent
        source: root.source
        sourceClipRect: Qt.rect(4, 0, 1, 6)
        smooth: false
        visible: root.bitmapAvailable
    }

    function glyphCell(character) {
        let value = character.length ? character[0] : " "
        switch (value) {
        case "°": value = "0"; break
        case "Ç": value = "C"; break
        case "ü": value = "u"; break
        case "è": case "ë": case "ê": case "é": value = "e"; break
        case "á": case "à": case "â": value = "a"; break
        case "ç": value = "c"; break
        case "í": case "ì": case "î": case "ï": value = "i"; break
        case "É": value = "E"; break
        case "æ": value = "a"; break
        case "Æ": value = "A"; break
        case "ó": case "ò": case "ô": value = "o"; break
        case "ú": case "ù": case "û": value = "u"; break
        case "ÿ": value = "y"; break
        case "Ü": value = "U"; break
        case "ƒ": value = "f"; break
        case "Ñ": case "ñ": value = "n"; break
        }

        const code = value.charCodeAt(0)
        if (code >= 65 && code <= 90) return Qt.point((code - 65) * glyphWidth, 0)
        if (code >= 97 && code <= 122) return Qt.point((code - 97) * glyphWidth, 0)
        if (code >= 48 && code <= 57) return Qt.point((code - 48) * glyphWidth, 6)

        const rowOne = {
            "\u0001": 10, ".": 11, ":": 12, "(": 13, ")": 14, "-": 15,
            "'": 16, "`": 16, "!": 17, "_": 18, "+": 19, "\\": 20,
            "/": 21, "[": 22, "{": 22, "<": 22, "]": 23, "}": 23,
            ">": 23, "~": 24, "^": 24, "&": 25, "%": 26, ",": 27,
            "=": 28, "$": 29, "#": 30
        }
        if (rowOne[value] !== undefined) return Qt.point(rowOne[value] * glyphWidth, 6)

        const rowTwo = { "Å": 0, "å": 0, "Ö": 1, "ö": 1, "Ä": 2, "ä": 2, "?": 3, "*": 4 }
        if (rowTwo[value] !== undefined) return Qt.point(rowTwo[value] * glyphWidth, 12)
        if (value === "\"") return Qt.point(26 * glyphWidth, 0)
        if (value === "@") return Qt.point(27 * glyphWidth, 0)
        return Qt.point(30 * glyphWidth, 0)
    }

    function resetScroll() {
        scrollOffset = 0
    }

    function advanceScroll() {
        if (!scrolling || cycleWidth <= 0) {
            scrollOffset = 0
            return
        }
        scrollOffset += scrollStep
        if (scrollOffset >= cycleWidth) scrollOffset %= cycleWidth
    }

    onTextChanged: resetScroll()
    onSourceChanged: resetScroll()
    onScrollingChanged: if (!scrolling) resetScroll()

    Item {
        x: -(root.scrollOffset % root.glyphWidth)
        width: root.width + root.glyphWidth
        height: root.glyphHeight

        Repeater {
            model: root.bitmapAvailable ? Math.min(root.displayText.length,
                Math.ceil(root.width / root.glyphWidth) + 1) : 0
            SkinSprite {
                required property int index
                readonly property int textIndex: index + Math.floor(root.scrollOffset / root.glyphWidth)
                readonly property point cell: root.glyphCell(root.displayText[textIndex] || " ")
                objectName: "classicBitmapGlyph" + index
                x: index * root.glyphWidth
                width: root.glyphWidth
                height: root.glyphHeight
                source: root.source
                sheetX: cell.x
                sheetY: cell.y
                visible: cell.x + width <= atlas.sourceSize.width
                    && cell.y + height <= atlas.sourceSize.height
            }
        }
    }

    Timer {
        interval: root.scrollInterval
        repeat: true
        running: root.automaticScroll && root.bitmapAvailable && root.scrolling && root.visible
        onTriggered: root.advanceScroll()
    }
}

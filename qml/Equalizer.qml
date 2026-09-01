import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: root

    required property var app

    width: 720
    height: 460
    minimumWidth: 720
    maximumWidth: 720
    minimumHeight: 460
    maximumHeight: 460
    title: qsTr("Equalizer")
    color: palette.window
    flags: Qt.Dialog

    readonly property var bandLabels: [
        "20", "25", "31.5", "40", "50", "63", "80", "100", "125", "160",
        "200", "250", "315", "400", "500", "630", "800", "1k", "1.2k",
        "1.6k", "2k", "2.5k", "3.1k", "4k", "5k", "6.3k", "8k", "10k",
        "12k", "16k", "20k"
    ]
    readonly property var presetNames: app.equalizer_preset_names.split("\n")

    function dbLabel(value) {
        const rounded = Math.round(value * 10) / 10
        return (rounded > 0 ? "+" : "") + rounded.toFixed(
            Math.abs(rounded % 1) > 0.01 ? 1 : 0) + " dB"
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 12
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            CheckBox {
                id: enabledCheck

                text: qsTr("Enabled")
                checked: root.app.equalizer_enabled
                onToggled: root.app.update_equalizer_enabled(checked)
                contentItem: Label {
                    leftPadding: enabledCheck.indicator.width
                        + enabledCheck.spacing
                    text: enabledCheck.text
                    color: enabledCheck.enabled
                        ? root.palette.windowText
                        : root.palette.placeholderText
                    font: enabledCheck.font
                    verticalAlignment: Text.AlignVCenter
                }
            }
            CheckBox {
                id: genreTrackingCheck

                text: qsTr("Tracking genre tags")
                checked: root.app.equalizer_track_genre
                onToggled: root.app.update_equalizer_tracking(checked)
                contentItem: Label {
                    leftPadding: genreTrackingCheck.indicator.width
                        + genreTrackingCheck.spacing
                    text: genreTrackingCheck.text
                    color: genreTrackingCheck.enabled
                        ? root.palette.windowText
                        : root.palette.placeholderText
                    font: genreTrackingCheck.font
                    verticalAlignment: Text.AlignVCenter
                }
            }
            Item { Layout.fillWidth: true }
            Button {
                text: qsTr("Flatten EQ")
                icon.name: "edit-clear"
                onClicked: root.app.flatten_equalizer()
            }
            Button {
                text: qsTr("Level Preamp")
                icon.name: "audio-volume-high"
                onClicked: root.app.level_equalizer_preamp()
            }
            Label {
                text: qsTr("Preset:")
                color: root.palette.windowText
            }
            ComboBox {
                id: presetSelector

                Layout.preferredWidth: 155
                model: root.presetNames
                currentIndex: Math.max(0, root.presetNames.indexOf(
                    root.app.equalizer_preset))
                onActivated: index => root.app.select_equalizer_preset(
                    root.presetNames[index])
                Accessible.name: qsTr("Equalizer preset")
            }
        }

        Rectangle {
            id: graph

            function bandCenterForGain(gain) {
                const firstBand = bandsRepeater.itemAt(0)
                if (!firstBand || !firstBand.sliderControl)
                    return 52 + (20 - gain) / 40 * (height - 76)
                const slider = firstBand.sliderControl
                const handleHeight = slider.handle ? slider.handle.height : 0
                const travel = slider.availableHeight - handleHeight
                return bandsArea.y + slider.y + slider.topPadding
                    + handleHeight / 2 + (20 - gain) / 40 * travel
            }

            Layout.fillWidth: true
            Layout.fillHeight: true
            color: root.palette.base
            border.color: root.palette.mid
            radius: 3

            Repeater {
                model: 9
                delegate: Rectangle {
                    required property int index

                    x: 42
                    y: graph.bandCenterForGain(20 - index * 5) - height / 2
                    width: graph.width - 51
                    height: index === 4 ? 2 : 1
                    color: index === 4 ? root.palette.mid : root.palette.alternateBase

                    Label {
                        anchors.right: parent.left
                        anchors.rightMargin: 6
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.dbLabel(20 - parent.index * 5)
                        color: root.palette.placeholderText
                        font.pixelSize: 9
                    }
                }
            }

            Item {
                id: preampArea

                x: 42
                y: 19
                width: 42
                height: graph.height - 38

                Label {
                    anchors.top: parent.top
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: qsTr("Preamp")
                    color: root.palette.windowText
                    font.pixelSize: 9
                }
                Slider {
                    id: preampSlider

                    anchors.top: parent.top
                    anchors.topMargin: 18
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    orientation: Qt.Vertical
                    from: -20
                    to: 20
                    stepSize: 0.1
                    value: root.app.equalizer_preamp_db
                    onMoved: root.app.update_equalizer_preamp(value)
                    ToolTip.visible: hovered || pressed
                    ToolTip.text: root.dbLabel(value)
                    Accessible.name: qsTr("Equalizer preamp")
                }
            }

            Rectangle {
                x: preampArea.x + preampArea.width
                y: 20
                width: 1
                height: graph.height - 40
                color: root.palette.mid
            }

            Item {
                id: bandsArea

                x: 91
                y: 18
                width: graph.width - x - 8
                height: graph.height - 36

                Row {
                    anchors.fill: parent
                    spacing: 0

                    Repeater {
                        id: bandsRepeater

                        model: 31
                        delegate: Item {
                            id: band

                            required property int index
                            property alias sliderControl: bandSlider

                            width: bandsArea.width / 31
                            height: bandsArea.height

                            Label {
                                anchors.top: parent.top
                                anchors.topMargin: band.index % 2 === 0 ? 0 : 13
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: root.bandLabels[band.index]
                                color: root.palette.windowText
                                font.pixelSize: 8
                            }
                            Slider {
                                id: bandSlider

                                anchors.top: parent.top
                                anchors.topMargin: 28
                                anchors.bottom: parent.bottom
                                anchors.horizontalCenter: parent.horizontalCenter
                                orientation: Qt.Vertical
                                from: -20
                                to: 20
                                stepSize: 0.1
                                value: {
                                    const revision = root.app.equalizer_revision
                                    return root.app.equalizer_band_gain(band.index)
                                        + revision * 0
                                }
                                onMoved: root.app.update_equalizer_band(
                                    band.index, value)
                                ToolTip.visible: hovered || pressed
                                ToolTip.text: root.dbLabel(value)
                                Accessible.name: root.bandLabels[band.index]
                                    + qsTr(" Hz equalizer band")
                            }
                        }
                    }
                }

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.RightButton
                    hoverEnabled: false

                    function drawAt(position) {
                        const index = Math.max(0, Math.min(30,
                            Math.floor(position.x / width * 31)))
                        const sliderTop = 28
                        const usableHeight = height - sliderTop
                        const gain = Math.max(-20, Math.min(20,
                            20 - (position.y - sliderTop) / usableHeight * 40))
                        root.app.update_equalizer_band(index, gain)
                    }

                    onPressed: mouse => drawAt(mouse)
                    onPositionChanged: mouse => {
                        if (pressed)
                            drawAt(mouse)
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("Right-drag across the bands to draw an equalizer shape")
            color: root.palette.placeholderText
            font.pixelSize: 10
        }
    }
}

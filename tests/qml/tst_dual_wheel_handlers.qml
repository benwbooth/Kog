// SPDX-License-Identifier: GPL-3.0-or-later

// The playlist stacks a vertical AND a horizontal kinetic handler on one
// list. tst_kinetic_wheel covers each axis on separate views; this file
// covers the delivery between two handlers sharing one item — where the
// horizontal axis used to go dead because a shadowed orientation property
// left Qt filtering x-axis events before any handler saw them.
import QtQuick
import QtTest
import "../../qml"

Item {
    width: 320
    height: 200

    ListView {
        id: dualView

        anchors.fill: parent
        model: 50
        delegate: Item {
            required property int index
            width: 900
            height: 20
        }
        contentWidth: 900
        boundsBehavior: Flickable.StopAtBounds
        clip: true

        KineticWheelHandler {
            id: dualVertical
            view: dualView
        }
        KineticWheelHandler {
            id: dualHorizontal
            view: dualView
            orientation: Qt.Horizontal
        }
    }

    TestCase {
        name: "DualWheelHandlers"
        when: windowShown

        function init() {
            dualVertical.stop();
            dualHorizontal.stop();
            dualView.contentX = 0;
            dualView.contentY = 0;
        }

        function test_plainVerticalStillWorks() {
            mouseWheel(dualView, 160, 100, 0, -120, Qt.NoButton, Qt.NoModifier);
            tryVerify(function () {
                return dualView.contentY > 0;
            }, 100);
            verify(dualView.contentY > 0, "plain wheel should move contentY");
            compare(dualView.contentX, 0, "plain wheel must not move contentX");
        }

        function test_nativeHorizontalWheelMomentum() {
            mouseWheel(dualView, 160, 100, -120, 0, Qt.NoButton, Qt.NoModifier);
            tryVerify(function () {
                return dualView.contentX > 0;
            }, 100);
            verify(dualView.contentX > 0, "native horizontal wheel should move contentX");
            const first = dualView.contentX;
            wait(80);
            verify(dualView.contentX > first,
                "content should keep moving after the wheel stops");
        }

        function test_shiftWheelDrivesHorizontalAxis() {
            mouseWheel(dualView, 160, 100, 0, -120, Qt.NoButton, Qt.ShiftModifier);
            tryVerify(function () {
                return dualView.contentX > 0;
            }, 100);
            verify(dualView.contentX > 0,
                "Shift+wheel should scroll horizontally with momentum");
            verify(dualView.contentY === 0,
                "Shift+wheel must not scroll the vertical axis");
        }

        function test_verticalMomentumSurvivesHorizontalRun() {
            // A Shift+wheel run borrows the horizontal axis; once it ends the
            // engine must return to the vertical axis for plain wheels.
            mouseWheel(dualView, 160, 100, 0, -120, Qt.NoButton, Qt.ShiftModifier);
            tryVerify(function () {
                return dualView.contentX > 0;
            }, 100);
            dualHorizontal.stop();
            dualVertical.stop();
            mouseWheel(dualView, 160, 100, 0, -120, Qt.NoButton, Qt.NoModifier);
            tryVerify(function () {
                return dualView.contentY > 0;
            }, 100);
            verify(dualView.contentY > 0,
                "a plain wheel after a Shift+wheel run must scroll vertically");
        }
    }
}

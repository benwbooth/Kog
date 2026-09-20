// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml"

Item {
    width: 320
    height: 200

    ListView {
        id: view

        anchors.fill: parent
        model: 100
        delegate: Item {
            required property int index
            width: 320
            height: 20

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                preventStealing: true
                scrollGestureEnabled: false
            }
        }
        boundsBehavior: Flickable.StopAtBounds

        KineticWheelHandler {
            id: kineticWheel
            view: view
        }
    }

    Flickable {
        id: horizontalView

        width: 320
        height: 200
        visible: false
        contentWidth: 1200
        contentHeight: 200
        boundsBehavior: Flickable.StopAtBounds

        KineticWheelHandler {
            id: horizontalWheel
            view: horizontalView
            orientation: Qt.Horizontal
        }
    }

    TestCase {
        name: "KineticWheel"
        when: windowShown

        function init() {
            kineticWheel.stop();
            horizontalWheel.stop();
            view.contentY = 0;
            view.contentX = 0;
            horizontalView.contentX = 0;
            horizontalView.visible = false;
        }

        function cleanup() {
            kineticWheel.stop();
            horizontalWheel.stop();
            horizontalView.visible = false;
        }

        function test_mouseWheelMomentumContinuesAfterNotch() {
            mouseWheel(view, view.width / 2, view.height / 2, 0, -120, Qt.NoButton, Qt.NoModifier);
            tryVerify(function () {
                return view.contentY > 0;
            }, 100);
            verify(kineticWheel.velocity > 0, "the physical mouse wheel event should reach the kinetic handler");
            const firstPosition = view.contentY;
            wait(80);
            verify(view.contentY > firstPosition, "content should keep moving after the initial wheel impulse");
            verify(kineticWheel.velocity > 0, "momentum should still be decelerating");
        }

        function test_touchpadPixelGestureContinuesAfterFingerLift() {
            verify((kineticWheel.acceptedDevices & PointerDevice.TouchPad) !== 0);
            kineticWheel.applyPixelDelta(-18);
            wait(12);
            kineticWheel.applyPixelDelta(-18);
            const fingerLiftPosition = view.contentY;
            compare(fingerLiftPosition, 36);
            wait(80);
            verify(view.contentY > fingerLiftPosition, "content should keep moving after pixel gesture input stops");
            verify(kineticWheel.velocity > 0, "touchpad momentum should still be decelerating");
        }

        function test_directionChangeReversesMomentum() {
            view.contentY = 500;
            kineticWheel.start(-1);
            wait(32);
            kineticWheel.start(1);
            verify(kineticWheel.velocity < 0);
            const firstPosition = view.contentY;
            wait(80);
            verify(view.contentY < firstPosition);
        }

        function test_stopsAtBoundary() {
            view.contentY = kineticWheel.maximumContent();
            kineticWheel.start(-1);
            wait(32);
            compare(view.contentY, kineticWheel.maximumContent());
            compare(kineticWheel.velocity, 0);
        }

        function test_horizontalWheelMomentumAndBoundary() {
            horizontalView.visible = true;
            waitForRendering(horizontalView);
            horizontalWheel.start(-2);
            wait(80);
            verify(horizontalView.contentX > 0,
                "contentX should keep moving after the initial wheel impulse");
            horizontalWheel.stop();
            horizontalView.contentX = horizontalWheel.maximumContent();
            horizontalWheel.start(-1);
            wait(32);
            compare(horizontalView.contentX, horizontalWheel.maximumContent());
            compare(horizontalWheel.velocity, 0);
        }

        function test_shiftWheelDrivesHorizontalAxis() {
            // Shift+wheel stays on the y axis, so Qt hands the event to the
            // VERTICAL handler; that handler drives the horizontal engine
            // itself. A lone horizontal handler never sees it.
            view.contentWidth = 1200;
            mouseWheel(view, 160, 100, 0, -240, Qt.NoButton, Qt.ShiftModifier);
            tryVerify(function () {
                return view.contentX > 0;
            }, 100);
            verify(view.contentX > 0,
                "Shift+wheel should scroll horizontally with momentum");
            verify(view.contentY === 0,
                "Shift+wheel must not scroll the vertical axis");
        }

        function test_nativeHorizontalWheelMomentum() {
            // A native x-axis event reaches a horizontal-oriented handler
            // directly and glides after the wheel stops.
            horizontalView.visible = true;
            waitForRendering(horizontalView);
            mouseWheel(horizontalView, 160, 100, -120, 0, Qt.NoButton, Qt.NoModifier);
            tryVerify(function () {
                return horizontalView.contentX > 0;
            }, 100);
            verify(horizontalView.contentX > 0,
                "native horizontal wheel should scroll with momentum");
        }
    }
}

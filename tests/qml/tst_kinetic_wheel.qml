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
        }
        boundsBehavior: Flickable.StopAtBounds

        KineticWheelHandler {
            id: kineticWheel
            view: view
        }
    }

    TestCase {
        name: "KineticWheel"
        when: windowShown

        function init() {
            kineticWheel.stop();
            view.contentY = 0;
        }

        function cleanup() {
            kineticWheel.stop();
        }

        function test_momentumContinuesAfterImpulse() {
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
            view.contentY = kineticWheel.maximumContentY();
            kineticWheel.start(-1);
            wait(32);
            compare(view.contentY, kineticWheel.maximumContentY());
            compare(kineticWheel.velocity, 0);
        }
    }
}

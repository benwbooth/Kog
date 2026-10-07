"""Open the MML view for a playing NSF and check it follows playback."""

import fcntl
import os
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
from pathlib import Path

import pyte

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("KOG_TUI_TEST_BINARY", ROOT / "target/debug/kog-tui"))
SONG = ROOT / "native/game-music-emu/test.nsf"
COLUMNS, ROWS = 120, 40

with tempfile.TemporaryDirectory(prefix="kog-tui-mml-") as base:
    env = os.environ.copy()
    for key, subdir in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")):
        env[key] = str(Path(base, subdir))
    env["TERM"] = "xterm-256color"
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
    process = subprocess.Popen(
        [str(BINARY)] if BINARY.name == "kog-tui" else [str(BINARY), "--tui"],
        stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True,
    )
    os.close(slave)
    screen = pyte.Screen(COLUMNS, ROWS)
    stream = pyte.Stream(screen)

    def drain(seconds=0.2):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([master], [], [], min(0.05, until - time.monotonic()))[0]:
                try:
                    data = os.read(master, 65_536)
                except OSError:
                    break
                if not data:
                    break
                stream.feed(data.decode("utf8", "replace"))

    def send(data, seconds=0.25):
        os.write(master, data if isinstance(data, bytes) else data.encode())
        drain(seconds)

    def text():
        return "\n".join(screen.display)

    def wait_for(value, timeout=10):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if value in text():
                return
            drain(0.1)
        raise AssertionError((value, screen.display))

    def shortcut(label):
        for y, line in enumerate(screen.display):
            x = line.find(label)
            if x >= 0:
                for offset, character in enumerate(label):
                    if screen.buffer[y][x + offset].underscore:
                        return character.lower().encode()
        raise AssertionError((label, screen.display))

    def highlighted():
        # Sounding notes are painted bold on the selection colour.
        cells = []
        for y in range(4, ROWS - 1):
            run = ""
            for x in range(COLUMNS):
                cell = screen.buffer[y][x]
                if cell.bold and cell.bg not in ("default",) and not screen.display[y].lstrip().startswith(";"):
                    run += cell.data
                elif run:
                    cells.append(run.strip())
                    run = ""
            if run:
                cells.append(run.strip())
        return [cell for cell in cells if cell and cell[-1:].isalnum() and cell[0] in "abcdefgx&^"]

    try:
        wait_for("Search playlist")
        send(b"\x1ba")
        wait_for("Add file or folder")
        send(os.fsencode(SONG) + b"\r")
        wait_for("Added")
        send(b" ")
        wait_for("Playing")
        send(b"\x1bv")
        wait_for("Channel Inspector")
        send(b"\x1b" + shortcut("Channel Inspector"))
        wait_for("[4 MML]")
        send("4")
        # The NSF has no length, so recording runs to the ten-minute cap; the
        # first bars must appear while it is still going.
        wait_for("; bar", 30)
        assert "Still recording" in text(), screen.display
        def wait_highlight(timeout=4):
            until = time.monotonic() + timeout
            while time.monotonic() < until:
                if found := highlighted():
                    return found
                drain(0.1)
            raise AssertionError(("no sounding note highlighted", screen.display))

        first = wait_highlight()
        drain(1.5)
        second = wait_highlight()
        if os.environ.get("KOG_MML_SCREEN"):
            print("\n".join(screen.display))
        send(b"\x1b")
        send("q")
        wait_for("Confirm exit")
        send("y")
        process.wait(timeout=5)
        print(f"TUI MML view records the song and highlights sounding notes ({first[:3]} then {second[:3]}): PASS")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)

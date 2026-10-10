"""Build a custom effect in the terminal player's Effects screen."""

import fcntl
import json
import sqlite3
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

with tempfile.TemporaryDirectory(prefix="kog-tui-effects-") as base:
    env = os.environ.copy()
    for key, subdir in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")):
        env[key] = str(Path(base, subdir))
    env["TERM"] = "xterm-256color"
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
    process = subprocess.Popen(
        [str(BINARY)] if BINARY.name == "kog-tui" else [str(BINARY), "--tui"],
        stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True, cwd=base,
    )
    os.close(slave)
    screen = pyte.Screen(COLUMNS, ROWS)
    stream = pyte.Stream(screen)
    raw = bytearray()

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
                raw.extend(data)
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
        raise AssertionError(value + "\n" + "\n".join(screen.display))

    def shortcut(label):
        for y, line in enumerate(screen.display):
            x = line.find(label)
            if x >= 0:
                for offset, character in enumerate(label):
                    if screen.buffer[y][x + offset].underscore:
                        return character.lower().encode()
        raise AssertionError((label, screen.display))

    def highlighted():
        # Sounding notes are painted on the highlight colour.
        cells = []
        for y in range(4, ROWS - 1):
            run = ""
            for x in range(COLUMNS):
                cell = screen.buffer[y][x]
                if cell.bg == "50c8ef":
                    run += cell.data
                elif run:
                    cells.append(run.strip())
                    run = ""
            if run:
                cells.append(run.strip())
        return [cell for cell in cells if cell and cell[0] in "abcdefgx&^"]

    try:
        wait_for("Search playlist")
        send(b"\x1bp")  # Alt+P: the Preferences menu
        wait_for("Effects…")
        send(b"\x1b[F\r")  # End, then Enter: the last entry
        wait_for("Use effects")
        send("n")
        wait_for("Empty")
        send("2")  # the first template: Reverb built from blocks
        wait_for("Custom effect · Reverb")
        wait_for("Parallel paths")
        send("l")
        wait_for("LFO 1")
        send(b"\x1b")
        wait_for("Custom effects")
        wait_for("Reverb · 10 blocks")
        send("c")
        send("1")
        wait_for("1. Reverb [on]")
        with sqlite3.connect(Path(base, "data", "kog", "kog.db")) as db:
            (value,) = db.execute("select value from app_state where namespace='preferences' and key='effects-settings'").fetchone()
        saved = json.loads(value)
        assert saved["chain"][0]["patch"] == "Reverb", saved["chain"]
        assert saved["patches"][0]["modulators"][0]["kind"] == "lfo"
        send(b"\x1b")
        print("TUI effects: custom effect built from a template, added to the chain and saved: PASS")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)

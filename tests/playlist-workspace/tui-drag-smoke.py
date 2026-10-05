#!/usr/bin/env python3
"""Real SGR mouse events and rendered insertion gaps in an isolated TUI.

Run: uv run --with pyte python tests/playlist-workspace/tui-drag-smoke.py
Requires target/debug/kog-tui (or KOG_TUI_BINARY).
"""
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shutil
import signal
import sqlite3
import struct
import tempfile
import termios
import time

import pyte

repo = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="kog-tui-drag-") as directory:
    root = Path(directory)
    for name in ("config/kog", "data/kog", "cache", "runtime", "tracks"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    (root / "config/kog/output-volume").write_text("0")
    database = root / "data/kog/kog.db"
    names = [f"{'keep' if n % 2 == 0 else 'other'}-{n:02}.wav" for n in range(12)]
    with sqlite3.connect(database) as db:
        db.executescript("""
            CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL);
            CREATE TABLE playlist_entries(id INTEGER PRIMARY KEY AUTOINCREMENT, playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, kind TEXT NOT NULL, path TEXT NOT NULL, entry TEXT NOT NULL DEFAULT '', fragment TEXT);
            INSERT INTO playlists(name,position) VALUES('Drag smoke',0);
        """)
        for position, name in enumerate(names):
            path = root / "tracks" / name
            shutil.copyfile(repo / "tests/fixtures/codec-libs/tone.wav", path)
            db.execute("INSERT INTO playlist_entries(playlist_id,position,kind,path) VALUES(1,?,'local',?)", (position, str(path)))

    pid, terminal = pty.fork()
    if pid == 0:
        os.environ.update(XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"), XDG_CACHE_HOME=str(root / "cache"), XDG_RUNTIME_DIR=str(root / "runtime"), TERM="xterm-256color")
        os.environ.pop("KOG_SESSION_ID", None)
        binary = os.environ.get("KOG_TUI_BINARY", str(repo / "target/debug/kog-tui"))
        os.execv(binary, [binary])
    screen = pyte.Screen(120, 34)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")
    fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 34, 120, 0, 0))

    def drain(seconds=0.25):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([terminal], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                try:
                    data = os.read(terminal, 65536)
                except OSError:
                    break
                stream.feed(decoder.decode(data))

    def send(keys):
        os.write(terminal, keys)
        drain()

    def mouse(button, row, col=70, release=False):
        send(f"\x1b[<{button};{col};{row}{'m' if release else 'M'}".encode())

    def checkpoint():
        with sqlite3.connect(database) as db:
            row = db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='tui:default'").fetchone()
        return json.loads(row[0]) if row else None

    def order():
        return [Path(track["path"]).name for track in checkpoint()["queue"]]

    def expect_order(expected):
        for _ in range(30):
            drain(0.1)
            if order() == expected:
                return
        raise AssertionError(f"Expected queue {expected}, got {order()}")

    def markers():
        return [i + 1 for i, line in enumerate(screen.display) if "Insert here" in line]

    def expect_marker(row):
        assert markers() == [row], f"Marker rows {markers()}, expected {row}\n" + "\n".join(screen.display)

    def unchanged(expected):
        # Wait through a persistence flush: motion must not mutate the session.
        drain(1.1)
        assert order() == expected, "Queue changed before release or after cancellation"

    try:
        drain(2)
        send(b"\x1b[Z")  # Library -> saved playlists
        send(b"\x1b[B")  # Favorites -> Drag smoke
        send(b"\r")
        drain(0.5)
        send(b"\x01")  # select all draft rows
        send(b"a")  # add to queue, without starting audio
        send(b"\x1b1")  # Alt+1: focus queue
        expect_order(names)
        drain(1)

        mouse(0, 4)  # first track
        mouse(32, 7)  # before fourth track
        expect_marker(7)
        mouse(32, 7)  # repeated motion must not move the gap
        expect_marker(7)
        assert names[3][:-4] in screen.display[7], "Target row was overwritten by marker"
        unchanged(names)
        mouse(0, 7, release=True)
        names.insert(2, names.pop(0))
        expect_order(names)
        assert not markers(), "Marker survived successful drop"

        mouse(0, 7)
        mouse(32, 4)
        expect_marker(4)
        mouse(0, 4, release=True)
        names.insert(0, names.pop(3))
        expect_order(names)

        mouse(0, 4)
        mouse(32, 19)  # blank space after last track
        expect_marker(16)
        mouse(0, 19, release=True)
        names.append(names.pop(0))
        expect_order(names)

        mouse(0, 4)
        mouse(32, 8)
        expect_marker(8)
        send(b"\x1b")
        drain(0.4)
        assert not markers(), "Escape did not clear preview"
        assert "Exit Kog" not in "\n".join(screen.display), "Escape opened exit confirmation"
        mouse(0, 8, release=True)
        unchanged(names)

        mouse(0, 4)
        mouse(32, 8)
        mouse(32, 8, col=4)  # leave queue for sidebar
        assert not markers(), "Outside motion kept preview"
        mouse(0, 8, col=4, release=True)
        unchanged(names)

        mouse(0, 4)
        mouse(32, 8)
        mouse(0, 2, release=True)  # directly release on the top tab strip
        assert not markers()
        unchanged(names)

        # Filtered row positions must map to the complete queue's insertion gap.
        send(b"Fkeep\r")
        filtered = [name for name in names if name.startswith("keep")]
        mouse(0, 4)
        mouse(32, 6)
        expect_marker(6)
        unchanged(names)
        mouse(0, 6, release=True)
        source = names.index(filtered[0])
        target = names.index(filtered[2])
        names.insert(target - 1, names.pop(source))
        expect_order(names)
        send(b"F\x01\x7f\r")  # clear filter
        send(b"\x1b[H")  # first track

        # A smaller terminal forces scrolling while the source stays selected.
        screen.resize(lines=14, columns=120)
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 14, 120, 0, 0))
        drain(0.5)
        bottom = next(i for i, line in enumerate(screen.display) if "tracks ·" in line)
        if "‹" in screen.display[bottom - 1]:
            bottom -= 1
        mouse(0, 4)
        mouse(32, bottom)
        for _ in range(5):
            mouse(65, bottom)  # wheel down while holding track
        expect_marker(bottom)
        unchanged(names)
        mouse(0, bottom, release=True)
        names.append(names.pop(0))
        expect_order(names)
        assert not markers()

        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        print("TUI DRAG PASS: visible gap, stable motion, release-only moves, first/middle/end, Escape, outside release, filtering, drag scrolling")
    except BaseException:
        print("\n".join(screen.display))
        raise
    finally:
        if pid:
            os.kill(pid, signal.SIGTERM)
            os.waitpid(pid, 0)
        os.close(terminal)

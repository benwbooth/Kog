#!/usr/bin/env python3
"""Compare real queue/draft terminal cells and exercise their shared controls.

uv run --with pyte python tests/playlist-workspace/tui-table-smoke.py
KOG_TUI_BINARY can select the normal kog launcher; KOG_TUI_SCREEN_DIR saves frames.
"""
import codecs
from contextlib import closing
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import sqlite3
import struct
import tempfile
import termios
import time

import pyte

repo = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="kog-tui-table-") as directory:
    root = Path(directory)
    for name in ("config/kog", "data/kog", "cache", "runtime", "tracks"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    (root / "config/kog/output-volume").write_text("0")
    titles = ["Zulu", "Alpha", "Echo"] + [f"Song {n:02}" for n in range(3, 20)]
    database = root / "data/kog/kog.db"
    with sqlite3.connect(database) as db:
        db.executescript("""
            CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL);
            CREATE TABLE playlist_entries(id INTEGER PRIMARY KEY AUTOINCREMENT, playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, kind TEXT NOT NULL, path TEXT NOT NULL, entry TEXT NOT NULL DEFAULT '', fragment TEXT);
            INSERT INTO playlists(name,position) VALUES('Reference playlist',0);
        """)
        for position, title in enumerate(titles):
            path = root / "tracks" / f"track-{position:02}.wav"
            wav = bytearray((repo / "tests/fixtures/codec-libs/tone.wav").read_bytes())
            info = b"INFO"
            for key, value in [(b"INAM", title), (b"IART", "Demo Artist"), (b"IPRD", "Demo Album")]:
                data = value.encode() + b"\0"
                info += key + struct.pack("<I", len(data)) + data + b"\0" * (len(data) % 2)
            wav += b"LIST" + struct.pack("<I", len(info)) + info
            struct.pack_into("<I", wav, 4, len(wav) - 8)
            path.write_bytes(wav)
            db.execute("INSERT INTO playlist_entries(playlist_id,position,kind,path) VALUES(1,?,'local',?)", (position, str(path)))
    db.close()

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
                    stream.feed(decoder.decode(os.read(terminal, 65536)))
                except OSError as error:
                    if error.errno != 5:
                        raise
                    # EIO is the normal PTY end-of-file after Ctrl+C.
                    break

    def send(keys):
        os.write(terminal, keys)
        drain()

    def mouse(button, x, y, release=False):
        send(f"\x1b[<{button};{x};{y}{'m' if release else 'M'}".encode())

    def click(x, y):
        mouse(0, x, y)
        mouse(0, x, y, True)

    def checkpoint():
        with closing(sqlite3.connect(database)) as db:
            row = db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='tui:default'").fetchone()
        return json.loads(row[0]) if row else None

    def wait(predicate, message):
        started = time.monotonic()
        for _ in range(100):
            drain(0.1)
            if predicate():
                return
        raise AssertionError(f"{message} after {time.monotonic() - started:.2f}s")

    def cells(row):
        return [tuple(screen.buffer[row][x]) for x in range(41, 120)]

    def save_frame(name):
        if directory := os.environ.get("KOG_TUI_SCREEN_DIR"):
            destination = Path(directory)
            destination.mkdir(parents=True, exist_ok=True)
            frame = [[screen.buffer[y][x]._asdict() for x in range(screen.columns)] for y in range(screen.lines)]
            (destination / f"{name}.json").write_text(json.dumps(frame))
            (destination / f"{name}.txt").write_text("\n".join(screen.display))

    try:
        drain(2)
        assert "Play Queue" in screen.display[1], "Queue tab is missing at the top"
        send(b"\x1b[Z\x1b[B\r")
        wait(lambda: "Zulu" in screen.display[3] and "Demo Album" in screen.display[3], "Draft metadata was not decoded")
        assert "Reference playlist" in screen.display[1], "Draft tab is not above the header"
        assert all(label in screen.display[2] for label in ("#", "Title", "Artist", "Album", "Size"))
        assert not checkpoint()["queue"], "Opening the editor changed the queue"
        send(b"\x01a\x1b[H")  # Add all; return to one selected draft row.
        wait(lambda: len(checkpoint()["queue"]) == 20, "Add to Queue failed")
        draft_cells = [cells(row) for row in range(2, 23)]
        save_frame("draft")
        send(b"\x1b1")
        wait(lambda: checkpoint()["workspace"]["active"] == "queue", "Queue focus failed")
        assert [cells(row) for row in range(2, 23)] == draft_cells, "Queue and draft table cells/styles differ"
        save_frame("queue")
        queue = checkpoint()["queue"]
        current = checkpoint()["current"]

        # The top tab hitbox must choose the draft, not sort/select a queue row.
        click(screen.display[1].index("Reference playlist") + 3, 2)
        wait(lambda: checkpoint()["workspace"]["active"] == "local:1", "Top tab click missed")
        header = screen.display[2]
        edge = header.index("│", header.index("Title")) + 1
        mouse(0, edge, 3)
        mouse(32, edge + 6, 3)
        mouse(0, edge + 6, 3, True)
        resized = screen.display[2][41:]
        assert resized != header[41:], "Draft column resize did not change width"
        send(b"\x1b1")
        assert screen.display[2][41:] == resized, "Column widths differ between tabs"
        click(screen.display[1].index("Reference playlist") + 3, 2)

        edge = screen.display[2].index("│", screen.display[2].index("Title")) + 1
        mouse(0, edge, 3)
        mouse(32, 116, 3)
        mouse(0, 116, 3, True)
        assert "‹" in screen.display[28], "Draft horizontal scrollbar was not drawn"
        before = screen.display[2][41:]
        mouse(67, 80, 6)
        assert screen.display[2][41:] != before, "Draft horizontal wheel did not scroll"
        send(b"HaH")  # Shared column keyboard controls: fit to contents.

        # Header sort and search edit only the draft, with correct visible-row selection.
        click(screen.display[2].index("Title") + 3, 3)
        wait(lambda: "Alpha" in screen.display[3], "Draft header sort did not reorder rows")
        send(b"FAlpha\r\x1b[H")
        assert "Alpha" in screen.display[3] and "Zulu" not in "\n".join(screen.display[4:25])
        drain(1.1)
        assert checkpoint()["queue"] == queue and checkpoint()["current"] == current, "Draft controls changed playback queue"
        send(b"F\x01\x7f\r")

        screen.resize(lines=14, columns=120)
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 14, 120, 0, 0))
        drain(0.5)
        before = screen.display[3][41:]
        mouse(65, 80, 5)
        assert screen.display[3][41:] != before, "Draft vertical wheel did not scroll"
        click(70, 4)
        drain(1.1)
        state = checkpoint()["workspace"]["tabs"][0]["draft"]
        selected = next(i for i, row in enumerate(state["rows"]) if row["id"] in state["selected"])
        assert selected == 3, f"Scrolled row hit test selected {selected}, expected 3"
        assert "Reference playlist" in screen.display[1] and "Title" in screen.display[2]

        screen.resize(lines=20, columns=50)
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 20, 50, 0, 0))
        drain(0.5)
        assert "Reference playlist" in screen.display[1] and "Title" in screen.display[2], "Narrow table lost top tabs/header"
        save_frame("narrow")
        close_x = screen.display[1].index("×") + 1
        click(close_x, 2)
        wait(lambda: checkpoint()["workspace"]["pending_close"] == "local:1", "Tab close button missed dirty confirmation")
        assert "Unsaved changes:" in "\n".join(screen.display)
        send(b"c")
        wait(lambda: checkpoint()["workspace"]["pending_close"] is None, "Dirty close cancellation failed")
        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        print("TUI TABLE PASS: top tabs, identical queue/draft cells, metadata, shared resize/scroll controls, draft sort/filter, scrolled selection, narrow layout, close/cancel, queue isolation")
    except BaseException:
        print("\n".join(screen.display))
        print("Saved active tab:", checkpoint()["workspace"]["active"])
        print("Saved queue:", len(checkpoint()["queue"]))
        for log in root.rglob("*.log"):
            print(log.name, log.read_text(errors="replace")[-5000:])
        raise
    finally:
        if pid:
            os.kill(pid, signal.SIGTERM)
            os.waitpid(pid, 0)
        os.close(terminal)

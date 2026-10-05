#!/usr/bin/env python3
"""Exercise the real Edit submenu with pointer input in a private terminal.

uv run --with pyte python tests/playlist-workspace/tui-edit-menu-smoke.py
"""
import codecs
from contextlib import closing
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
with tempfile.TemporaryDirectory(prefix="kog-edit-menu-") as directory:
    root = Path(directory)
    for name in ("config/kog", "data/kog", "cache", "runtime", "tracks"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    (root / "config/kog/output-volume").write_text("0")
    database = root / "data/kog/kog.db"
    names = [f"track-{n}.wav" for n in range(3)]
    with closing(sqlite3.connect(database)) as db, db:
        db.executescript("""
            CREATE TABLE playlists(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, position INTEGER NOT NULL);
            CREATE TABLE playlist_entries(id INTEGER PRIMARY KEY AUTOINCREMENT, playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                position INTEGER NOT NULL, kind TEXT NOT NULL, path TEXT NOT NULL, entry TEXT NOT NULL DEFAULT '', fragment TEXT);
            INSERT INTO playlists(name,position) VALUES('Edit test',0);
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
    screen = pyte.Screen(120, 40)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")
    fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

    def drain(seconds=0.3):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([terminal], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
                try:
                    stream.feed(decoder.decode(os.read(terminal, 65536)))
                except OSError:
                    break

    def send(keys):
        os.write(terminal, keys)
        drain()

    def checkpoint():
        with closing(sqlite3.connect(database)) as db:
            row = db.execute("SELECT value FROM app_state WHERE namespace='sessions' AND key='tui:default'").fetchone()
        return json.loads(row[0]) if row else None

    def draft():
        return checkpoint()["workspace"]["tabs"][0]["draft"]

    def order():
        return [Path(row["entry"]["path"]).name for row in draft()["rows"]]

    def wait(predicate, message):
        for _ in range(60):
            drain(0.1)
            if predicate():
                return
        raise AssertionError(message)

    def click_label(label):
        y, line = next((y, line) for y, line in enumerate(screen.display) if label in line)
        x = line.rindex(label) + 2
        send(f"\x1b[<0;{x};{y+1}M\x1b[<0;{x};{y+1}m".encode())

    def edit(label):
        send(b"m\x1be")  # Main menu -> Alt+E: Edit.
        assert "╭─ Edit " in "\n".join(screen.display), "Edit submenu did not open"
        click_label(label)

    try:
        drain(2)
        send(b"\x1b[Z\x1b[B\r\x1b[H")
        wait(lambda: checkpoint()["workspace"]["active"] == "local:1" and len(draft()["rows"]) == 3, "Playlist did not open")
        edit("Save Changes")  # Clean draft: disabled action keeps menu open.
        assert "╭─ Edit " in "\n".join(screen.display), "Disabled Save activated"
        send(b"\x1b"); send(b"\x1b")
        edit("Move Down")
        moved = [names[1], names[0], names[2]]
        wait(lambda: order() == moved, "Move Down failed")
        edit("Undo")
        wait(lambda: order() == names, "Draft Undo failed")
        edit("Redo")
        wait(lambda: order() == moved, "Draft Redo failed")
        edit("Select All")
        wait(lambda: len(draft()["selected"]) == 3, "Select All failed")
        edit("Remove Selected")
        wait(lambda: not order(), "Remove Selected failed")
        edit("Undo")
        wait(lambda: order() == moved, "Remove Undo failed")
        edit("Clear Selection")
        wait(lambda: not draft()["selected"], "Clear Selection failed")
        edit("Clear Playlist")
        wait(lambda: not order(), "Clear Playlist failed")
        edit("Undo")
        wait(lambda: order() == moved, "Clear Undo failed")
        edit("Save Changes")
        drain(0.8)
        with closing(sqlite3.connect(database)) as db:
            stored = [Path(row[0]).name for row in db.execute("SELECT path FROM playlist_entries ORDER BY position")]
        assert stored == moved, "Save Changes missed the database"
        assert not checkpoint()["queue"], "Draft edits changed hidden queue"
        edit("Reload Saved Playlist")
        wait(lambda: order() == moved, "Reload failed")
        send(b"a\x1b1")
        wait(lambda: len(checkpoint()["queue"]) == 3, "Add to Queue failed")
        edit("Undo Append")
        wait(lambda: not checkpoint()["queue"], "Queue Undo Append failed")
        edit("Redo Append")
        wait(lambda: len(checkpoint()["queue"]) == 3, "Queue Redo Append failed")
        edit("Select All")
        wait(lambda: len(checkpoint()["selection"]["indices"]) == 3, "Queue Select All failed")
        edit("Remove Selected")
        wait(lambda: not checkpoint()["queue"], "Queue Remove Selected failed")
        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        print("TUI EDIT MENU PASS: mouse and Alt+E, disabled Save, queue/draft selection, remove, reorder, clear, undo/redo, save/reload, queue isolation")
    except BaseException:
        print("\n".join(screen.display))
        raise
    finally:
        if pid:
            os.kill(pid, signal.SIGTERM)
            os.waitpid(pid, 0)
        os.close(terminal)

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

    def resize(columns, lines):
        screen.resize(lines=lines, columns=columns)
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH", lines, columns, 0, 0))
        drain(0.5)

    def button(label):
        row, text = next((row, text) for row, text in enumerate(screen.display) if label in text)
        return text.index(label) + 1, row + 1

    def stored_paths():
        with closing(sqlite3.connect(database)) as db:
            return [row[0] for row in db.execute("SELECT path FROM playlist_entries WHERE playlist_id=1 ORDER BY position")]

    def selected_rows():
        saved = checkpoint()
        if saved["workspace"]["active"] == "queue":
            return set(saved["selection"]["indices"])
        draft = saved["workspace"]["tabs"][0]["draft"]
        return {index for index, row in enumerate(draft["rows"]) if row["id"] in draft["selected"]}

    def visible_rows():
        start = screen.display[2].index("#")
        end = screen.display[2].index("│", start)
        return {int(line[start:end]) - 1: row for row, line in enumerate(screen.display[3:], 3)
                if line[start:end].strip().isdecimal()}

    def expect_selection(expected):
        expected = set(expected)
        wait(lambda: selected_rows() == expected, f"Wrong selected tracks: expected {sorted(expected)}")
        visible = visible_rows()
        column = screen.display[2].index("Title")
        painted = {index for index, row in visible.items() if screen.buffer[row][column].bg == selection_background}
        if painted != expected.intersection(visible):
            save_frame("selection-failure")
        assert painted == expected.intersection(visible), f"Persisted selection {sorted(expected)}, highlighted rows {sorted(painted)}"

    def choose(index, modifiers=0):
        row = visible_rows()[index] + 1
        mouse(modifiers, 70, row)
        mouse(modifiers, 70, row, True)

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
        send(b"\x1b[H")  # Compare equal selections as well as equal content.
        assert [cells(row) for row in range(2, 23)] == draft_cells, "Queue and draft table cells/styles differ"
        save_frame("queue")
        queue = checkpoint()["queue"]
        current = checkpoint()["current"]

        # Alt-click ranges followed by individual Ctrl-click toggles must repaint
        # the clicked row immediately, without needing another pointer event.
        for key in ("queue", "local:1"):
            if key != "queue":
                click(screen.display[1].index("Reference playlist") + 3, 2)
                wait(lambda: checkpoint()["workspace"]["active"] == key, "Draft tab focus failed")
            choose(0)
            selection_background = screen.buffer[3][screen.display[2].index("Title")].bg
            choose(7, 8)
            expect_selection(range(8))
            remaining = set(range(8))
            for index in (3, 5, 1, 0, 7, 4, 6, 2):
                choose(index, 16)
                remaining.remove(index)
                expect_selection(remaining)
                row = visible_rows()[index]
                assert screen.buffer[row][screen.display[2].index("Title")].bold, "Deselected cursor lost its keyboard focus cue"
                if key == "queue":
                    assert f"Selected {len(remaining)} tracks" in screen.display[-1], "Selection count is stale"
            save_frame(f"{key.replace(':', '-')}-deselected")
            choose(2, 16)
            expect_selection({2})
            choose(2, 16)
            expect_selection(set())

            send(b"Ftrack-1\r")
            choose(10)
            choose(14, 8)
            choose(12, 16)
            expect_selection({10, 11, 13, 14})
            send(b"F\x01\x7f\r\x1b[H")

            resize(120, 14)
            mouse(65, 120, 5)  # Scroll using the shared vertical scrollbar.
            visible = sorted(visible_rows())
            assert visible[0] > 0, "Selection test did not scroll"
            choose(visible[0])
            choose(visible[-1], 8)
            choose(visible[len(visible) // 2], 16)
            expect_selection(set(visible) - {visible[len(visible) // 2]})
            resize(120, 34)
            send(b"\x1b[H")
        assert checkpoint()["queue"] == queue, "Selection gestures changed queue contents"
        send(b"\x1b1")
        wait(lambda: checkpoint()["workspace"]["active"] == "queue", "Queue refocus failed")

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
        assert "Unsaved changes" in screen.display[6], "Close dialog is not centered"
        assert "Reference playlist" in screen.display[7], "Dialog does not identify the playlist"
        assert all(label in screen.display[10] for label in ("[Save]", "[Discard]", "[Cancel]"))
        save_frame("unsaved-narrow")
        original_paths = stored_paths()
        draft_before = checkpoint()["workspace"]["tabs"][0]["draft"]
        click(1, 1)  # Background menu, sidebar, transport, and wheel must not act.
        click(2, 4)
        click(15, 19)
        mouse(65, 40, 4)
        send(b"p\x1b1")
        assert checkpoint()["workspace"]["active"] == "local:1"
        assert checkpoint()["workspace"]["tabs"][0]["draft"] == draft_before
        assert "Unsaved changes" in screen.display[6], "Background input dismissed the dialog"
        send(b"\r")  # Cancel is the initial selection.
        wait(lambda: checkpoint()["workspace"]["pending_close"] is None, "Default Cancel failed")
        assert stored_paths() == original_paths, "Cancel wrote the draft"

        send(b"\x17")
        resize(120, 34)
        assert "Unsaved changes" in screen.display[13], "Resized dialog is not centered"
        save_frame("unsaved-wide")
        save_x, save_y = button("[Save]")
        cancel_x, cancel_y = button("[Cancel]")
        selected_background = screen.buffer[cancel_y - 1][cancel_x].bg
        send(b"\t")
        assert screen.buffer[save_y - 1][save_x].bg == selected_background, "Tab did not select Save"
        send(b"\x1b[Z")
        assert screen.buffer[cancel_y - 1][cancel_x].bg == selected_background, "BackTab did not select Cancel"
        send(b"\x1b[D\x1b[C")
        assert screen.buffer[cancel_y - 1][cancel_x].bg == selected_background, "Arrow navigation failed"
        click(*button("[Cancel]"))
        wait(lambda: checkpoint()["workspace"]["pending_close"] is None, "Mouse Cancel failed")

        send(b"\x17")
        resize(24, 14)
        assert "Unsaved changes" in screen.display[2]
        assert button("[Save]")[1] < button("[Discard]")[1] < button("[Cancel]")[1], "Small dialog buttons must stack"
        save_frame("unsaved-small")
        send(b"\x1b")
        wait(lambda: checkpoint()["workspace"]["pending_close"] is None, "Escape did not cancel")

        resize(120, 34)
        send(b"\x17\t\r")  # Tab from Cancel to Save, then confirm.
        wait(lambda: checkpoint()["workspace"]["active"] == "queue", "Save did not close the draft")
        expected_paths = [row["entry"]["path"] for row in draft_before["rows"]]
        assert expected_paths != original_paths, "Fixture did not edit the saved order"
        assert stored_paths() == expected_paths, "Save did not persist the draft order"
        send(b"\t\r")  # Tracks -> saved playlists; reopen the selected playlist.
        wait(lambda: checkpoint()["workspace"]["active"] == "local:1", "Could not reopen saved playlist")
        send(b"\x1b[Hd\x17")
        wait(lambda: checkpoint()["workspace"]["pending_close"] == "local:1", "Removal did not create a dirty draft")
        click(*button("[Discard]"))
        wait(lambda: checkpoint()["workspace"]["active"] == "queue", "Mouse Discard did not close the draft")
        assert stored_paths() == expected_paths, "Discard changed saved contents"
        assert checkpoint()["queue"] == queue, "Close dialog controls changed the queue"
        send(b"\x03")
        os.waitpid(pid, 0)
        pid = 0
        print("TUI TABLE PASS: shared table controls, Alt-range/Ctrl-deselect matches persisted selection and highlights in queue/draft/filtered/scrolled views, distinct cursor cue, selection counts, centered close dialog at 120/50/24 columns, keyboard/mouse controls, Save/Discard/Cancel, queue isolation")
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

"""Check shuffle and repeat order when album tags arrive after playback starts."""

import fcntl
import os
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
import wave
from pathlib import Path

import pyte
from mutagen.id3 import TALB, TRCK
from mutagen.wave import WAVE


BINARY = Path(
    os.environ.get(
        "KOG_TUI_TEST_BINARY",
        Path(__file__).resolve().parents[1] / "target/debug/kog",
    )
)
COLUMNS, ROWS = 120, 32


def write_track(path, album, number):
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(8000)
        output.writeframes(b"\0\0" * 240_000)
    tagged = WAVE(path)
    tagged.add_tags()
    tagged.tags.add(TALB(encoding=3, text=album))
    tagged.tags.add(TRCK(encoding=3, text=str(number)))
    tagged.save()


class Tui:
    def __init__(self, base, hold):
        env = os.environ.copy()
        for key, subdir in (
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
        ):
            env[key] = str(Path(base, subdir))
        env["TERM"] = "xterm-256color"
        env["KOG_TUI_TEST_METADATA_HOLD"] = str(hold)
        self.master, slave = pty.openpty()
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLUMNS, 0, 0))
        self.process = subprocess.Popen(
            [str(BINARY)] if BINARY.name == "kog-tui" else [str(BINARY), "--tui"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=env,
            close_fds=True,
        )
        os.close(slave)
        self.screen = pyte.Screen(COLUMNS, ROWS)
        self.stream = pyte.Stream(self.screen)

    def text(self):
        return "\n".join(self.screen.display)

    def drain(self, seconds=0.2):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([self.master], [], [], min(0.05, until - time.monotonic()))[0]:
                try:
                    data = os.read(self.master, 65_536)
                except OSError:
                    break
                if not data:
                    break
                self.stream.feed(data.decode("utf8", "replace"))

    def send(self, data, seconds=0.2):
        os.write(self.master, data if isinstance(data, bytes) else data.encode())
        self.drain(seconds)

    def wait_for(self, value, timeout=8):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if value in self.text():
                return
            self.drain(0.1)
        raise AssertionError((value, self.screen.display))

    def row(self, text):
        for y, line in enumerate(self.screen.display):
            if text in line and "Playing" not in line:
                return y
        raise AssertionError((text, self.screen.display))

    def add(self, paths):
        # Held rows have no title yet, so count rows instead of reading names.
        for count, path in enumerate(paths, 1):
            self.send(b"\x1ba")  # Main-menu Add File accelerator.
            self.wait_for("Add file or folder")
            self.send(os.fsencode(path) + b"\r")
            self.wait_for(f"{count} tracks")

    def double_click_row(self, index):
        y = self.row("#   │Title") + 1 + index
        click = f"\x1b[<0;50;{y + 1}M\x1b[<0;50;{y + 1}m"
        self.send(click, 0.05)
        self.send(click)

    def wait_footer(self, title, timeout=8):
        # Held tracks have no title, so the "Playing" status cannot name them.
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            footer = next((line for line in self.screen.display if "⏮" in line), "")
            if title in footer:
                return
            self.drain(0.1)
        raise AssertionError((title, footer))

    def next(self, expected):
        self.send(">")
        self.wait_footer(expected)

    def quit(self):
        self.send("q", 0.5)
        if "Confirm exit" in self.text():
            y = self.row("[Exit]")
            x = self.screen.display[y].find("[Exit]") + 2
            self.send(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            raise AssertionError(self.screen.display)
        assert self.process.returncode == 0, self.process.returncode

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)
        os.close(self.master)


def release_tags(tui, hold, album):
    assert album not in tui.text(), "album tags loaded before the hold was released"
    hold.unlink()
    tui.wait_for(album)


def check_album_shuffle(base):
    # Untagged, every row looks like one album, so shuffle follows list order.
    # Once tags arrive, the rest of Alpha must follow before Beta begins.
    paths = [Path(base, name) for name in ("song-a.wav", "song-b.wav", "song-c.wav", "song-d.wav")]
    for path, (album, number) in zip(
        paths, (("Alpha", 2), ("Beta", 1), ("Alpha", 1), ("Beta", 2))
    ):
        write_track(path, album, number)
    hold = Path(base, "hold")
    hold.touch()
    tui = Tui(base, hold)
    try:
        tui.wait_for("Search playlist")
        tui.add(paths)
        tui.send("S")
        tui.wait_for("Shuffle: albums")
        tui.double_click_row(0)
        tui.wait_for("Playing")
        release_tags(tui, hold, "Alpha")
        tui.wait_footer("song-a")
        tui.next("song-c")
        tui.next("song-b")
        tui.next("song-d")
        tui.quit()
    finally:
        tui.close()


def check_album_repeat(base):
    # Untagged, Repeat Album would wrap from the last row to the first. With
    # late tags it must wrap to the first track of the playing album instead.
    paths = [Path(base, name) for name in ("song-q.wav", "song-p.wav", "song-r.wav")]
    for path, (album, number) in zip(paths, (("Echo", 1), ("Delta", 1), ("Delta", 2))):
        write_track(path, album, number)
    hold = Path(base, "hold")
    hold.touch()
    tui = Tui(base, hold)
    try:
        tui.wait_for("Search playlist")
        tui.add(paths)
        tui.send("RR")
        tui.wait_for("Repeat: album")
        tui.double_click_row(2)
        tui.wait_for("Playing")
        release_tags(tui, hold, "Delta")
        tui.wait_footer("song-r")
        tui.next("song-p")
        tui.next("song-r")
        tui.quit()
    finally:
        tui.close()


for check in (check_album_shuffle, check_album_repeat):
    with tempfile.TemporaryDirectory(prefix="kog-tui-late-album-") as base:
        check(base)
print("Late album tags keep TUI album shuffle and Repeat Album order: PASS")

"""Verify the TUI displays exact and readable file sizes from loaded metadata."""

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


IDS = [
    "index", "star", "status", "rating", "title", "albumartist", "artist",
    "composer", "album", "length", "filesizebytes", "filesize", "date",
    "genre", "track", "playcount", "path", "filename", "codec",
    "samplerate", "bitspersample", "bitrate",
]

with tempfile.TemporaryDirectory(prefix="kog-tui-size-") as base:
    root = Path(base)
    song = root / "song.wav"
    with wave.open(str(song), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(8000)
        output.writeframes(b"\0\0" * 80_000)
    assert song.stat().st_size == 160_044

    env = os.environ.copy()
    for key, subdir in (
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
    ):
        env[key] = str(root / subdir)
    env["TERM"] = "xterm-256color"
    config = Path(env["XDG_CONFIG_HOME"], "kog")
    config.mkdir(parents=True)
    shown = {"index", "title", "filesizebytes", "filesize"}
    config.joinpath("tui-column-layout").write_text(
        ";".join(f"{id},{20 if id == 'title' else 15},{int(id in shown)}" for id in IDS)
    )

    binary = Path(os.environ.get(
        "KOG_TUI_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/kog-tui"
    ))
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    process = subprocess.Popen(
        [str(binary)] if binary.name == "kog-tui" else [str(binary), "--tui"],
        stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True,
    )
    os.close(slave)
    screen = pyte.Screen(120, 40)
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

    def send(data):
        os.write(master, data if isinstance(data, bytes) else data.encode())
        drain()

    def wait_for(text, timeout=20):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if text in "\n".join(screen.display):
                return
            drain(0.1)
        raise AssertionError((text, screen.display[:18], screen.display[-3:]))

    try:
        wait_for("Size (bytes)")
        send(b"\x1ba")
        wait_for("Add file or folder")
        send(os.fsencode(song) + b"\r")
        wait_for("160044")
        wait_for("156.3 KiB")
        send("q")
        process.wait(timeout=5)
        assert process.returncode == 0, process.returncode
        print("TUI exact and readable file-size columns: PASS")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)

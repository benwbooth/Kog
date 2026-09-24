"""Exercise the TUI's embedded API server and shared settings on a PTY."""

import base64
import fcntl
import json
import os
import pty
import select
import socket
import ssl
import struct
import subprocess
import tempfile
import termios
import time
import urllib.error
import urllib.request
from pathlib import Path

import pyte

with tempfile.TemporaryDirectory(prefix="kog-tui-server-controls-") as root:
    root = Path(root)
    music = root / "Music"
    music.mkdir()
    env = os.environ.copy()
    for key, folder in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"), ("XDG_CACHE_HOME", "cache")):
        env[key] = str(root / folder)
    env["TERM"] = "xterm-256color"
    with socket.socket() as available:
        available.bind(("127.0.0.1", 0))
        port = available.getsockname()[1]
    master, slave = pty.openpty()
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    executable = str(Path(__file__).resolve().parents[1] / "target/debug/kog")
    process = subprocess.Popen([executable, "--tui"], stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True)
    os.close(slave)
    screen = pyte.Screen(120, 40)
    stream = pyte.Stream(screen)

    def drain(seconds=0.2):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([master], [], [], min(0.05, max(0, end - time.time())))
            if ready:
                try:
                    data = os.read(master, 65536)
                except OSError:
                    return
                stream.feed(data.decode("utf8", "replace"))

    def send(value, seconds=0.2):
        os.write(master, value if isinstance(value, bytes) else value.encode())
        drain(seconds)

    def click(x, y):
        send(f"\x1b[<0;{x+1};{y+1}M\x1b[<0;{x+1};{y+1}m")

    def wait_for(value, seconds=10):
        deadline = time.time() + seconds
        while time.time() < deadline:
            if value in "\n".join(screen.display):
                return
            drain(0.1)
        raise AssertionError((value, screen.display[:22], screen.display[-3:], list(root.rglob('*.pem'))))

    def server_menu():
        click(5, 0)
        click(10, 13)
        click(10, 17)
        assert "API Server" in screen.display[1], screen.display[:20]

    try:
        wait_for("Search playlist")
        click(1, 0)
        send(str(music) + "\r", 1.2)
        music_settings = list(root.rglob("music-directory"))
        assert music_settings and music_settings[0].read_text() == str(music), (screen.display[:25], screen.display[-3:], list(root.rglob("*")))
        server_menu()
        click(10, 6)
        send(b"\x01" + str(port).encode() + b"\r", 0.5)
        wait_for("Server settings saved")
        server_menu()
        click(10, 7)
        wait_for("Server settings saved")
        server_menu()
        click(10, 2)
        wait_for("Server running at")
        address = f"http://127.0.0.1:{port}"
        config_file = next((root / "config").rglob("server.json"))
        config = json.loads(config_file.read_text())
        assert config["enabled"] and config["auth"] == "token" and config["token"]
        assert config_file.stat().st_mode & 0o777 == 0o600
        request = urllib.request.Request(address + "/api/library", headers={
            "Authorization": "Bearer " + config["token"],
            "X-Kog-Device": "tui-server-control-test",
        })
        for _ in range(30):
            try:
                with urllib.request.urlopen(request, timeout=1) as response:
                    listing = json.load(response)
                    assert listing["path"] == str(music), listing
                    break
            except (OSError, urllib.error.HTTPError):
                time.sleep(0.1)
        else:
            raise AssertionError("embedded server did not serve the library")
        server_menu()
        click(10, 16)
        wait_for("tui-server-control-test")
        send(b"\x1b")
        server_menu()
        click(10, 17)
        send("tui-server-control-test\r")
        wait_for("blocked")
        try:
            urllib.request.urlopen(request, timeout=2)
            raise AssertionError("blocked device was served")
        except urllib.error.HTTPError as error:
            assert error.code == 403, error.code
        server_menu()
        click(10, 3)
        wait_for("Server stopped")
        assert not json.loads(config_file.read_text())["enabled"]
        server_menu()
        click(10, 14)
        assert json.loads(config_file.read_text())["default_codec"] == "opus"
        server_menu()
        click(10, 15)
        send(b"\x01" + b"64\r")
        assert json.loads(config_file.read_text())["cache_bytes"] == 64 * 1024 * 1024
        server_menu()
        click(10, 7)
        assert json.loads(config_file.read_text())["auth"] == "basic"
        server_menu()
        click(10, 10)
        send("demo\r")
        server_menu()
        click(10, 11)
        send("secret\r")
        wait_for("Server settings saved")
        basic_config = json.loads(config_file.read_text())
        assert basic_config["credentials"]["username"] == "demo"
        assert basic_config["credentials"]["password_hash"] and '"secret"' not in config_file.read_text(), (basic_config, screen.display[-3:])
        server_menu()
        click(10, 12)
        assert json.loads(config_file.read_text())["tls"]["mode"] == "selfsigned"
        send(b"\x1b", 0.5)
        server_menu()
        click(10, 2)
        secure_request = urllib.request.Request(f"https://127.0.0.1:{port}/api/library", headers={
            "Authorization": "Basic " + base64.b64encode(b"demo:secret").decode(),
            "X-Kog-Device": "tui-https-test",
        })
        for _ in range(30):
            try:
                with urllib.request.urlopen(secure_request, timeout=1, context=ssl._create_unverified_context()) as response:
                    assert json.load(response)["path"] == str(music)
                    break
            except (OSError, urllib.error.HTTPError):
                time.sleep(0.1)
        else:
            diagnostic = next((root / 'config').rglob('tui-diagnostics.log'))
            raise AssertionError(("embedded HTTPS server did not serve the library", screen.display[:20], screen.display[-3:], diagnostic.read_text()))
        server_menu()
        click(10, 3)
        wait_for("Server stopped")
        certificate = root / 'config' / 'kog' / 'tls' / 'self-signed.pem'
        private_key = root / 'config' / 'kog' / 'tls' / 'self-signed.key'
        assert certificate.is_file() and private_key.is_file()
        server_menu()
        click(10, 13)
        send(str(certificate) + "\r", 1.2)
        wait_for("PEM private key file path")
        send(str(private_key) + "\r", 1.2)
        wait_for("Server settings saved")
        imported_tls = json.loads(config_file.read_text())["tls"]
        assert imported_tls["mode"] == "pem" and Path(imported_tls["certificate_path"]).is_file()
        assert Path(imported_tls["private_key_path"]).stat().st_mode & 0o777 == 0o600
        server_menu()
        click(10, 2)
        wait_for("Server running at https://")
        with urllib.request.urlopen(secure_request, timeout=3, context=ssl._create_unverified_context()) as response:
            assert json.load(response)["path"] == str(music)
        server_menu()
        click(10, 3)
        wait_for("Server stopped")
        print("TUI server token/basic auth, HTTP/HTTPS/PEM, address, codec/cache, start/stop and device controls: PASS")
        send("q")
        process.wait(timeout=5)
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        os.close(master)

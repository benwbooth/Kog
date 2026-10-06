#!/usr/bin/env python3
"""Exercise tab drags with SGR mouse events and persisted session checkpoints.

uv run --with pyte python tests/playlist-workspace/tui-tab-drag-smoke.py
"""
import codecs
import copy
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
with tempfile.TemporaryDirectory(prefix="kog-tab-drag-") as directory:
    root = Path(directory)
    for name in ("config", "data/kog", "cache", "runtime"):
        (root / name).mkdir(parents=True)
    (root / "runtime").chmod(0o700)
    database = root / "data/kog/kog.db"
    pid = 0
    def start():
        global pid, terminal, screen, stream, decoder
        pid, terminal = pty.fork()
        if not pid:
            os.environ.update({f"XDG_{key}_HOME":str(root / value) for key,value in [("CONFIG","config"),("DATA","data"),("CACHE","cache")]})
            os.environ.update(XDG_RUNTIME_DIR=str(root / "runtime"), TERM="xterm-256color")
            os.environ.pop("KOG_SESSION_ID", None)
            binary = os.environ.get("KOG_TUI_BINARY", str(repo / "target/debug/kog-tui"))
            os.execv(binary, [binary])
        screen = pyte.Screen(120, 28)
        stream = pyte.Stream(screen)
        decoder = codecs.getincrementaldecoder("utf-8")("replace")
        fcntl.ioctl(terminal, termios.TIOCSWINSZ, struct.pack("HHHH",28,120,0,0))
        drain(1.8)
    def drain(seconds=.2):
        end = time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([terminal],[],[],min(.05,max(0,end-time.monotonic())))[0]:
                try: data=os.read(terminal,65536)
                except OSError: break
                stream.feed(decoder.decode(data))
    def send(keys): os.write(terminal,keys); drain()
    def mouse(x,y,button=0,release=False): send(f"\x1b[<{button};{x};{y}{'m' if release else 'M'}".encode())
    def stop():
        global pid
        send(b"\x03"); os.waitpid(pid,0); pid=0; os.close(terminal)
    def checkpoint():
        with sqlite3.connect(database) as db:
            return json.loads(db.execute("select value from app_state where namespace='sessions' and key='tui:default'").fetchone()[0])
    def keys():
        w=checkpoint()['workspace']; result=[t['key'] for t in w['tabs']];result.insert(w.get('queue_position',0),'queue');return result
    def position(label):
        for y,line in enumerate(screen.display):
            if label in line:return line.index(label)+2,y+1
        raise AssertionError(f"Missing {label}\n"+'\n'.join(screen.display))
    def drag(label,destination,outside=False):
        x,y=position(label);tx,ty=position(destination)
        mouse(x,y);mouse(tx-3,ty,32);drain(.3)
        assert keys()==expected, 'Changed during preview'
        mouse(tx-3,ty+4 if outside else ty,release=True);drain(1.1)
    try:
        start();stop()
        saved=checkpoint()
        def tab(key,name):
            rows=[{'id':1,'entry':{'kind':'local','path':'/a.wav'}}]
            return dict(key=key,name=name,scope='local',playlist_id=1,readonly=False,
                draft=dict(rows=rows,selected=[1],anchor=1),saved=[],undo=[dict(rows=[],selected=[],anchor=None)],redo=[],revision=1,loading=None,saving=None,error=None)
        saved['workspace']=dict(tabs=[tab('a','Alpha'),tab('b','Beta'),tab('c','Gamma')],active='b',serial=10,pending_close=None)
        with sqlite3.connect(database) as db: db.execute("update app_state set value=? where namespace='sessions' and key='tui:default'",(json.dumps(saved),))
        start();original=checkpoint();expected=['queue','a','b','c']
        drag('Gamma','Play Queue');expected=['c','queue','a','b'];assert keys()==expected
        assert checkpoint()['workspace']['active']=='b'
        drag('Play Queue','Beta');expected=['c','a','queue','b'];assert keys()==expected
        drag('Alpha','Gamma',outside=True);assert keys()==expected
        x,y=position('Alpha');tx,ty=position('Gamma');mouse(x,y);mouse(tx,ty,32);send(b'\x1b');mouse(tx,ty,release=True);drain(1.1);assert keys()==expected
        assert 'Exit Kog' not in '\n'.join(screen.display)
        current=checkpoint()
        assert current['workspace']['active']=='b'
        assert current['queue']==original['queue'] and current['current']==original['current']
        assert sorted(current['workspace']['tabs'],key=lambda t:t['key'])==sorted(original['workspace']['tabs'],key=lambda t:t['key'])
        stop();start();assert keys()==expected;assert checkpoint()['workspace']['active']=='b';stop()
        print('TUI TAB DRAG PASS: queue and draft moves, release-only commit, outside/Escape cancellation, active/draft/selection/undo/queue preservation, restart order')
    finally:
        if pid: os.kill(pid,signal.SIGTERM);os.waitpid(pid,0);os.close(terminal)

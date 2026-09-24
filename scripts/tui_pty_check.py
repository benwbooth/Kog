import fcntl, http.server, json, os, pty, re, select, signal, sqlite3, struct, subprocess, tempfile, threading, time, urllib.parse, wave, zipfile, zlib
from pathlib import Path
import pyte
from mutagen.wave import WAVE

root=tempfile.TemporaryDirectory(prefix='kog-tui-pty-')
base=root.name
music=os.path.join(base,'Music')
os.makedirs(os.path.join(music,'album','sub'))
os.makedirs(os.path.join(base,'.HiddenMusic'))
for name in ('album/a.wav','album/b.wav','album/sub/c.wav'):
    with wave.open(os.path.join(music,name),'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8000)
with zipfile.ZipFile(os.path.join(music,'album','pack.zip'),'w') as z:
    z.write(os.path.join(music,'album','a.wav'),'inner/deep.wav')
long_path=os.path.join(base,'long.wav')
with wave.open(long_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*240000)
second_path=os.path.join(base,'second.wav')
with wave.open(second_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
third_path=os.path.join(base,'third.wav')
with wave.open(third_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
remote_root=tempfile.TemporaryDirectory(prefix='kog-tui-remote-fixture-')
remote_wav=os.path.join(remote_root.name,'remote.wav')
with wave.open(remote_wav,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
remote_requests=[]
class RemoteHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self,*args):pass
    def respond(self,code,payload):
        body=json.dumps(payload).encode()
        self.send_response(code);self.send_header('Content-Type','application/json')
        self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def do_GET(self):
        url=urllib.parse.urlsplit(self.path)
        query=urllib.parse.parse_qs(url.query)
        remote_requests.append((url.path,query,self.headers.get('Authorization')))
        if url.path=='/api/stream':
            if query.get('token')!=['PTY remote token']:
                self.respond(401,{'error':'invalid token'});return
            body=Path(remote_wav).read_bytes()
            self.send_response(200);self.send_header('Content-Type','audio/wav')
            self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
            return
        if self.headers.get('Authorization')!='Bearer PTY remote token':
            self.respond(401,{'error':'invalid token'});return
        file={'name':'remote.wav','path':'/virtual/album/remote.wav','kind':'local','entry':'','fragment':None}
        archive_file={'name':'song.wav','path':'/virtual/album/pack.zip','kind':'archive','entry':'inner/song.wav','fragment':None}
        if url.path=='/api/library':
            path=query.get('path',['/virtual'])[0]
            if path=='/virtual':
                self.respond(200,{'path':'/virtual','directories':[{'name':'album','path':'/virtual/album'}],'files':[]})
            elif path=='/virtual/album':
                self.respond(200,{'path':path,'directories':[{'name':'pack.zip','path':'/virtual/album/pack.zip'}],'files':[file]})
            elif path=='/virtual/album/pack.zip':
                self.respond(200,{'path':path,'directories':[{'name':'inner','path':path+'/inner'}],'files':[]})
            elif path=='/virtual/album/pack.zip/inner':
                self.respond(200,{'path':path,'directories':[],'files':[archive_file]})
            else:self.respond(404,{'error':'unknown folder'})
        elif url.path=='/api/library/search':
            self.respond(200,{'results':[file] if 'remote' in query.get('q',[''])[0] else [],'generation':1,'done':True})
        else:self.respond(404,{'error':'unknown endpoint'})
    def do_POST(self):
        url=urllib.parse.urlsplit(self.path)
        remote_requests.append((url.path,{},self.headers.get('Authorization')))
        if self.headers.get('Authorization')!='Bearer PTY remote token':
            self.respond(401,{'error':'invalid token'});return
        if url.path!='/api/expand':
            self.respond(404,{'error':'unknown endpoint'});return
        body=self.rfile.read(int(self.headers.get('Content-Length','0')))
        files=json.loads(body)
        self.respond(200,{'tracks':[[file] for file in files]})
remote_server=http.server.ThreadingHTTPServer(('127.0.0.1',0),RemoteHandler)
remote_server.daemon_threads=True
threading.Thread(target=remote_server.serve_forever,daemon=True).start()
env=os.environ.copy()
for key,sub in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:
    env[key]=os.path.join(base,sub)
env['TERM']='xterm-256color'
for key in ('KOG_MIDI_ENGINE','KOG_SOUNDFONT','KOG_SC55_ROMS','KOG_MT32_ROMS','KOG_MT32_GM_PROGRAM_MAPPING'):
    env.pop(key,None)
master,slave=pty.openpty()
def resize(cols,rows):
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH',rows,cols,0,0))
import termios
resize(120,40)
binary=Path(os.environ.get('KOG_TUI_TEST_BINARY', str(Path(__file__).resolve().parents[1] / 'target/debug/kog')))
args=[str(binary)] if binary.name == 'kog-tui' else [str(binary),'--tui']
p=subprocess.Popen(args,stdin=slave,stdout=slave,stderr=slave,env=env,close_fds=True)
os.close(slave)
screen=pyte.Screen(120,40); stream=pyte.Stream(screen)
def drain(seconds=.3):
    end=time.time()+seconds
    while time.time()<end:
        ready,_,_=select.select([master],[],[],max(0,min(.05,end-time.time())))
        if ready:
            try: data=os.read(master,65536)
            except OSError: break
            if not data: break
            stream.feed(data.decode('utf8','replace'))
def send(data,seconds=.25):
    os.write(master,data if isinstance(data,bytes) else data.encode());drain(seconds)
def click(x,y,button=0):
    send(f'\x1b[<{button};{x+1};{y+1}M\x1b[<{button};{x+1};{y+1}m')
def choose_music_folder(path):
    click(1,0)
    wait_for('Select Music Folder')
    send(b'\x0c\x01'+os.fsencode(path)+b'\r',.5)
    wait_for('Choose This Folder')
    send(b'\x0f',.4)
    assert 'Select Music Folder' not in '\n'.join(screen.display)
def folder_row(name):
    for i,line in enumerate(screen.display):
        if line.find('▱ '+name)>15:return i
    raise AssertionError((name,'not in folder chooser',screen.display))
def row(text):
    for i,line in enumerate(screen.display):
        if text in line:return i
    raise AssertionError((text,'not on screen',screen.display[:15],screen.display[-3:]))
def menu_item(title,index):
    header=f'╭─ {title} '
    for y,line in enumerate(screen.display):
        x=line.find(header)
        if x>=0:
            click(x+3,y+1+index)
            return
    raise AssertionError((header,'not on screen',screen.display[:20]))
def assert_saved_count(name,count):
    sidebar=screen.display[row(name)].split('│',1)[0]
    assert sidebar.rstrip().endswith(str(count)),(name,count,sidebar)
def wait_for(text, timeout=5):
    end=time.time()+timeout
    while time.time()<end:
        if text in '\n'.join(screen.display): return
        drain(.1)
    raise AssertionError((text,'not on screen',screen.display[:15],screen.display[-3:]))
def save_snapshot(path):
    from PIL import Image, ImageDraw, ImageFont
    font_path=subprocess.check_output(['fc-match','DejaVu Sans Mono','-f','%{file}'],text=True).strip()
    font=ImageFont.truetype(font_path,16)
    image=Image.new('RGB',(1200,800),'#202123');draw=ImageDraw.Draw(image)
    named={'default':None,'black':'#000000','white':'#f4f4f4','brightwhite':'#ffffff','blue':'#4778ac','brightblue':'#6faee8','cyan':'#55b5bc','brightcyan':'#6cdbe0'}
    def color(value,fallback):
        resolved=named.get(value,value)
        return fallback if resolved is None else ('#'+resolved if len(resolved)==6 and not resolved.startswith('#') else resolved)
    for yy in range(40):
        for xx in range(120):
            cell=screen.buffer[yy][xx]
            fg=color(cell.fg,'#dedede');bg=color(cell.bg,'#202123')
            if cell.reverse:fg,bg=bg,fg
            draw.rectangle((xx*10,yy*20,(xx+1)*10-1,(yy+1)*20-1),fill=bg)
            if cell.data.strip():draw.text((xx*10,yy*20-1),cell.data,font=font,fill=fg)
    image.save(path)
try:
    wait_for('Search playlist')
    click(1,0)
    wait_for('Select Music Folder')
    send(b'\x0c')
    assert 'Select Music Folder' in '\n'.join(screen.display),('after ctrl l',screen.display)
    send(b'\x01'+os.fsencode(base)+b'/discard-me',.6)
    wait_for('Select Music Folder')
    send(b'\x17',.3)
    wait_for(base+'/',5)
    send(b'\r',.3)
    assert '▱ Music' in '\n'.join(screen.display),screen.display
    send('.',.3)
    assert '▱ .HiddenMusic' in '\n'.join(screen.display)
    send('.',.3)
    assert '▱ .HiddenMusic' not in '\n'.join(screen.display)
    if folder_snapshot:=os.environ.get('KOG_TUI_FOLDER_SNAPSHOT_PATH'):
        save_snapshot(folder_snapshot)
    folder_y=folder_row('Music')
    click(30,folder_y);click(30,folder_y)
    assert '▱ album' in '\n'.join(screen.display)
    choose_y=row('[ Choose This Folder ]')
    click(screen.display[choose_y].find('[ Choose This Folder ]')+4,choose_y)
    assert 'Select Music Folder' not in '\n'.join(screen.display)
    click(1,0);wait_for('Select Music Folder')
    cancel_y=row('[ Cancel ]')
    click(screen.display[cancel_y].find('[ Cancel ]')+4,cancel_y)
    assert 'Select Music Folder' not in '\n'.join(screen.display)
    wait_for('album')
    y=row('album');click(10,y)
    assert 'a.wav' in '\n'.join(screen.display)
    click(10,row('a.wav'));send('v');click(10,row('b.wav'))
    assert 'Selected 2 tree items' in screen.display[-1],screen.display[-1]
    zrow=row('pack.zip');click(10,zrow)
    assert 'inner' in '\n'.join(screen.display)
    click(10,row('inner'))
    assert 'deep.wav' in '\n'.join(screen.display)
    click(10,y);click(10,y);drain(.5)
    assert 'Added 4 tracks' in screen.display[-1], screen.display[-1]
    session_path=Path(base)/'config/kog/tui-session.json'
    until=time.time()+5
    while time.time()<until:
        if session_path.exists() and len(json.loads(session_path.read_text())['tracks'])>=4:
            break
        drain(.1)
    else:
        raise AssertionError('terminal playlist was not saved during the session')
    assert '━' in screen.display[35],screen.display[35]
    initial_header=screen.display[1]
    click(60,2)
    send(b'\x1b[C')  # playlist Right scrolls columns
    assert screen.display[1]!=initial_header,screen.display[1]
    send(b'\x1b[D')  # playlist Left scrolls back
    assert screen.display[1]==initial_header,screen.display[1]
    send('\x1b[<67;81;10M')  # native horizontal wheel right
    assert screen.display[1]!=initial_header,screen.display[1]
    shifted_header=screen.display[1]
    send('\x1b[<69;81;10M')  # Shift + vertical wheel down
    assert screen.display[1]!=shifted_header,screen.display[1]
    click(110,35)
    assert 'Album' in screen.display[1],screen.display[1]
    click(42,35)
    assert screen.display[1]==initial_header,screen.display[1]
    send('\x1b[<0;44;36M\x1b[<32;111;36M\x1b[<0;111;36m')
    assert screen.display[1]!=initial_header,screen.display[1]
    click(42,35)
    assert screen.display[1]==initial_header,screen.display[1]
    click(60,20,2)
    assert '╭─ Playlist' in '\n'.join(screen.display),screen.display[19:23]
    click(62,22,2)
    assert '╭─ Playlist' in '\n'.join(screen.display),screen.display[19:23]
    click(62,21)
    assert 'Add file or folder' in '\n'.join(screen.display),screen.display[17:23]
    send(b'\x1b',.3)
    click(50,0);send('a.wv');send(b'\x1b[D');send('a')
    assert 'a.wav' in screen.display[0] and 'a.wav' not in screen.display[-1]
    assert 'a' in screen.display[2][41:] and not screen.display[3][41:].strip(), screen.display[:10]
    send(b'\x1b',.3)
    click(10,3);send('b.wav',.7)
    assert 'b.wav' in '\n'.join(screen.display)
    wait_for('matches for b.wav')
    send(b'\x1b',.3)
    send('\x1b[<0;41;9M');send('\x1b[<32;51;9M');send('\x1b[<0;51;9m')
    assert screen.display[8][50]=='│', screen.display[8][45:55]
    click(107,36)
    assert '%' in screen.display[36]
    send('\x1b[<0;104;37M');send('\x1b[<32;111;37M');send('\x1b[<0;111;37m')
    assert '64%' in screen.display[36],screen.display[36]
    click(100,36);assert '0%' in screen.display[36]
    click(100,36);assert '64%' in screen.display[36]
    send('+');assert '69%' in screen.display[36]
    click(45,36)
    assert 'Shuffle: albums' in screen.display[-1],screen.display[-1]
    click(5,0)
    assert 'Save Current Playlist' in '\n'.join(screen.display) and 'Alt+S' in '\n'.join(screen.display)
    click(10,6);send('PTY Saved\r',.3)
    wait_for('Saved 4 tracks')
    click(60,2);click(60,4,8)
    assert 'Selected 3 tracks' in screen.display[-1],screen.display[-1]
    send('v');click(60,5)
    assert 'Selected 2 tracks' in screen.display[-1],screen.display[-1]
    send('v');assert 'click the last row' in screen.display[-1]
    send(b'\x1b',.4);assert 'Range selection cancelled' in screen.display[-1]
    assert_saved_count('PTY Saved',4)
    db_files=list(Path(base).rglob('kog.db'))
    assert len(db_files)==1,db_files
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==4
    click(60,2,2)
    assert 'Add to Saved Playlist' in '\n'.join(screen.display)
    click(62,5,2)
    assert 'Add to Saved Playlist' in '\n'.join(screen.display)
    click(62,5);send('PTY Saved\r',.3)
    assert_saved_count('PTY Saved',5)
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==5
    click(60,2);click(5,0);click(10,7);send('PTY Selection\r',.3)
    wait_for('Saved 1 tracks')
    click(10,row('PTY Saved'));click(10,row('PTY Selection'),8)
    assert 'Selected 2 playlists' in screen.display[-1],screen.display[-1]
    click(10,row('PTY Saved'))
    assert_saved_count('PTY Selection',1)
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==6
    click(95,1,2)
    assert 'Show/Hide Artist' in '\n'.join(screen.display)
    menu_item('Columns',3)
    assert 'Artist' not in screen.display[1]
    click(95,1,2);menu_item('Columns',3)
    assert 'Artist' in screen.display[1]
    artist_at=screen.display[1].find('Artist')
    send(f'\x1b[<0;{artist_at+1};2M')
    send(f'\x1b[<32;{artist_at+9};2M')
    send(f'\x1b[<0;{artist_at+9};2m')
    assert screen.display[1].find('Artist')>artist_at,screen.display[1]
    click(95,1,2);menu_item('Columns',5)
    assert 'Columns fitted' in screen.display[-1],screen.display[-1]
    click(95,1,2);send(b'\x1b[B'*6+b'\r')
    assert 'Visible Columns' in '\n'.join(screen.display)
    assert '╭─ Columns' in '\n'.join(screen.display)
    menu_item('Visible Columns',11)
    wait_for('Genre shown')
    click(95,1,2);send(b'\x1b[B'*6+b'\r')
    menu_item('Visible Columns',1)
    wait_for('★ shown')
    star_at=screen.display[1].find('★')
    assert star_at>0,screen.display[1]
    click(star_at,2);wait_for('Starred ')
    click(star_at,2);wait_for('Unstarred ')
    column_layout=list(Path(base).rglob('tui-column-layout'))
    assert len(column_layout)==1,column_layout
    assert any(part.startswith('genre,') and part.endswith(',1') for part in column_layout[0].read_text().split(';'))
    click(60,2)
    send(']'*8)
    assert 'Genre' in screen.display[1],screen.display[1]
    send('['*8)
    assert 'Title' in screen.display[1],screen.display[1]
    click(60,2);click(60,5,4);send(b'\x1b[3~')
    assert 'Removed 4 track(s)' in screen.display[-1],screen.display[-1]
    saved_row=row('PTY Saved');click(10,saved_row);click(10,saved_row)
    assert 'Added 5 tracks' in screen.display[-1],screen.display[-1]
    title_at=screen.display[1].find('Title')
    click(title_at+2,1);click(title_at+2,1)
    assert 'deep' in screen.display[2][50:],screen.display[2]
    send('\x1b[<0;61;3M');send('\x1b[<32;61;5M');send('\x1b[<0;61;5m')
    assert 'Moved track 1 to 3' in screen.display[-1],screen.display[-1]
    send(b'\x1b[1;5A')
    assert 'Moved track 3 to 2' in screen.display[-1],screen.display[-1]
    click(60,2);click(60,4,16);send(b'\x1b[3~')
    assert 'Removed 2 track(s)' in screen.display[-1],screen.display[-1]
    click(5,0);click(10,9)
    assert 'Playlist cleared' in screen.display[-1],screen.display[-1]
    click(74,36)
    assert 'Random Radio: on' in screen.display[-1],screen.display[-1]
    click(55,36)
    wait_for('Playing',30)
    assert '▶' in screen.display[2][50:],screen.display[2]
    click(65,36)
    until=time.time()+30
    while time.time()<until and '2 ' not in screen.display[3][50:80]:drain(.2)
    assert '2 ' in screen.display[3][50:80],screen.display[3]
    click(74,36);click(5,0);click(10,9)
    send('m');send(b'\x1b[B'*7);send(b'\r')
    assert '╭─ Kog' in '\n'.join(screen.display)
    assert '╭─ View' in '\n'.join(screen.display)
    assert screen.display[row('Alt+V')].find('╭─ View') > screen.display[row('Alt+V')].find('Alt+V')
    if menu_snapshot:=os.environ.get('KOG_TUI_MENU_SNAPSHOT_PATH'):
        save_snapshot(menu_snapshot)
    menu_item('Kog',10)
    assert '╭─ Playback' in '\n'.join(screen.display)
    send(b'\x1b',.4)
    assert '╭─ Kog' in '\n'.join(screen.display) and '╭─ Playback' not in '\n'.join(screen.display)
    send('m')
    click(10,row('album'))
    file_row=row('b.wav');click(10,file_row);click(10,file_row);drain(.3)
    assert '▶ b' in screen.display[2][50:],screen.display[2]
    click(10,1);assert '▸ Files' in screen.display[1]
    click(10,1);assert '▾ Files' in screen.display[1]
    lists_row=row('Playlists');click(10,lists_row)
    assert '▸ Playlists' in '\n'.join(screen.display)
    click(10,row('▸ Playlists'));assert '▾ Playlists' in '\n'.join(screen.display)
    tree_row=row('b.wav');click(10,tree_row,2)
    assert '╭─ File' in '\n'.join(screen.display)
    click(12,tree_row+4)
    wait_for('Starred b.wav')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute('SELECT count(*) FROM stars').fetchone()[0]==1
    click(10,tree_row,2);send(b'\x1b[B'*5+b'\r')
    wait_for('Blacklisted 1 item(s)')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM blacklist WHERE kind='song' AND path=?",(os.path.join(music,'album','b.wav'),)).fetchone()[0]==1
    send(b'\t\t\x1b[H')
    assert_saved_count('Favorites',1)
    favorite_row=row('Favorites');click(10,favorite_row);click(10,favorite_row)
    assert 'Added 1 tracks' in screen.display[-1],screen.display[-1]
    send('n');send('PTY Rename\r',.3);wait_for('Created PTY Rename')
    send('r');send(b'\x7f'*10+b'PTY Deleted\r',.3);wait_for('Renamed to PTY Deleted')
    send(b'\x1b[3~');send('yes\r',.3);wait_for('Playlist deleted')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlists WHERE name IN ('PTY Rename','PTY Deleted')").fetchone()[0]==0
    click(10,row('PTY Saved'),2)
    assert '╭─ Saved Playlist' in '\n'.join(screen.display)
    send(b'\x1b[B'*4+b'\r')
    send('\r',.3)
    wait_for('Duplicated playlist as PTY Saved copy')
    assert_saved_count('PTY Saved copy',5)
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries pe JOIN playlists p ON p.id=pe.playlist_id WHERE p.name='PTY Saved copy'").fetchone()[0]==5
    click(10,row('PTY Saved'),2);send(b'\x1b[B'*6+b'\r');send(b'\r',.3)
    wait_for('Exported 5 tracks')
    assert Path(music,'PTY Saved.m3u').is_file()
    click(10,row('PTY Selection'),2);send(b'\x1b[B\r')
    wait_for('Playing')
    click(10,row('PTY Saved'),2);send(b'\x1b[B'*2+b'\r')
    wait_for('Playing ')
    click(60,2,2);send(b'\x1b[B'*6+b'\r')
    wait_for('Located ')
    click(5,2)
    click(5,0);click(10,2);send(os.path.join(music,'album','sub','c.wav')+'\r',.3)
    wait_for('Added c.wav')
    click(5,0);click(10,3);send('https://example.invalid/track.mp3\r',.3)
    # A playing queue can advance while the URL is added and replace the
    # transient status with a decoder error. Check the durable queue row.
    click(50,0);send('track.mp3',.3)
    assert any('☁ track' in line for line in screen.display[2:35]),screen.display[:12]
    send(b'\x1b',.3)
    click(5,0);click(10,13)
    assert '╭─ Preferences' in '\n'.join(screen.display),screen.display[:18]
    menu_item('Preferences',3);send(b'\x7f'*4+'Rock\r'.encode(),.3)
    assert 'Equalizer: On · Rock' in screen.display[-1],screen.display[-1]
    click(5,0);click(10,13);menu_item('Preferences',8);send('3\r')
    wait_for('Equalizer gain -20 to 20 dB')
    send(b'\x7f'*10+b'4.5\r')
    wait_for('Equalizer: On · Custom')
    click(5,0);click(10,11);menu_item('View',3)
    wait_for('+4.5 dB')
    send(b'\x1b',.4)
    click(5,0);click(10,11);menu_item('View',1)
    wait_for('Format:')
    assert 'Sample Rate:' in '\n'.join(screen.display) and 'Bits Per Sample:' in '\n'.join(screen.display)
    send(b'\x1b',.4)
    click(5,0);click(10,11);menu_item('View',2)
    wait_for('No embedded lyrics')
    send(b'\x1b',.4)
    click(5,0);click(10,9)
    click(5,0);click(10,2);send(long_path+'\r',.3)
    wait_for('Added long.wav')
    click(5,0);click(10,2);send(second_path+'\r',.3)
    wait_for('Added second.wav')
    click(60,3);send('Q')
    assert '⏭1' in screen.display[3],screen.display[3]
    click(60,2);send('X')
    assert '■' in screen.display[2],screen.display[2]
    send(b'\r')
    wait_for('0:30')
    assert 'Ⅱ' in screen.display[36],screen.display[36]
    click(80,37)
    wait_for('Ready to play',5)
    assert '⏭1' in screen.display[3],screen.display[3]
    send(b'\r')
    assert 'Ⅱ' in screen.display[36],screen.display[36]
    click(65,36)
    wait_for('Playing second.wav')
    assert '⏭1' not in screen.display[3],screen.display[3]
    click(50,36)
    wait_for('Playing long.wav')
    click(5,0);click(10,11);menu_item('View',6)
    wait_for('Compact Player')
    assert 'Title' not in screen.display[1],screen.display[1]
    if compact_snapshot:=os.environ.get('KOG_TUI_COMPACT_SNAPSHOT_PATH'):
        save_snapshot(compact_snapshot)
    click(59,19)
    assert '▶' in screen.display[36],screen.display[36]
    click(59,19)
    assert 'Ⅱ' in screen.display[36],screen.display[36]
    click(62,17)
    assert any(clock in screen.display[37] for clock in ('0:14','0:15','0:16')),screen.display[37]
    click(59,20)
    assert '84%' in screen.display[36],screen.display[36]
    click(50,0)
    assert 'Title' in screen.display[1],screen.display[1]
    click(5,0);click(10,11);menu_item('View',1)
    wait_for('Position:')
    first_position=re.search(r'Position: (\d+:\d+)', '\n'.join(screen.display)).group(1)
    drain(1.6)
    second_position=re.search(r'Position: (\d+:\d+)', '\n'.join(screen.display)).group(1)
    assert first_position!=second_position,(first_position,second_position)
    send(b'\x1b',.4)
    click(5,0);click(10,11);menu_item('View',4)
    wait_for('Visualizer · Spectrum')
    send(b'\x1b',.4)
    click(55,36)
    assert '▶' in screen.display[36],screen.display[36]
    click(72,37)
    assert any(clock in screen.display[37] for clock in ('0:19','0:20','0:21')),screen.display[37]
    if snapshot_path:=os.environ.get('KOG_TUI_SNAPSHOT_PATH'):
        save_snapshot(snapshot_path)
    click(60,36)
    assert '▶' in screen.display[36],screen.display[36]
    click(5,0);click(10,2);send(os.path.join(music,'album','b.wav')+'\r',.3)
    wait_for('Added b.wav')
    click(60,4);click(60,4)
    wait_for('Ready to play',5)
    click(5,0);click(10,6);send('PTY Prune\r',.3)
    wait_for('Saved 3 tracks')
    assert any('PTY Prune' in line for line in screen.display[30:36]),screen.display[30:40]
    os.remove(second_path)
    click(10,row('PTY Prune'),2)
    assert 'Saved Playlist' in '\n'.join(screen.display),screen.display[27:40]
    send(b'\x1b[B'*7)
    assert 'Remove Missing Files' in '\n'.join(screen.display),screen.display[27:40]
    send(b'\r',.4)
    wait_for('Removed 1 missing file')
    assert_saved_count('PTY Prune',2)
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries pe JOIN playlists p ON p.id=pe.playlist_id WHERE p.name='PTY Prune'").fetchone()[0]==2
    click(60,4);send('Q')
    click(60,2);send('X')
    title_at=screen.display[1].find('Title')
    click(title_at+2,1);click(title_at+2,1)
    assert 'b' in screen.display[row('⏭1')],screen.display[row('⏭1')]
    assert 'long' in screen.display[row('■')],screen.display[row('■')]
    click(60,row('⏭1'));send(b'\x1b[1;5A')
    assert 'b' in screen.display[row('⏭1')],screen.display[row('⏭1')]
    click(60,row('second'));send(b'\x1b[3~')
    assert 'b' in screen.display[row('⏭1')],screen.display[row('⏭1')]
    assert 'long' in screen.display[row('■')],screen.display[row('■')]
    resize(72,24);screen.resize(lines=24,columns=72);os.kill(p.pid,signal.SIGWINCH);drain(.5)
    assert 'Title' in screen.display[1],screen.display[:3]
    assert screen.display[8][42]=='│',screen.display[8][38:46]
    resize(48,20);screen.resize(lines=20,columns=48);os.kill(p.pid,signal.SIGWINCH);drain(.5)
    send('/c.wav',.5)
    assert 'c.wav' in screen.display[0],screen.display[0]
    assert 'c.wav' in screen.display[-1] and ('matches' in screen.display[-1] or 'Searching' in screen.display[-1]),screen.display[-1]
    send(b'\x1b',.5)
    wait_for('Search files')
    assert 'Search files' in screen.display[0],screen.display[:5]
    click(10,2);click(10,2);wait_for('Added 4 tracks')
    click(10,2,2);assert '╭─ File' in '\n'.join(screen.display)
    send(b'\x1b',.4)
    click(38,18)
    assert '%' in screen.display[18]
    click(5,0)
    assert 'Save Current' in '\n'.join(screen.display) and 'Alt+S' in '\n'.join(screen.display)
    menu_item('Kog',9)
    assert '╭─ Kog' in '\n'.join(screen.display) and '╭─ View' in '\n'.join(screen.display)
    resize(72,24);screen.resize(lines=24,columns=72);os.kill(p.pid,signal.SIGWINCH);drain(.5)
    assert '╭─ Kog' in '\n'.join(screen.display) and '╭─ View' in '\n'.join(screen.display)
    resize(48,20);screen.resize(lines=20,columns=48);os.kill(p.pid,signal.SIGWINCH);drain(.5)
    send(b'\x1b',.4)
    assert '╭─ Kog' in '\n'.join(screen.display) and '╭─ View' not in '\n'.join(screen.display)
    send(b'\x1b',.4)
    scroll_dir=os.path.join(music,'zzscroll')
    os.makedirs(scroll_dir)
    for number in range(50):
        with wave.open(os.path.join(scroll_dir,f'track{number:02}.wav'),'wb') as w:
            w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8)
    send('o');send(b'\x0f',.4)
    wait_for('zzscroll')
    click(10,row('zzscroll'))
    send(b'\x1b[F')
    assert 'track49' in '\n'.join(screen.display),screen.display
    send(b'\x1b[H')
    for _ in range(22):send('\x1b[<65;10;6M',.01)
    assert 'track49' in '\n'.join(screen.display),screen.display
    send(b'\x1b[H')
    click(10,row('zzscroll'));click(10,row('zzscroll'))
    wait_for('Added 50 tracks',10)
    send('c');send(b'\x1b[F')
    assert 'track49' in '\n'.join(screen.display),screen.display
    send(b'\x1b[H')
    for _ in range(22):send('\x1b[<65;40;6M',.01)
    assert 'track49' in '\n'.join(screen.display),screen.display
    resize(120,40);screen.resize(lines=40,columns=120);os.kill(p.pid,signal.SIGWINCH);drain(.5)
    with wave.open(second_path,'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
    click(5,0);click(10,9)
    assert 'Playlist cleared' in screen.display[-1],screen.display[-1]
    for path in (long_path,second_path,third_path):
        click(5,0);click(10,2);send(path+'\r',.3)
        wait_for(f'Added {Path(path).name}',10)
    send('SS')
    assert 'Shuffle: all' in screen.display[-1],screen.display[-1]
    click(60,2);click(60,2)
    wait_for('Playing long.wav')
    shuffled=['long.wav']
    for _ in range(2):
        send('>')
        shuffled.append(screen.display[-1].split('Playing ',1)[-1].strip())
    assert set(shuffled)=={'long.wav','second.wav','third.wav'},shuffled
    send('<')
    assert shuffled[1] in screen.display[-1],(shuffled,screen.display[-1])
    send('S')
    assert 'Shuffle: off' in screen.display[-1],screen.display[-1]
    for mode in ('one','album','all','off'):
        send('R')
        assert f'Repeat: {mode}' in screen.display[-1],screen.display[-1]
    artist_at=screen.display[1].find('Artist')
    assert artist_at>0,screen.display[1]
    click(artist_at+2,1,2);send(b'\x1b[B'*7+b'\r')
    assert 'Column moved' in screen.display[-1],screen.display[-1]
    assert screen.display[1].find('Artist')<screen.display[1].find('Title'),screen.display[1]
    send(b'\t\x1b[H')
    if '▸ Playlists' in '\n'.join(screen.display):click(10,row('▸ Playlists'))
    saved_row=row('PTY Saved');selection_row=row('PTY Selection')
    click(10,saved_row);click(10,selection_row,16)
    click(10,saved_row,2);send(b'\r')
    assert 'Added 1 tracks' in screen.display[-1],screen.display[-1]
    assert ' 9 ' in screen.display[10][50:],screen.display[2:12]
    click(10,saved_row);click(10,selection_row,4)
    click(10,saved_row,2);send(b'\x1b[B'*5+b'\r')
    send('yes\r',.3)
    wait_for('Deleted 2 playlists')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlists WHERE name IN ('PTY Saved','PTY Selection')").fetchone()[0]==0
    click(60,2,2);send(b'\x1b[B'*11+b'\r')
    wait_for('Blacklisted 1 item(s)')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM blacklist WHERE kind='song' AND path=?",(long_path,)).fetchone()[0]==1
    click(60,2,2);send(b'\x1b[B'*12+b'\r')
    wait_for('Blacklisted 1 item(s)')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM blacklist WHERE kind='folder' AND path=?",(base,)).fetchone()[0]==1
        remove_id=db.execute("SELECT id FROM blacklist WHERE kind='folder' AND path=?",(base,)).fetchone()[0]
    click(5,0);click(10,13);send(b'\x1b[B'*9+b'\r')
    wait_for('Blacklist · use Preferences')
    send(b'\x1b',.4)
    click(5,0);click(10,13);send(b'\x1b[B'*10+b'\r')
    send(str(remove_id)+'\r',.3)
    wait_for('Removed from blacklist')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute('SELECT count(*) FROM blacklist WHERE id=?',(remove_id,)).fetchone()[0]==0
    trash_path=os.path.join(music,'album','sub','c.wav')
    click(10,row('c.wav'),2);send(b'\x1b[B'*7+b'\r')
    assert 'move selected item to trash' in '\n'.join(screen.display).lower()
    send('yes\r',.3)
    wait_for('Moved '+trash_path+' to trash',10)
    assert not os.path.exists(trash_path)
    assert not any('c.wav' in line[:50] for line in screen.display[:36]),screen.display[:15]
    radio_root=os.path.join(base,'RadioOnly')
    os.makedirs(radio_root)
    for name in ('ban.wav','kept.wav'):
        with wave.open(os.path.join(radio_root,name),'wb') as w:
            w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
    choose_music_folder(radio_root)
    wait_for('ban.wav')
    click(10,row('ban.wav'),2);send(b'\x1b[B'*5+b'\r')
    wait_for('Blacklisted 1 item(s)')
    click(5,0);click(10,9)
    assert 'Playlist cleared' in screen.display[-1],screen.display[-1]
    click(74,36);click(55,36)
    wait_for('Playing kept.wav',30)
    assert 'Playing ban.wav' not in screen.display[-1],screen.display[-1]
    click(5,0);click(10,9)
    assert 'Playlist cleared' in screen.display[-1],screen.display[-1]
    for path in (long_path,third_path):
        click(5,0);click(10,2);send(path+'\r',.3)
    click(60,2);click(60,3,16)
    send('e')
    wait_for('Editing tags for 2 file(s)')
    assert 'Save Changes' in '\n'.join(screen.display)
    send(b'\x1b[B'*3+b'\r')
    assert 'Album' in '\n'.join(screen.display)
    send('PTY Album\r',.4)
    assert 'Save Changes' in '\n'.join(screen.display)
    send(b'\x1b[B'*15+b'\r')
    wait_for('Updated tags for 2 file(s)',10)
    for path in (long_path,third_path):
        tags=WAVE(path).tags
        assert tags.getall('TALB')[0].text==['PTY Album'],(path,tags)
    choose_music_folder(base)
    wait_for('third.wav')
    click(5,0);click(10,9)
    click(10,row('long.wav'));click(10,row('third.wav'),4)
    send('a')
    wait_for('Added 3 track(s)')
    assert all(name.removesuffix('.wav') in '\n'.join(line[52:] for line in screen.display[2:15]) for name in ('long.wav','second.wav','third.wav'))
    click(5,0);click(10,9)
    choose_music_folder(base)
    wait_for('third.wav')
    click(10,row('long.wav'));click(10,row('third.wav'),16)
    send('a')
    wait_for('Added 2 track(s)')
    assert 'second' not in '\n'.join(line[52:] for line in screen.display[2:15]),screen.display[2:15]
    click(10,row('long.wav'),2);send(b'\x1b[B'*5+b'\r')
    wait_for('Blacklisted 1 item(s)')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM blacklist WHERE kind='song' AND path=?",(third_path,)).fetchone()[0]==1
    click(10,row('long.wav'));send(b'\x1b[1;2B'*2);send('a')
    wait_for('Added 3 track(s)')
    send(b'\x01')
    assert 'Selected ' in screen.display[-1] and 'tree items' in screen.display[-1],screen.display[-1]
    click(5,0);click(10,9)
    click(10,row('RadioOnly'))
    click(10,row('RadioOnly'),2);send(b'\x1b[B\r')
    wait_for('Playing ban.wav',10)
    batch_paths=[os.path.join(base,name) for name in ('batch1.wav','batch2.wav')]
    for path in batch_paths:
        with wave.open(path,'wb') as w:
            w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8000)
    choose_music_folder(base)
    wait_for('batch2.wav')
    click(10,row('batch1.wav'));click(10,row('batch2.wav'),16)
    send(b'\x1b[3~')
    send('yes\r',.4)
    wait_for('Moved '+batch_paths[1]+' to trash',10)
    assert not any(os.path.exists(path) for path in batch_paths),batch_paths
    click(10,row('RadioOnly'),2);send(b'\x1b[B'*8+b'\r')
    wait_for('kept.wav')
    assert 'long.wav' not in '\n'.join(line[:50] for line in screen.display[:20])
    click(10,row('kept.wav'),2);send(b'\x1b[B'*9+b'\r')
    assert any('▸ ▱ RadioOnly' in line[:50] for line in screen.display[:20]),screen.display[:20]
    artwork_path=os.path.join(base,'cover.png')
    def png_chunk(kind,payload):
        return struct.pack('>I',len(payload))+kind+payload+struct.pack('>I',zlib.crc32(kind+payload))
    artwork_pixels=b''.join(
        b'\0'+bytes(channel for x in range(24) for channel in (40+x*7,70+y*5,180-x*4))
        for y in range(24)
    )
    with open(artwork_path,'wb') as artwork:
        artwork.write(b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',24,24,8,2,0,0,0))+png_chunk(b'IDAT',zlib.compress(artwork_pixels))+png_chunk(b'IEND',b''))
    art_track=os.path.join(base,'art.wav')
    with wave.open(art_track,'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*480000)
    click(5,0);click(10,9)
    click(5,0);click(10,2);send(art_track+'\r',.3)
    click(60,2);click(60,2)
    wait_for('Playing art.wav')
    send('e');wait_for('Editing tags for 1 file(s)')
    send(b'\x1b[B'*13+b'\r');send(artwork_path+'\r',.3)
    send(b'\x1b[B'*15+b'\r');wait_for('Updated tags for 1 file(s)',10)
    assert WAVE(art_track).tags.getall('APIC')
    wait_for('▀▀▀▀',10)
    assert '▀▀▀▀' in screen.display[36][1:8],screen.display[36][:12]
    assert 'Ⅱ' in screen.display[36],screen.display[36]
    click(3,36)
    wait_for('Click or Esc to close')
    assert any(line.count('▀')>=24 for line in screen.display),screen.display
    if artwork_snapshot:=os.environ.get('KOG_TUI_ARTWORK_SNAPSHOT_PATH'):
        save_snapshot(artwork_snapshot)
    click(60,20)
    assert 'Click or Esc to close' not in '\n'.join(screen.display)
    send('e');wait_for('Editing tags for 1 file(s)')
    send(b'\x1b[B'*14+b'\r');send(b'\x1b[B'*15+b'\r')
    wait_for('Updated tags for 1 file(s)',10)
    removed_art=WAVE(art_track).tags
    assert removed_art is None or not removed_art.getall('APIC')
    remote_address=f'http://127.0.0.1:{remote_server.server_port}'
    click(5,0);click(10,15);menu_item('Remote Server',0);send(remote_address+'\r',.4)
    wait_for('Authentication failed',10)
    click(5,0);click(10,15);menu_item('Remote Server',1);send('PTY remote token\r',.4)
    wait_for('Connected to '+remote_address,10)
    assert any('▱ album' in line[:50] for line in screen.display[:20]),screen.display[:20]
    click(10,row('▱ album'));wait_for('remote.wav',10)
    click(10,row('remote.wav'));click(10,row('remote.wav'))
    wait_for('Playing remote.wav',10)
    assert any(path=='/api/stream' and query.get('token')==['PTY remote token'] for path,query,_ in remote_requests),remote_requests
    click(10,row('pack.zip'));wait_for('inner',10)
    click(10,row('inner'));wait_for('song.wav',10)
    click(10,row('song.wav'));click(10,row('song.wav'))
    wait_for('Playing song.wav',10)
    assert any(path=='/api/stream' and query.get('kind')==['archive'] and query.get('entry')==['inner/song.wav'] for path,query,_ in remote_requests),remote_requests
    click(10,3);send('remote',.8)
    wait_for('remote matches for remote',10)
    send(b'\x1b',.4);wait_for('Connected to '+remote_address,10)
    click(5,0);click(10,15);menu_item('Remote Server',7)
    wait_for('Added 2 tracks from folder',10)
    click(5,0);click(10,15);menu_item('Remote Server',8)
    wait_for('Showing local library',10)
    remote_config=list(Path(base).rglob('tui-remote-server.json'))
    assert len(remote_config)==1 and os.stat(remote_config[0]).st_mode & 0o777==0o600
    assert any(path=='/api/library' and auth=='Bearer PTY remote token' for path,_,auth in remote_requests)
    assert any(path=='/api/expand' and auth=='Bearer PTY remote token' for path,_,auth in remote_requests)
    click(5,0);click(10,13);menu_item('Preferences',11)
    wait_for('Opening files: clearAndPlay')
    click(10,row('art.wav'));click(10,row('art.wav'))
    wait_for('Playing art.wav')
    assert 'remote.wav' not in '\n'.join(line[52:] for line in screen.display[2:20])
    click(5,0);click(10,13);menu_item('Preferences',11)
    wait_for('Opening files: enqueue')
    click(10,row('second.wav'));click(10,row('second.wav'))
    wait_for('Added 1 track(s)')
    assert 'Ⅱ' in screen.display[36] and 'art' in '\n'.join(line[52:] for line in screen.display[2:20]),screen.display[36]
    diagnostics=list(Path(base).rglob('tui-diagnostics.log'))
    assert len(diagnostics)==1 and os.stat(diagnostics[0]).st_mode & 0o777==0o600
    if os.path.isdir(f'/proc/{p.pid}/fd'):
        assert os.path.samefile(f'/proc/{p.pid}/fd/1',diagnostics[0])
        assert os.path.samefile(f'/proc/{p.pid}/fd/2',diagnostics[0])
    click(5,0);click(10,13);menu_item('Preferences',12)
    assert '╭─ MIDI Synthesis' in '\n'.join(screen.display),screen.display[:12]
    assert '╭─ Kog' in '\n'.join(screen.display) and '╭─ Preferences' in '\n'.join(screen.display)
    menu_item('MIDI Synthesis',0)
    wait_for('MIDI backend: opl3windows')
    assert next(Path(base).rglob('midi-engine')).read_text()=='opl3windows'
    click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',4)
    wait_for('MT-32 GM program mapping: off')
    assert next(Path(base).rglob('mt32-gm-program-mapping')).read_text()=='false'
    click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',1)
    send(b'\x01'+os.path.join(base,'missing.sf2').encode()+b'\r')
    wait_for('Opening SoundFont:')
    click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',1)
    send(b'\x01\r')
    wait_for('MIDI SoundFont updated')
    assert next(Path(base).rglob('soundfont-path')).read_text()==''
    for y in (4,5):
        click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',y-2)
        send(b'\x01\r')
        wait_for('ROM directory updated')
    click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',5)
    wait_for('MIDI backend: opl3windows')
    assert 'MT-32 GM program mapping: off' in '\n'.join(screen.display)
    send(b'\x1b',.3)
    subsong_dir=os.path.join(base,'SubsongFixture')
    os.makedirs(subsong_dir)
    header=bytearray(128)
    header[:5]=b'NESM\x1a';header[5]=1;header[6]=3;header[7]=1
    header[8:10]=(0x8000).to_bytes(2,'little')
    header[10:12]=(0x8000).to_bytes(2,'little')
    header[12:14]=(0x8001).to_bytes(2,'little')
    Path(subsong_dir,'game.nsf').write_bytes(header+b'\x60\x60')
    choose_music_folder(base)
    wait_for('SubsongFixture')
    click(10,row('SubsongFixture'));wait_for('game.nsf')
    click(10,row('game.nsf'));send('a')
    wait_for('Added 3 track(s)')
    assert all(f'game [{number}]' in '\n'.join(line[52:] for line in screen.display[2:20]) for number in (1,2,3)),screen.display[2:14]
    click(5,0);click(10,9)
    click(10,row('SubsongFixture'));click(10,row('SubsongFixture'))
    wait_for('Added 3 tracks from folder',10)
    click(5,0);click(10,11);menu_item('View',5)
    wait_for('Supported Formats')
    assert '.m3u' in '\n'.join(screen.display),screen.display[:20]
    send(b'\x1b',.3)
    click(5,0);click(10,13);menu_item('Preferences',13)
    wait_for('Read CUE sheets in folders: off')
    click(5,0);click(10,13);menu_item('Preferences',14)
    wait_for('Read M3U/PLS in folders: off')
    assert next(Path(base).rglob('read-cue-sheets-in-folders')).read_text()=='false'
    assert next(Path(base).rglob('read-playlists-in-folders')).read_text()=='false'
    click(5,0);click(10,13);menu_item('Preferences',16)
    wait_for('Automatic cover downloads: off')
    assert next(Path(base).rglob('download-cover-art')).read_text()=='false'
    fake_rom_archive=os.path.join(base,'incomplete-roms.zip')
    with zipfile.ZipFile(fake_rom_archive,'w') as archive:
        archive.writestr('nested/control.rom',b'not a real ROM')
        archive.writestr('nested/pcm.rom',b'not a real ROM')
    for y in (8,9):
        click(5,0);click(10,13);menu_item('Preferences',12);menu_item('MIDI Synthesis',y-2)
        send(fake_rom_archive+'\r',.3)
        wait_for('Incomplete ROM set',10)
    assert not list(Path(base).rglob('control.rom'))
    click(5,0);click(10,9)
    recover_paths=[os.path.join(base,f'recover{number}.wav') for number in (1,2,3)]
    for number,path in enumerate(recover_paths,1):
        with wave.open(path,'wb') as w:
            w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000)
            w.writeframes(b'\0\0'*8000*(3 if number==1 else 5))
        click(5,0);click(10,2);send(path+'\r',.3)
        wait_for(f'Added recover{number}.wav')
    click(60,2);click(60,2)
    wait_for('Playing recover1.wav')
    os.unlink(recover_paths[1])
    wait_for('Playing recover3.wav',10)
    assert '▶' in screen.display[4][50:],screen.display[4]
    print('search, local/remote tree/archive navigation/trash/blacklist/group selection, divider/column drag/visibility/reorder, volume, radio/blacklist/queue/stop-after, playback/seek/completion/order/error recovery, saved-list CRUD/export/prune/multi-selection, tag fields/artwork/playback resume, selection/reorder/sort, menus/dialogs, equalizer/visualizer, narrow wheel/keyboard navigation, resize: PASS')
    send(b'\x1b',.3);send('q')
    p.wait(timeout=5)
    session=json.loads(session_path.read_text())
    saved_paths=[track['path'] for track in session['tracks']]
    assert all(path in saved_paths for path in recover_paths),saved_paths
    assert session['selectedIndex'] < len(saved_paths),session
    os.close(master)
    master,slave=pty.openpty()
    resize(120,40)
    p=subprocess.Popen(args,stdin=slave,stdout=slave,stderr=slave,env=env,close_fds=True)
    os.close(slave)
    screen=pyte.Screen(120,40);stream=pyte.Stream(screen)
    wait_for('recover3.wav')
    send('q');p.wait(timeout=5)
    restored=json.loads(session_path.read_text())
    assert [track['path'] for track in restored['tracks']]==saved_paths,restored
    print('terminal playlist survives quit and relaunch: PASS')
finally:
    if p.poll() is None:p.terminate();p.wait(timeout=5)
    os.close(master)
    remote_server.shutdown();remote_server.server_close()
    remote_root.cleanup()
    root.cleanup()

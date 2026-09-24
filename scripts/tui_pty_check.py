import fcntl, os, pty, select, signal, sqlite3, struct, subprocess, tempfile, time, wave, zipfile
from pathlib import Path
import pyte

root=tempfile.TemporaryDirectory(prefix='kog-tui-pty-')
base=root.name
music=os.path.join(base,'Music')
os.makedirs(os.path.join(music,'album','sub'))
for name in ('album/a.wav','album/b.wav','album/sub/c.wav'):
    with wave.open(os.path.join(music,name),'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8000)
with zipfile.ZipFile(os.path.join(music,'album','pack.zip'),'w') as z:
    z.write(os.path.join(music,'album','a.wav'),'inner/deep.wav')
long_path=os.path.join(base,'long.wav')
with wave.open(long_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
second_path=os.path.join(base,'second.wav')
with wave.open(second_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
third_path=os.path.join(base,'third.wav')
with wave.open(third_path,'wb') as w:
    w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*80000)
env=os.environ.copy()
for key,sub in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:
    env[key]=os.path.join(base,sub)
env['TERM']='xterm-256color'
master,slave=pty.openpty()
def resize(cols,rows):
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH',rows,cols,0,0))
import termios
resize(120,40)
p=subprocess.Popen([str(Path(__file__).resolve().parents[1] / 'target/debug/kog'),'--tui'],stdin=slave,stdout=slave,stderr=slave,env=env,close_fds=True)
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
def row(text):
    for i,line in enumerate(screen.display):
        if text in line:return i
    raise AssertionError((text,'not on screen',screen.display[:15]))
def wait_for(text, timeout=5):
    end=time.time()+timeout
    while time.time()<end:
        if text in '\n'.join(screen.display): return
        drain(.1)
    raise AssertionError((text,'not on screen',screen.display[:15]))
try:
    wait_for('Search playlist')
    click(1,0)
    assert 'Music folder' in '\n'.join(screen.display)
    assert screen.cursor.y not in (0,39),screen.cursor.y
    send(music+'\r',.4)
    wait_for('album')
    y=row('album');click(10,y)
    assert 'a.wav' in '\n'.join(screen.display)
    zrow=row('pack.zip');click(10,zrow)
    assert 'inner' in '\n'.join(screen.display)
    click(10,row('inner'))
    assert 'deep.wav' in '\n'.join(screen.display)
    click(10,y);click(10,y);drain(.5)
    assert 'Added 4 tracks' in screen.display[-1], screen.display[-1]
    click(50,0);send('a.wv');send(b'\x1b[D');send('a')
    assert 'a.wav' in screen.display[0] and 'a.wav' not in screen.display[-1]
    assert 'a' in screen.display[2][41:] and not screen.display[3][41:].strip(), screen.display[:10]
    send(b'\x1b',.3)
    click(10,3);send('b.wav',.7)
    assert 'b.wav' in '\n'.join(screen.display)
    assert 'matches for b.wav' in screen.display[-1],screen.display[-1]
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
    assert 'Save Current Playlist' in '\n'.join(screen.display)
    click(10,6);send('PTY Saved\r',.3)
    wait_for('Saved 4 tracks')
    db_files=list(Path(base).rglob('kog.db'))
    assert len(db_files)==1,db_files
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==4
    click(60,2,2)
    assert 'Add to Saved Playlist' in '\n'.join(screen.display)
    click(62,5);send('PTY Saved\r',.3)
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==5
    click(60,2);click(5,0);click(10,7);send('PTY Selection\r',.3)
    wait_for('Saved 1 tracks')
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries").fetchone()[0]==6
    click(95,1,2)
    assert 'Show/Hide Artist' in '\n'.join(screen.display)
    click(90,5)
    assert 'Artist' not in screen.display[1]
    click(95,1,2);click(90,5)
    assert 'Artist' in screen.display[1]
    artist_at=screen.display[1].find('Artist')
    send(f'\x1b[<0;{artist_at+1};2M')
    send(f'\x1b[<32;{artist_at+9};2M')
    send(f'\x1b[<0;{artist_at+9};2m')
    assert screen.display[1].find('Artist')>artist_at,screen.display[1]
    click(95,1,2);click(90,7)
    assert 'Columns fitted' in screen.display[-1],screen.display[-1]
    click(95,1,2);send(b'\x1b[B'*6+b'\r')
    assert 'Visible Columns' in '\n'.join(screen.display)
    send(b'\x1b[B'*11+b'\r')
    assert 'Genre shown' in screen.display[-1],screen.display[-1]
    click(95,1,2);send(b'\x1b[B'*6+b'\r')
    send(b'\x1b[B\r')
    assert '★ shown' in screen.display[-1],screen.display[-1]
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
    assert '╭─ View' in screen.display[1],screen.display[1]
    send(b'\x1b',.4);send('m')
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
    send(b'\t\t\x1b[H')
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
    with sqlite3.connect(db_files[0]) as db:
        assert db.execute("SELECT count(*) FROM playlist_entries pe JOIN playlists p ON p.id=pe.playlist_id WHERE p.name='PTY Saved copy'").fetchone()[0]==5
    click(10,row('PTY Saved'),2);send(b'\x1b[B'*6+b'\r');send(b'\r',.3)
    wait_for('Exported 5 tracks')
    assert Path(music,'PTY Saved.m3u').is_file()
    click(10,row('PTY Selection'),2);send(b'\x1b[B\r')
    wait_for('Playing')
    click(10,row('PTY Saved'),2);send(b'\x1b[B'*2+b'\r')
    assert 'Playing ' in screen.display[-1],screen.display[-1]
    click(60,2,2);send(b'\x1b[B'*6+b'\r')
    wait_for('Located ')
    click(5,2)
    click(5,0);click(10,2);send(os.path.join(music,'album','sub','c.wav')+'\r',.3)
    wait_for('Added c.wav')
    click(5,0);click(10,3);send('https://example.invalid/track.mp3\r',.3)
    wait_for('Added track.mp3')
    click(5,0);click(10,13)
    assert '╭─ Preferences' in screen.display[1]
    click(10,5);send(b'\x7f'*4+'Rock\r'.encode(),.3)
    assert 'Equalizer: On · Rock' in screen.display[-1],screen.display[-1]
    click(5,0);click(10,13);click(10,10);send('3\r')
    wait_for('Equalizer gain -20 to 20 dB')
    send(b'\x7f'*10+b'4.5\r')
    wait_for('Equalizer: On · Custom')
    click(5,0);click(10,11);click(10,5)
    wait_for('+4.5 dB')
    send(b'\x1b',.4)
    click(5,0);click(10,11);click(10,3)
    wait_for('Codec:')
    send(b'\x1b',.4)
    click(5,0);click(10,11);click(10,4)
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
    wait_for('0:10')
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
    click(5,0);click(10,11);click(10,6)
    wait_for('Visualizer · Spectrum')
    send(b'\x1b',.4)
    click(55,36)
    assert '▶' in screen.display[36],screen.display[36]
    click(72,37)
    assert '0:06' in screen.display[37] or '0:07' in screen.display[37],screen.display[37]
    if snapshot_path:=os.environ.get('KOG_TUI_SNAPSHOT_PATH'):
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
        image.save(snapshot_path)
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
    assert 'Save Current Playlist' in '\n'.join(screen.display)
    send(b'\x1b',.4)
    scroll_dir=os.path.join(music,'zzscroll')
    os.makedirs(scroll_dir)
    for number in range(50):
        with wave.open(os.path.join(scroll_dir,f'track{number:02}.wav'),'wb') as w:
            w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8)
    send('o');send('\r',.4)
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
    print('search, tree/archive navigation, divider/column drag/visibility/reorder, volume, radio/queue/stop-after, playback/seek/completion/order, saved-list CRUD/export/prune/multi-selection, selection/reorder/sort, menus/dialogs, equalizer/visualizer, narrow wheel/keyboard navigation, resize: PASS')
    send(b'\x1b',.3);send('q')
    p.wait(timeout=5)
finally:
    if p.poll() is None:p.terminate();p.wait(timeout=5)
    os.close(master)
    root.cleanup()

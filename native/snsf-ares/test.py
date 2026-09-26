#!/usr/bin/env python3
"""Native SNSF checks using Kog's original generated SNES/APU program."""
from pathlib import Path
import re, struct, zlib, subprocess, tempfile, sys
exe = Path(sys.argv[1]).resolve()
source = (Path(__file__).resolve().parents[2] / 'crates/kog-audio/src/psf.rs').read_text().split('pub fn test_snsf_rom()')[1]
programs = []
for name in ['CPU_PROGRAM', 'SPC_PROGRAM']:
    body = source.split('const ' + name)[1].split('&[')[2].split('];')[0]
    programs.append(bytes(int(n,16) for n in re.findall('0x[0-9a-f]+',body)))
rom = bytearray(32768)
rom[:sum(map(len,programs))] = b''.join(programs)
rom[0x7fc0:0x7fd5] = b'KOG SYNTHETIC SNSF   '
rom[0x7fd5:0x7fdc] = bytes([0x20,0,5,0,1,0x33,0])
rom[0x7fdc:0x7fe0] = bytes([255,255,0,0])
rom[0x7ffc:0x7ffe] = bytes([0,128])
def container(data=b'', offset=0, reserved=b'', tags=b''):
    compressed = zlib.compress(struct.pack('<II',offset,len(data))+data) if data else b''
    return b'PSF\x23'+struct.pack('<III',len(reserved),len(compressed),zlib.crc32(compressed))+reserved+compressed+b'[TAG]'+tags
def run(path, start=0, ok=True):
    p=subprocess.run([str(exe),str(path),str(start),'1000','0'],capture_output=True,timeout=15)
    assert (p.returncode == 0) == ok, (path.name,p.returncode,p.stderr)
    return p.stdout
def pcm(data):
    h=struct.unpack('<8sIIIIQQ5I',data[:60]); assert h[:5] == (b'KOGPSF1\0',1,0x23,32000,2)
    return data[60+sum(h[-5:]):]
with tempfile.TemporaryDirectory(prefix='kog-snsf-test-') as directory:
    d=Path(directory)
    root=d/'original.snsf';root.write_bytes(container(rom,tags=b'title=Original\n'))
    baseline=run(root);audio=pcm(baseline)
    assert len(audio)==128000 and any(audio)
    assert pcm(run(root,16000))==audio[64000:]
    assert pcm(run(root,64000))==b''
    inspector=exe.parent/'kog-snsf-loader-test'
    if inspector.exists():
        base=d/'relative.snsflib'
        base.write_bytes(container(rom[256:],256,reserved=struct.pack('<III',0,6,5)+b'\x11\xbb',tags=b'title=Library\n_sramfill=0xff\n'))
        overlay=d/'relative.minisnsf'
        overlay.write_bytes(container(b'B',3,reserved=struct.pack('<III',0,5,5)+b'\xaa',tags=b'_lib=relative.snsflib\ntitle=Root\n_sramfill=0x42\n'))
        subprocess.run([str(inspector),str(overlay)],check=True)
    # First ROM offset is the base for later mini and _libN overlays.
    padded=b'\0'*256+rom
    base=d/'base.snsflib';base.write_bytes(container(rom[256:],256,tags=b'title=Library\n'))
    mini=d/'mini.minisnsf';mini.write_bytes(container(rom[:256],0xffffff00,tags=b'_lib=base.snsflib\n'))
    # Deliberate wrap is invalid; all address sums use checked arithmetic.
    run(mini,ok=False)
    base.write_bytes(container(rom,tags=b'title=Library\n'))
    mini.write_bytes(container(rom[:1],tags=b'_lib=base.snsflib\ntitle=Mini\n'))
    assert pcm(run(mini))==audio and b'Mini' in run(mini)[:64]
    patch=d/'patch.snsflib';patch.write_bytes(container(rom[:1]))
    mini.write_bytes(container(tags=b'_lib=base.snsflib\n_lib2=patch.snsflib\n'))
    assert pcm(run(mini))==audio
    mini.write_bytes(container(tags=b'_lib=mini.minisnsf\n'));run(mini,ok=False)
    mini.write_bytes(container(tags=b'_lib=missing.snsflib\n'));run(mini,ok=False)
    # Dependencies must stay inside the canonical root file directory.
    nested=d/'nested';nested.mkdir()
    outside=d/'outside.snsflib';outside.write_bytes(container(rom))
    traversal=nested/'traversal.minisnsf'
    traversal.write_bytes(container(tags=b'_lib=../outside.snsflib\n'))
    run(traversal,ok=False)
    sibling=d/'nested-extra';sibling.mkdir()
    (sibling/'outside.snsflib').write_bytes(container(rom))
    traversal.write_bytes(container(tags=b'_lib=../nested-extra/outside.snsflib\n'))
    run(traversal,ok=False)
    linked=nested/'linked.snsflib';linked.symlink_to(outside)
    traversal.write_bytes(container(tags=b'_lib=linked.snsflib\n'))
    run(traversal,ok=False)
    (nested/'linked-directory').symlink_to(sibling,target_is_directory=True)
    traversal.write_bytes(container(tags=b'_lib=linked-directory/outside.snsflib\n'))
    run(traversal,ok=False)
    # An in-root symlink is valid and retains deterministic PCM.
    inside=nested/'inside.snsflib';inside.write_bytes(container(rom))
    linked.unlink();linked.symlink_to(inside)
    traversal.write_bytes(container(tags=b'_lib=linked.snsflib\n'))
    assert pcm(run(traversal))==audio
    for name,data in {
        'crc':baseline[:16],
        'bad-rom-range':container(b'X',0x1000000),
        'bad-sram-range':container(rom,reserved=struct.pack('<III',0,5,131072)+b'X'),
        'short-reserved':container(rom,reserved=b'X'),
        'save-state':container(rom,reserved=struct.pack('<II',1,0)),
        'bad-time':container(rom,tags=b'length=nan\n'),
        'bad-memory':container(rom,tags=b'_memory=HiROM\n'),
        'bad-video':container(rom,tags=b'_video=unknown\n'),
    }.items():
        path=d/(name+'.snsf');path.write_bytes(data);run(path,ok=False)
    for data in [container(rom,reserved=struct.pack('<II',0xffffffff,0)),
                 container(rom,tags=b'_sramfill=0x42\n'),
                 container(rom,tags=b'_video=PAL\n')]:
        root.write_bytes(data);assert any(pcm(run(root)))
    # Decompression bomb must fail before psflib's dynamically growing allocation.
    bomb=zlib.compress(bytes(16*1024*1024+9));root.write_bytes(b'PSF\x23'+struct.pack('<III',0,len(bomb),zlib.crc32(bomb))+bomb)
    run(root,ok=False)
    root.write_bytes(container(rom))
    concurrency=exe.parent/'kog-snsf-concurrency-test'
    if concurrency.exists(): subprocess.run([str(concurrency),str(root)],check=True,timeout=20)
print('SNSF audio, deterministic seek, dependencies, tags, PAL and malformed bounds passed')

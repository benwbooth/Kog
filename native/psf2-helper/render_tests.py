#!/usr/bin/env python3
"""Original generated PSF/PSF2 fixtures, bounded-loader gates, and no-JIT smoke tests.
Copyright (C) 2026 Kog contributors. SPDX-License-Identifier: GPL-3.0-or-later
"""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib


def words(seq):
    return b''.join(struct.pack('<I', v) for v in seq)


def sound_program(ps2):
    t0, t1 = 8, 9
    code = [0x3c000000 | (t0 << 16) | (0x1f90 if ps2 else 0x1f80)]
    if not ps2:
        code.append(0x34000000 | (t0 << 21) | (t0 << 16) | 0x1c00)
    def write(offset, value):
        code.extend([0x34000000 | (t1 << 16) | value,
                     0xa4000000 | (t0 << 21) | (t1 << 16) | offset])
    if ps2:
        write(0x19a, 0x8000)
        write(0x1a8, 0)
        write(0x1aa, 0x800)
    else:
        write(0x1aa, 0xc000)
        write(0x1a6, 0x200)
    for value in [0x0300, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777, 0x9999, 0x7777]:
        write(0x1ac if ps2 else 0x1a8, value)
    if ps2:
        registers = [(0,0x3fff),(2,0x3fff),(4,0x1000),(6,0xf),(8,0),
                     (0x1c0,0),(0x1c2,0x800),(0x1c4,0),(0x1c6,0x800),(0x1a0,1)]
    else:
        registers = [(0x180,0x3fff),(0x182,0x3fff),(0,0x3fff),(2,0x3fff),
                     (4,0x1000),(6,0x200),(8,0xf),(10,0),(14,0x200),(0x188,1)]
    for offset,value in registers:
        write(offset,value)
    code.extend([0x1000ffff,0])
    return words(code)


def psx_exe():
    code = sound_program(False)
    header = bytearray(2048)
    header[:8] = b'PS-X EXE'
    for offset,value in [(0x10,0x80010000),(0x18,0x80010000),(0x1c,len(code)),(0x30,0x801fff00)]:
        struct.pack_into('<I',header,offset,value)
    header[113:126] = b'North America'
    return bytes(header)+code


def irx():
    code = sound_program(True)
    iopmod = (0x100+len(code)+3)&~3
    section = (iopmod+282+3)&~3
    data = bytearray(section+80)
    data[:7] = b'\x7fELF\x01\x01\x01'
    for offset,value in [(16,0xff80),(18,8),(40,52),(42,32),(44,1),(46,40),(48,2),(iopmod+24,0x100)]:
        struct.pack_into('<H',data,offset,value)
    for offset,value in [(20,1),(28,52),(32,section),(52,1),(56,0x100),(68,len(code)),
                         (72,len(code)),(76,7),(80,16),(iopmod+12,len(code)),
                         (section+44,0x70000080),(section+56,iopmod),(section+60,282),(section+72,4)]:
        struct.pack_into('<I',data,offset,value)
    data[0x100:0x100+len(code)] = code
    data[iopmod+26:iopmod+34] = b'kogpsf2\0'
    return bytes(data)


def psf(version, program=b'', reserved=b'', tags='length=0.5\nfade=0.1\n'):
    compressed = zlib.compress(program) if program else b''
    return b'PSF'+bytes([version])+struct.pack('<III',len(reserved),len(compressed),zlib.crc32(compressed))+reserved+compressed+b'[TAG]'+tags.encode()


def psf2():
    executable=irx()
    compressed=zlib.compress(executable)
    # One PSF2 filesystem entry, one zlib block.
    reserved=struct.pack('<I',1)+b'psf2.irx'.ljust(36,b'\0')+struct.pack('<III',52,len(executable),len(executable))
    reserved+=struct.pack('<I',len(compressed))+compressed
    return psf(2,reserved=reserved)


def parse(output, fmt, start):
    assert output[:8]==b'KOGPSF1\0'
    version, actual, rate, channels = struct.unpack_from('<IIII',output,8)
    total, main = struct.unpack_from('<QQ',output,24)
    lengths=struct.unpack_from('<IIIII',output,40)
    assert (version,actual,rate,channels,total,main)==(1,fmt,44100,2,26460,22050)
    pcm=output[60+sum(lengths):]
    assert len(pcm)==max(0,total-start)*4
    assert any(pcm), 'silent generated audio'
    return pcm


def main():
    ap=argparse.ArgumentParser()
    ap.add_argument('helper',type=Path)
    ap.add_argument('--reference',type=Path)
    ap.add_argument('--output',type=Path)
    ap.add_argument('--exec-guard',type=Path)
    args=ap.parse_args()
    with tempfile.TemporaryDirectory(prefix='kog-psf-render-') as tmp:
        base=args.output or Path(tmp)
        base.mkdir(parents=True,exist_ok=True)
        fixtures={'tone.psf':psf(1,psx_exe()),'tone.psf2':psf2()}
        fixtures['tone.psflib']=psf(1,psx_exe(),tags='length=0.5\nfade=0.1\n')
        overlay=bytearray(2048);overlay[:8]=b'PS-X EXE'
        fixtures['tone.minipsf']=psf(1,bytes(overlay),tags='_lib=tone.psflib\nlength=0.5\nfade=0.1\n')
        fixtures['empty.minipsf']=psf(1,tags='_lib=tone.psflib\nlength=0.5\nfade=0.1\n')
        fixtures['pal.psf']=psf(1,psx_exe(),tags='_refresh=50\nlength=0.5\nfade=0.1\n')
        for name,data in fixtures.items(): (base/name).write_bytes(data)
        def run(name,start=0,helper=args.helper):
            return subprocess.run([str(helper.resolve()),str(base/name),str(start),'500','100'],capture_output=True,timeout=30, env=dict(os.environ, **({'LD_PRELOAD':str(args.exec_guard.resolve())} if args.exec_guard and helper==args.helper else {})))
        outputs={}
        for name in ['tone.psf','tone.minipsf','empty.minipsf','pal.psf','tone.psf2']:
            fmt=2 if name.endswith('psf2') else 1
            for start in [0,3200]:
                p=run(name,start);assert p.returncode==0,(name,p.stderr)
                outputs[name,start]=parse(p.stdout,fmt,start)
                (base/f'{name}.{start}.pcm').write_bytes(outputs[name,start])
                print(name,start,'PCM SHA256',hashlib.sha256(outputs[name,start]).hexdigest())
            assert outputs[name,3200]==outputs[name,0][3200*4:], 'seek prefix mismatch'
        assert outputs['tone.psf',0]==outputs['empty.minipsf',0]
        assert outputs['tone.psf',0]==outputs['tone.minipsf',0], 'miniPSF overlay entry inheritance'
        bad={}
        data=bytearray(fixtures['tone.psf']);data[12]^=1;bad['crc.psf']=bytes(data)
        data=bytearray(psx_exe());struct.pack_into('<I',data,0x18,0x801ffffc);bad['bounds.psf']=psf(1,bytes(data))
        bad['cycle.minipsf']=psf(1,bytes(overlay),tags='_lib=cycle.minipsf\n')
        bad['wrong-version.minipsf']=psf(1,bytes(overlay),tags='_lib=tone.psf2\n')
        for name,data in bad.items():
            (base/name).write_bytes(data)
            p=run(name);assert p.returncode!=0,(name,'malformed file accepted')
            assert not p.stdout,(name,'published header before validation failed')
        if args.reference:
            p=run('tone.psf2',helper=args.reference);assert p.returncode==0,p.stderr
            ref=parse(p.stdout,2,0)
            current=outputs['tone.psf2',0]
            print('PSF2 old-JIT PCM comparison:', 'identical' if ref==current else f'{sum(a!=b for a,b in zip(ref,current))} bytes differ (instruction scheduling changed)')
        if args.exec_guard and args.reference:
            p=subprocess.run([str(args.reference.resolve()),str(base/'tone.psf2'),'0','500','100'],
                             capture_output=True,env=dict(os.environ,LD_PRELOAD=str(args.exec_guard.resolve())))
            assert p.returncode==97, ('guard did not catch old JIT',p.returncode,p.stderr)
            print('No-executable-memory guard: all interpreter renders passed; old JIT rejected')
        print('PSF1, miniPSF, PSF2, seek suffixes and four malformed-input gates passed')
if __name__=='__main__': main()

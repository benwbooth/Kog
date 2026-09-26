#!/usr/bin/env python3
"""Reproduce the pinned ares SNSF subset from an ares checkout (no network)."""
from pathlib import Path
import shutil, sys, re, subprocess, hashlib
src=Path(sys.argv[1]); dst=Path(__file__).resolve().parent.parent/'ares-snsf'
revision = subprocess.check_output(['git', '-C', str(src), 'rev-parse', 'HEAD'], text=True).strip()
assert revision == '4cb8d92b441557cb6bcaf133c4cbc7f6819b1122', 'Unexpected ares revision'
subprocess.run(['git', '-C', str(src), 'diff', '--quiet', 'HEAD'], check=True)
for name in ['LICENSE','ares/ares','ares/sfc','nall/nall','libco']:
    p=dst/name
    if p.is_dir(): shutil.rmtree(p)
    p.parent.mkdir(parents=True,exist_ok=True)
    if (src/name).is_dir(): shutil.copytree(src/name,p)
    else: shutil.copy2(src/name,p)
for cpu in ['wdc65816','spc700','arm7tdmi','gsu','hg51b','upd96050']:
    p=dst/'ares/component/processor'/cpu
    if p.exists(): shutil.rmtree(p)
    shutil.copytree(src/'ares/component/processor'/cpu,p)
# Every mutable namespace-level machine instance is local to its renderer worker.
# References to a slot's cartridge must have the same storage duration as the slot.
for p in (dst/'ares/sfc').rglob('*'):
    if p.suffix not in ['.hpp','.cpp']: continue
    t=p.read_text()
    t=re.sub(r'^extern (\w+&? \w+);$',r'extern thread_local \1;',t,flags=re.M)
    t=re.sub(r'^(?!struct |class |enum )(\w+&? \w+(?: = [\w.]+|\{"[^"\n]+"\})?);$',r'thread_local \1;',t,flags=re.M)
    p.write_text(t)
for name,replacements in {
    'ares/ares/platform.hpp':[('extern Platform* platform;', 'extern thread_local Platform* platform;')],
    'ares/ares/ares.hpp':[('#include <sljit.h>',''),('Threaded = true','Threaded = false'),('extern atomic<bool> _runAhead;','extern thread_local atomic<bool> _runAhead;')],
    'ares/ares/debug/debug.hpp':[('extern Debug _debug;', 'extern thread_local Debug _debug;')],
    'ares/ares/debug/debug.cpp':[('Debug _debug;', 'thread_local Debug _debug;'),('  print(', '  print(stderr, ')],
    'ares/ares/scheduler/scheduler.hpp':[('extern Scheduler scheduler;', 'extern thread_local Scheduler scheduler;')],
    'ares/ares/scheduler/thread.cpp':[('static std::vector<EntryPoint>', 'static thread_local std::vector<EntryPoint>'),('static u8 stack[', 'static thread_local u8 stack[')],
}.items():
    p=dst/name;t=p.read_text()
    for a,b in replacements:
        assert a in t,(name,a)
        t=t.replace(a,b)
    p.write_text(t)
# The renderer doesn't use JIT allocation. Keep it out of the source tree entirely.
for p in (dst/'ares/ares/memory').glob('fixed-allocator.cpp'): p.unlink()
# No UI artwork other than the core's controller crosshairs is required.
r=dst/'ares/ares/resource'; shutil.rmtree(r);r.mkdir()
t='#pragma once\nnamespace Resource::Sprite::SuperFamicom {\n'
for color in ['green','red']:
    data=(src/f'ares/ares/resource/sprite/sfc/crosshair-{color}.png').read_bytes()
    t+=f'inline constexpr unsigned char Crosshair{color.title()}[] = {{'+','.join(map(str,data))+'};\n'
(r/'resource.hpp').write_text(t+'}\n')
shutil.copy2(src/'mia/Database/Super Famicom Boards.bml',dst/'boards.bml')
# Preserve analyzer source as an auditable donor; compile only extracted pure methods.
shutil.copy2(src/'mia/medium/super-famicom.cpp',dst/'super-famicom-analyzer.cpp')
# Generate only the pure header analysis routines, without the desktop loader or
# its firmware database. Their bodies remain byte-for-byte upstream source.
a=(src/'mia/medium/super-famicom.cpp').read_text()
methods=['region','videoRegion','board','label','serial','romSize','programRomSize','dataRomSize','expansionRomSize','firmwareRomSize','ramSize','expansionRamSize','scoreHeader']
head='// Generated from pinned ISC ares/mia/medium/super-famicom.cpp by vendor.py.\nstruct SnsfCartridgeAnalysis {\n  std::vector<u8> rom;\n  u32 headerAddress = 0;\n  auto size() const -> u32 { return rom.size(); }\n'
bodies=''
for method in methods:
    match=re.search(r'auto SuperFamicom::'+method+r'\([^\n]+',a)
    begin=match.start(); end=a.find('\n}\n',begin)+3
    signature=a[begin:a.find('{',begin)].replace('SuperFamicom::','').strip()
    head+='  '+signature+';\n'
    bodies+=a[begin:end].replace('SuperFamicom::','SnsfCartridgeAnalysis::')+'\n'
(dst/'analyzer.hpp').write_text(head+'};\n'+bodies)
# Embed board definitions as data, avoiding runtime installed-resource paths.
(dst/'boards.hpp').write_text('inline constexpr char snsfBoards[] = R"KOGBOARDS('+ (dst/'boards.bml').read_text()+')KOGBOARDS";\n')

for p in (dst/'libco').glob('*.c'):
    t=p.read_text()
    t=t.replace('static void (*co_swap)', 'static thread_local void (*co_swap)').replace('static void (fastcall *co_swap)', 'static thread_local void (fastcall *co_swap)')
    p.write_text(t)

# Machine objects include multi-MiB enhancement-chip RAM. Store these on the
# heap: inline TLS storage would consume most of a worker's native stack budget.
for p in (dst/'ares/sfc').rglob('*'):
    if p.suffix not in ['.cpp', '.hpp']: continue
    t=p.read_text()
    t=re.sub(r'^extern thread_local (ARMDSP|HitachiDSP) (\w+);$',r'extern thread_local \1& \2;',t,flags=re.M)
    def heap(m):
        kind,name,args=m.groups()
        args=args[1:-1] if args else ''
        return f'thread_local std::unique_ptr<{kind}> kog_owner_{name} = std::make_unique<{kind}>({args});\nthread_local {kind}& {name} = *kog_owner_{name};'
    t=re.sub(r'^thread_local (ARMDSP|HitachiDSP) (\w+)(\{"[^"\n]+"\})?;$',heap,t,flags=re.M)
    p.write_text(t)


# Only temporary() is needed by the cartridge source. Avoid desktop bundle and
# user-directory APIs in the embedded library.
a=(src/'nall/nall/path.cpp').read_text()
begin=a.index('NALL_HEADER_INLINE auto temporary()')
end=a.index('\n}\n',begin)+3
(Path(__file__).resolve().parent/'nall-path.cpp').write_text('// Extracted unchanged from pinned ISC nall/path.cpp by vendor.py.\n#include <nall/path.hpp>\nnamespace nall::Path {\n'+a[begin:end]+'\n}\n')
p=dst/'libco/settings.h'
t=p.read_text().replace('#define thread_local __thread','#if defined(_MSC_VER)\n      #define thread_local __declspec(thread)\n    #else\n      #define thread_local __thread\n    #endif')
t+='\n#ifdef LIBCO_MPROTECT\n#error "Kog SNSF requires statically linked libco switch code"\n#endif\n'
p.write_text(t)
# Reconstruct machines between invocations on a reused worker: a console's
# hardware reset intentionally preserves some state, whereas a new decoder must
# begin with the same counters and coroutine bookkeeping as a fresh process.
# Collect the exact namespace instances from their original unity include order.
paths=[]
def visit(p):
    text=p.read_text()
    for line in text.splitlines():
        match=re.match(r'#include [<"]([^">]+\.cpp)[>"]',line)
        if match:
            target=(p.parent/match[1]) if '"' in line else src/'ares'/match[1]
            if target.exists(): visit(target)
        match=re.match(r'^(?!struct |class |enum )(\w+) (\w+)(\{"[^"\n]+"\})?;$',line)
        if match: paths.append(match.groups())
visit(src/'ares/sfc/sfc.cpp')
names=[name for kind,name,args in paths]
t='// Generated reconstruction order from pinned ares/sfc/sfc.cpp.\n'
t+='inline void resetSnsfMachine() {\nusing namespace ares::SuperFamicom;\n(void)ares::SuperFamicom::system; // initialize this worker before reconstructing instances\n'
for name in reversed(names): t+=f'std::destroy_at(&ares::SuperFamicom::{name});\n'
for kind,name,args in paths:
    arguments=', '+args[1:-1] if args else ''
    t+=f'std::construct_at(&ares::SuperFamicom::{name}{arguments});\n'
t+='Thread::EntryPoints().clear();\n}\n'
(dst/'reset.hpp').write_text(t)

# Inventory the original imported files (before the transformations above).
originals=[]
for p in sorted(dst.rglob('*')):
    if not p.is_file(): continue
    relative=p.relative_to(dst)
    donor=src/relative
    if donor.is_file(): originals.append(f"{hashlib.sha256(donor.read_bytes()).hexdigest()}  {relative}")
for name in ['mia/medium/super-famicom.cpp','mia/Database/Super Famicom Boards.bml',
             'ares/ares/resource/sprite/sfc/crosshair-green.png','ares/ares/resource/sprite/sfc/crosshair-red.png']:
    originals.append(f"{hashlib.sha256((src/name).read_bytes()).hexdigest()}  {name}")
(dst/'UPSTREAM_FILES.sha256').write_text('\n'.join(originals)+'\n')

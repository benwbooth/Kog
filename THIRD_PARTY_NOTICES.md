# Third-party notices

## FFmpeg

Kog dynamically links the system FFmpeg libraries `libavformat`, `libavcodec`,
`libavutil`, and `libswresample` through the bridge in
`native/ffmpeg_bridge.cpp`. FFmpeg source is not vendored in this repository.
The pinned Nix development shell currently resolves FFmpeg 9.0.1 and overrides
the package with `withGPL = false` and `withVersion3 = false`. The resulting
binary's own license output identifies it as GNU Lesser General Public License
version 2.1 or (at your option) any later version. Builds outside that shell
must supply a GPL-3.0-or-later-compatible FFmpeg configuration. The
conservative pinned configuration is retained as the tested baseline;
GPLv3-compatible FFmpeg components may be enabled by other builds.

FFmpeg is Copyright (c) the FFmpeg developers and contributors identified by
the upstream project. The linked configuration is distributed under the GNU
Lesser General Public License, version 2.1 or later; a copy is in
`LICENSES/LGPL-2.1.txt`. Kog's 768-byte AC-3 regression fixture is encoded from
a generated 880 Hz sine wave and contains no third-party media. The 992-byte
MP3 CueSheet fixture is likewise encoded with FFmpeg/libmp3lame from a generated
880 Hz sine and adds only synthetic ID3v2 CUESHEET metadata.

## Cog OPL3Windows and Nuked OPL3

The source under `native/opl3w`, except Kog's `kog_opl3w.cpp` and
`kog_opl3w.h` C ABI wrapper, is copied from the MIDI plugin in
[Cog](https://github.com/losnoco/Cog) at commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722`.

The OPL3Windows synthesizer, General MIDI timbre table, chip wrapper, and Nuked
OPL3 1.7.1 core are Copyright (C) Apogee Software, Ltd., Alexey Khokholov
(Nuke.YKT), and the contributors identified in their source headers. They are
distributed under the GNU General Public License, version 2 or (at your option)
any later version. Kog uses them under GPL version 3 as part of the
GPL-3.0-or-later application; see `LICENSE`.

The resampler is Copyright (C) 2004-2008 Shay Green and Copyright (C)
2015-2022 Christopher Snowhill. It is distributed under the GNU Lesser General
Public License, version 2.1 or (at your option) any later version. A copy is in
`LICENSES/LGPL-2.1.txt`.

## Munt / libmt32emu

The `native/munt` Git submodule is the official
[Munt](https://github.com/munt/munt) repository at release 2.8.2, commit
`3b05ec276f9e605af86b0eaef7f5eda43477a31f`. Kog statically builds only the
libmt32emu library, C interface, and internal resampler, then calls that API
through `native/mt32emu_bridge.cpp`. It does not build or invoke Munt's Qt,
command-line, driver, or daemon frontends.

libmt32emu is Copyright (C) 2003-2009 Dean Beeler and Jerome Fisher, and
Copyright (C) 2011-2026 Dean Beeler, Jerome Fisher, and Sergey V. Mikayev. It
is distributed under the GNU Lesser General Public License, version 2.1 or (at
your option) any later version. Its complete source and exact license texts are
retained in `native/munt/mt32emu`; a copy of the LGPL-2.1 text is also in
`LICENSES/LGPL-2.1.txt`.

Munt identifies compatible MT-32, CM-32L, and LAPC-I control/PCM ROM images at
runtime. Kog does not contain, download, or redistribute Roland firmware or
sample ROM data; users must supply files obtained from hardware they own.
Roland product names are used only to identify compatibility and do not imply
affiliation or endorsement.

## Nuked SC-55 and kog-sc55-helper

The `native/nuked-sc55` Git submodule is J.C. Moyer's maintained reusable
backend fork of [Nuked SC-55](https://github.com/jcmoyer/Nuked-SC55), pinned to
release 0.6.1 at commit
`50dcddeacfdf6fcfbcc948ca08cf4ad5fac08980`. The fork attributes the original
implementation to nukeykt and identifies additional contributors in its
README and source history.

Nuked SC-55 is distributed under the original MAME license. Its terms prohibit
selling redistributions and use in commercial products or activity, require
complete source for modified redistributions subject to the stated system
component exception, and require preservation of its copyright, conditions,
and disclaimer. The exact terms are retained in `native/nuked-sc55/LICENSE`
and copied to `LICENSES/Nuked-SC55-original-MAME.txt`.

Kog compiles only the fork's emulator backend and hash-based ROM loader into
the separately identified optional `kog-sc55-helper`; it does not build the
SDL, RtMidi, standard frontend, renderer frontend, or GUI. No Nuked SC-55
object is linked into the GPL-3.0-or-later Kog executable. Kog's adapter in
`native/sc55-helper` is marked `LicenseRef-Nuked-SC55` and distributed under
the same terms as that combined helper. The process boundary and protocol in
`native/sc55-helper/PROTOCOL.md` do not relicense the emulator. Binary/source
distributors must retain the complete pinned corresponding source and notices
and independently review the noncommercial restriction.

The helper locates supported model ROMs by their upstream-known hashes. Kog
does not contain, download, or redistribute Roland firmware, wave ROMs, or
other proprietary ROM data; users must supply any required files themselves.

## libADLMIDI

The `native/libadlmidi` Git submodule is the official
[libADLMIDI](https://github.com/Wohlstand/libADLMIDI) repository at commit
`d114c313c9f6a54b6a93adef2b077810136cf508`. Original ADLMIDI code is
Copyright (c) 2010-2014 Joel Yliluoma; the library API and current project are
Copyright (c) 2015-2026 Vitaly Novichkov and contributors identified upstream.

Kog statically builds the MIDI sequencer, MUS/XMI conversion support, embedded
banks, and Nuked OPL3 family. It disables the other emulator families, tools,
tests, and optional HQ resampler. The build sets `BUILD_NO_GREY_BANKS=ON`, which
selects upstream's `inst_db_no_grey.cpp`; the separately identified grey-zone
bank set is not embedded in Kog.

The enabled source retains the upstream project's component-specific
GPL-3.0-or-later, GPL-2.0-or-later, LGPL-2.1-or-later,
LGPL-2.0-or-later, MIT, BSD-3-Clause, Boost-1.0, public-domain, and embedded-bank
terms. The complete source, root GPLv3 and LGPL-2.1 texts, component notices,
and individual bank notices remain in the pinned submodule. These terms are
compatible with Kog's GPL-3.0-or-later application; libADLMIDI is not relicensed
by Kog. The DMX MUS regression score is generated by Kog's tests and contains
no third-party music, game code, or samples.

## Game Music Emu

The `native/game-music-emu` Git submodule is the official
[Game Music Emu](https://github.com/libgme/game-music-emu) source at release
0.6.5, commit `9e23d10f9fd2a6a2f33b10912dd8dc7153258995`. Kog builds a selected
set of its emulators as a static native library and calls its public C API.
The upstream `test.nsf` and `test.m3u` files are used as playback fixtures.

Game Music Emu is Copyright Shay Green and the contributors identified by the
upstream project. It is distributed under the GNU Lesser General Public
License, version 2.1. The complete upstream source and license are retained in
the submodule, and a copy of the license is also in `LICENSES/LGPL-2.1.txt`.

## libvgm

The `native/libvgm` Git submodule is the official
[libvgm](https://github.com/ValleyBell/libvgm) source at Cog's pinned commit
`867223e7c33d63de115d1ab955f784c44f19040a`. Kog builds the static utilities,
emulation, and player libraries and registers the VGM, S98, DRO, and GYM
engines through its own small C ABI wrapper.

libvgm and its emulation cores are Copyright ValleyBell and the contributors
identified in the individual source headers. Those headers identify code under
BSD-3-Clause, GPL-2.0, GPL-2.0-or-later, LGPL-2.1-or-later, and MIT terms. The
complete corresponding source and per-file notices are retained in the
submodule. Copies of the BSD-3-Clause, MIT, GPL-2.0, and LGPL-2.1 texts are in
`LICENSES`.

The GPL-2.0-only YMF278B implementation remains in the unmodified upstream
submodule but is not compiled or linked into Kog's GPLv3 executable. All other
configured libvgm chip families remain enabled. OPL4/YMF278B playback will use
a compatible replacement or separately licensed helper in a future parity
milestone. The Yamaha YRW801 sample ROM is not included.

## libopenmpt and bundled sample decoders

The `native/openmpt` Git submodule is the official
[OpenMPT](https://github.com/OpenMPT/openmpt) source at release 0.8.7, commit
`11363ff11ba021b1cf1533da17d9fdf20c8d883c`, matching Cog's bundled release.
Kog builds libopenmpt as a static C++17 library and calls its public C API.

libopenmpt is Copyright (c) 2004-2026 OpenMPT Project Developers and
Contributors and Copyright (c) 1997-2003 Olivier Lapicque. It is distributed
under the BSD 3-Clause license. The complete source and license are retained
in the submodule, and the exact license text is copied to
`LICENSES/OpenMPT-BSD-3-Clause.txt`.

Kog enables the decoder copies carried by that source tree: miniz under the
MIT license, minimp3 under CC0-1.0, and stb_vorbis under its MIT option. Their
complete source and license notices remain in `native/openmpt/include`; Kog's
MIT text is also in `LICENSES/MIT.txt`.

## HivelyTracker

The `native/hivelytracker` Git submodule is the official
[HivelyTracker](https://github.com/pete-gordon/hivelytracker) source at commit
`f393ca7c6416f00bcb574b334a7e8b57dcb19eb2`, version 1.9 plus its upstream
post-release fixes. Kog builds the portable Windows replayer sources behind a
small C ownership and streaming bridge; no Windows runtime code is used.

HivelyTracker is Copyright (c) 2006-2018 Pete Gordon and distributed under
the BSD 3-Clause license. The complete source and license are retained in the
submodule, and the exact license text is copied to
`LICENSES/HivelyTracker-BSD-3-Clause.txt`.

## syntrax-c and kog-syntrax-helper

The `native/syntrax-c` Git submodule is losnoco's canonical
[syntrax-c](https://bitbucket.org/losnoco/syntrax-c) source at commit
`1184fb9ef562d20dab26e419052982d1c3329b76`. Its seven source and header files
are byte-for-byte identical to the plain-C renderer in Cog commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722`. Kog compiles that portable library
into the separate `kog-syntrax-helper` and does not copy or translate Cog's
Objective-C plugin classes.

syntrax-c is Copyright (c) Reinier van Vliet and Christopher Snowhill and each
upstream file identifies itself as GPL-3.0-only. The GPL version 3 text is in
Kog's root `LICENSE`. Kog's adapter under `native/syntrax-helper` is
GPL-3.0-or-later; the combined helper executable is distributed under GPL
version 3 only. Its process boundary contains faults from the legacy parser
and renderer and is not a license workaround. The protocol is documented in
`native/syntrax-helper/PROTOCOL.md`.

Kog's tests construct a packed two-subsong JXS song with an original synthetic
wavetable. It contains no third-party song, sample, or recording.

## orgorg

Kog uses version 0.2.1 of the
[orgorg](https://github.com/kpqi5858/orgorg) Rust crate for Organya synthesis.
orgorg is Copyright (c) 2025 kpqi5858 and is distributed under the MIT
license; the exact license is in `LICENSES/orgorg-MIT.txt`.

The original Cave Story wavetable and PixTone drum data are not included in
Kog. Users may point Kog at their own `soundbank.wdb` or an extracted
`wavetable.dat`/`drums.dat` pair. Kog's tests instead generate a small Org-02
song and synthetic wavetable specifically for the test run.

## vgmstream

The `native/vgmstream` Git submodule is the official
[vgmstream](https://github.com/vgmstream/vgmstream) source at r2117 commit
`05dbda9b930b8d174f03387fb626d97d827d0647`. Kog builds the static library with
its native codecs and built-in G.722.1 enabled, and calls its public C API
through a small ownership and metadata bridge. The optional FFmpeg, Vorbis,
mpg123, G.719, ATRAC9, CELT, and Speex dependencies are not part of this
baseline build.

vgmstream is Copyright (c) 2008-2025 Adam Gashlin, Fastelbja, Ronny Elfert,
bnnm, Christopher Snowhill, NicknineTheEagle, bxaimc, Thealexbarney,
CyberBotX, EdnessP, and other contributors identified by the upstream source.
It is distributed under the permissive ISC-style terms retained in the
submodule and copied verbatim to `LICENSES/vgmstream-ISC.txt`. Kog's generated
VAG fixture and `!tags.m3u` file are created by its tests and contain no game
content.

## AdPlug and libbinio

The `native/adplug` submodule is Cog maintainer kode54's
[AdPlug](https://github.com/kode54/adplug) fork at Cog's exact commit
`4e0141ab41ac4ebf388b765d669eb656376d04fd` (version 2.3.4-beta). The
`native/libbinio` submodule is AdPlug's matching binary-I/O dependency at
Cog's exact commit `e2f8d50c53102c618d675c3310e09a0e0bdf49cd`. Kog builds both
statically, uses AdPlug's bundled Nuked OPL3 emulator through a small C ABI
bridge, and namespaces its OPL symbols from Kog's separate MIDI synthesizer.
The upstream `test/2.CMF` is used as the first playback fixture. Cog's optional
AdPlug song database is not yet bundled.

AdPlug and libbinio are Copyright (C) Simon Peter and the contributors named
in their source and are distributed under the GNU Lesser General Public
License, version 2.1 or (at your option) any later version. Their complete
source and exact license texts are retained in the two submodules; a copy of
the LGPL-2.1 text is also in `LICENSES/LGPL-2.1.txt`.

## libsidplayfp and reSIDfp

The `native/libsidplayfp` submodule is Cog maintainer kode54's
[libsidplayfp](https://github.com/kode54/libsidplayfp) fork at Cog's exact
commit `519d1201efcc6c97f7cc3506947875d21a9bd195` (version 2.4.0a). Kog builds
the emulated C64 engine and in-tree reSIDfp synthesizer statically behind a
small C ABI bridge. The source checkout contains the MUS player assembly but
does not commit the generated C includes, so
`native/libsidplayfp-generated/sidtune` retains Cog's exact `xa`-generated
outputs from the same pinned source.

libsidplayfp, reSIDfp, and the generated player code are Copyright (C) Simon
White, Dag Lem, Antti Lankila, Leandro Nini, and the contributors identified
in their source headers. They are distributed under the GNU General Public
License, version 2 or (at your option) any later version. Their complete
source and license are retained in the submodule; Kog uses them under GPL
version 3 as part of this GPL-3.0-or-later application. Kog's deterministic
PSID fixture is generated by its tests and contains original synthetic 6502
code. Commodore C64 ROM images are not included.

## mGBA

The `native/mgba` submodule is Cog maintainer kode54's
[mGBA](https://github.com/kode54/mGBA) fork at Cog's exact commit
`f6b1854c373fd7cdf8571b9d8568f68bc2decdb1`. Kog builds a minimal static GBA
core behind a bounded C ABI bridge for GSF and minigsf playback.

mGBA is Copyright (c) 2013-2016 Jeffrey Pfau and its contributors and is
distributed under the Mozilla Public License, version 2.0. Its complete source
and exact license text are retained in the submodule. The bundled inih source
retains its BSD license under `native/mgba/res/licenses/inih.txt`. Kog uses
mGBA's high-level startup and does not include a proprietary GBA BIOS. Its GSF
tests generate an original ARM program and PSF wrappers and contain no Nintendo
logo, firmware, or game data.

## Highly Quixotic

The `native/highly-quixotic` submodule is kode54's standalone
[Highly Quixotic](https://github.com/kode54/Highly_Quixotic) repository at
commit `1150a17696dbd044f215f823166a5f2b6519cd5f`. This is the portable C
Z80/Kabuki/QSound engine wrapped by Cog's Objective-C QSF plugin. Kog builds
the active `qsound.c`, `qsound_ctr.c`, `kabuki.c`, and `z80.c` sources behind
its own bounded C ABI bridge and shares psflib's PSF version 0x41 parser.

Highly Quixotic is Copyright (C) Christopher Snowhill and the contributors
identified by its history; its HLE QSound mixer is by Ian Karlsson with thanks
to Valley Bell. The repository is distributed under the GNU General Public
License, version 3, whose exact text is retained as
`native/highly-quixotic/LICENSE.TXT` and is also Kog's root `LICENSE`.

Kog generates modified copies of `qsound.c` and `qsound_ctr.c` in Cargo's
build output. Those GPLv3 derivatives make the C inline helpers portable,
bound banked Z80 and sample-ROM reads for untrusted files, handle sample-ROM
allocation failure, and free the DSP's copied sample ROM without freeing its
embedded state. The pinned submodule is not modified. Kog's QSF tests generate
an original Z80 program, sample waveform, and PSF wrappers and contain no
Capcom program, audio, or game data.

## Highly Theoretical

The `native/highly-theoretical` submodule is kode54's standalone
[Highly Theoretical](https://github.com/kode54/Highly_Theoretical) repository
at commit `2998a4bf550949cd2daee249e725a64462cf15e0`. This is the portable
Saturn/Dreamcast sound emulator beneath Cog's Objective-C SSF/DSF plugin. Kog
builds `sega.c`, the SCSP/AICA and ARM components, and the C68k implementation
behind its own bounded C ABI bridge, and shares psflib's PSF 0x11/0x12 parser.

The Highly Theoretical repository carries the GNU General Public License,
version 3, in `native/highly-theoretical/LICENSE.TXT`; its bundled C68k files
are explicitly distributed under the GNU General Public License, version 2 or
(at your option) any later version. Kog selects that GPL-compatible C68k route.
The upstream submodule also contains Musashi, whose notice permits only
non-commercial use without a separate license, and Starscream, whose terms
forbid commercial use; neither alternative is compiled or linked into Kog.
Their source and individual notices remain unmodified in the submodule and do
not change the license of Kog's executable.

Kog generates patched copies of `satsound.c` and `yam.c` in Cargo's build
output to make an upstream pointer conversion explicit and to make the calling
convention portable to AArch64. The pin itself is not modified. Kog's tests
generate original synthetic 68000 and ARM programs, waveforms, and PSF wrappers
and contain no Sega firmware, program, audio, or game data.

## LazyUSF2

The `native/lazyusf2` submodule is the maintained
[LazyUSF2](https://bitbucket.org/losnoco/lazyusf2) repository at commit
`f771b33f3a9f96f351ab43635a4b8529fa26a47d`. It is the cross-platform
Nintendo 64 emulator core beneath Cog's Objective-C USF plugin. Kog compiles
the upstream x86 or x86-64 dynarec where supported and the cached interpreter
on other architectures, together with the HLE/LLE RSP audio paths, behind its
own bounded C ABI bridge and psflib's PSF version 0x21 parser.

LazyUSF2 is assembled from Mupen64Plus core and RSP-HLE code whose compiled
source headers permit redistribution under GNU GPL version 2 or any later
version, CC0-dedicated RSP-LLE/vector code, and BSD-licensed CIC and debugger
components. Copyright belongs to the Mupen64Plus, LazyUSF2, RSP-HLE, RSP-LLE,
NetBSD, X-Scale, and other contributors identified in the retained source
headers and `native/lazyusf2/rsp_hle/LICENSES`. The complete corresponding
source and notices remain in the pinned submodule; copies of GPL-2.0,
BSD-2-Clause, and BSD-3-Clause terms are also in `LICENSES`. These terms are
compatible with Kog's GPL-3.0-or-later application.

Kog's tests generate a sparse Project64 save state containing an original
MIPS program and synthetic stereo waveform. They include no Nintendo firmware,
ROM image, proprietary program, game data, or recorded audio.

## libupse and kog-psf-helper

The `native/libupse` submodule is kode54's cross-platform
[libupse](https://github.com/kode54/libupse) repository at commit
`e3f1192e55e3eb5e1a22b84ed2c4f5a0e0786d85`. It supplies PlayStation PSF and
miniPSF emulation with a high-level BIOS implementation, so Kog does not copy
Cog's Objective-C decoder or its embedded Sony BIOS data.

libupse's source headers identify the project under GNU General Public License
version 2. Kog conservatively treats the revision as GPL-2.0-only. It is not
linked into Kog's GPL-3.0-or-later executable: the build creates the separate
`kog-psf-helper` program, combining libupse only with the adapter sources under
`native/psf-helper`, which are also GPL-2.0-only. A copy of the license is in
`LICENSES/GPL-2.0.txt`; the complete corresponding libupse source and its
individual notices remain in the pinned submodule. Binary distributions must
install the helper beside Kog and provide its corresponding source and notices
under those terms.

The Rust application and helper communicate through the independently
documented metadata/PCM stream in `native/psf-helper/PROTOCOL.md`. The helper
prevalidates bounded xSF structure and contains legacy-core process failures;
it is not an operating-system sandbox. Kog's tests generate original MIPS code,
SPU register writes, an ADPCM waveform, and PSF wrappers. They include no Sony
firmware, game program, game data, or recorded audio.

## Play! and kog-psf2-helper

The `native/play` submodule is Jean-Philip Desjardins' cross-platform
[Play!](https://github.com/jpd002/Play-) emulator at commit
`04bde0df87ee7c0e2f0151b51bb2cc22c88541da`. Kog reuses Play!'s PSF player,
IOP high-level BIOS, CPU, and SPU2 implementation for PSF2 and miniPSF2 instead
of translating Cog's Objective-C plugin or redistributing Sony firmware.

Play!, Framework (`587f278917acc0026bf5fc34b39f995fc26bd015`), and CodeGen
(`a5009f7dca062695b8e5aebbd71e67b4ddfa9251`) use permissive BSD two-clause
terms; their complete notices remain in the pinned recursive submodules. The
exact Play! notice is copied to `LICENSES/Play-BSD-2-Clause.txt`. The helper's
reachable dependency set also includes BSD-licensed libchdr, xxHash, and zstd;
the public-domain LZMA SDK; system zlib; and platform-provided OpenSSL, bzip2,
and ICU libraries where selected by Play!'s CMake build. Their license files
and copyright notices remain in `native/play/deps`, and binary packagers must
carry the corresponding notices for the libraries they distribute.

Kog's GPL-3.0-or-later adapter under `native/psf2-helper` and Play! are compiled
only into the separate `kog-psf2-helper` executable; no Play! object is linked
into the Kog executable. The helper uses the versioned metadata/PCM protocol in
`native/psf2-helper/PROTOCOL.md`. It validates PSF2 containers, dependency
chains, filesystem blocks, and IRX/ELF bounds before Play! sees them. This is a
process boundary, not an operating-system sandbox.

Tests construct an original MIPS IOP module that writes a synthetic ADPCM
waveform to emulated SPU2 registers, then wrap it in generated PSF2/miniPSF2
filesystems. They contain no Sony BIOS, firmware, game code, game data, or
recorded audio.

## melonDS and kog-2sf-helper

The `native/melonds` submodule is the official cross-platform
[melonDS](https://github.com/melonDS-emu/melonDS) emulator at the 1.1 release
commit `b86390e4428bf38ce4c1ce0e9ca446d6d25955e8`. Kog reuses its maintained
Nintendo DS CPU, memory, free BIOS/firmware generation, cartridge, and SPU
implementation for 2SF and mini2SF instead of translating Cog's Objective-C++
wrapper or redistributing Nintendo firmware.

melonDS is Copyright (c) 2016-2026 Arisotura and contributors and is licensed
under GNU General Public License version 3 or later. The complete license and
per-file copyright notices remain in the pinned submodule; its GPL text is
identical to Kog's root `LICENSE`. Kog's GPL-3.0-or-later adapter under
`native/twosf-helper`, melonDS, psflib, and system zlib are compiled into the
separate `kog-2sf-helper` executable. This is a fault-containment boundary, not
a license workaround. Binary distributors must install the helper beside Kog
and provide the corresponding source and notices under their respective
terms.

The helper uses the metadata/PCM protocol in
`native/twosf-helper/PROTOCOL.md`, validates bounded 2SF ROM/save mappings and
Nintendo DS executable ranges, and builds melonDS without its Qt/SDL frontend,
JIT, OpenGL renderer, or debugger. Tests create an original ARM program,
synthetic PCM waveform, minimal Nintendo DS ROM, and 2SF wrappers. They include
no Nintendo BIOS, firmware, copyrighted game program, game data, recorded
audio, or commercial ROM image.

## libsnsf9x and kog-snsf-helper

The `native/libsnsf9x` submodule is Deewiant's dedicated Linux
[libsnsf9x](https://github.com/Deewiant/libsnsf9x) library at commit
`e53bff56fbb7c29d5222c60b81a54b762ad9cec7`. It is a stripped and
Linux-portable SNSF player based on snsf9x 0.04.10 and Snes9x 1.53. Kog calls
its published `IXSFDRV` C interface from the separately identified optional
`kog-snsf-helper`; no libsnsf9x object is linked into the main Kog executable.
The CMake build retains the pinned sources unchanged and creates one generated
copy of `xsfc/xsfdrv.c` whose `const LPVOID` parameter is spelled
`const void *`, fixing a current-compiler function-pointer diagnostic without
changing behavior.

The Snes9x-derived source is Copyright the Snes9x contributors identified in
its retained headers and permits source and binary use for non-commercial
purposes while describing Snes9x as freeware for personal use. Its complete
notice remains at
`native/libsnsf9x/snsf9x/snes9x/docs/snes9x-license.txt` and is copied to
`LICENSES/Snes9x.txt`. The S-SMP/S-DSP APU
source additionally retains GNU Lesser General Public License version 2.1
terms at `native/libsnsf9x/snsf9x/snes9x/apu/license.txt`; Kog's copy of that
license is in `LICENSES/LGPL-2.1.txt`. The upstream repository has no single
top-level license declaration, so Kog conservatively distributes the entire
optional helper only under the Snes9x non-commercial terms plus every retained
component notice. The adapter sources under `native/snsf-helper` are marked
`LicenseRef-Snes9x` for that combined program. Kog's non-commercial project
intent does not relicense these components or make them GPL-compatible.

Before libsnsf9x runs, the MIT-licensed psflib resolves and verifies a bounded
SNSF dependency tree. Kog assembles the validated ROM/SRAM state into one
dependency-free SNSF image, so libsnsf9x's older file loader never receives
user-controlled companion paths. The helper uses the metadata/PCM protocol in
`native/snsf-helper/PROTOCOL.md`; its process boundary is fault and license
isolation, not an operating-system sandbox. Tests generate original 65C816 and
SPC700 programs plus a synthetic BRR waveform. They include no Nintendo
firmware, game program, commercial ROM, game data, or recorded audio.

## SSEQPlayer and psflib

The `native/sseqplayer` submodule is kode54's official
[SSEQPlayer](https://github.com/kode54/SSEQPlayer) repository at commit
`77222d3657adff358fb4e610d3e56bb7ada8ec24`. Kog builds its portable Nintendo
DS sequence, bank, and waveform replayer behind a bounded C ABI bridge. The
`native/psflib` submodule is kode54's official
[psflib](https://github.com/kode54/psflib) repository at commit
`95509e0c6f13d769593bbf51a1b0e0efdc355ba1`; it parses PSF version 0x25,
resolves NCSF library chains, and uses the system zlib for decompression.

SSEQPlayer is Copyright Naram Qashat (CyberBotX), fincs, and the contributors
identified by its source. It is distributed under the Do What The Fuck You
Want To Public License, version 2, whose exact text is retained in the
submodule and copied to `LICENSES/WTFPL-2.txt`. psflib is Copyright (c)
2012-2015 Christopher Snowhill and is distributed under the MIT license; its
complete source and license are retained in the submodule, and the MIT text is
also in `LICENSES/MIT.txt`. Kog's NCSF fixtures generate an original SDAT,
SSEQ, SBNK, SWAR, PCM waveform, and PSF wrappers during tests and contain no
Nintendo or game data.

## compress-tools and libarchive

Kog uses version 0.16.1 of the
[compress-tools](https://github.com/OSSystems/compress-tools-rs) Rust crate
under its MIT license option and dynamically links the system
[libarchive](https://github.com/libarchive/libarchive) library. Neither
libarchive source nor a libarchive binary is vendored in this repository. The
pinned Nix development shell currently resolves libarchive 3.8.9.

compress-tools is Copyright the OSSystems contributors and is available under
MIT or Apache-2.0; Kog uses the MIT option, whose text is in
`LICENSES/MIT.txt`. The libarchive distribution is Copyright Tim Kientzle and
its contributors and uses permissive two-clause terms for most runtime source,
with the upstream `COPYING` file and individual source headers controlling.

The 109-byte RAR5 archive encoded in `src/archive.rs` is the decoded form of
libarchive's `test_read_format_rar5_stored.rar.uu` regression fixture. Its
corresponding test source is Copyright (c) 2018 Grzegorz Antoniak and is
redistributed under the two-clause BSD terms copied to
`LICENSES/BSD-2-Clause.txt`. The 7Z fixture was generated locally from an empty
regular file with libarchive 3.8.9; the ZIP and GZ fixtures are generated by
Kog's tests.

## TinySoundFont minimal SoundFont test fixture

The 484-byte `MinimalSoundFont` data encoded in `src/decoder.rs` is derived
from `examples/example1.c` in
[TinySoundFont](https://github.com/schellingb/TinySoundFont). It is used only
by Kog's decoder tests.

Copyright (C) 2017-2023 Bernhard Schelling. Based on SFZero, Copyright (C)
2012 Steve Folta.

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

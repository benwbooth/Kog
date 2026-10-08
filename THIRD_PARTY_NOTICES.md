# Third-party notices

## Webamp Modern (integration source; not shipped in the player yet)

`native/webamp` pins [Webamp](https://github.com/captbaritone/webamp) at
`88ed5815d968c201962f6549915579b3d2f93c5e` for its modern skin renderer and
reimplemented MAKI interpreter. Its source license is MIT, Copyright (c) 2015
Jordan Eldredge; the full notice is in `native/webamp/LICENSE.txt`. Kog does not
currently bundle this renderer, its JavaScript dependencies, or its example
skins into the executable. Those dependencies and their notices must be audited
when the renderer bundle is integrated.

## Layer Shell Qt

Linux builds can load KDE's [Layer Shell Qt](https://github.com/KDE/layer-shell-qt)
QML module to position the now-playing popup on Wayland without taking keyboard
focus. The Nix shell supplies it; the Flatpak manifest pins version 6.5.5 at
`76fbe5deb9d9545be98d1900b5d0df868616759e`. Its interface and QML bindings are
licensed under LGPL-2.1-only OR LGPL-3.0-only OR LicenseRef-KDE-Accepted-LGPL;
Kog uses the LGPL-2.1 option (see `LICENSES/LGPL-2.1.txt`). Upstream copyright
holders include Aleix Pol Gonzalez and Drew DeVault. It is loaded as a shared
library, not translated or statically incorporated into Kog.

## FFmpeg

Kog dynamically links the system FFmpeg libraries `libavformat`, `libavcodec`,
`libavutil`, and `libswresample` through the decoder and streaming encoder
bridges in `native/ffmpeg_bridge.cpp` and
`native/ffmpeg_encoder_bridge.cpp`. FFmpeg source is not vendored in this
repository.
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

## Cog equalizer presets

`assets/Cog.q1.json` contains the equalizer preset data from
[Cog](https://github.com/losnoco/Cog) at commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722`. The preset names and values are
distributed with Kog under the GNU General Public License, version 3 or later;
see `LICENSE`. Kog's Rust interpolation and DSP implementation are new code
matched to Cog's documented 31-band behavior.

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

## Spleen

`qml/fonts/spleen-6x12.otf` is the 6×12 size of Frederic Cambus's
[Spleen](https://github.com/fcambus/spleen) bitmap font, release 2.1.0,
unmodified. The Channel Inspector's tracker view uses it on every frontend.
It is BSD-2-Clause licensed; the license is in
`LICENSES/Spleen-BSD-2-Clause.txt`.

## Nuked SC-55

The `native/nuked-sc55` Git submodule is J.C. Moyer's reusable backend fork of
[Nuked SC-55](https://github.com/jcmoyer/Nuked-SC55), pinned to release 0.7.0
at commit `e8a6bdc7149dae2f849a8bad8ac790e21e77b2f7`. That source is now
GPL-2.0-or-later, as stated in its README and source headers. Its license is
retained in `native/nuked-sc55/LICENSE` and copied to
`LICENSES/Nuked-SC55-GPL-2.0-or-later.txt`. Kog uses the GPL version 3 option
when combining this backend with its GPL-3.0-or-later code.

Kog compiles the emulator backend and ROM loader, without the SDL, RtMidi,
standard frontend, renderer frontend, or GUI. All frontends link the same core
into their audio backend through `kog_sc55_render`. The adapter source in
`native/sc55-helper` is GPL-3.0-or-later. Firmware and waveform ROMs are
user-provided and are never bundled.

The helper locates supported model ROMs by their upstream-known hashes. Kog
does not contain, download, or redistribute Roland firmware, wave ROMs, or
other proprietary ROM data; users must supply any required files themselves.

## SpessaSynth Core C

The `native/spessasynth-core` Git submodule is kode54's portable C11
[SpessaSynth Core C](https://github.com/kode54/spessasynth_core_c) repository,
pinned to the same revision used by Cog commit `c17be856`, commit
`28a362aa65a1035e2b5f2730001843f8f81e8564`. Kog statically builds the upstream
library with its examples and optional SF3 Vorbis/FLAC decoders disabled, then
uses the file, MIDI loader, EMIDI filter, and writer APIs through
`native/spessasynth_midi_bridge.c`. Kog currently uses this dependency only to
convert MIDS/MDS, LDS, and XMF/MXMF containers to Standard MIDI; synthesis
continues through Kog's separately documented engines. Zlib support remains
enabled for compressed XMF FileNodes.

SpessaSynth Core C is Copyright (C) Christopher Snowhill, Spessasus, and the
contributors identified by the upstream repository. It is distributed under
the Apache License, version 2.0. The complete source and exact license text are
retained in the pinned submodule at `native/spessasynth-core/LICENSE`.

The MIDS, LDS, and XMF fixtures in `src/decoder.rs` are original deterministic
test data generated in memory and contain no third-party music, samples, or
sound banks.

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

## Cog SFM, higan SPC700 and Kog's embedded renderer

`native/cog-gme-sfm` retains the LGPL-2.1-or-later GME SFM/BML/DSP
subset from [Cog](https://github.com/losnoco/Cog) commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722`. The SFM/BML implementation
is Copyright 2013-2026 Christopher Snowhill; SPC DSP is Copyright 2007
Shay Green. The former GPL2-only CPU/SMP code is replaced with
[higan v095](https://github.com/higan-emu/higan/tree/b0e862613b3c6cfaf3d8088403e144d5da98cd43),
whose release declares GPLv3. Kog's adaptations and the exact donor paths
are recorded in `native/cog-gme-sfm/PROVENANCE.md`; GPLv3 and LGPL2.1
texts are retained there. `native/sfm-embedded` is a newly written
GPL-3.0-or-later renderer, linked into the shared backend. The old
GPL2-only `native/sfm-helper` wrapper is historical source, not part
of the build or binary packages. No third-party capture or music is bundled.

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
into both the shared audio backend and the `kog-syntrax-helper` regression
executable. It does not copy or translate Cog's Objective-C plugin classes.

syntrax-c is Copyright (c) Reinier van Vliet and Christopher Snowhill and each
upstream file identifies itself as GPL-3.0-only. The GPL version 3 text is in
Kog's root `LICENSE`. Kog's adapter under `native/syntrax-helper` is
GPL-3.0-or-later. The shared Rust audio backend now links the GPL-3.0-only
renderer as a static library and uses its private in-process PCM stream on all
frontends. The combined binary is distributed under GPL version 3. The helper
executable remains a protocol regression target. The protocol is documented in
`native/syntrax-helper/PROTOCOL.md`.

Kog's tests construct a packed two-subsong JXS song with an original synthetic
wavetable. It contains no third-party song, sample, or recording.

## orgorg

Kog uses version 0.2.1 of the
[orgorg](https://github.com/kpqi5858/orgorg) Rust crate for Organya synthesis.
orgorg is Copyright (c) 2025 kpqi5858 and is distributed under the MIT
license; the exact license is in `LICENSES/orgorg-MIT.txt`.

The pinned 0.2.1 source is included in `vendor/orgorg` with its MIT license.
Kog adds a read-only channel-state accessor for inspection; provenance is
recorded in `vendor/orgorg/KOG.md`.

The original Cave Story wavetable and PixTone drum data are not included in
Kog. Users may point Kog at their own `soundbank.wdb` or an extracted
`wavetable.dat`/`drums.dat` pair. Kog's tests instead generate a small Org-02
song and synthetic wavetable specifically for the test run.

## vgmstream

The `native/vgmstream` Git submodule is the official
[vgmstream](https://github.com/vgmstream/vgmstream) source at r2117 commit
`05dbda9b930b8d174f03387fb626d97d827d0647`. Kog builds the static library with
its native codecs, G.722.1, FFmpeg, Vorbis, MPEG, ATRAC9, CELT and Speex
on every native platform, through `native/vgmstream-kog`. G.719 remains
excluded: upstream documents unclear redistribution terms for its reference
implementation. No upstream Windows codec DLLs are used.

The additional pinned Git submodules are built as static libraries:

| Library | Pin | License notice |
| --- | --- | --- |
| Ogg | v1.3.5 | `LICENSES/Ogg-BSD.txt` |
| Vorbis | v1.3.7 | `LICENSES/Vorbis-BSD.txt` |
| mpg123 | 1.31.1 (`aec901b7`) | `LICENSES/mpg123-LGPL.txt` |
| LibAtrac9 | `6a9e00f6` | `LICENSES/LibAtrac9-MIT.txt` |
| Speex | Speex-1.2.1 | `LICENSES/Speex-BSD.txt` |
| CELT | v0.6.1 / v0.11 | `LICENSES/CELT-0061-BSD.txt`, `LICENSES/CELT-0110-BSD.txt` |

CELT symbols are namespaced to keep its two incompatible versions isolated
from one another and other linked audio libraries. LibAtrac9's DLL annotations
are removed from a generated build copy for static linkage; pinned sources
are unchanged. FFmpeg is the same library already used by Kog's main decoder.

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

## Historical libupse adapter

`native/libupse` at `e3f1192e55e3eb5e1a22b84ed2c4f5a0e0786d85` and
`native/psf-helper` are retained as historical comparison sources under
their GPL-2.0-only terms, with the license in `LICENSES/GPL-2.0.txt`.
They are no longer built, linked, or included in binary packages.
PSF1 playback now uses the Play! HLE libraries and Kog interpreter below.

## Play! and kog-psf2-helper

The `native/play` submodule is Jean-Philip Desjardins' cross-platform
[Play!](https://github.com/jpd002/Play-) emulator at commit
`04bde0df87ee7c0e2f0151b51bb2cc22c88541da`. Kog reuses Play!'s PSF player,
PS1/IOP high-level BIOS, SPU and SPU2 implementation for PSF1/PSF2 and their mini variants instead
of translating Cog's Objective-C plugin or redistributing Sony firmware.

Play!, Framework (`587f278917acc0026bf5fc34b39f995fc26bd015`), and CodeGen
(`a5009f7dca062695b8e5aebbd71e67b4ddfa9251`) use permissive BSD two-clause
terms; their complete notices remain in the pinned recursive submodules. The
exact notices are copied to `LICENSES/Play-BSD-2-Clause.txt`,
`LICENSES/Play-Framework-BSD.txt`, and `LICENSES/Play-CodeGen-BSD.txt`. The helper's
reachable dependency set also includes BSD-licensed libchdr, xxHash, and zstd;
the public-domain LZMA SDK; system zlib; and platform-provided OpenSSL, bzip2,
and ICU libraries where selected by Play!'s CMake build. Copies of the four
bundled dependency notices are under `LICENSES/Play-libchdr-BSD.txt`,
`LICENSES/Play-xxHash-BSD.txt`, `LICENSES/Play-zstd-BSD.txt`, and
`LICENSES/Play-LZMA-public-domain.txt` so release bundles carry them.

Kog's GPL-3.0-or-later adapter and integer/CP0 interpreter under
`native/psf2-helper` are static libraries used by every frontend. Generated
source patches select the interpreter before CPU construction and adapt HLE
trampolines to architectural load delays. The submodule is unchanged. This
path does not execute generated native code or need a Sony BIOS. The optional
standalone executable remains a protocol regression tool. Containers,
dependency chains, PS-X EXE uploads, PSF2 filesystem blocks and IRX/ELF
bounds are validated before emulation. In-process decoding does not provide
process fault isolation. Fixtures generate original MIPS/SPU programs and
ADPCM data; no Sony firmware, music or game data is bundled.

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
`native/twosf-helper`, melonDS, psflib, and system zlib are linked into the
shared Rust audio backend on Unix and iOS. The separate `kog-2sf-helper`
executable remains a protocol regression target. This in-process path does not
provide fault isolation; distributors must provide the corresponding source
and notices under their respective terms.

The helper uses the metadata/PCM protocol in
`native/twosf-helper/PROTOCOL.md`, validates bounded 2SF ROM/save mappings and
Nintendo DS executable ranges, and builds melonDS without its Qt/SDL frontend,
JIT, OpenGL renderer, or debugger. Tests create an original ARM program,
synthetic PCM waveform, minimal Nintendo DS ROM, and 2SF wrappers. They include
no Nintendo BIOS, firmware, copyrighted game program, game data, recorded
audio, or commercial ROM image.

## ares SNSF renderer and historical Snes9x adapter

`native/ares-snsf` is a headless subset of
[ares](https://github.com/ares-emulator/ares) revision
`4cb8d92b441557cb6bcaf133c4cbc7f6819b1122`. Its ISC notice and component
notices are retained in that directory and copied into `LICENSES` for binary
packages. Kog builds the Super Famicom CPU/SMP/DSP/PPU and cartridge support,
nall and statically compiled libco coroutine support. Thread-local adaptations
isolate simultaneous player instances. The new GPL-3.0-or-later adapter in
`native/snsf-ares` uses MIT psflib and zlib for bounded container/library
loading, then supplies ROM/SRAM directly to ares. No Snes9x objects or adapter
code are linked. External enhancement-chip firmware is not redistributed.

The old `native/libsnsf9x` submodule at
`e53bff56fbb7c29d5222c60b81a54b762ad9cec7` and `native/snsf-helper`
remain historical comparison sources with their original Snes9x
personal/non-commercial and LGPL2.1 notices. They are no longer built or
packaged. Their retained source grants are not changed by this migration.

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
under its MIT license option and links [libarchive](https://github.com/libarchive/libarchive).
Desktop packages use the platform library. iOS and Android build static
libarchive 3.8.8 with zlib, XZ/liblzma 5.8.2, bzip2 1.0.8, LZ4 1.10.0 and the
same pinned Zstandard source as Play!. Compression sources are pinned Git
submodules and are compiled for the target by `native/archive-deps`.
Notices: `LICENSES/XZ-0BSD.txt`, `LICENSES/bzip2.txt`,
`LICENSES/LZ4-BSD.txt` and `LICENSES/Play-zstd-BSD.txt`. The full XZ library
is installed as `libkog_liblzma.a` to avoid confusing it with Play!'s unrelated
LZMA SDK archive. The shared mobile build fails if any required compression
backend was not enabled.

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

## Lofty and base64

Kog uses version 0.25.1 of the
[Lofty](https://github.com/Serial-ATA/lofty-rs) Rust crate for bounded metadata
reading and writing, including embedded artwork. Kog also uses version 0.22.1
of the [base64](https://github.com/marshallpierce/rust-base64) Rust crate to
transfer artwork previews between Rust and QML as data URLs.

Both crates are available under MIT or Apache-2.0. Kog uses their MIT license
option, whose text is in `LICENSES/MIT.txt`.

## chardetng and mpris-server

Kog uses version 1.0.0 of
[chardetng](https://github.com/hsivonen/chardetng) to detect undeclared legacy
character encodings in user-visible metadata, playlists, CUE sheets, and
archive entry names. On Linux, Kog uses version 0.10.0 of
[mpris-server](https://github.com/SeaDve/mpris-server) to expose the standard
MPRIS2 D-Bus player interface for desktop media controls and hardware media
keys without invoking an external helper.

chardetng is available under MIT or Apache-2.0. Kog uses its
MIT license option, whose text is in `LICENSES/MIT.txt`. mpris-server is
Copyright Dave Patrick Caberto and is distributed under the Mozilla Public
License, version 2.0; its complete source and license are available from the
crate source linked above. These crates' target-specific transitive
dependencies retain the terms recorded in `Cargo.lock` and their distributed
crate sources.

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

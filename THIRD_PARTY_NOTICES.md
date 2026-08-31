# Third-party notices

## FFmpeg

Kog dynamically links the system FFmpeg libraries `libavformat`, `libavcodec`,
`libavutil`, and `libswresample` through the bridge in
`native/ffmpeg_bridge.cpp`. FFmpeg source is not vendored in this repository.
The pinned Nix development shell currently resolves FFmpeg 9.0.1 and overrides
the package with `withGPL = false` and `withVersion3 = false`. The resulting
binary's own license output identifies it as GNU Lesser General Public License
version 2.1 or (at your option) any later version. Builds outside that shell
must supply a GPL-2.0-compatible FFmpeg configuration; the default Nix
`ffmpeg-headless` configuration is intentionally not used because it enables
GPLv3 components.

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
any later version. Kog distributes them under GPL version 2 as part of the
GPL-2.0-only application; see `LICENSE`.

The resampler is Copyright (C) 2004-2008 Shay Green and Copyright (C)
2015-2022 Christopher Snowhill. It is distributed under the GNU Lesser General
Public License, version 2.1 or (at your option) any later version. A copy is in
`LICENSES/LGPL-2.1.txt`.

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
submodule. Copies of the BSD-3-Clause and MIT texts are in `LICENSES`; GPL-2.0
is Kog's root `LICENSE`, and LGPL-2.1 is in `LICENSES/LGPL-2.1.txt`.

The Yamaha YRW801 sample ROM is not included. Kog only loads a user-provided
`yrw801.rom` beside a music file when libvgm requests it.

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
source and license are retained in the submodule; Kog distributes them under
GPL version 2 as part of this GPL-2.0-only application. Kog's deterministic
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

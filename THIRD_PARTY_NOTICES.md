# Third-party notices

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

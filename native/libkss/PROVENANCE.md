# libkss provenance

From `digital-sound-antiques/libkss` commit
`2ed958823237d054da1ca7965620d8e62a0f9a1d`, with its sound-chip and CPU
modules at:

- emu2149 `1ca770bf7bd480251eee6608b4a3e5caddea931e` (MIT)
- emu2212 `ee35dbe099f7242253e9b5b2a93847161a355c7b` (MIT)
- emu2413 `a1d5120903b75f93d3eb7690e34a29c0b4add903` (MIT)
- emu76489 `36a784abd7d64fcf89be35015059531a154e5a9a` (MIT)
- emu8950 `f82a22e80b68820f263084aa8506b12eb1c9b4dd` (MIT)
- kmz80 `86f9e79db4557360db42efe2f760a94b685f2ee9` (public domain, Mamiya)

libkss itself is under the ISC-style licence in `LICENSE.md`.

Left out: the `modules/drivers` submodule (MML music drivers, which the
upstream licence notes are not under libkss's licence), `src/kss2vgm.c`,
the examples, tests and CMake files. Kog builds with `EXCLUDE_DRIVER_ALL`,
so the MGS/BGM/MPK/OPX/MBM converters compile to stubs and only KSS files
are played. No source files are modified.

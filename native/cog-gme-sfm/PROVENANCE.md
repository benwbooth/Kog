# Cog GME SFM source provenance

This directory contains the minimal portable C++ source subset needed for
Cog's SFM decoder. It was copied verbatim from `Frameworks/GME` in
`losnoco/Cog` commit `c17be85654a64170c86bb8bbb4b59fd7b6795722`.

Kog does not use or translate Cog's Objective-C GME plugin. The separately
built `kog-sfm-helper` calls the reusable `Sfm_Emu` C++ class directly and
streams a bounded binary PCM protocol to the Rust application.

The Game Music Emu, SFM, BML, resampler, filter, and SPC DSP files carry
LGPL-2.1-or-later notices. The imported higan SPC700/SMP integration is kept
under the accompanying GPL-2.0 terms. To avoid linking GPL-2.0-only code into
Kog's GPL-3.0-or-later executable, the complete imported subset is compiled
only into the independently identified helper process.

No game data or music capture is included, and users do not need to supply a
firmware file. The imported core contains the same 64-byte SPC700 IPL bootstrap
table that Cog distributes in `Spc_Sfm.cpp` as part of this implementation.

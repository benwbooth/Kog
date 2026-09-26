# SFM source provenance

The GME, BML, resampler/filter and SPC_DSP subset originates from
`losnoco/Cog` commit `c17be85654a64170c86bb8bbb4b59fd7b6795722`,
`Frameworks/GME`. Those files retain their LGPL-2.1-or-later notices.
Kog uses the C++ SFM snapshot reader, not the Objective-C plugin.

The previous GPL2-only SPC700/SMP integration has been replaced from
**higan v095**, commit `b0e862613b3c6cfaf3d8088403e144d5da98cd43`:

- `processor/spc700/{spc700.cpp,spc700.hpp,algorithms.cpp,instructions.cpp,memory.hpp,registers.hpp}`
- `sfc/smp/{smp.hpp,memory.cpp,timing.cpp}` and reset register initialization
  from `sfc/smp/smp.cpp`.

That release declares `License = "GPLv3"` in `emulator/emulator.hpp`.
Upstream: <https://github.com/higan-emu/higan/tree/b0e862613b3c6cfaf3d8088403e144d5da98cd43>.
The applicable GPLv3 text is included in `LICENSE-GPL-3.0.txt`. No newer
upstream license is retroactively applied to the former GPL2 sources.

Kog adaptations: fixed-width integer and endian aliases in place of nall;
remove desktop debugger/serialization dependencies; per-instance timers,
DSP and CPU state; SFM log reads replace main-CPU port synchronization;
four-bit timer wrap; bounded clock-stop and SLEEP/STOP behavior. SFM has no
external interrupt source that wakes SLEEP; halted CPU time continues to
advance the DSP. The new per-stream scheduler, sample counting and DSP
bridge avoid the previous null-pointer sample-buffer arithmetic. No
unmarked old DSP wrapper implementation is linked.

`native/sfm-embedded` is a new GPL3-or-later adapter, using the existing
versioned Kog PCM protocol. The old GPL2-only `native/sfm-helper` adapter
is retained solely as historical comparison source and is not built or
packaged. The new adapter is linked on desktop, Android and iOS. GME's
private symbols are prefixed during compilation to avoid mixing this
older ABI with Kog's regular libgme. The retained LGPL BML parser has
local fixes for length-bounded scans, empty lines/keys and UTF-8 whitespace
handling; the adapter also bounds metadata nesting before parsing.

No music capture or game ROM is included. The LGPL snapshot implementation
retains its existing 64-byte SPC700 IPL bootstrap. Tests generate their own
SPC program and BRR waveform.

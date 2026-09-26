# ares SNSF subset

Source: https://github.com/ares-emulator/ares
Revision: `4cb8d92b441557cb6bcaf133c4cbc7f6819b1122`

This is the Super Famicom core, its six interpreter processors, shared ares
node/scheduling support, nall and libco. `LICENSE` preserves the complete upstream
license file. The ares project ISC grant covers the imported core and support
source; individual notices remain intact. No desktop frontend, other console
core, SLJIT source, JIT allocator implementation or enhancement-chip firmware is
built. Some unused nall headers remain to preserve portable platform includes.

`../snsf-ares/vendor.py CHECKOUT` reproduces the subset and local changes from the
pinned checkout. Generated `analyzer.hpp` retains unchanged pure methods from
`mia/medium/super-famicom.cpp`; the original donor is retained alongside it.
`boards.bml` is the upstream cartridge mapping database, embedded by `boards.hpp`.
Only the core controller crosshair resources are retained in generated form.

Local changes:

- Super Famicom machine globals, platform/debug state, scheduler entrypoints,
  coroutine serializer scratch space and libco switch-pointer initialization
  use thread-local storage. ARM and Hitachi objects have per-thread owning heap
  storage because their inline RAM exceeds ordinary worker stack/TLS budgets.
- `reset.hpp` reconstructs machines in reverse/forward source definition order
  between decoders on the same worker. Hardware power() alone preserves some
  DSP state; fresh decoders require complete initial state. Coroutine handles
  are destroyed before scheduler reconstruction.
- Video presentation uses the calling worker; all PPU, CPU, interrupt, DMA,
  APU and cartridge timing remains active.
- The unused SLJIT include and executable fixed allocator implementation are
  excluded. `LIBCO_MP` is enabled; `LIBCO_MPROTECT` is rejected at compile time.
  Coroutine switching uses statically linked machine instructions.
- Debug notices go to stderr, keeping the PCM protocol uncorrupted.
- Only nall's unchanged `Path::temporary()` is compiled from its path utility;
  desktop bundle and user directory APIs are omitted.

The adapter has its own GPL-3.0-or-later source and uses Kog's existing MIT
psflib. It was written from SNSF specification v0.03 and ares's APIs; the old
LicenseRef-Snes9x helper is not an input to the importer. The standard 64-byte
SPC700 IPL, already used by Kog, is supplied separately in the adapter; this
hardware firmware is not claimed to be ISC emulator source. No ARM, NEC, Cx4,
Super Game Boy or other external firmware is bundled. Boards requiring those
assets or a secondary cartridge fail with an explicit error.

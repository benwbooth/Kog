# iOS decoder replacement investigation

Investigated with GPT-6 Astra on 2026-09-26. Status: source review and a Linux
SFM CPU prototype; the replacement decoders are not yet implemented in Kog.

## Recommended direction

There are concrete routes for all three formats, but no verified single drop-in library that solves every requirement. Prioritize SFM's small compatible-core rebase, then an SNSF adapter to ares; make a deliberate PSF1 choice between firmware-assisted playback with an existing C interpreter and a larger firmware-free Play! interpreter project. Static packaging is only one requirement: runtime code generation matters too.

| Format | Preferred route | Why | Main remaining work |
|---|---|---|---|
| SFM | Keep Kog's LGPL SFM/BML/DSP code and rebase restricted SPC700/SMP integration onto GPLv3 higan v095, or modern GPL3+ higan if source-grant audit favors it | v095 exposes nearly identical registers, instructions, timing, and timer state; local CPU replacement prototype produced identical generated audio | Complete SMP, DSP-wrapper and helper-wrapper provenance audit/rebase; recreate queue adapter; verify captures, loops, timer/halt behavior, cancellation, ARM64 |
| SNSF / miniSNSF | Build a compatible bounded psflib ROM/SRAM adapter using the existing behavior as a reference; replace Snes9x decoder with headless ares Super Famicom core | ISC core has CPU/APU, cartridge mapper and audio callback; no SNES CPU JIT needed | New core embedding/build, board/ROM/SRAM adapter, timing and coprocessor coverage, firmware inventory, battery/performance and device tests |
| PSF1 / miniPSF, requiring no firmware | Extend already-pinned BSD Play! PS1 route, with a real interpreter execution backend | Existing PSF1 loader, HLE BIOS and SPU are already present; same IOP interpreter effort can also address PSF2 | Implement or adapt a compatible complete R3000/IOP interpreter; preserve delay/exception/interrupt semantics; corpus and performance validation |
| PSF1 / miniPSF, if user-supplied firmware is acceptable | Highly Experimental GPL3 fork plus psflib | C interpreter and direct PS-X EXE / PCM APIs already exist; smaller audio-focused core | Audit license provenance and firmware handling, safely import/generate required HE-format BIOS, loader and iOS tests |

Effort ordering is relative engineering judgment, not a schedule: SFM is the smallest focused port; SNSF is a medium integration and fidelity project; a complete firmware-free Play! interpreter is the largest and highest technical risk. HE is a smaller integration only if the firmware requirement is acceptable.

## Material correction: existing PSF2 iOS linking does not prove device playback

Pinned Play! `04bde0df87ee7c0e2f0151b51bb2cc22c88541da` has an actual PSF1 implementation:

- [PsfLoader.cpp](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/tools/PsfPlayer/Source/PsfLoader.cpp#L38): `Iop::CPsfSubSystem(false)` selects PS1. `LoadPsxRecurse` validates PSF version, loads `_lib`, overlays executable text, and loads `_lib2` onward while preserving PC/SP.
- [PsxBios.cpp](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/Source/psx/PsxBios.cpp#L49): HLE BIOS initialization; `LoadExe` starts at line 130. It checks `PS-X EXE`, initializes PC/GP/SP and copies text to emulated RAM. Its release-build bounds protection is not sufficient for untrusted imports; retain Kog's bounded parser and independently validate before calling it.
- [Iop_PsfSubSystem.cpp](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/tools/PsfPlayer/Source/Iop_PsfSubSystem.cpp#L11): both modes use the same CPU/device loop and render stereo PCM at 44.1 kHz.
- [License.txt](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/License.txt): BSD 2-clause license.

However, [Iop_SubSystem.cpp line 53](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/Source/iop/Iop_SubSystem.cpp#L53) constructs `CGenericMipsExecutor`. The name means generic MIPS executor infrastructure, **not an interpreter**. `BasicBlock.cpp` emits native code through `CMipsJitter`/`CMemoryFunction`. The CodeGen submodule's `src/MemoryFunction.cpp` selects Mach VM allocation/protection on iOS (lines 15-29) and executable permissions for generated code.

[BasicBlock.cpp lines 150-172](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/Source/BasicBlock.cpp#L150) has an alternative `AOT_USE_CACHE` path, but it asserts every block exists in the pre-generated cache and supplies no interpreter fallback. [Upstream iOS CMake lines 73-76](https://github.com/jpd002/Play-/blob/04bde0df87ee7c0e2f0151b51bb2cc22c88541da/tools/PsfPlayer/Source/ui_ios/CMakeLists.txt#L73) explicitly links a generated `PsfBlocks.o` when `USE_AOT_CACHE` is enabled. A fixed cache is not a solution for arbitrary user-imported executable PSF tracks. Kog's current iOS build enables neither that cache nor an interpreter.

Therefore, using Play!'s existing PS1 route is a strong way to reuse the HLE/loader/SPU, but not a ready ordinary-iPhone solution by merely adding the static library. Apple's runtime signed-code restrictions are documented at [Apple Platform Security](https://support.apple.com/en-za/guide/security/sec15bfe098e/web). No iOS device execution was performed in this investigation.

## SFM: narrowly scoped replacement has real evidence

Kog's `native/cog-gme-sfm/gme/Spc_Sfm.cpp` lines 9-19 explicitly grant LGPL-2.1-or-later. Its BML parser and SPC_DSP sources carry compatible LGPL notices. The imported old higan files lack individual grants, and the existing subset contains GPL2 terms/provenance; do not relabel that tree based on a newer upstream license.

higan v095 is pinned at `b0e862613b3c6cfaf3d8088403e144d5da98cd43`. The release identifies `License = "GPLv3"` at [emulator/emulator.hpp line 13](https://github.com/higan-emu/higan/blob/b0e862613b3c6cfaf3d8088403e144d5da98cd43/emulator/emulator.hpp#L13). Its `processor/spc700` and `sfc/smp` have no contrary individual license notices. This is a project-level license declaration, not an independently recovered grant from every contributor; a production vendor import should preserve that provenance and finish the component audit. Current higan has an explicit [GPL-3.0-or-later project license](https://github.com/higan-emu/higan/blob/8f4df010715298455b92cfc0821ea833770a8b5a/LICENSE.txt), with ISC nall/libco support libraries, and is a fallback donor if the old release's license evidence is insufficient.

v095 [SPC700 header](https://github.com/higan-emu/higan/blob/b0e862613b3c6cfaf3d8088403e144d5da98cd43/processor/spc700/spc700.hpp) has the same `op_io`, `op_read`, `op_write`, `op_step` model and register layout as the imported implementation. Its [SMP header](https://github.com/higan-emu/higan/blob/b0e862613b3c6cfaf3d8088403e144d5da98cd43/sfc/smp/smp.hpp) retains the same test-register flags and timer stage state consumed by SFM metadata. Modern higan/ares SMP has changed timing/state layouts, making it a larger SFM snapshot conversion.

SFM is more than SPC playback. In Kog's current implementation, reads of `$F4-$F7` consume the SFM log sequentially, remember the last value per port, and jump to a loop position at end. Metadata restores CPU registers, timer stages, DSP phase, echo history and per-voice internal state (`Spc_Sfm.cpp` from line 239). The replacement must retain those details. v095's original port reads instead synchronize with the emulated main CPU, so that region needs an SFM-specific adapter. Its `op_wait` and TEST clock-stop behavior also need cancellation-aware treatment; blindly transplanting infinite wait loops could hang a worker. Preserve the DSP snapshot representation by retaining the LGPL SPC_DSP implementation where possible. The small unmarked `higan/dsp/dsp.*` wrapper needs its own provenance/reimplementation decision too. In addition, `native/sfm-helper/sfm_helper.cpp` itself declares `GPL-2.0-only` at line 4. Replacing the emulator core does not change that wrapper's license. Original Kog-owned wrapper portions need a verified ownership/provenance basis for compatible relicensing, or a new compatible wrapper must be written; no blanket relabeling of the helper file is justified.

### Prototype actually run

All work is under `/tmp`, with no tracked source edits:

- Script: `/tmp/kog-sfm-prototype.py`.
- Sources/binary/fixture/output: `/tmp/kog-sfm-v095-prototype/`.
- Copied current SFM subset into that directory, replaced the six active SPC700 files (`spc700.cpp`, `spc700.hpp`, `algorithms.cpp`, `instructions.cpp`, `memory.hpp`, `registers.hpp`) from v095. Replaced integer/endian aliases for this little-endian Linux prototype; removed unused debugger/serialization dependencies. No nall or emulator UI dependency was required.
- Built with `g++ -std=c++17 -O2 -fwrapv -DBLARGG_LITTLE_ENDIAN=1` and current SMP/DSP/loader/helper.
- Extracted Kog's exact original generated SFM fixture from `crates/kog-audio/src/sfm.rs::test_sfm_bytes` (SPC program, BRR waveform, metadata; no game data).
- Compared complete helper protocol plus PCM against existing `target/debug/build/kog-audio-8760c0d2247bee6f/out/sfm-helper/bin/kog-sfm-helper` at start frames 0 and 3200. Both comparisons were byte-identical and audio payload was nonzero.

| Start frame | Bytes | SHA256 of complete output (both implementations) |
|---|---:|---|
| 0 | 76955 | `d76199961096f0ec8af5e5a0ea0d871ada1af739fb515803116d30296bab4131` |
| 3200 | 64155 | `f5d3db9467249ed7463359e01c230831fa8435746b6510de0336e5aff5bc8737` |

**Limits:** this proves CPU-source replacement compatibility for one generated fixture and a seek offset. The prototype still retains the old SMP/DSP wrapper and GPL2-only helper wrapper and is not a completed compatible-license migration. It does not prove full opcode/capture fidelity, log loop semantics, ARM64 compilation, iPhone playback, or battery performance.

## SNSF: preserve assembly behavior with a compatible adapter, replace emulation

Kog already has the critical format-specific logic in `native/snsf-helper/snsf_helper.cpp`: `LoadState` at line 229, `mapProgram` at 239, `mapReserved` at 267, and `loadSections` at 295. It assembles libraries with psflib, maps relative ROM offsets with bounds, and overlays SRAM reserved records. The file actually declares `SPDX-License-Identifier: LicenseRef-Snes9x` at line 4, even though its copyright identifies Kog contributors. Kog authorship alone does not make that file GPL3. Extracting original Kog-owned portions requires verifying their provenance and obtaining or exercising the applicable rights to publish them under a compatible license; do not simply copy or relabel the entire helper. Alternatively, implement a new compatible adapter from the SNSF format specification and MIT psflib, preserving the observed bounds and mapping behavior. The resulting ROM/SRAM should be supplied directly to a new core adapter instead of repackaging into a sanitized SNSF for Snes9x.

Inspected ares commit `4cb8d92b441557cb6bcaf133c4cbc7f6819b1122`:

- [LICENSE](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/LICENSE): core ISC grant; additional bundled components have their own notices. Vendor the needed core and audit the actual subset, not the complete desktop application.
- [SNES core source list](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/sfc/CMakeLists.txt): WDC65816, SPC700 and relevant coprocessors, CPU/SMP/DSP, cartridge, PPU, system and scheduling. These CPU cores are interpreters.
- [System API](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/sfc/system/system.cpp#L14): `SuperFamicom::load(Node::System&, name)`, then system power/run; NTSC and PAL profiles.
- [Platform API](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/ares/platform.hpp#L16): supply virtual `pak(Node::Object)` for ROM/RAM and `audio(Node::Audio::Stream)` for samples.
- [DSP output](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/sfc/dsp/dsp.cpp#L17): stereo stream at `apuFrequency / 768`.
- [Cartridge connect](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/sfc/cartridge/cartridge.cpp#L17) reads pak board/region, loads mapper and coprocessors. The existing [Super Famicom analyzer](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/mia/medium/super-famicom.cpp) can inform the ROM-to-pak adapter, but assumes desktop resource/database/firmware infrastructure; it is not a direct SNSF API.

Full SNES timing must continue even if video output is discarded: ripped drivers can use the main CPU, interrupts, PPU and coprocessors. Do not assume audio-only output permits removing all those devices. ares core objects are global, so serialize use/reset per decoder or explicitly make context ownership safe.

The libco AArch64 backend can use statically linked code in `__TEXT,__text` (default `LIBCO_MPROTECT` is disabled; see `libco/settings.h` and `libco/aarch64.c`). It need not generate guest code at runtime; keep that configuration in an iOS port and test actual Apple ABI behavior. Firmware assets are a separate inventory: ordinary SNES playback uses the 64-byte IPL already present in Kog, while some enhancement chips need additional firmware. Do not infer every bundled firmware blob is ISC merely because the core is.

No ares SNSF loader, complete headless build, or playback was verified. This is a concrete integration plan supported by source APIs. A first acceptance test should render Kog's synthetic `test_snsf_rom`, then real representative LoROM/HiROM and coprocessor sets, with miniSNSF library overlays, SRAM, startup, looping, seeking and PCM comparison.

bsnes-plus also has a direct [SNSF loader plugin](https://github.com/devinacker/bsnes-plus/blob/a9789fab9a26859153c2963defe186fcaaa80ca2/snesmusic/snesmusic.cpp#L156), but it is an older mixed-license tree with explicit GPL2-only pieces (e.g. its bitmap font code); it does not establish a cleaner ready-made library for this task. Prefer the existing mapping behavior implemented in a compatible adapter plus a clearly licensed core, with the helper-file licensing issue resolved as described above.

## PSF alternatives and firmware

Highly Experimental is a genuine interpreter-based audio library. The current inspected kode54 GitHub copy `ccffd7b5f21ab8feaed78544b677e7da0ad43ce7` has no readily identified license grant in the inspected tree, so do not silently assume it is GPL3 from another fork. The [myst6re fork](https://github.com/myst6re/highly_experimental/tree/8a045a8619116ce2b4c11387f527cc48636ef7a9) ships `LICENSE.TXT` with GPL3 and credits kode54; final vendor choice should verify source provenance with the intended upstream revision.

Its [Readme.txt](https://github.com/myst6re/highly_experimental/blob/8a045a8619116ce2b4c11387f527cc48636ef7a9/Readme.txt#L6) explicitly says it uses Sony BIOS code, historically stripped from a PS2 BIOS with PS1 compatibility modules. It is not a firmware-free HLE core. `bios_set_image` supplies the transformed image; `psx_init`, `psx_get_state_size(1)`, `psx_clear_state`, `psx_upload_psxexe` and `psx_execute` expose the required playback API in [psx.h](https://github.com/myst6re/highly_experimental/blob/8a045a8619116ce2b4c11387f527cc48636ef7a9/include/highly_experimental/psx.h). A standard raw PS1 BIOS is not automatically the required HE-format BIOS. Shipping a permissively licensed OpenPSF wrapper does not remove this dependency. A firmware importer/converter is real product work; do not bundle unaudited Sony data.

Current ares PS1 has [an explicit CPU interpreter](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/ps1/cpu/cpu.cpp#L12), but [its system loads `bios.rom`](https://github.com/ares-emulator/ares/blob/4cb8d92b441557cb6bcaf133c4cbc7f6819b1122/ares/ps1/system/system.cpp#L91), and there is no verified PSF loader. It would require both PSF integration and a firmware strategy; it is therefore below Play! for firmware-free Kog integration and below HE for an audio-focused firmware-assisted option.

Mednafen/Beetle offers mature interpreter components and GPL-2.0-or-later CPU notices (inspected Beetle revision `5718ab9b829599687671503f11494ca6a0049c57`, `mednafen/psx/cpu.c` lines 5-10). This is compatible with a GPL3 combined project, unlike GPL2-only libupse. However, it is a larger full-console integration with BIOS and PSF rip-compatibility considerations; no firmware-free ready-made Kog route was demonstrated. It may be a source of a compatible interpreter, but transplanting one is a substantial semantic integration, not a simple object-file replacement.

## Integration and acceptance

Implement the adapters in the shared `kog-audio` backend. Its existing
`EmbeddedHelper` transport already runs native renderers on worker threads
with PCM streaming, backpressure, cancellation and error propagation. PSF1
and SNSF can use `psf.rs::spawn_embedded_renderer`; SFM needs the equivalent
embedded runner. The same implementation should serve Qt, TUI, Web, Android
and iOS, preserving each player's separate queue.

Update native CMake targets, `crates/kog-audio/build.rs`, the Android and iOS
native build scripts, iOS linker inputs and desktop packaging together.
Retire each executable helper from runtime packaging only after its replacement
passes the corresponding playback checks. Retain comparison tools separately
while validating the new core.

For each format, validate dependency resolution, bounds and malformed input,
metadata precedence, audible PCM, seeking, loop/fade timing, end of stream,
cancellation and switching tracks. Synthetic fixtures establish specific
behaviors; representative real captures are required for compatibility claims.
Check concurrency explicitly for cores with global state. For iOS, run an
ordinary signed device build launched without a debugger, with local imported
files and no server, and measure decoding speed and sustained playback.

## Verification limits

Only the SFM CPU replacement prototype was built and executed, on Linux.
SNSF and PlayStation replacement recommendations are supported by source
inspection, not completed playback integrations. No iOS device execution or
ARM64 performance measurement was performed in this investigation. This commit
adds research documentation and corrects the existing PSF2 iOS status; it does
not change runtime decoder behavior.

The temporary prototype paths above are local investigation artifacts and are
not included in the repository. The pinned sources, fixture generator and
output hashes identify the comparison. Licensing notes distinguish inspected
grants/declarations from remaining provenance work; the CPU prototype does not
change the licenses of the old SMP or helper wrappers.

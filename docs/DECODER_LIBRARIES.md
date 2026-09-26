# Embedded SFM, SNSF and PSF decoders

All Kog frontends now route these formats through native libraries in
`kog-audio`. Qt, TUI and the web server use the same implementations as
Android and iOS; each player still owns its own queue and playback state.
No SFM, PSF1 or SNSF child executable is installed or launched.

## Implementations

| Format | Decoder | State and execution |
| --- | --- | --- |
| SFM | LGPL Cog SFM/BML/SPC DSP, GPLv3 higan v095 SPC700/SMP, new Kog adapter | Per-instance CPU/timers/DSP/event log; bounded halt and clock-stop rendering; symbols isolated from regular libgme |
| SNSF / miniSNSF | ISC ares `4cb8d92`, new bounded psflib ROM/SRAM adapter | Interpreted CPU/APU with full PPU timing; per-worker core/coroutine state, large devices allocated on the heap; 32 kHz stereo output |
| PSF1 / miniPSF | BSD Play! `04bde0d` PS1 HLE BIOS/SPU and new Kog R3000 interpreter | No Sony BIOS or native-code generation; bounded PS-X EXE and dependency loading; 44.1 kHz stereo output |
| PSF2 / miniPSF2 | Same interpreter with Play! IOP HLE BIOS/SPU2/filesystem | Replaces the former JIT constructor before any executable-memory allocation; bounded filesystem/IRX validation |

The Play! checkout stays immutable: CMake creates patched copies selecting the
interpreter and fixing HLE trampoline load-delay scheduling. The interpreter
implements R3000 integer and CP0 instructions, branch and load delays,
exceptions and Play!'s explicit BIOS/import traps. COP2/GTE is not implemented
by this audio path; files using it fail explicitly. This is not a complete
PlayStation game emulator.

The ares adapter handles ordinary LoROM/HiROM and supported internal
coprocessors such as S-DD1. Boards requiring external enhancement-chip
firmware or a second cartridge fail with an explicit error. Such firmware is
not bundled. Source pins, copyright notices and import modifications are
recorded with each native source directory and in `THIRD_PARTY_NOTICES.md`.

## Shared playback rules

PSF/SNSF retain outer metadata precedence, tagged or default length/fade,
common Rust fade application, exact end of stream and reconstruction/discard
seeking. SFM retains its snapshot metadata, logged port replay and GME fade;
seeking renders and discards the prefix with the same block boundaries as
continuous playback, preserving silence lookahead and the fade envelope.
`EmbeddedHelper` transports PCM on a private socket/pipe, with backpressure.
A Rust worker-local cancellation flag also reaches native code during a long
seek, when no PCM write is available to detect a closed reader.

The legacy libupse/Snes9x wrappers remain historical comparison sources under
their original grants. They are no longer Cargo dependencies or release
assets. The old SFM CMake entry point forwards to the compatible new test
renderer and cannot combine the old GPL2-only wrapper with the GPLv3 core.

## Reproducible focused checks

From the development shell:

```sh
cargo test --locked -p kog-audio --lib sfm_decoder
cargo test --locked -p kog-audio --lib psf_decoder
cargo test --locked -p kog-audio --lib embedded_helper
cmake -S native/sfm-embedded -B /tmp/kog-sfm-tests -DKOG_SFM_BUILD_TESTS=ON
cmake --build /tmp/kog-sfm-tests
ctest --test-dir /tmp/kog-sfm-tests --output-on-failure
```

Native SFM tests cover unterminated BML length bounds, UTF-8 values, port-log
looping/exhaustion, per-instance state, four-bit timer wrap, SLEEP/STOP,
clock-stop and skipped samples. The same tests and generated SFM render
passed with AddressSanitizer and UndefinedBehaviorSanitizer on Linux.
The new renderer's complete protocol and PCM are byte-identical to the
pre-migration renderer for the generated fixture from frame 0:

- `d76199961096f0ec8af5e5a0ea0d871ada1af739fb515803116d30296bab4131`

Seeks to frames 3200, 8000 and 17600 produce exact suffixes of that full
render, including the fade. This fixes the old skip path's difference from
continuous playback.

The Play! native tests compare 100 deterministic 200-operation programs
against the original JIT's registers, HI/LO and RAM, alongside specific
delay/exception/division cases. PSF1, miniPSF, tag-only miniPSF, PAL-tag and
PSF2 generated fixtures render nonzero audio and exact seek suffixes. A Linux
interposer rejecting executable-memory protection changes permits the new
path and rejects the old JIT path. Generated PSF2 PCM matches the prior JIT
renderer byte-for-byte. These establish specific instruction and audio
behaviors, not compatibility with every PSF rip.

SNSF native tests exercise audible generated playback, exact seek, library
and SRAM overlay precedence, malformed bounds and decompression limits,
root-directory containment for parent paths and symlinks,
PAL timing, simultaneous streams, progress while another reader is stalled,
cancellation during seek, and reuse after cancellation. Local external
HiROM and S-DD1 music samples were also rendered; those files are not included
in the repository and are not a comprehensive compatibility corpus.

## Platform verification limits

Linux native checks and all 19 focused shared Rust regression tests passed
(five SFM, thirteen PSF-family, and one transport cancellation test).
All three native library sets also cross-compile for Android
ARM64 using NDK 28.2.13676358. Cross-compilation is not Android device playback.
The iOS build and Xcode linker inputs use the same static library set and no
PSF JIT requirement. An ordinary signed iPhone build, launched outside a
debugger with local imported files, still needs device verification. macOS
and Windows execution and a wider real SFM/PSF corpus also remain unverified
by this change. No Mac restart or device installation is performed here.

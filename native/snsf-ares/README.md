# Embedded SNSF renderer

`kog_snsf_embedded_run(path, start_frame, length_ms, fade_ms, descriptor,
error, error_capacity)` takes ownership of the output descriptor, emits the
existing KOGPSF1 protocol (SNSF format byte, stereo signed 16-bit little-endian
PCM at 32 kHz), closes the writer, and returns zero on success. The caller must
export `bool kog_decoder_cancelled(void)` with a worker-specific cancellation
value. Each invocation runs entirely on its calling worker; multiple workers
have independent ares state. A slow reader cannot lock out another stream.

Libraries installed by CMake:

- `libkog_snsf_embedded.a`
- `libkog_ares_snsf.a`
- `libkog_snsf_psflib.a`

Link zlib, the C++ runtime, threads and the platform dynamic-library support
library where applicable. `PSFLIB_SOURCE` selects the existing psflib directory.
The protocol CLI and native tests are optional and disabled by default:

```sh
cmake -S native/snsf-ares -B build/snsf -DCMAKE_BUILD_TYPE=Release \
  -DKOG_SNSF_BUILD_EXECUTABLE=ON -DKOG_SNSF_BUILD_TESTS=ON
cmake --build build/snsf
python3 native/snsf-ares/test.py build/snsf/kog-snsf-ares
```

The loader validates immutable file buffers before handing them to psflib:
32 MiB/file, 16 MiB decompressed ROM, 1 MiB reserved records, 64 KiB/file tags,
128 opens and 256 MiB aggregate compressed/decompressed input. Dependencies
are resolved canonically and must stay inside the root file directory, including
through symlinks. CRC and record
ranges are checked before ROM/SRAM writes. Mini/library load order follows
psflib; root tags win over nested libraries. `_memory`, `_video`, `_sramfill`,
length/fade and SRAM overlays are supported. Save-state records fail explicitly.

Native tests cover synthetic audible output, exact seek suffix, EOF, ordinary
and numbered libraries, relative ROM offsets, SRAM overlay/fill precedence,
root metadata precedence, malformed records/CRC/ranges, cycles/missing libraries,
a decompression bomb, PAL, simultaneous deterministic streams, an undrained
output pipe, cancellation during seek and worker reuse after cancellation.

Additional local validation used six seconds each from public Chrono Trigger
HiROM and Star Ocean S-DD1 miniSNSF sets from https://snsf.caitsith2.net/.
Both produced nonzero PCM; these copyrighted fixtures are not included here.
This is compatibility evidence for those inputs, not bit-exact parity with
Snes9x or a complete game corpus. Signed iOS/Android device execution, mobile
performance and firmware-dependent chip support have not been verified.

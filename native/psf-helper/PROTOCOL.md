<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Kog PSF helper protocol

`kog-psf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS` opens one
PSF-family file, writes one header to standard output, then writes PCM until the
declared end. Diagnostics go to standard error and any nonzero exit status is a
decode failure. Kog launches a new process with a new `START_FRAME` for every
seek; the helper reconstructs libupse state and discards complete decoded frames
before streaming that exact frame.

All integers and PCM samples are little-endian. The version-1 header is:

| Field | Size |
| --- | ---: |
| Magic `KOGPSF1\0` | 8 bytes |
| Protocol version (`1`) | `u32` |
| xSF format version (`1`; `2` is reserved for the PSF2 milestone) | `u32` |
| Sample rate | `u32` |
| Channel count | `u32` |
| Total frames, including fade | `u64` |
| Main frames, before fade | `u64` |
| Title, artist, game/album, genre, and date byte lengths | five `u32` values |
| The five metadata values in that order | raw bytes of the declared lengths |

Metadata is decoded lossily as UTF-8 and is capped at 64 KiB by the parent.
The remainder of standard output is interleaved signed-16 PCM with the declared
channel count and rate. Version 1 currently emits 44.1 kHz stereo. The parent
converts samples to float and applies one linear fade from `main_frames` through
`total_frames`.

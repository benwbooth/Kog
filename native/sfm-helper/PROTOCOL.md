<!-- SPDX-License-Identifier: GPL-2.0-only -->

# Kog SFM helper protocol

`kog-sfm-helper PATH START_FRAME` validates one SFM file, restores Cog's
portable SPC700/SMP/DSP renderer, discards complete frames through
`START_FRAME`, and writes a header followed by PCM. Diagnostics go to standard
error; a nonzero exit status is a decode failure.

All integers and PCM samples are little-endian. The version-1 header is:

| Field | Size |
| --- | ---: |
| Magic `KOGSFM1\0` | 8 bytes |
| Protocol version (`1`) | `u32` |
| Sample rate (`32000`) | `u32` |
| Channel count (`2`) | `u32` |
| Total frames, including fade | `u64` |
| Main frames, before fade | `u64` |
| System, title, game, author, copyright, and date | six length-prefixed byte strings |
| PCM | interleaved signed-16 stereo through `total frames` |

Each string starts with a `u32` byte length. The parent caps every string at
64 KiB and decodes it lossily as UTF-8. The helper caps the input at 256 MiB,
BML metadata at 4 MiB, playback at twelve hours, and validates state/log
offsets plus restored DSP indices before the legacy core runs. Cog's timing
policy is preserved: a missing length becomes 150 seconds, a declared loop is
played twice, and a missing fade becomes eight seconds.

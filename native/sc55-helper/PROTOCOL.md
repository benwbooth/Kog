# Kog Nuked SC-55 helper protocol

`kog-sc55-helper <schedule> <ROM-directory> <start-frame> [ROM-set]` reads a
Kog-generated MIDI schedule and writes one little-endian header followed by
interleaved signed 16-bit stereo PCM.

The schedule format is:

| Field | Type |
| --- | --- |
| magic (`KOGSCM1` plus NUL) | 8 bytes |
| protocol version (`1`) | `u32` |
| total duration | `u64` nanoseconds |
| event count | `u32` |
| each event's absolute time | `u64` nanoseconds |
| event byte length | `u32` |
| raw UART bytes | byte array |

The response format is:

| Field | Type |
| --- | --- |
| magic (`KOGSC551`) | 8 bytes |
| protocol version (`1`) | `u32` |
| native sample rate | `u32` |
| channels (`2`) | `u32` |
| total frames | `u64` |
| requested start frame | `u64` |
| ROM-model name length | `u32` |
| ROM-model name | UTF-8 bytes |
| PCM | little-endian `i16` frames |

Rust parses Standard MIDI and RIFF RMID with Midly, merges tracks in stable
source order, schedules tempo or SMPTE timing in nanoseconds, and serializes
channel messages and SysEx. The helper bounds the schedule to 256 MiB, two
million events, one MiB per UART event, and 24 hours. It uses upstream's ROM
hash detector, sends a GS reset, performs the same 24-million-step startup used
by the upstream renderer, then streams deterministic native-rate PCM. Seeking
starts a fresh helper and suppresses frames before the requested position.

The helper is a separate optional program because Nuked SC-55's original MAME
license includes non-commercial restrictions incompatible with Kog's GPL main
executable. Roland firmware and waveform ROMs are never bundled.

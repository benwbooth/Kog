# Kog Syntrax helper protocol

`kog-syntrax-helper <file> <zero-based-subsong> <start-frame>` writes one
little-endian header followed by interleaved signed 16-bit stereo PCM:

| Field | Type |
| --- | --- |
| magic (`KOGJXS1\0`) | 8 bytes |
| protocol version (`1`) | `u32` |
| sample rate | `u32` |
| channels | `u32` |
| total frames, including fade | `u64` |
| main frames, before fade | `u64` |
| subsong count | `u32` |
| selected zero-based subsong | `u32` |
| title byte length | `u32` |
| title | raw JXS title bytes |
| PCM | little-endian `i16` frames |

The helper scans the packed JXS structure with checked arithmetic before the
legacy parser runs, limits files to 256 MiB, renders two loops like Cog, adds
Cog's eight-second fade only for looping songs, and caps native duration scans
at the renderer's 30-minute limit. Seeking starts a fresh helper and advances
the same deterministic replayer path Cog uses.

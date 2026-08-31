# Cog format parity matrix

Reference: `losnoco/Cog` commit
`c17be85654a64170c86bb8bbb4b59fd7b6795722` (2026-08-16). This inventory is
derived from Cog's decoder and container plugins. `Done` means a backend and
its parity corpus pass the fidelity gates in `ARCHITECTURE.md`; `Partial` means
real code exists but the full family has not passed those gates; `Not started`
is required work, not a claim of support.

## Conventional audio

| State | Cog family | Extensions / source behavior | Kog backend |
| --- | --- | --- | --- |
| Partial | CoreAudio/FFmpeg overlap | AAC/ADTS, AIFF, ALAC, CAF, FLAC, MP1/2/3, MP4/M4A, Ogg/Vorbis, Opus, WAV, Matroska/WebM combinations accepted by Symphonia. WAV and FLAC fixtures have passed the current probe/play/advance smoke; the family corpus is incomplete. | `rodio-symphonia` |
| Not started | FFmpeg | WMA, ASF, TAK, APE, AC-3, DTS/DTS-HD, TTA, TwinVQ, RealAudio, DSD/DSF/DFF/DSDIFF/WSd and unsupported container/codec combinations | FFmpeg adapter |
| Not started | WavPack | WV, WVP including correction files | libwavpack |
| Not started | Musepack | MPC | libmpcdec or FFmpeg |
| Not started | Shorten | SHN | libshn/FFmpeg |
| Not started | APL | APL link files and referenced source ranges | Kog APL container + selected PCM backend |
| Not started | HTTP/HLS | remote HTTP sources, M3U8/HLS | network source + FFmpeg |

## Tracker and computer-music formats

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Not started | OpenMPT | Runtime-enumerated libopenmpt module and archive extensions | libopenmpt |
| Not started | DUMB/BASSMODS/modplay/playptmod | IT/ITZ, XM/XMZ, S3M/S3Z, MOD/MDZ, STM/STZ, PTM, MTM, 669, PSM, AM, J2B, DSM, AMF, OKT/OKTA, UMX, MO3, STK, M15, FST | libopenmpt first; compatibility fallbacks only where corpus proves a difference |
| Not started | HivelyTracker | HVL, AHX and subsongs | hivelytracker |
| Not started | Organya | ORG plus bundled wavetable/PXT instruments | organya renderer |
| Not started | Syntrax | JXS and subsongs | libsyntrax |

## Chiptune and game audio

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Partial | Game Music Emu | A real PCM backend is wired for AY, GBS, HES, KSS, NSF/NSFE, SAP, and SPC, with metadata, seek, M3U companions, multitrack expansion where applicable, and Cog-compatible default loop/fade timing. The official NSF fixture passes probe, non-silent decode, seek, metadata, duration, and live Qt playback tests; the other routed formats still need corpus validation. SFM and SGC are not routed because upstream libGME 0.6.5 does not implement Cog's fork-specific support. Configurable synthesis policy also remains. | `game-music-emu` (upstream libGME 0.6.5) |
| Not started | HighlyComplete | PSF/miniPSF, PSF2, SSF, DSF, QSF, GSF, NCSF, 2SF, USF, SNSF and mini variants; dependency libraries | Cog's GPL-compatible emulator cores behind Rust adapters |
| Partial | libvgm | VGM/VGZ, S98, DRO, and GYM route to Cog's exact pinned libvgm revision with real 24-bit PCM converted to the shared float stream, common tags, native format/version, seek, Cog-compatible loop/fade/end-silence policy, and optional sibling YRW801 ROM resolution. A deterministic SN76489 VGM 1.50 fixture passes duration, codec, non-silent decode, seek, and live Qt playback. VGZ, S98, DRO, GYM, loop/fade completion, metadata-bearing files, and YRW801-dependent playback still need corpus gates; libvgm subsongs are not implemented. | `libvgm` at `867223e7c33d63de115d1ab955f784c44f19040a` |
| Not started | vgmstream | Runtime-enumerated game-stream extensions; companion files, TXTP, subsongs, loops and fades | libvgmstream |
| Not started | SID | SID, MUS; ROM selection, subtunes, song-length database | libsidplayfp |
| Not started | AdPlug | Runtime-enumerated AdLib/OPL formats and subsongs | AdPlug + Nuked OPL3 |

## MIDI

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Partial | MIDI containers | MID, MIDI, and KAR Standard MIDI Files (formats 0 and 1) plus RIFF RMID are parsed and rendered; SMF format 2, MIDS, MDS, HMI, HMP, HMQ, MUS, XMI, LDS, XMF, and MXMF remain missing | `midi-rustysynth-sf2`, `midi-opl3windows` |
| Partial | SoundFont synthesis | Configurable and persisted SF2 selection, 48 kHz stereo rendering, duration, seek, and end-of-stream behavior are implemented. SF3, per-file flavor selection, and synthesis/effect controls remain missing. | RustySynth |
| Partial | OPL synthesis | Cog's OPL3Windows General MIDI timbre table, 18-voice engine, and Nuked OPL3 1.7.1 core render format-0/1 SMF and RMID with tempo/SMPTE timing, controls, pitch bend, drums, duration, and seek. Cog's DMX banks and AdPlug input formats remain separate work. | `midi-opl3windows` |
| Not started | Roland emulation | MT-32/CM-32L and SC-55 behavior exposed by Cog preferences | Munt and Nuked SC-55 where legally redistributable |

## Containers, playlists, and metadata

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Not started | CueSheet | CUE plus embedded cue sheets in OGG/Opus/FLAC/WV/MP3; per-track ranges | Rust cue parser + delegated decoder |
| Not started | Playlists | M3U/M3U8, PLS; relative paths and URLs | Rust playlist containers |
| Not started | Archives | ZIP, RAR, 7Z, RSN, VGM7Z, GZ; nested source/companion resolution | libarchive/sevenz-rust adapter |
| Partial | Tags | Cog's TagLib metadata read/write surface and album art. Common read-only fields are wired through Lofty; editing and artwork are missing. | Lofty plus TagLib fallback |

## Cross-cutting Cog behavior still required

- decoder priority and content probing rather than extension-only selection;
- remote, archive, and local source abstractions;
- stable fragment identifiers and persisted subsong identity across every
  remaining multitrack backend;
- configurable loop counts, fade length, and indefinite playback;
- ReplayGain, gapless playback, resampling, equalizer, pitch/tempo, and output
  device selection;
- metadata editing, album art, ratings, play counts, cue metadata, and library
  persistence;
- Last.fm, notifications, remote control, spectrum/visualization, lyrics, and
  media-key integration.

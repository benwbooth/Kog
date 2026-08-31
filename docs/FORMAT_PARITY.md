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
| Partial | FFmpeg | Cog's WMA, ASF, TAK, M4R, M2A/MPA, APE, AC-3, DTS/DTS-HD, TTA, TwinVQ, RealAudio, WebA, DSF/DFF/DSDIFF/WSd, and overlapping conventional extensions route through a real libavformat/libavcodec/libswresample backend. It keeps native rate/channel layout, converts to interleaved float PCM, reads common metadata/properties, and seeks through the demuxer. The pinned Nix shell supplies FFmpeg 9.0.1 with GPL and version-3 components disabled and an LGPL-2.1-or-later license result. A deterministic four-frame AC-3 fixture passes routing, duration, bitrate, codec, audible PCM, seek, priority, and EOS tests. The wider format/metadata corpus, exact seek across every demuxer, DSD policy, gapless trim, chapters/subtracks, artwork, content probing, remote custom I/O, and Windows/macOS build gates remain. | system FFmpeg via pkg-config; pinned Nix configuration tested at 9.0.1 |
| Partial | WavPack | WV and WVP now route through FFmpeg. A WavPack/correction-file corpus, explicit sibling WVC resolution, lossless/float/DSD properties, and comparison with Cog's dedicated libwavpack path remain. | FFmpeg baseline; dedicated libwavpack only if parity requires it |
| Partial | Musepack | MPC now routes through FFmpeg; SV7/SV8 corpus, seek, metadata, and behavioral comparison with Cog's libmpcdec plugin remain. | FFmpeg baseline; libmpcdec fallback if required |
| Partial | Shorten | SHN now routes through FFmpeg; a redistributable corpus and comparison with Cog's dedicated Shorten decoder remain. | FFmpeg baseline; dedicated fallback if required |
| Not started | APL | APL link files and referenced source ranges | Kog APL container + selected PCM backend |
| Not started | HTTP/HLS | remote HTTP sources, M3U8/HLS | network source + FFmpeg |

## Tracker and computer-music formats

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Partial | OpenMPT | Cog's exact libopenmpt 0.8.7 release is pinned and built from source. The backend verifies and advertises the 68 native extensions returned by that build, expands zero-based subsongs, reads common metadata and the long native format name, renders 44.1 kHz stereo float PCM with Cog's gain, stereo-separation, 8-tap interpolation, volume-ramping, synchronous-seek, normal-repeat, and Amiga-emulation defaults, and seeks by time. A generated ProTracker MOD passes format routing, metadata, duration, audible PCM, and seek tests. The full 68-format corpus, repeat-one, configurable sample rate/interpolation, and Cog's MDZ/MDR/S3Z/XMZ/ITZ/MPTMZ archive aliases remain. | `libopenmpt` 0.8.7 at `11363ff11ba021b1cf1533da17d9fdf20c8d883c` |
| Partial | DUMB/BASSMODS/modplay/playptmod | Native IT, XM, S3M, MOD, STM, PTM, MTM, 669, PSM, J2B, DSM, AMF, OKT, UMX, MO3, STK, M15, and FST routing now overlaps the OpenMPT backend. Compressed aliases and any behavioral differences still require corpus-driven compatibility fallbacks. | libopenmpt first; compatibility fallbacks only where corpus proves a difference |
| Partial | HivelyTracker | AHX and HVL route through the official HivelyTracker 1.9 replayer at `f393ca7`. Kog reads the native title, expands the main song and declared subsongs, scans two loops with a two-hour safety bound, applies Cog's default eight-second fade, renders stereo float PCM at 44.1 kHz, and supports restart-and-skip seeking. Official upstream AHX/HVL songs and a deterministic two-subsong HVL derivative pass routing, duration, audible PCM, and seek tests. A wider corpus, repeat-one, configurable rate/fade, and comparison with Cog's modified blip-buffer output remain. | `hivelytracker` 1.9 plus upstream fixes at `f393ca7c6416f00bcb574b334a7e8b57dcb19eb2` |
| Partial | Organya | Org-02 and Org-03 route through `orgorg` 0.2.1, with 44.1 kHz stereo float PCM, Cog's default two-loop/eight-second-fade timing, loop metadata, and restart-and-skip seek. Kog accepts a user-supplied `soundbank.wdb` or extracted `wavetable.dat`/`drums.dat` pair and deliberately does not redistribute Cave Story's synthesis assets. A deterministic Org-02 song and synthetic bank pass routing, duration, audible PCM, seek, and end-of-stream tests. An independently redistributable wider song corpus, percussion comparison against Cog, Cog's Lanczos behavior, repeat-one, configurable sample rate/loop/fade, and a soundbank picker remain. | `orgorg` 0.2.1 |
| Blocked | Syntrax | JXS and subsongs. Cog's only located renderer is GPL-3.0-only, which cannot be linked into Kog's GPL-2.0-only program; no compatible maintained implementation was found. This needs either an independently implemented compatible core or an explicit whole-project license decision. | Pending licensing/clean-room decision |

## Chiptune and game audio

| State | Cog family | Extensions / behavior | Kog backend |
| --- | --- | --- | --- |
| Partial | Game Music Emu | A real PCM backend is wired for AY, GBS, HES, KSS, NSF/NSFE, SAP, and SPC, with metadata, seek, M3U companions, multitrack expansion where applicable, and Cog-compatible default loop/fade timing. The official NSF fixture passes probe, non-silent decode, seek, metadata, duration, and live Qt playback tests; the other routed formats still need corpus validation. SFM and SGC are not routed because upstream libGME 0.6.5 does not implement Cog's fork-specific support. Configurable synthesis policy also remains. | `game-music-emu` (upstream libGME 0.6.5) |
| Not started | HighlyComplete | PSF/miniPSF, PSF2, SSF, DSF, QSF, GSF, NCSF, 2SF, USF, SNSF and mini variants; dependency libraries | Cog's GPL-compatible emulator cores behind Rust adapters |
| Partial | libvgm | VGM/VGZ, S98, DRO, and GYM route to Cog's exact pinned libvgm revision with real 24-bit PCM converted to the shared float stream, common tags, native format/version, seek, Cog-compatible loop/fade/end-silence policy, and optional sibling YRW801 ROM resolution. A deterministic SN76489 VGM 1.50 fixture passes duration, codec, non-silent decode, seek, and live Qt playback. VGZ, S98, DRO, GYM, loop/fade completion, metadata-bearing files, and YRW801-dependent playback still need corpus gates; libvgm subsongs are not implemented. | `libvgm` at `867223e7c33d63de115d1ab955f784c44f19040a` |
| Partial | vgmstream | Upstream r2117 is pinned at `05dbda9b` and registered last using its runtime-enumerated 700-plus specialist extensions, while its common extension table remains unclaimed. The backend uses the public API for filesystem and companion-file reopening, TXTP, zero-based subsong expansion, Cog's default two-loop/eight-second-fade timing, up-to-six-channel float PCM, native sample-frame seek, codec/bitrate properties, and sibling `!tags.m3u` title, artist, album, date, and track fields. A generated PlayStation VAG passes routing, metadata, duration, audible PCM, seek, and exact end-of-stream tests. The portable baseline currently enables native codecs and built-in G.722.1; FFmpeg, Vorbis, mpg123, G.719, ATRAC9, CELT, and Speex-backed streams plus a broad real-format corpus, repeat-one, configurable loop/fade, and more companion/subsong fixtures remain. | `vgmstream` r2117 at `05dbda9b930b8d174f03387fb626d97d827d0647` |
| Partial | SID | Cog's exact libsidplayfp `519d1201` revision and reSIDfp core are pinned and built from source. ROM-free PSID files route through a real 44.1 kHz signed-16-to-float source with zero-based subtune expansion, title/artist/release metadata, mono or multi-SID stereo output, Cog's 150-second default plus eight-second fade, stereo-width transform, and 32x accelerated seek with an exact residual. A generated two-subtune PSID passes routing, metadata, audible PCM, seek, ROM-policy, and exact end-of-stream tests. Kog does not redistribute C64 ROMs: RSID and BASIC-compatible tunes fail explicitly until user ROM selection exists. Raw MUS/MUS+STR routing, the song-length database, repeat-one, configurable synthesis rate/filter/fade, user-ROM configuration, and a broad PSID corpus remain. | libsidplayfp 2.4.0a at `519d1201efcc6c97f7cc3506947875d21a9bd195` + reSIDfp |
| Partial | AdPlug | Cog's exact AdPlug `4e0141ab` and libbinio `e2f8d50c` revisions are pinned and built from source. The backend advertises the 51 unique extensions enumerated by that build, expands zero-based subsongs, reads native type/title/author metadata, calculates duration, renders 44.1 kHz stereo through its bundled Nuked OPL3 core, and performs exact restart-and-render seeking. Registry order preserves the MIDI, libvgm, and OpenMPT routes for shared MID, VGM/VGZ/DRO, and S3M extensions. Upstream's `2.CMF` fixture passes routing, duration, audible PCM, seek, and exact end-of-stream tests. Cog's AdPlug database, its behavioral effects, content probing for ambiguous extensions such as MUS, configurable synthesis rate, repeat-one, and a broad 51-format corpus remain. | AdPlug 2.3.4-beta at `4e0141ab41ac4ebf388b765d669eb656376d04fd` + libbinio at `e2f8d50c53102c618d675c3310e09a0e0bdf49cd` + bundled Nuked OPL3 |

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

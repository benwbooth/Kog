# Kog

Kog is a new cross-platform local music player written in Rust with a Qt Quick
interface connected through CXX-Qt. Its target is feature parity with
[Cog](https://github.com/losnoco/Cog), including a faithful recreation of Cog's
desktop interface and broad playback support for conventional audio, MIDI,
tracker modules, chiptunes, and game-audio formats.

The current milestone contains a working Cog-style main window, filesystem
browser, metadata playlist, search, inspector and mini-player windows, and a
real conventional-audio playback path with pause, stop, seek, volume, and
automatic track advance. It also renders format-0/1 MID, MIDI, KAR, and RIFF
RMID files through either a validated, persisted SF2 SoundFont using RustySynth
or Cog's OPL3Windows synthesizer and its Nuked OPL3 core, with duration probing
and seeking. The first specialist backend is also live: AY, GBS, HES, KSS,
NSF/NSFE, SAP, and SPC route to pinned Game Music Emu 0.6.5 code with
multitrack expansion, companion M3U metadata, Cog's loop/fade policy, and seek.
The NSF path has passed the current real-PCM and live-UI gates; the other GME
formats still need corpus coverage. VGM/VGZ, S98, DRO, and GYM now route to the
same libvgm revision used by Cog, with real PCM, metadata, seek, loop/fade
policy, and optional YRW801 ROM lookup. The generated VGM path has passed the
current real-PCM, seek, and live-UI gates; the rest of the libvgm family still
needs corpus coverage. Cog's exact libopenmpt 0.8.7 release is now statically
built with its bundled miniz, minimp3, and stb_vorbis decoders. Its 68 native
tracker extensions route through a real stereo-float source with runtime
extension verification, subsongs, metadata, duration, Cog-compatible render
settings, and seek. A deterministic ProTracker MOD passes the current PCM and
seek gates; compressed module aliases and the wider tracker corpus remain.
AHX and HVL now route through the official HivelyTracker 1.9 replayer with
title metadata, subsong expansion, two-loop duration scanning, Cog's default
eight-second fade, real stereo PCM, and seek. Official AHX/HVL songs plus a
deterministic two-subsong derivative pass the current routing, PCM, and seek
gates. Org-02 and Org-03 Organya files now route through the MIT-licensed
`orgorg` renderer with real stereo PCM, loop/fade timing, and seek. Kog does not
redistribute Cave Story's synthesis assets; a deterministic original-format
fixture and synthetic bank gate the backend. SFM, SGC, and the other chiptune
families remain explicitly unclaimed. The pinned upstream vgmstream r2117 core
now supplies a lowest-priority game-stream backend with its runtime-enumerated
700-plus specialist extensions, companion-file and TXTP reopening, subsongs,
loop/fade timing, `!tags.m3u` metadata, multichannel float PCM, and seek. A
generated PlayStation VAG passes routing, metadata, audible PCM, seek, and
exact end-of-stream gates. The first portable build deliberately enables
vgmstream's native codecs plus built-in G.722.1; formats requiring its optional
FFmpeg, Vorbis, mpg123, G.719, ATRAC9, CELT, or Speex dependencies remain
explicitly partial. Cog's exact AdPlug revision and matching libbinio pin now
add 51 runtime-enumerated AdLib/OPL extensions, native subsong expansion,
metadata, duration, 44.1 kHz stereo output through the bundled Nuked OPL3
core, and exact restart-and-render seeking. AdPlug's upstream `2.CMF` fixture
passes routing, audible PCM, seek, and end-of-stream gates; Cog's AdPlug
database and the wider 51-format corpus remain. Cog's exact libsidplayfp
revision now handles ROM-free PSID files through reSIDfp, including subtune
expansion, title/artist/release metadata, mono or multi-SID stereo output,
Cog's 150-second plus eight-second-fade default, and accelerated seek. A
generated two-subtune PSID passes routing, audible PCM, metadata, seek, and
exact end-of-stream gates. Kog deliberately does not redistribute C64 ROMs;
RSID/BASIC tunes report that user ROM selection is still required, and raw
MUS routing and the song-length database remain parity work. A system FFmpeg
adapter supplies the broad conventional fallback used for Cog's AC-3,
DTS, WMA/ASF, TAK, APE, TTA, TwinVQ, RealAudio, DSD-container, WavPack,
Musepack, and Shorten families. It preserves the decoded channel layout and
sample rate, converts planar or packed native samples to interleaved float
PCM, reports common metadata and stream properties, and seeks through the
native demuxer. A generated AC-3 stream passes routing, duration, audible PCM,
seek, priority, and end-of-stream gates. The pinned Nix shell deliberately
builds a conservative FFmpeg configuration without GPL-only or version-3
components, yielding an LGPL-2.1-or-later library compatible with Kog's
GPL-3.0-or-later license; the wider format corpus and exact-seek validation
remain.

NCSF and minincsf now route through the official SSEQPlayer and psflib
projects, including relative `_lib` dependency resolution, common PSF tags,
Cog's 150-second/eight-second defaults, tagged length/fade behavior, 44.1 kHz
stereo PCM, and deterministic seek. A fully synthetic SDAT/PSF pair gates
audible rendering, exact duration/end behavior, metadata, malformed input, and
mini-library loading; a broad independently redistributable NCSF corpus
remains.

GSF and minigsf now route through Cog's exact pinned mGBA revision and the
same psflib parser, including relative `_lib` dependency resolution, common
PSF tags, Cog's 150-second/eight-second defaults, tagged length/fade behavior,
32,768 Hz stereo PCM, and deterministic seek. Tests construct an original
tiny ARM program that drives the emulated GBA PSG and wrap it in generated
PSF files; no Nintendo BIOS, logo, or game data is included. The fixture gates
audible rendering, exact duration/end behavior, metadata, malformed input,
mini-library loading, and archive companion lookup. A broad independently
redistributable GSF corpus and direct comparison with Cog remain.

Monkey's Audio Image Link (`.apl`) files now accept the original CRLF or
portable LF header, resolve their local image path with Cog-compatible relative
Windows-path handling, and play only the declared start/finish PCM-frame range
through the FFmpeg backend. Duration, relative seek, the first selected sample,
audible PCM, and exact end-of-stream are gated with a generated image/link pair.
URL image references, one-bit DSD block scaling, and a real Monkey's Audio corpus
remain parity work.

External `.cue` files now expand into playlist tracks and delegate their audio
images to the bounded FFmpeg source. The parser handles multiple `FILE` blocks,
Cog's persistent metadata state, 75 Hz CD-frame or raw-sample indexes, UTF-8,
BOM-marked UTF-16, Windows-1252 fallback, and ReplayGain fields. Ogg, Opus,
FLAC, WavPack, and MP3 retain their normal decoder routes unless a content probe
finds an embedded `CUESHEET` tag. Generated WAV/CUE and tagged-MP3 fixtures gate
metadata, routing priority, multi-file boundaries, duration, exact first/last
samples, relative seek, audible PCM, and end-of-stream. Remote image URLs,
one-bit DSD frame scaling, ReplayGain application, stable track-number fragments,
and broad real-world cue corpora remain parity work.

M3U, M3U8, and PLS files now expand into the playlist in file order. M3U keeps
Cog's comment/`EXTINF` behavior, while PLS consumes only `File…=` entries.
Relative POSIX and Windows paths, local `file://` URLs, trailing numeric subsong
fragments, nested playlists, Cog's UTF-8 → GB18030 → Windows-1251 → Latin-1
decode order, classic-Mac/Windows line endings, cycle detection, and a nesting
safety limit are implemented. Missing, unsupported, or remote entries remain
visible as UI warnings while valid local tracks are retained. Network sources,
HLS, playlist writing, and non-numeric fragment schemes remain parity work.

ZIP, RAR, 7Z, RSN, VGM7Z, and raw GZ files now expand through the system
libarchive library before decoder selection. Kog preserves physical archive
order and stable `archive :: entry` identities while extracting a safe
temporary companion tree, so formats such as APL can reopen sibling audio.
Traversal paths, duplicate destinations, links/devices, oversized entries,
and oversized archives are rejected or surfaced as warnings. Deterministic
ZIP and GZ fixtures pass end-to-end WAV playback; real 7Z and RAR5 fixtures
gate extraction; ZIP-gated APL playback proves relative image lookup; and
archived NCSF and GSF mini/library pairs prove PSF dependency lookup.
Encrypted and multipart archives, nested archives, broad RAR/7Z corpora, and
Windows/macOS runtime gates remain parity work.

## Project direction

- Treat Cog as the behavioral and interface reference.
- Support Linux, Windows, and macOS as first-class targets.
- Preserve subsongs, loop points, fades, companion files, metadata, seeking,
  and decoder-selection behavior rather than merely opening each extension.
- Use maintained native or upstream decoding libraries behind a shared Kog
  decoder contract. Feature parity does not require line-by-line ports of
  Cog's Objective-C plugins.
- Keep Qt at the presentation boundary. Playback, library management, decoder
  orchestration, and policy belong in the Rust core.
- Verify supported formats against a versioned playback corpus.

Likely decoder families include FFmpeg, libopenmpt, Game Music Emu, vgmstream,
libvgm, libsidplayfp, AdPlug, a SoundFont synthesizer, and an OPL3 MIDI
synthesizer. Dependencies and redistributable assets are selected under the
project's documented license policy. Non-commercial decoders are permitted
for this non-commercial project when their own terms permit redistribution,
but they retain those terms and must be kept outside the GPL application
binary when the licenses are incompatible.

See [the architecture](docs/ARCHITECTURE.md),
[UI parity matrix](docs/UI_PARITY.md), and
[format parity matrix](docs/FORMAT_PARITY.md) for the executable backend
contract, fidelity gates, and the complete Cog-derived worklist. The
[license policy](docs/LICENSING.md) defines the boundary for GPLv3 and
separately licensed optional decoder helpers.

## Development

On NixOS or another system with Nix installed:

```sh
git clone --recurse-submodules https://github.com/benwbooth/Kog.git
cd Kog
nix develop
cargo run
```

For an existing checkout, initialize native sources with
`git submodule update --init --recursive` before building.

Choose RustySynth or OPL3Windows under **Edit → Preferences → MIDI**. RustySynth
requires an SF2 bank; Kog also accepts `KOG_SOUNDFONT=/path/to/bank.sf2` and
`KOG_MIDI_ENGINE=rustysynth-sf2|opl3windows` for isolated testing and packaged
deployments.

Organya needs a user-owned synthesis bank. Put `soundbank.wdb` or the
`wavetable.dat` and `drums.dat` pair beside the `.org` file, put them in Kog's
platform `organya` configuration/data directory, or set
`KOG_ORGANYA_SOUNDBANK` to the bank file or containing directory. The
MIT-licensed [orgorg player](https://github.com/kpqi5858/orgorg/tree/main/orgorg-player)
can extract `wavetable.dat` and `drums.dat` from the original freeware
`Doukutsu.exe` without requiring Kog to redistribute those assets.

The direct Cargo build requires Rust, C and C++ compilers, CMake, `pkg-config`,
FFmpeg development libraries (`libavformat`, `libavcodec`, `libavutil`, and
`libswresample`), zlib, libarchive 3.2 or newer, and Qt 6 with Qt Quick and Qt
Quick Controls. FFmpeg must be built under terms compatible with
GPL-3.0-or-later. Kog's Nix shell provides the known-compatible conservative
configuration used by the regression gates.

## License

Kog-authored code is licensed under GPL-3.0-or-later. Third-party decoder
libraries and assets retain their own licenses; bundled test-fixture
attribution is recorded in [the third-party notices](THIRD_PARTY_NOTICES.md),
and optional non-commercial components follow the
[license policy](docs/LICENSING.md).

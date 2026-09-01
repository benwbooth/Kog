# Kog

Kog is a new cross-platform local music player written in Rust with a Qt Quick
interface connected through CXX-Qt. Its target is feature parity with
[Cog](https://github.com/losnoco/Cog), including a faithful recreation of Cog's
desktop interface and broad playback support for conventional audio, MIDI,
tracker modules, chiptunes, and game-audio formats.

The current milestone contains a working Cog-style main window with a
cross-platform hamburger menu, a lazy expandable filesystem tree, metadata
playlist, search, inspector, lyrics, equalizer, mini-player, and multi-track
tag-editor windows. The editor preserves mixed fields unless changed, writes
common fields plus lyrics and embedded cover art to direct local files, and
uses a native artwork picker. Kog uses platform-native file/folder dialogs and
inherits the active Qt palette and control style, including KDE's
desktop/Breeze style when available. The milestone also
has a real conventional-audio playback path with pause, stop, seek, volume,
automatic track advance, and native output-device enumeration. Kog can follow
the system default or persist a specific output by stable backend ID and name,
recover from backend ID changes, refresh the device list, and switch a running
or paused track while retaining its position. Cog's 31-band graphic equalizer
is live across the shared output path for every decoder, with the original 22 presets and exact
10-to-31-band interpolation, preamp leveling, genre tracking, right-drag curve
editing, strict persistence, and live coefficient changes. It also renders
format-0/1 MID, MIDI, KAR, and RIFF RMID files, plus format-2 SMF files as
independent named playlist subsongs. Cog's pinned SpessaSynth Core C parser
also converts DirectMusic MIDS/MDS, Loudness LDS, and XMF/MXMF containers to
bounded Standard MIDI in-process. Those streams play through either a
validated, persisted SF2 SoundFont using RustySynth
or Cog's OPL3Windows synthesizer and its Nuked OPL3 core, with duration probing
and seeking. A fourth in-process route statically links Munt 2.8.2/libmt32emu
for MT-32 and CM-32L synthesis. It detects a compatible control/PCM ROM pair
from a user-selected directory, preserves channel messages and fragmented
SysEx, renders 48 kHz stereo PCM, and seeks by deterministic reconstruction.
No Munt program needs to be installed or launched. An optional third route uses
the maintained Nuked SC-55 0.6.1 backend in an automatically built, bundled
companion process, detects supported
Roland models from user-owned ROM hashes, sends complete MIDI/SysEx byte
streams, and supports deterministic seek. Users do not install Nuked SC-55 or
any of its frontends separately. Kog does not include Roland ROMs, and the SC-55 path still needs a
real-ROM PCM/corpus gate. Legacy HMI, HMP/HMQ, DMX MUS, and Miles XMI files route through
the maintained libADLMIDI library and its Nuked OPL3 renderer, with native
subsong expansion, metadata, duration, and seeking. Kog builds libADLMIDI with
its explicitly cleared embedded-bank database and excludes its grey-zone bank
set. The first specialist backend is also live: AY, GBS, HES, KSS,
NSF/NSFE, SAP, and SPC route to pinned Game Music Emu 0.6.5 code with
multitrack expansion, companion M3U metadata, Cog's loop/fade policy, and seek.
The NSF path has passed the current real-PCM and live-UI gates; the other GME
formats still need corpus coverage. Cog's fork-specific SFM renderer is built
from a minimal pinned portable source subset in a bundled GPL-2.0-only helper,
with native metadata, 32 kHz stereo PCM, loop/fade timing, bounded input, and
restart-and-render seek. A fully generated SFM state gates routing, metadata,
audible PCM, seek, fade, and exact end-of-stream behavior. VGM/VGZ, S98, DRO,
and GYM now route to the
same libvgm revision used by Cog, with real PCM, metadata, seek, loop/fade
policy, and optional YRW801 ROM lookup. The generated VGM path has passed the
current real-PCM, seek, and live-UI gates; the rest of the libvgm family still
needs corpus coverage. Cog's exact libopenmpt 0.8.7 release is now statically
built with its bundled miniz, minimp3, and stb_vorbis decoders. Its 68 native
tracker extensions route through a real stereo-float source with runtime
extension verification, subsongs, metadata, duration, Cog-compatible render
settings, and seek. A deterministic ProTracker MOD passes the current PCM and
seek gates. Cog's MDZ, MDR, S3Z, XMZ, ITZ, and MPTMZ compressed-module aliases
are recognized by their outer suffix, safely extracted through the bounded
archive layer, and decoded by the same libopenmpt source while retaining the
outer-file identity. A ZIP-wrapped generated module gates every alias through
routing, extraction, metadata, audible PCM, and seek; the wider tracker corpus
remains.
AHX and HVL now route through the official HivelyTracker 1.9 replayer with
title metadata, subsong expansion, two-loop duration scanning, Cog's default
eight-second fade, real stereo PCM, and seek. Official AHX/HVL songs plus a
deterministic two-subsong derivative pass the current routing, PCM, and seek
gates. JXS now routes through the canonical portable `syntrax-c` library at
the same revision embedded by Cog, not through translated Objective-C. A
separate fault-containment helper validates the legacy packed structure,
expands subsongs, reports native titles, renders Cog's cubic 44.1 kHz stereo
path with two-loop/eight-second-fade timing, and supports deterministic seek.
A generated two-subsong JXS song gates routing, PCM, metadata, malformed input,
fade, seek, and exact end-of-stream behavior. Org-02 and Org-03 Organya files
now route through the MIT-licensed
`orgorg` renderer with real stereo PCM, loop/fade timing, and seek. Kog does not
redistribute Cave Story's synthesis assets; a deterministic original-format
fixture and synthetic bank gate the backend. SGC and the other chiptune
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

QSF and miniqsf now route through the standalone Highly Quixotic engine that
Cog's Objective-C plugin wraps, rather than translating the plugin. The pinned
GPLv3 core emulates the Z80, Kabuki decryption, and Capcom QSound DSP; psflib
supplies relative `_lib` chains and PSF tags. Kog adds bounded KEY/Z80/sample
sections, Cog's 150-second/eight-second timing defaults, 24,038 Hz stereo PCM,
and deterministic seek. An original synthetic Z80 program and sample waveform
gate audible playback, metadata, exact end behavior, malformed input,
mini-library loading, and archive companion lookup without Capcom code or game
data. A broad independently redistributable QSF corpus and direct comparison
with Cog remain.

SSF, minissf, DSF, and minidsf now route through kode54's standalone Highly
Theoretical engine, again reusing the cross-platform core beneath Cog instead
of translating its Objective-C plugin. Kog selects the GPL-2.0-or-later C68k
backend rather than the separately licensed Musashi or Starscream alternatives,
shares psflib for relative `_lib` chains and tags, bounds uploads to the Saturn
or Dreamcast sound RAM, renders 44.1 kHz stereo PCM, and seeks by deterministic
reconstruction. Original synthetic 68000 and ARM programs gate audible playback,
metadata, timing, malformed input, mini-library loading, and archive companion
lookup without Sega firmware or game data. A broad independently redistributable
SSF/DSF corpus and direct comparison with Cog remain.

USF and miniusf now route through losnoco's maintained LazyUSF2 core, the
cross-platform Nintendo 64 emulator already wrapped by Cog's Objective-C
plugin. Kog shares psflib for relative `_lib` chains and tags, validates and
bounds the core's reserved ROM/save-state block format before upload, renders
resampled 44.1 kHz stereo PCM, and uses LazyUSF2's restart/discard path for
seek. A sparse generated Project64 state with an original MIPS sound program
gates audible playback, metadata, timing/fade, malformed input, mini-library
loading, and archive companion lookup without Nintendo firmware, ROMs, or
game code. Cog's initial leading-silence stripping, a broad independently
redistributable USF corpus, and direct comparison remain.

PSF and miniPSF now reuse kode54's cross-platform libupse emulator instead of
translating Cog's Objective-C Highly Experimental plugin or redistributing its
embedded BIOS data. Because libupse is treated conservatively as GPL-2.0-only,
it is built as the separate `kog-psf-helper` program and is not linked into
Kog's GPL-3.0-or-later executable. A bounded binary protocol carries metadata
and 44.1 kHz signed-16 stereo PCM back to Rust; each seek starts a fresh helper
and discards to the exact frame. The helper prevalidates PSF sections,
decompression, executable RAM bounds, tag sizes, library depth, and missing
dependencies before invoking libupse. Generated PSF/miniPSF files with an
original MIPS SPU program gate audible playback, metadata, tagged/default
timing, fade/EOS, malformed input, seeking, and archive companion lookup.

PSF2 and miniPSF2 use Play!'s portable PSF player and high-level IOP BIOS
behind a separate `kog-psf2-helper`; Kog does not translate Cog's Objective-C
wrapper or ship a Sony BIOS. The helper validates bounded PSF2 filesystems,
zlib blocks, dependency depth/cycles, and loadable IRX/ELF ranges before
starting Play!, then streams 44.1 kHz signed-16 stereo PCM through the same
versioned Rust protocol. Generated PSF2/miniPSF2 files with original MIPS/SPU2
code gate routing, metadata, audible playback after seek, tagged/default
timing, exact EOS, malformed input, missing/cyclic libraries, and archive
companion lookup. Broad corpus comparison and Windows/macOS runtime gates
remain.

SNSF and miniSNSF reuse the dedicated cross-platform libsnsf9x playback
library at a pinned revision behind a separate `kog-snsf-helper`; Kog does not
translate Cog's Objective-C++ wrapper. The helper lets psflib resolve and
validate bounded dependency chains, assembles one sanitized dependency-free
SNSF image, and passes only that image through libsnsf9x's public C API. It
streams 32 kHz signed-16 stereo output through the common xSF protocol.
Generated SNSF/miniSNSF files containing original 65C816, SPC700, DSP, and BRR
data gate routing, metadata precedence, audible playback after seek,
tagged/default timing, exact EOS, malformed input, path containment, and
archive companion lookup. The optional helper retains Snes9x's non-commercial
terms and is not linked into the GPL-3.0-or-later Kog executable; broad corpus
and direct Cog comparison plus Windows/macOS runtime gates remain.

2SF and mini2SF reuse the official melonDS 1.1 emulation core behind a
separate `kog-2sf-helper`; Kog does not translate Cog's Objective-C++ wrapper
or ship Nintendo BIOS, firmware, ROM, or game assets. The helper combines
psflib's dependency traversal with bounded ROM/save mappings, validates the
Nintendo DS executable ranges, and streams melonDS's 32,728 Hz signed-16
stereo output through the common xSF protocol. Generated 2SF/mini2SF files
containing original ARM/SPU code gate routing, metadata precedence, audible
playback after seek, tagged/default timing, exact EOS, malformed input,
missing/cyclic libraries, and archive companion lookup. A broad
redistributable corpus, exact comparison with Cog, and Windows/macOS runtime
gates remain.

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
safety limit are implemented. HTTP(S) entries are retained as real playlist
sources, and local or remote HLS manifests route to the linked FFmpeg libraries
instead of being flattened as ordinary M3U entries. Kog's hamburger menu exposes
a themed Add URL dialog, and a generated direct HTTP stream plus HLS
manifest/segment pass an in-process network decode gate without spawning an
external binary. Cog-style **Save As…** and **Save Selection As…** commands use
the platform save dialog and write M3U, M3U8, or PLS. They retain relative local
paths where safe, file and HTTP(S) URLs, numeric subsong/CUE fragments, and Cog's
`unpack://` identity for a specific archived member instead of persisting Kog's
temporary extraction path. Deterministic save/reopen gates cover each identity.
Remote conventional-playlist recursion, richer stream metadata and buffering,
and non-numeric fragment schemes remain parity work.

ZIP, RAR, 7Z, RSN, VGM7Z, and raw GZ files now expand through the system
libarchive library before decoder selection. Kog preserves physical archive
order and stable `archive :: entry` identities while extracting a safe
temporary companion tree, so formats such as APL can reopen sibling audio.
Traversal paths, duplicate destinations, links/devices, oversized entries,
and oversized archives are rejected or surfaced as warnings. Deterministic
ZIP and GZ fixtures pass end-to-end WAV playback; real 7Z and RAR5 fixtures
gate extraction; ZIP-gated APL playback proves relative image lookup; and
archived NCSF, GSF, QSF, SSF, USF, PSF, PSF2, SNSF, and 2SF mini/library pairs
prove xSF dependency lookup.
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
libvgm, libsidplayfp, AdPlug, libADLMIDI, a SoundFont synthesizer, OPL3 MIDI,
and optional Roland emulation. Dependencies and redistributable assets are selected under the
project's documented license policy. License-restricted decoders retain their
own terms and require either a compatible replacement or an independently
reviewed optional-program boundary; Kog's non-commercial intent alone does not
make incompatible code safe to link into the GPL application.

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

Cargo builds `kog-psf-helper`, `kog-psf2-helper`, `kog-snsf-helper`,
`kog-2sf-helper`, `kog-syntrax-helper`, `kog-sfm-helper`, and
`kog-sc55-helper` automatically for local playback; users do not install
third-party player or renderer programs.
Binary packages bundle the companion processes they distribute beside the Kog
executable and carry each process's corresponding notices. Isolated tests may override their locations
with
`KOG_PSF_HELPER=/path/to/kog-psf-helper` and
`KOG_PSF2_HELPER=/path/to/kog-psf2-helper`, or
`KOG_SNSF_HELPER=/path/to/kog-snsf-helper` and
`KOG_2SF_HELPER=/path/to/kog-2sf-helper`. Syntrax tests and packages may use
`KOG_SYNTRAX_HELPER=/path/to/kog-syntrax-helper`; SFM tests and packages may
use `KOG_SFM_HELPER=/path/to/kog-sfm-helper`; SC-55 tests and packages may use
`KOG_SC55_HELPER=/path/to/kog-sc55-helper`.

Choose RustySynth, OPL3Windows, Nuked SC-55, or Munt under
**Hamburger menu → Preferences → Synthesis**. RustySynth requires an SF2 bank.
SC-55 requires
a directory containing a complete supported ROM set obtained from hardware
you own; Kog accepts `KOG_SC55_ROMS=/path/to/rom-directory` and never downloads
or bundles those files. Munt is linked into Kog and likewise requires a
user-supplied compatible MT-32 or CM-32L control/PCM ROM pair; select its
directory in Preferences or set `KOG_MT32_ROMS=/path/to/rom-directory`.
Isolated tests and packages may also set
`KOG_SOUNDFONT=/path/to/bank.sf2` and
`KOG_MIDI_ENGINE=rustysynth-sf2|opl3windows|nuked-sc55|munt-mt32`.

Organya needs a user-owned synthesis bank. Put `soundbank.wdb` or the
`wavetable.dat` and `drums.dat` pair beside the `.org` file, put them in Kog's
platform `organya` configuration/data directory, or set
`KOG_ORGANYA_SOUNDBANK` to the bank file or containing directory. The
MIT-licensed [orgorg player](https://github.com/kpqi5858/orgorg/tree/main/orgorg-player)
can extract `wavetable.dat` and `drums.dat` from the original freeware
`Doukutsu.exe` without requiring Kog to redistribute those assets.

The direct Cargo build requires Rust, C and C++23 compilers, CMake, `pkg-config`,
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

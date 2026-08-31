# Kog architecture

Kog treats Cog as a behavioral oracle, not as an implementation constraint.
The Objective-C plugin boundary is replaced by a Rust decoder registry whose
backends may use safe Rust, C, or C++ libraries.

## Layers

1. **Qt Quick UI** recreates Cog's windows, toolbar, file tree, playlist,
   inspectors, menus, keyboard controls, drag-and-drop, and accessibility.
2. **CXX-Qt application models** expose only typed state and commands. QML
   never owns playback or decoder policy.
3. **Rust playback core** owns the playlist, decoder selection, transport,
   output device, timing, repeat/shuffle policy, and metadata.
4. **Decoder backends** probe a source and append decoded PCM to the shared
   mixer. Each backend declares seeking, subsongs, loop metadata, and companion
   file support independently.
5. **Native library adapters** isolate unsafe FFI and convert library-specific
   streams into interleaved floating-point PCM.

The installed backends are `rodio-symphonia` for conventional audio,
`ffmpeg` as the broad conventional fallback behind Symphonia,
`midi-rustysynth-sf2` for Standard MIDI File and RIFF RMID rendering through a
user-selected SF2 SoundFont, and `midi-opl3windows` for the same MIDI containers
through Cog's OPL3Windows General MIDI engine and Nuked OPL3 1.7.1 core.
`game-music-emu` wraps the pinned upstream libGME 0.6.5 C API for AY, GBS, HES,
KSS, NSF/NSFE, SAP, and SPC. These backends provide real playback, pause, stop,
seek, position, volume, and automatic next-track behavior. Specialist backends
remain separate so a broad fallback cannot erase behavior such as PSF
dependency resolution, VGMStream subsongs, AdPlug subsongs, or
emulator-authentic Roland MIDI synthesis. `libvgm` wraps Cog's exact pinned
libvgm revision for VGM/VGZ, S98, DRO, and GYM. `libopenmpt` wraps Cog's exact
0.8.7 release for the 68 extensions returned by that pinned native build.
`hivelytracker` wraps the official portable 1.9 replayer for AHX and HVL.
`orgorg` 0.2.1 renders Organya Org-02/Org-03 songs from a user-supplied
soundbank. `vgmstream` is the final specialist fallback for its large runtime
extension table, after every narrower backend and excluding its common-format
table so it cannot steal WAV, Ogg, or other conventional containers. `adplug`
wraps Cog's exact AdPlug and libbinio revisions and is ordered after MIDI,
libvgm, and OpenMPT so their shared MID, VGM/DRO, and S3M extensions retain
Cog's intended specialized routes. `libsidplayfp-residfp` wraps Cog's exact
libsidplayfp revision for ROM-free PSID data and is ordered before AdPlug.

The FFmpeg adapter discovers `libavformat`, `libavcodec`, `libavutil`, and
`libswresample` with pkg-config and keeps all native ownership behind a small
C ABI. The demuxer selects the best audio stream; the codec and resampler
preserve its sample rate and channel layout while converting packed or planar
native samples to interleaved 32-bit float PCM. Probe maps common container or
stream metadata, native codec name, duration, bitrate, and bit depth. Seek
flushes the demuxer, codec, resampler, and buffered PCM together, then drops
pre-target decoded frames when timestamps are available. It is registered
after Symphonia so existing MP3/AAC/FLAC/MP4/WAV/Ogg routes do not change, and
before specialist backends without claiming their extensions. The pinned Nix
shell overrides FFmpeg with both GPL and version-3 components disabled; the
result's own license banner reports LGPL-2.1-or-later, which is compatible with
Kog's GPL-2.0-only application. A four-frame synthetic AC-3 stream gates
format routing, source priority, properties, audible PCM, seeking, and clean
end-of-stream behavior. Wider family and metadata corpora, sample-accurate
seeking across every demuxer, attached artwork, chapters/subtracks, gapless
trim data, and remote custom I/O remain parity work.

Decoder settings are shared between the probe registry and playback registry.
The MIDI engine and current SF2 path are persisted in the platform
configuration directory; the SF2 is validated before it is accepted and cached
by path and modification time. Both MIDI engines render interleaved 48 kHz
stereo floating-point PCM. The OPL3 path uses a small C ABI around Cog's
GPL-compatible native engine; Midly merges format-0/1 tracks and schedules
legacy MIDI messages at exact output frames. Seeking recreates the selected
synthesizer and deterministically advances it to the requested frame.

The GME adapter owns each native emulator handle in one Rust `Source`, converts
its stereo signed-16 PCM to the shared floating-point stream, and uses 44.1 kHz
except for Cog-compatible 32 kHz SPC playback. Probe-time subsong expansion
creates stable `(path, zero-based subsong)` identities. Companion M3U metadata
can restrict/reorder that set and supplies titles and lengths; malformed or
unreadable companions remain non-fatal but are surfaced as warnings. The
current fixed defaults match Cog: 150 seconds when no duration exists, two
loops when loop metadata exists, and an eight-second fade when none is given.

The libvgm adapter registers its VGM, S98, DRO, and GYM player engines and owns
the native player plus input memory for the lifetime of one Rust `Source`. It
requests libvgm's packed 32-bit representation of its internal signed 24-bit
PCM and converts that stream to stereo floating point at 44.1 kHz without
discarding the internal precision. Probe metadata maps TITLE, ARTIST, GAME, and
DATE tags and reports the native format/version string. The advertised
duration is Cog's one-pass duration; playback applies Cog's default two-loop,
eight-second-fade, and half-second end-silence policy. A sibling `yrw801.rom`
is supplied through libvgm's file callback when present and otherwise remains
an optional user-provided asset.

The OpenMPT adapter builds the upstream C++17 sources as a static library and
uses libopenmpt's stable C API behind one Rust owner. Bundled miniz, minimp3,
and stb_vorbis preserve compressed-sample support without platform package
dependencies. Every module is loaded with synchronous sample seeking, then
uses Cog's normal-play defaults: repeat count zero, 0 mB master gain, 100%
stereo separation, 8-tap interpolation, automatic volume ramping, and Amiga
resampler emulation. Probe-time expansion gives every subsong a stable
`(path, zero-based subsong)` identity; rendering produces 44.1 kHz stereo
floating-point PCM and seeking uses libopenmpt's time-position API. The
extension table is asserted against the native library during tests so source
configuration and registry routing cannot silently diverge.

The Hively adapter builds the upstream portable C replayer at commit
`f393ca7` and keeps its state behind a narrow C bridge. It loads from the
shared in-memory source, expands the native main song plus every declared
subsong, and reports the embedded title. A bounded dry run detects two song
ends and derives duration using Cog's 44.1 kHz, two-loop, eight-second-fade
policy. Playback converts the replayer's stereo 16-bit frames to the shared
float stream, applies the fade, and exposes deterministic restart-and-skip
seeking. The official AHX and HVL examples gate both parsers, while a
test-only HVL derivative with a second valid start position gates stable
subsong identities.

The Organya adapter owns the song, normalized wavetable/drum samples, and the
self-referential `orgorg` player as one safe Rust source. It discovers either
the `soundbank.wdb` format used by `orgorg-player` or that player's extracted
`wavetable.dat`/`drums.dat` pair beside the song, in Kog's platform data
directories, or through `KOG_ORGANYA_SOUNDBANK`. Kog does not bundle Cave
Story's synthesis data. Playback uses 44.1 kHz stereo float PCM, Cog's default
two-loop/eight-second-fade policy, and deterministic restart-and-skip seeking.
A generated Org-02 song and synthetic wavetable gate parsing, loop duration,
audible PCM, fade completion, routing, and seek without redistributing game
content.

The vgmstream adapter pins upstream r2117 at commit `05dbda9b` and builds the
static core through its CMake target. A small C bridge owns the public
`libvgmstream` handle, filesystem streamfile, and companion-file behavior while
Rust receives interleaved floating-point PCM and immutable format metadata.
The adapter expands native subsongs into stable zero-based identities, applies
Cog's default two-loop/eight-second-fade policy, exposes loop and companion
capabilities, seeks in native sample frames, and reads common fields from a
sibling `!tags.m3u`. Runtime extension validation excludes vgmstream's common
formats and the registry orders this backend last. The portable baseline
enables native codecs and built-in G.722.1 only; optional external codec
families are tracked separately rather than silently claimed. A generated
mono PlayStation VAG gates public API version, extension routing, tag parsing,
duration, audible PCM, seek, and exact end-of-stream behavior.

The AdPlug adapter pins kode54's revision `4e0141ab` and libbinio revision
`e2f8d50c`, builds all 51 runtime-enumerated player extensions, and renders
through that fork's bundled Nuked OPL3 core. The C++ bridge owns the native
player and emulator, calculates Cog-compatible length at 44.1 kHz, expands
zero-based subsongs, converts stereo signed-16 synthesis to the shared float
stream, and implements exact seeking by rewinding and rendering discarded
audio. Its Nuked OPL symbols are namespaced to coexist with Kog's independent
OPL3Windows MIDI core. AdPlug is registered immediately before vgmstream and
after the narrower overlapping backends. Upstream's Creative Music File
`2.CMF` gates the exact extension/version pin, type metadata, duration,
audible PCM, seeking, and end-of-stream behavior. The optional database and a
wider format corpus remain explicit parity work.

The SID adapter pins kode54's libsidplayfp revision `519d1201`, builds its
reSIDfp emulator and generated 6502 player data, and accepts self-contained
PSID files from memory. It expands every tune into stable zero-based subtune
identities, maps the PSID title to album when a file has multiple subtunes,
reports artist and release metadata, and chooses mono or stereo according to
the number of declared SID chips. Playback converts native signed-16 samples
to the shared float stream at 44.1 kHz, applies Cog's 150-second default plus
eight-second fade and stereo-width transform, and seeks using libsidplayfp's
32x fast-forward mode followed by an exact residual render. Kog does not copy
Cog's embedded copyrighted C64 ROM arrays: RSID and BASIC-compatible tunes are
identified and rejected with an actionable user-ROM error until ROM selection
exists. A generated two-subtune PSID gates the exact revision, routing,
metadata, audible PCM, accelerated seek, fade duration, and exact end of
stream. Raw MUS/MUS+STR routing, configurable synthesis policy, user ROMs, and
the song-length database remain parity work.

## Decoder contract

`DecoderBackend` is the current executable contract. Every backend supplies:

- stable identifier and display name;
- extensions used for initial routing;
- explicit capability flags;
- optional subsong enumeration;
- a probe that returns duration, sample rate, channel count, common metadata,
  track number, codec, bitrate/bit depth when known, and non-fatal warnings;
- an append operation that gives the shared player a real decoded source.

The contract still needs source sniffing, per-backend loop/fade configuration,
general companion/dependency resolution, and deterministic error categories.
Those additions must not weaken the existing real playback paths.

## Fidelity gates

- A versioned corpus must contain at least one independently redistributable
  sample for every decoder family and every behavior class.
- Format support requires successful probe, decode, audible PCM, duration,
  seek (when Cog supports it), metadata, and clean end-of-stream handling.
- Multi-track formats require exact subsong count and stable subsong identity.
- Looped game formats require loop and fade assertions, not just a timeout.
- Companion-file formats require both success and missing-dependency cases.
- UI parity is checked with deterministic rendered states against Cog reference
  captures at the same logical window size.
- Window and interaction coverage is tracked separately in
  [`UI_PARITY.md`](UI_PARITY.md); a visually similar main shell is not treated
  as whole-application UI parity.
- Linux, Windows, and macOS builds and runtime smoke tests are independent
  release gates.

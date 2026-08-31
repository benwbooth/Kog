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

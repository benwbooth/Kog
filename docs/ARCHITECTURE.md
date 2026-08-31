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

The installed backends are `rodio-symphonia` for conventional audio and
`midi-rustysynth-sf2` for Standard MIDI File and RIFF RMID rendering through a
user-selected SF2 SoundFont. Both provide real playback, pause, stop, seek,
position, volume, and automatic next-track behavior. Specialist backends
remain separate so a broad fallback cannot erase behavior such as PSF
dependency resolution, VGMStream subsongs, or emulator-authentic OPL and
Roland MIDI synthesis.

Decoder settings are shared between the probe registry and playback registry.
The current SF2 path is validated before it is accepted, persisted in the
platform configuration directory, and cached by path and modification time.
MIDI is rendered to interleaved 48 kHz stereo floating-point PCM; seeking
reconstructs the sequencer and deterministically advances it to the requested
frame.

## Decoder contract

`DecoderBackend` is the current executable contract. Every backend supplies:

- stable identifier and display name;
- extensions used for initial routing;
- explicit capability flags;
- a probe that returns duration, sample rate, and channel count;
- an append operation that gives the shared player a real decoded source.

The contract will grow before the first specialist backend to include source
sniffing, metadata, subsong enumeration, loop/fade policy, companion file
resolution, and deterministic error categories. Those additions must be made
without weakening the existing real playback path.

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

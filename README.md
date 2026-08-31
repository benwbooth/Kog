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
needs corpus coverage. SFM, SGC, and the other chiptune, game-audio, and tracker
families remain explicitly unclaimed.

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
synthesizer. Dependencies and redistributable assets will be selected only
after license review.

See [the architecture](docs/ARCHITECTURE.md),
[UI parity matrix](docs/UI_PARITY.md), and
[format parity matrix](docs/FORMAT_PARITY.md) for the executable backend
contract, fidelity gates, and the complete Cog-derived worklist.

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

The direct Cargo build requires Rust, C and C++ compilers, CMake, and Qt 6 with
Qt Quick and Qt Quick Controls.

## License

Kog is licensed under GPL-2.0-only. Third-party decoder libraries and assets
retain their own licenses; bundled test-fixture attribution is recorded in
[the third-party notices](THIRD_PARTY_NOTICES.md).

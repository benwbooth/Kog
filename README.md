# Kog

Kog is a new cross-platform local music player written in Rust with a Qt Quick
interface connected through CXX-Qt. Its target is feature parity with
[Cog](https://github.com/losnoco/Cog), including a faithful recreation of Cog's
desktop interface and broad playback support for conventional audio, MIDI,
tracker modules, chiptunes, and game-audio formats.

This repository currently contains only the verified CXX-Qt application
foundation. It does not play audio yet.

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

## Development

On NixOS or another system with Nix installed:

```sh
nix develop
cargo run
```

The direct Cargo build requires Rust, a C++ compiler, CMake, and Qt 6 with Qt
Quick and Qt Quick Controls.

## License

Kog is licensed under GPL-2.0-only. Third-party decoder libraries and assets
retain their own licenses.

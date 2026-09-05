# Modern Winamp skin integration: engine decision

Modern `.wal` skins remain unsupported until an engine is integrated and tested.
They contain XML layouts, images, fonts, and MAKI bytecode. Supporting the file
extension or drawing a static background would not implement the skin.

## Reuse candidates inspected

- [qtWasabi](https://github.com/qtWasabi/qtWasabi) is a Qt-native renderer, but its
  build explicitly requires user-supplied Winamp source for the MAKI VM and BFC.
  Its own MIT license does not supply redistribution rights for that separate
  source. It is not currently selected for Kog's public binary releases.
- [WasabiQT](https://github.com/kleberbaum/WasabiQT) describes an earlier native
  approach, but the inspected repository is a bootstrap design rather than an
  embeddable implementation.
- [Webamp Modern](https://github.com/captbaritone/webamp/tree/master/packages/webamp-modern)
  provides an MIT-licensed browser renderer and a reimplemented MAKI interpreter.
  Its documented gaps include EQ, some global actions, color handling, and
  particular skin/layout behavior. Embedding it would require Qt WebEngine;
  Kog's normal interface would remain QML and its Rust decoder core would still
  perform playback. This would be experimental compatibility, not all-skin parity.

The user has been asked whether the larger bundled browser dependency is
acceptable or whether Kog should stay native. No engine has been vendored or
enabled pending that choice.

## Acceptance requirements for either route

- Actual MAKI-driven interaction, checked against several real `.wal` skins.
- Skin transport, seek, volume, metadata, and playback state connected to Kog,
  without a second browser/media-player decoding path.
- Separate modern/classic gallery filters; modern archives must not be handed
  to the classic bitmap renderer.
- Bounded archive expansion, asset dimensions, XML includes, and VM execution.
- No native executables or DLLs from skins; no arbitrary filesystem/network
  access or unrestricted host API exposed to scripts.
- Explicit unsupported-feature diagnostics and a reliable return to Kog's
  normal queue if a skin fails or hangs.
- Reproducible Windows, macOS, AppImage, and Flatpak packaging, with appropriate
  third-party notices. No unlicensed sample skins bundled with Kog.

MilkDrop-style rendering is a separate integration. Native libprojectM and the
browser-based Butterchurn are candidates, not currently shipped backends.

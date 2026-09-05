# Modern Winamp skin integration: engine decision

Modern `.wal` skins use an experimental Qt WebEngine integration.
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

The user approved the Qt WebEngine route. Webamp source is pinned at
`88ed5815d968c201962f6549915579b3d2f93c5e` in `native/webamp`, and the Nix
development shell supplies Qt WebEngine, Qt WebChannel, and Node.js for building
the renderer bundle. `web/modern` contains the adapter and committed bundle;
`ModernPlayer.qml` exposes a narrow WebChannel command interface. The normal
player remains QML, and all audio continues through Kog's decoder/playback core.
Modern imports and the separate gallery filter are enabled. Native plugins,
some upstream MAKI APIs, balance, and playback-rate controls remain unsupported.
See `web/modern/README.md` for build instructions and current limits.

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

MilkDrop-style rendering is a separate integration. Modern skins' visualization
slots currently use Kog's PCM spectrum, not a claimed MilkDrop implementation.
Native libprojectM and browser-based Butterchurn remain candidates.

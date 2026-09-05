# Skins and visualizers

## Current scope

Classic Winamp 2 main-player bitmap skins (`.wsz` / `.zip`) are optional.
The normal Cog-style UI remains the default. Classic transport, seek, volume,
shuffle and repeat drive the same AppController as the normal player. EQ and
playlist buttons open Kog's existing windows; they do not claim legacy skinned
EQ/playlist parity. Windowshade, shaped windows, custom cursors, balance and
modern `.wal`/MAKI skins are not implemented.

The read-only gallery queries Internet Archive's `winampskins` collection,
excluding `winampskinsmodern`. It requests 24 results per page and only queries
when opened or searched. Preview images use Archive's thumbnail service.
Searches and downloads contact Internet Archive directly. No submission service,
account, background synchronization or collection mirroring is involved.

Downloads use the Qt Network library already linked by Kog, on a worker thread.
HTTPS is required; redirects must stay on archive.org or its subdomains. Requests
have a 30-second timeout, five-redirect limit and byte caps. Search/metadata is
limited to 2 MiB and downloads to 32 MiB. Missing archives and collection bundles
with multiple candidates report errors rather than guessing which to install.

The existing libarchive extraction path rejects traversal, nonregular entries,
duplicates and expansion limits. Skins add a 512-entry / 8 MiB per-entry /
32 MiB expanded cap, then validate bitmap dimensions and decodeability before
keeping a flat whitelist of BMP sheets. No scripts, DLLs or executables are
installed or run. Successful installations live under Kog's platform data
directory, in `skins/classic-*`, with a `skin.json` manifest retaining source,
creator and license metadata where Archive supplies it. Kog does not grant any
license to third-party artwork. No third-party skin artwork ships in the repo.

Sprite sheet coordinates were checked against the format mappings in
[Webamp's skinSprites.ts](https://github.com/captbaritone/webamp/blob/master/packages/webamp/js/skinSprites.ts).
This is a Qt renderer, not an embedded browser or a copy of Webamp's player.

## Audio visualization

`AudioMeterSource` taps the mixed, equalized decoded PCM into a bounded 2048-frame
mono ring of atomics. The callback does not allocate, lock, run an FFT, or render.
The UI requests frames at 30 Hz only while a visualization is visible. Each
request performs a Hann-windowed 2048-point FFT for 40 logarithmic spectrum bands
and supplies 256 recent PCM frames for the oscilloscope. Snapshots may straddle
audio callback blocks; they are display data, not a recording API. Stereo is
averaged to mono, so out-of-phase stereo material can cancel. The tap precedes
output-device buffering and output volume; it is not a microphone/loopback feed.

The Qt Quick Canvas renderer works with Qt's graphics backend on each platform;
it does not force OpenGL. It renders blue/green bars or an oscilloscope, and offers
full-screen display. Paused/stopped playback publishes silence; bars decay.

## Next backend: projectM

[libprojectM](https://github.com/projectM-visualizer/projectm) is a candidate for
MilkDrop-compatible presets. It accepts PCM and renders OpenGL; integrating it
requires a compatible rendering context, lifetime/thread coordination and
separately licensed preset/texture packs. It is not currently bundled, and Kog
does not load Winamp visualization DLLs. Prefer a bundled library integration
over launching an external visualizer program. Modern skins remain a separate
engine project, not a file-extension toggle.

## Checks

```sh
nix develop -c cargo test --locked
nix develop -c env QT_QPA_PLATFORM=offscreen QT_QUICK_CONTROLS_STYLE=Basic \
  qmltestrunner -input tests/qml -o -,txt
nix develop -c bash tests/native/run-skin-network.sh
```

For an opt-in live network test, pass a **new** destination filename to
`run-skin-network.sh`; it downloads a classic sample from Archive and checks
metadata, redirects and byte limits. Test a local real skin without installing
it into the user's library:

```sh
KOG_TEST_SKIN_ARCHIVE=/path/to/example.wsz nix develop -c \
  cargo test --locked imports_real_classic_skin -- --ignored
```

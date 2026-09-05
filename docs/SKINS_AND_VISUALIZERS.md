# Skins and visualizers

## Current scope

Classic Winamp 2 bitmap skins (`.wsz` / `.zip`) are optional.
The normal Cog-style UI remains the default. Classic transport, seek, volume,
shuffle and repeat drive the same AppController as the normal player. The PL
button toggles a PLEDIT-skinned playlist with selection, playback, removal,
reordering, file drops and save/add actions. EQ opens Kog's native equalizer.
The bottom Kog toolbar is hidden by default; right-click the main player or use
its top-left menu to show it. Playlist/toolbar visibility is remembered.
Previously installed classic skins need reimporting to acquire PLEDIT artwork;
otherwise the playlist uses a plain fallback. Windowshade, shaped windows,
custom cursors and balance are not implemented.

Modern `.wal` skins have experimental XML/MAKI rendering through Qt WebEngine.
Their transport, playlist, metadata, volume and EQ controls use Kog's host state.
The ten-band skin EQ is resampled to Kog's 31 bands on a logarithmic frequency
axis. Unsupported upstream APIs and skin-specific layouts may still fail. The
native footer always provides a return to Kog and the skin gallery.

The gallery offers separate classic (`winampskins` excluding modern) and modern
(`winampskinsmodern`) filters. It requests 24 results per page and only queries
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
keeping a flat whitelist of BMP sheets for classic skins. Modern imports validate
one XML root and raster assets, reject native executable plugins, and keep the
validated ZIP in `skins/modern-*`. MAKI is interpreted in the browser renderer;
it is not a native plugin. Network/file access is restricted to the bundled page
and selected skin, and the host controller itself is never exposed to the page.
Successful installations live under Kog's platform data directory, with a
`skin.json` manifest retaining source,
creator and license metadata where Archive supplies it. Kog does not grant any
license to third-party artwork. No third-party skin artwork ships in packages.

Sprite sheet coordinates were checked against the format mappings in
[Webamp's skinSprites.ts](https://github.com/captbaritone/webamp/blob/master/packages/webamp/js/skinSprites.ts).
The classic player is a native Qt renderer; modern skins use the pinned Webamp
Modern renderer with host-only audio. No sample skins are included in packages.

## Audio visualization

`AudioMeterSource` taps the mixed, equalized decoded PCM into a bounded 2048-frame
mono ring of atomics. The callback does not allocate, lock, run an FFT, or render.
The UI requests frames at 30 Hz only while a visualization is visible. Each
request performs a Hann-windowed 2048-point FFT for 40 logarithmic spectrum bands
and supplies 256 recent PCM frames for the oscilloscope. Snapshots may straddle
audio callback blocks; they are display data, not a recording API. Stereo is
averaged to mono, so out-of-phase stereo material can cancel. The tap follows
Kog's playback volume and equalizer, but precedes output-device buffering; it
is not a microphone/loopback feed.

The Qt Quick Canvas renderer works with Qt's graphics backend on each platform;
it does not force OpenGL. Six blue/green views are available: spectrum,
oscilloscope, scrolling spectrogram, radial spectrum, mirrored spectrum, and
waveform trails. Each offers full-screen display. Paused/stopped playback
publishes silence; bars decay. The spectrogram keeps at most 96 spectrum frames
and the trails keep ten PCM traces. Histories clear when the mode changes or
the window is hidden. No random or synthetic signal drives these views.

## Next backend: projectM

[libprojectM](https://github.com/projectM-visualizer/projectm) is a candidate for
MilkDrop-compatible presets. It accepts PCM and renders OpenGL; integrating it
requires a compatible rendering context, lifetime/thread coordination and
separately licensed preset/texture packs. It is not currently bundled, and Kog
does not load Winamp visualization DLLs. Prefer a bundled library integration
over launching an external visualizer program. Modern skin visualization slots
currently show the host PCM spectrum, not MilkDrop preset compatibility.

## Checks

```sh
nix develop -c cargo test --locked
nix develop -c env QT_QPA_PLATFORM=offscreen QT_QUICK_CONTROLS_STYLE=Basic \
  qmltestrunner -input tests/qml -o -,txt
nix develop -c bash tests/native/run-skin-network.sh
nix develop -c bash tests/native/run-modern-skin-smoke.sh
```

For an opt-in live network test, pass a **new** destination filename to
`run-skin-network.sh`; it downloads a classic sample from Archive and checks
metadata, redirects and byte limits. Test a local real skin without installing
it into the user's library:

```sh
KOG_TEST_SKIN_ARCHIVE=/path/to/example.wsz nix develop -c \
  cargo test --locked imports_real_classic_skin -- --ignored
```

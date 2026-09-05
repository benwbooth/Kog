# Kog modern-skin runtime

This directory builds the browser half of Kog's Winamp Modern skin support. It
uses the pinned `native/webamp/packages/webamp-modern` renderer directly. No CDN,
remote font, media URL, or runtime npm request is required or permitted.

## Build and test

From this directory:

```sh
npm ci --ignore-scripts
npm test
npm run build
```

`npm run build` produces the committed files in `dist/` and regenerates
`runtime.qrc`. Kog includes that resource file and loads
`qrc:/kog/modern/index.html`. The resource prefix is `/kog/modern`.

The optional `test/browser-harness.html` is a local smoke harness for the three
test-only WAL files in the pinned Webamp checkout. Those skins are never copied
into `dist/` or Kog's resources. A manual Chromium invocation is:

```sh
google-chrome --headless=new --disable-gpu \
  --allow-file-access-from-files --virtual-time-budget=15000 --dump-dom \
  "file://$PWD/test/browser-harness.html?skin=WinampModern566.wal"
```

## Architecture

`src/runtime.ts` constructs the real `UIRoot`, runs a `SkinEngine_WAL` subclass,
and uses a `ZipFileExtractor` subclass to normalize an archive with exactly one
root or single-wrapper `skin.xml`. The subclass only substitutes a safe playlist
GUI so metadata is inserted with `textContent`. Playback, seeking, volume, EQ,
playlist mutation, current codec/sample-rate/channel/bitrate information, and
file dialogs are forwarded to the `kog` Qt WebChannel
object; the Webamp `HTMLAudioElement` is never assigned a source or played.

`src/state-adapter.js` owns the validated state snapshot and command gateway.
Playlist rows are retained when high-frequency snapshots omit `tracks`. Kog's
Qt host sends rows through a separate persistent `tracksJson` property so
WebChannel batching cannot discard a playlist revision. Webamp's
ten EQ controls use its native -12 dB to +12 dB range at 60, 170, 310, 600,
1000, 3000, 6000, 12000, 14000, and 16000 Hz. Kog may have a wider native EQ
range, but values exposed to Modern skins are intentionally clamped to Webamp's
range.

The analyser facade reads only host-provided `visualization.wave` and
`visualization.spectrum` samples. The embedded Milkdrop slot is replaced with a
host-PCM spectrum because Butterchurn requires a live Web Audio node, which Kog
intentionally does not provide. Fullscreen/configuration visualization actions
are handed back to Kog.

## Security and limits

- CSP permits only bundled `qrc`, the fixed `kogskin://current/skin.wal` URL,
  and generated `data`/`blob` image and font content. Audio and frames are denied.
- The native request interceptor is still authoritative; browser checks are a
  second boundary.
- `clear`, `remove`, `move`, `swap`, `openFiles`, `savePlaylist`, and `restore` require
  a current trusted browser activation. Startup MAKI cannot invoke them.
- A MAKI event has a 100,000-instruction budget. Skin timers are clamped to a
  minimum of 16 ms and a maximum of one hour.
- Each skin build is limited to 4,096 XML include requests and 50,000 GUI objects;
  the native importer validates raster dimensions before accepting an archive.
- State JSON is limited to 16 MiB, 100,000 playlist rows, and 4,096 samples per
  visualization channel in the browser. Native archive and request validation
  remains authoritative.

The upstream repository's README licenses its code under MIT but separately
calls out the Winamp interface as Nullsoft property. For that reason the build
does not copy the `assets/freeform` PNGs, demo skins, or compiled fallback MAKI.
It bundles only the three renderer XML definitions required for Wasabi standard
frames/text, then resolves graphics and scripts from the user's skin archive.
Runtime dependency notices and license texts are generated into
`dist/THIRD_PARTY_NOTICES.txt`.

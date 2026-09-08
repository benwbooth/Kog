# Winamp skin fidelity

The target is faithful classic and modern skin behavior, not merely successful
loading. A passing smoke test does not establish compatibility with every skin.
Emulation defects remain work to do; calling support experimental does not close
them. External native plugins and unavailable online services are tracked
separately from passive skin artwork, layout, input, and MAKI behavior.

## Current regression coverage

| Surface | Evidence | Still required |
| --- | --- | --- |
| Classic bitmap title | Native Winamp glyph coordinates, per-pixel comparison with a real TEXT.BMP, deterministic marquee wrap, missing/undersized font fallback, bounded visible glyph count | Full title formatting and interactive ticker dragging; broader skin/font corpus |
| Classic controls and playlist | QML transport/state tests and a rendered classic window | Time remaining/hundreds, seek dragging, proper windowshade, independent docked windows, skinned equalizer, regions, custom cursors and balance |
| Modern localization | Real included XML string tables, translate=2 text/tooltip presentation, raw script text, distinct layout/script width measurements; ClassicPro tooltip assertions in QtWebEngine | Language-pack translation mode 1, title/label subclasses and resource ordering across a larger corpus |
| Modern lifecycle and host integration | MMD3 and ClassicPro native render/state tests followed by Winamp Modern reload; native library search/add/play and renderer isolation | Differential event traces, window topology and interaction coverage across other modern skins |

Classic reference: Winamp `Src/Winamp/draw.cpp` (glyph lookup) and
`draw_main.cpp` (bitmap title clipping, fill, and scrolling). Modern reference:
Wasabi `LocalesManager::lookupString`, `Text::getPreferences`,
`Text::getTextWidth`, and `GuiObjectWnd::getTip`. Source comparisons are not
substitutes for side-by-side original-Winamp executable captures.

## Acceptance discipline

For each repair, preserve an observable failing case, implement the native
behavior, and verify the affected interaction and rendering. Keep source/API
conformance, Kog runtime success, and original-Winamp differential comparison as
distinct evidence levels. Add skins by content identity and exercise their
states; do not count loading a skin as a complete interaction test.

The current classic player combines the main panel and playlist in one window.
Faithful docking, independent equalizer/playlist windows, and windowshade require
repairing that topology rather than adding visual approximations to the stacked
window. These are core emulation tasks, not plugin limitations.
## Modern screenshot regression work (2026-09-07)

The current local slice removes repeated bitmap-atlas warning pixels, bridges
single-container skin geometry to the native window, hides redundant host chrome,
corrects toggle-button mouse-down ordering, and resolves rendered color aliases.
The MAKI AND opcode no longer returns true for true AND false; this fixes the
ClassicPro side playlist reopening at zero width. Wasabi Search buttons now fit
their declared geometry, without HTML borders enlarging the control. The stopped
clock uses native millisecond long-time formatting and explicit text precedence.
Native ClassicPro tests cover the stopped timer, host resize, and a real Playlist-tab
click with a single full-width playlist. MMD3 and Winamp Modern reload smokes still
pass. The native local-file library uses skin-supplied colors and compact rows.
This is not full visual parity.

The native library remains a local-file tree, not Winamp's metadata media library;
the Playlist tab separately displays the host queue. ClassicPro diagnostics still
include the version-check getBuildNumber call and a notifier null-layout callback.
Multi-container skins still require separate native-window integration;
the new single-container host binding deliberately does not shrink their desktop
surface. Modern windows now request frameless native surfaces before creation,
and auxiliary containers no longer disable the main title-bar move binding.
Single-container geometry is refreshed to recover dropped startup messages.
Native move/resize gestures still require interactive desktop verification; the
title-bar regression checks the real press-to-native-move signal path separately.
That regression failed until Layer's default mover flag was restored, matching
Winamp `Src/Wasabi/api/skin/widgets/layer.cpp`'s constructor; explicit `move=0`
still disables dragging. The corrected real ClassicPro title-bar press reaches
the native move handler.

Both player windows now have a native bottom-right resize grip. Its press is
handled in QML directly, rather than waiting for a skin script/WebChannel callback.
Classic requests frameless chrome from creation and no longer fixes maximum
width/height to its initial size. In the current combined-window UI, changing its
width scales the main bitmap proportionally while extra height grows the playlist.
This is a Kog sizing policy, not a claim of original Winamp's independent-window
topology. Tests cover corner presses and resulting layout dimensions separately;
compositor-driven live dragging still needs desktop verification.
Do not claim all-skin compatibility; the coverage and remaining gaps above apply
to the 0.2.1 release.

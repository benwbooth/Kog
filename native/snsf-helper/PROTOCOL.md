# Kog SNSF helper protocol

`kog-snsf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS` writes the
same version 1 `KOGPSF1` metadata/PCM stream used by Kog's other xSF helpers.
The format field is `0x23`; audio is little-endian signed 16-bit stereo at
32,000 Hz. The five length-prefixed metadata fields are title, artist, album
(the SNSF `game` tag), genre, and date.

Kog restarts the helper and discards emulated frames for seeking. The helper
emits exactly the declared frames from `START_FRAME` through the end. Fade is
applied by Kog's Rust process wrapper so every xSF helper has identical timing
and seek behavior.

The process boundary contains faults in the legacy Snes9x-derived core; it is
not an operating-system sandbox. Before starting that core, the helper uses
the pinned psflib to validate CRCs and bounded dependency nesting, restricts
companions to the root file's directory tree, bounds ROM/SRAM mappings, and
assembles a dependency-free SNSF image for libsnsf9x's public C API.

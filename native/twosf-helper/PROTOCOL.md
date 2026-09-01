<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Kog 2SF helper protocol

`kog-2sf-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS` opens one
2SF-family file with psflib and the pinned melonDS core. It writes the common
version-1 Kog xSF stream header followed by 32728 Hz stereo signed-16 PCM until
the declared end. The header's xSF format field is `0x24`. Diagnostics go to
standard error and a nonzero exit status is a decode failure.

Kog launches a fresh helper for seeking. The helper reconstructs the Nintendo
DS state and discards PCM before `START_FRAME`. Integers and samples are
little-endian. Kog applies the tag/default duration and linear fade once in the
parent process.

The helper is a crash boundary, not an operating-system sandbox. Kog's adapter
bounds the xSF ROM/save mappings and validates the Nintendo DS executable
ranges before handing the image to melonDS. melonDS supplies the BIOS,
firmware, CPU, memory, and SPU emulation; Kog does not translate those parts of
Cog's Objective-C++ plug-in.

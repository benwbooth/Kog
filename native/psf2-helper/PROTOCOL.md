<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Kog PSF2 helper protocol

`kog-psf2-helper PATH START_FRAME DEFAULT_LENGTH_MS DEFAULT_FADE_MS` validates
and opens one PSF2-family file, writes the same version-1 stream header used by
`kog-psf-helper`, then writes 44.1 kHz stereo signed-16 PCM until the declared
end. The header's xSF format field is `2`. Diagnostics are written to standard
error and a nonzero exit status is a decode failure.

Kog launches a fresh process for a seek. The helper reconstructs Play!'s PS2
IOP/HLE-BIOS state and discards PCM before `START_FRAME`. Integers and samples
are little-endian. Kog applies the tag/default duration and one linear fade in
the parent process.

The helper is a crash and licensing boundary, not an operating-system sandbox.
It validates file, dependency, PSF2 filesystem, compressed-block, and IRX ELF
bounds before passing data to Play!'s parser.

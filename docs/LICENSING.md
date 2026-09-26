# Kog license policy

Kog-authored source code is licensed under the GNU General Public License,
version 3 or (at your option) any later version (`GPL-3.0-or-later`), except
adapters identified in their source and notices for GPL-2.0-only or
Snes9x-licensed helper programs. The GPL text for the main application is in
the repository root `LICENSE` file.

Third-party libraries, emulator cores, test fixtures, and user-supplied assets
are not relicensed. Their copyright notices and license terms remain in their
source trees and are summarized in `THIRD_PARTY_NOTICES.md`.

## Decoder boundaries

- Permissive, LGPL, MPL, GPL-3.0-only, and GPL-2.0-or-later components may be
  linked into the main application when their exact terms are compatible with
  GPL-3.0-or-later and their redistribution requirements are satisfied.
- Legacy HMI/HMP/HMQ/MUS/XMI playback statically links the pinned libADLMIDI
  source. Its enabled library, sequencer/converter, Nuked OPL3, structures, and
  bank components retain their GPL-3.0-or-later, GPL-2.0-or-later,
  LGPL-2.1-or-later, LGPL-2.0-or-later, MIT, BSD-3-Clause, Boost-1.0,
  public-domain, and per-bank terms, all compatible with distribution of the
  combined Kog executable under GPL-3.0-or-later. Kog builds with
  `BUILD_NO_GREY_BANKS=ON`; upstream's separately identified grey-zone bank set
  is not embedded. The retained submodule notices remain authoritative.
- MT-32/CM-32L playback statically links the pinned Munt 2.8.2 libmt32emu
  library under LGPL-2.1-or-later. That license is compatible with Kog's
  GPL-3.0-or-later combined executable. Binary distributors must retain the
  notices and provide the complete corresponding Munt and Kog source needed to
  relink a modified library, as required by the LGPL and GPL. Kog builds no
  Munt frontend and distributes no Roland ROM data.
- GPL-2.0-only components are not linked into the main application. The
  upstream libvgm YMF278B core remains disabled. Historical libupse and the
  former SFM/SNSF helper wrappers are retained as source only, with their own
  notices, and are no longer built or packaged.
- SFM links Cog's LGPL-2.1-or-later snapshot/DSP code with the GPLv3 higan
  v095 CPU/SMP rebase and a newly written GPL3-or-later Kog adapter. This
  combined executable is distributable under GPL version 3; preserve the
  donor and adaptation provenance in `native/cog-gme-sfm/PROVENANCE.md`.
- PSF1 and PSF2 link BSD-licensed Play! HLE and audio libraries with Kog's
  GPL3-or-later interpreter and bounded loaders. No Sony BIOS is bundled.
  The standalone executable is a regression tool, not a runtime dependency.
- 2SF playback statically links the official GPL-3.0-or-later melonDS core,
  psflib, and zlib on Linux, macOS, Android, and iOS. The separate
  `kog-2sf-helper` remains a desktop regression target. The pinned melonDS
  revision does not build with MSVC, so Windows 2SF remains unsupported. Binary
  distributions must carry the complete corresponding source and upstream
  copyright/license notices. melonDS is not relicensed by Kog.
- Syntrax playback statically links the GPL-3.0-only `syntrax-c` renderer on
  desktop and mobile. The separate `kog-syntrax-helper` remains a desktop
  protocol regression target. GPL-3.0-only is compatible with distribution of
  the combined executable under GPL version 3; binary distributions must
  retain the pinned source and notices.
- SNSF links the ISC ares Super Famicom subset and nall/libco components,
  retaining the notices of the actual vendored source set. Its new Kog
  adapter is GPL3-or-later, with MIT psflib and zlib. Snes9x and the old
  non-commercial helper adapter are not linked or included in binary
  packages. No external enhancement-chip firmware is bundled.
- SC-55 playback uses the pinned Nuked SC-55 0.7.0 backend. The upstream fork
  is now GPL-2.0-or-later, so Kog uses its GPL version 3 option for the combined
  audio library on all platforms. The pinned source, its GPL license,
  and Kog's adapter source accompany distributions. Roland ROMs are supplied by
  the user and are never included in Kog.
- A decoder whose license adds non-commercial or other GPL-incompatible terms
  does not become link-compatible merely because Kog is intended as a
  non-commercial project. Such a component requires an independently reviewed,
  clearly identified optional-program boundary or a compatible replacement;
  it is never represented as GPL-covered Kog code.
- Proprietary console BIOS images, game data, SoundFonts, Roland firmware/sample ROMs, and
  synthesis banks are not redistributed. A backend may load a user-owned asset
  when its format and applicable terms allow that use.

Every new decoder milestone records its pinned source revision, enabled source
set, license, asset policy, and redistribution boundary before support is
claimed.

# Kog license policy

Kog-authored source code is licensed under the GNU General Public License,
version 3 or (at your option) any later version (`GPL-3.0-or-later`), except the
small `native/snsf-helper` adapter identified as `LicenseRef-Snes9x`. That
adapter is distributed only with its separately licensed non-commercial SNSF
helper. The GPL text for the main application is in the repository root
`LICENSE` file.

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
- GPL-2.0-only components are not linked into the main application. The
  upstream libvgm tree contains one such YMF278B core; Kog explicitly disables
  it while retaining all other configured libvgm chip families. PSF playback
  instead compiles GPL-2.0-only libupse and its GPL-2.0-only Kog adapter into a
  separate `kog-psf-helper` program with a documented PCM protocol. The helper
  and its corresponding source must be distributed under their own terms and
  installed beside Kog; changing Kog's root license would not make GPL-2.0-only
  and GPL-3.0-only code link-compatible.
- PSF2 playback uses the BSD-licensed Play! emulator and its permissively
  licensed dependencies in a separate `kog-psf2-helper`. That boundary keeps
  the large emulator and legacy container parsers out of Kog's address space;
  it is an engineering and fault-containment choice, not a license workaround.
  The GPL-3.0-or-later Kog adapter, Play! notices, and dependency notices must
  accompany binary distributions. Play! is not relicensed by Kog, and no root
  license change is required to use its BSD-licensed code.
- 2SF playback uses the official GPL-3.0-or-later melonDS core plus psflib and
  system zlib in a separate `kog-2sf-helper`. These terms are compatible with
  Kog's GPL-3.0-or-later license. The executable boundary keeps a full emulator
  and untrusted xSF parsing out of Kog's address space; it is a fault-isolation
  decision, not a license workaround. Binary distributions must install the
  helper beside Kog and carry the complete corresponding source and upstream
  copyright/license notices. melonDS is not relicensed by Kog.
- Syntrax playback uses the GPL-3.0-only `syntrax-c` renderer in a separate
  `kog-syntrax-helper`. GPL-3.0-only is compatible with Kog's GPL-3.0-or-later
  source, while the combined helper executable is distributed under GPL
  version 3 only. The boundary contains faults from the legacy trusted-input
  parser and renderer; it is not a license workaround. Binary distributions
  must install the helper and retain the pinned source and notices.
- SNSF playback uses the pinned libsnsf9x library in the optional,
  independently identified `kog-snsf-helper` program. Its Snes9x-derived core
  permits personal/non-commercial use and adds terms incompatible with the
  main application's GPL-3.0-or-later license; its APU components also retain
  LGPL-2.1 terms. The helper and its adapter therefore retain those upstream
  terms, are not linked into Kog, and must be distributed with the complete
  retained notices and source. psflib and zlib do not alter that boundary.
  This matches Kog's non-commercial project intent but does not relicense
  Snes9x or make the two licenses link-compatible.
- A decoder whose license adds non-commercial or other GPL-incompatible terms
  does not become link-compatible merely because Kog is intended as a
  non-commercial project. Such a component requires an independently reviewed,
  clearly identified optional-program boundary or a compatible replacement;
  it is never represented as GPL-covered Kog code.
- Proprietary console BIOS images, game data, SoundFonts, sample ROMs, and
  synthesis banks are not redistributed. A backend may load a user-owned asset
  when its format and applicable terms allow that use.

Every new decoder milestone records its pinned source revision, enabled source
set, license, asset policy, and redistribution boundary before support is
claimed.

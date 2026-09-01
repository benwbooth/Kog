# Kog license policy

Kog-authored source code is licensed under the GNU General Public License,
version 3 or (at your option) any later version (`GPL-3.0-or-later`). The full
license is in the repository root `LICENSE` file.

Third-party libraries, emulator cores, test fixtures, and user-supplied assets
are not relicensed. Their copyright notices and license terms remain in their
source trees and are summarized in `THIRD_PARTY_NOTICES.md`.

## Decoder boundaries

- Permissive, LGPL, MPL, GPL-3.0-only, and GPL-2.0-or-later components may be
  linked into the main application when their exact terms are compatible with
  GPL-3.0-or-later and their redistribution requirements are satisfied.
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

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
  it while retaining all other configured libvgm chip families.
- A decoder whose license adds non-commercial or other GPL-incompatible terms
  may be supported because Kog is intended as a non-commercial project, but it
  must retain its own license and be packaged as a clearly identified optional
  helper rather than being represented as GPL-covered Kog code.
- Proprietary console BIOS images, game data, SoundFonts, sample ROMs, and
  synthesis banks are not redistributed. A backend may load a user-owned asset
  when its format and applicable terms allow that use.

Every new decoder milestone records its pinned source revision, enabled source
set, license, asset policy, and redistribution boundary before support is
claimed.

# Kog release packages

Kog's release workflow produces native packages from the same source revision:

- Windows: a portable ZIP containing `Kog.exe`, its bundled helper programs,
  Qt, codec DLLs, and an MSI installer for the same directory tree.
- macOS: separate Apple Silicon and Intel DMGs containing a self-contained
  `Kog.app`. Tagged builds also generate a Homebrew cask whose URLs and SHA-256
  values point at those exact release assets.
- Linux: an x86_64 AppImage, a portable AppDir tarball, a Flatpak bundle, and an
  OSTree Flatpak repository archive.

The portable Linux tarball is the practical replacement for a “static binary.”
Kog cannot honestly be distributed as one fully static executable while it uses
Qt Quick platform/QML plugins, FFmpeg, libarchive, desktop portals, and the host
audio stack. Both the AppImage and the AppDir tarball bundle the required shared
libraries while preserving the dynamic plugin model Qt requires.

The Nix shell and Flatpak include Layer Shell Qt for positioning custom
notifications above the panel on supported Wayland desktops. The older Qt
baseline used by the AppImage does not bundle that optional module; without it,
Wayland's compositor controls popup placement. Windows, macOS, and X11 use normal
window positioning. Notification positions are saved independently of playback
settings; drag the popup header to move it, or right-click it to reset.

Main-window position restoration on Wayland uses Qt's session-restore interface
when building against Qt 6.10 or newer with the matching private development
headers, and requires compositor support for that protocol. Other Wayland
builds still save size and maximized state; positioning remains compositor-owned.
Windows, macOS, and X11 save and restore normal geometry directly. Geometry tests
can be run with `nix develop -c bash tests/native/run-window-state.sh`.

The workflows build unsigned development artifacts on branch and pull-request
runs. Tagged `v*` builds create or update the matching GitHub release. macOS
artifacts are ad-hoc signed, not Apple-notarized; Windows artifacts are not
Authenticode-signed. Release signing and Apple notarization require maintainer
certificates and are intentionally separate from reproducible package creation.

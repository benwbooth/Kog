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

The workflows build unsigned development artifacts on branch and pull-request
runs. Tagged `v*` builds create or update the matching GitHub release. macOS
artifacts are ad-hoc signed, not Apple-notarized; Windows artifacts are not
Authenticode-signed. Release signing and Apple notarization require maintainer
certificates and are intentionally separate from reproducible package creation.

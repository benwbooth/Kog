# Building Kog

The supported development environment is the included Nix flake:

```sh
git clone --recurse-submodules https://github.com/benwbooth/Kog.git
cd Kog
nix develop
cargo run
```

For a fast change → compile → run loop, use the watcher: it rebuilds on change
and restarts the app, killing the previous instance first so the window reopens
with your change already in it. It is a thin wrapper over `watchexec` (also in
the shell, if you want to drive it yourself).

```sh
scripts/dev.sh              # rebuild and restart on change
scripts/dev.sh --check      # type-check only: fastest feedback, no linking
scripts/dev.sh --test       # rerun the workspace tests on change
scripts/dev.sh --release    # release profile
scripts/dev.sh --web        # also rebuild the wasm frontend each restart
scripts/dev.sh --step       # rebuild stale web assets and restart once
```

Keep `scripts/dev.sh` running for automatic restarts. Edits under
`crates/kog-web`, including CSS, rebuild the embedded frontend automatically;
`--web` forces that rebuild even when its sources have not changed.

`bacon` and `cargo-watch` are in the shell too if you prefer them.

The first build is the slow one: it compiles the native decoder libraries and
links a very large Qt binary. After that:

- Only the crates you touched are recompiled, and `mold` (configured in the dev
  shell) cuts the app's link step to seconds.
- Avoid editing `flake.nix` while iterating. It rebuilds the shell environment,
  which changes the compiler flags for every build script and forces the native
  libraries to rebuild.
- `--check` compiles no binary at all, so it is the quickest way to chase a type
  error.
- Do not iterate with `nix build`: that is a release build in a sandbox from a
  fresh source tree.

For an existing checkout, initialize the native sources first:

```sh
git submodule update --init --recursive
```

A direct Cargo build requires Rust, C and C++23 compilers, CMake, `pkg-config`,
Qt 6 with Qt Quick and Qt Quick Controls, FFmpeg development libraries, zlib,
and libarchive 3.2 or newer. On Wayland, install KDE's Layer Shell Qt QML module
for corner-anchored, draggable now-playing popups. The Nix shell supplies it,
along with the dependency versions and FFmpeg configuration used by Kog's
regression tests.

For mobile builds, see [Android](../android/README.md) and [iOS](../ios/README.md).
GitHub's **Cross-platform packages** workflow builds all desktop and mobile
packages together. Android APKs and unsigned iOS IPAs are included in tagged
releases; the iOS IPA needs signing for the intended device before installation.
For distribution, see [packaging](../packaging/README.md) and [releasing](RELEASING.md).

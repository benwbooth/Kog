#!/usr/bin/env bash
#
# Run Cargo in the target directory shared by the dev loop and the Nix package.
#
# Both sync the source to one fixed path and build there, so the package reuses
# whatever the dev loop compiled. Run through `nix build` directly, the loop
# would spend over a minute per edit copying native/ into the store.
#
# Usage: scripts/shared-build.sh <source dir> <cargo subcommand> [args...]

set -euo pipefail

source_dir="$(cd "$1" && pwd)"
shift

work_dir="${KOG_WORK_DIR:-/var/tmp/cargo-nix/kog}"

# The dev loop and Nix's build users all write here, so everything stays
# writable by everyone. Build scripts copy read-only headers out of the store.
umask 000
mkdir -p "$work_dir/src" "$work_dir/target" "$work_dir/cargo-home"
touch "$work_dir"

# The dev loop and Nix's build users can run at the same time. Cargo's own lock
# only covers Cargo, so hold one from syncing the source through adopting files:
# otherwise one side rewrites src/ or briefly empties a directory under target/
# while the other is compiling, and that side starts over from scratch.
flock="${KOG_FLOCK:-flock}"
exec {lock_fd}>>"$work_dir/build.lock"
if ! "$flock" -n "$lock_fd"; then
  echo "[shared-build] waiting for another build in $work_dir" >&2
  "$flock" "$lock_fd"
fi
open_target() {
  find "$work_dir/target" -mindepth 1 -user "$(id -u)" ! -perm -o+w -exec chmod a+rwX {} + 2>/dev/null || true
}
trap open_target EXIT

# Only rewrite files whose content changed: Cargo's freshness check relies on
# mtimes. From a checkout, copy what a Nix build would see and not build
# output: a file only one side has would make the other side rebuild.
sync=(rsync -lc --no-times --no-perms --chmod=a+rwX)
if [[ -e "$source_dir/.git" ]]; then
  # Nix fetches raw blobs, but some submodules ask Git for CRLF on checkout.
  # Any difference under native/ reruns the native build, so turn that off.
  git -C "$source_dir" submodule --quiet foreach --recursive '
    attributes="$(git rev-parse --git-path info/attributes)"
    grep -qsx "* -text" "$attributes" && exit 0
    mkdir -p "$(dirname "$attributes")"
    echo "* -text" >>"$attributes"
    git checkout -q -- .
  '
  file_list="$work_dir/source-files"
  {
    git -C "$source_dir" ls-files -z --recurse-submodules
    git -C "$source_dir" ls-files -z --others --exclude-standard
    (cd "$source_dir" && find crates/kog-server/web -type f -print0)
  } | sort -zu > "$file_list"
  "${sync[@]}" --from0 --files-from="$file_list" "$source_dir/" "$work_dir/src/"
  (cd "$work_dir/src" && find . ! -type d -printf '%P\0' | sort -z \
    | comm -z -23 - "$file_list" | xargs -0 -r rm -f)
else
  "${sync[@]}" -r --delete "$source_dir/" "$work_dir/src/"
fi

# Set by the flake: the vendored dependencies, at a store path that only
# changes with Cargo.lock. Also keeps kache out of these builds.
cmp -s "$KOG_CARGO_VENDOR/config.toml" "$work_dir/cargo-home/config.toml" \
  || cp -f "$KOG_CARGO_VENDOR/config.toml" "$work_dir/cargo-home/config.toml"

# Only read at run time here; set during a build it changes what build.rs does.
unset KOG_QML_DIR
# The linker wrapper rejects output paths outside the build directory.
export NIX_ENFORCE_PURITY=0
export CARGO_HOME="$work_dir/cargo-home"
export CARGO_TARGET_DIR="$work_dir/target"
export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
# Build scripts rerun when these differ, and the dev shell's are not the
# package's. The flake sets the search path for both.
export PKG_CONFIG_PATH="$(<"$KOG_PKG_CONFIG_PATH")"
export PKG_CONFIG_PATH_x86_64_unknown_linux_gnu="$PKG_CONFIG_PATH"
export CC=gcc CXX=g++ AR=ar
unset CFLAGS CXXFLAGS LDFLAGS ARFLAGS PKG_CONFIG_LIBDIR

cd "$work_dir/src"
# CMake's install step can't chmod what another user built, so a rerun of a
# build script fails until those files are ours. Keeps mtimes for Cargo.
adopt_target() {
  local path
  [[ -n "$(find "$work_dir/target" ! -user "$(id -u)" -print -quit)" ]] || return 1
  echo "[shared-build] taking over files from another build user" >&2
  while IFS= read -r -d '' path; do
    if [[ -d "$path" && ! -L "$path" ]]; then
      mv "$path" "$path.adopt"
      mkdir "$path"
      find "$path.adopt" -mindepth 1 -maxdepth 1 -exec mv -t "$path" {} +
      rmdir "$path.adopt"
    else
      cp -a --reflink=auto "$path" "$path.adopt"
      mv -f "$path.adopt" "$path"
    fi
  done < <(find "$work_dir/target" -depth -mindepth 1 ! -user "$(id -u)" -print0)
  open_target
}
cargo "$@" --offline --locked || { adopt_target && cargo "$@" --offline --locked; }

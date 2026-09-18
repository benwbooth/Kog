#!/usr/bin/env bash
#
# Fast change -> compile -> run loop, built on watchexec.
#
# Run inside the dev shell (`nix develop`, then `scripts/dev.sh`). watchexec
# rebuilds and restarts the app on every change, killing the previous instance
# first, so a Qt window reopens with your change already in it.
#
# Usage:
#   scripts/dev.sh                     # debug build, restart the app on change
#   scripts/dev.sh --release           # release build
#   scripts/dev.sh --check             # cargo check only (no link, fastest)
#   scripts/dev.sh --test              # rerun the workspace tests on change
#   scripts/dev.sh --web               # also rebuild the wasm frontend each restart
#   scripts/dev.sh -- <app args...>    # pass arguments through to the app
#
# Editing flake.nix rebuilds the whole dev shell environment and invalidates
# cargo's cache; avoid it while iterating.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 1

mode="run"        # run | check | test
profile="debug"
web=0
app_args=()

while (( $# )); do
  case "$1" in
    --release) profile="release" ;;
    --check) mode="check" ;;
    --test) mode="test" ;;
    --web) web=1 ;;
    --) shift; app_args=("$@"); break ;;
    -h|--help) sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1 (try --help)" >&2; exit 2 ;;
  esac
  shift
done

for tool in cargo watchexec; do
  command -v "$tool" >/dev/null || {
    echo "$tool is not on PATH: run this inside 'nix develop'." >&2
    exit 1
  }
done

profile_args=()
[[ "$profile" == "release" ]] && profile_args=(--release)

# Only the sources matter; watchexec otherwise skips gitignored paths (target/,
# the generated web assets) on its own.
watch_args=(
  --watch src
  --watch crates
  --watch qml
  --watch build.rs
  --watch Cargo.toml
  --exts rs,qml,toml,json,svg,css,html
  --debounce 300ms
  # Ask the app to quit, but do not let a wedged one hold up the restart.
  --stop-signal SIGTERM
  --stop-timeout 3s
  --restart
)

case "$mode" in
  check)
    # Plain --workspace: test and bench targets are compile-checked by --test,
    # and including them here would only slow the fastest feedback path down.
    command=(cargo check "${profile_args[@]}" --workspace)
    ;;
  test)
    command=(cargo test "${profile_args[@]}" --workspace)
    ;;
  run)
    command=(cargo run "${profile_args[@]}")
    (( ${#app_args[@]} )) && command+=(-- "${app_args[@]}")
    ;;
esac

if (( web )) && [[ "$mode" != "test" ]]; then
  # The app embeds the built frontend, so regenerate it before every run.
  shell_command="crates/kog-web/build.sh && ${command[*]}"
  exec watchexec "${watch_args[@]}" --shell=bash -- "$shell_command"
fi

exec watchexec "${watch_args[@]}" -- "${command[@]}"

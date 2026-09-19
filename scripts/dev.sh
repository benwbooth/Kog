#!/usr/bin/env bash
#
# Fast change -> compile -> run loop.
#
# The app is only replaced once a new build is ready: watchexec triggers a
# build step, and that step builds first and swaps the running binary
# afterwards. A failed build leaves the app you already have running alone, so
# you are never left without a window.
#
# Run inside the dev shell (`nix develop`, then `scripts/dev.sh`).
#
# Usage:
#   scripts/dev.sh                     # debug build, run, restart on change
#   scripts/dev.sh --release           # release build
#   scripts/dev.sh --check             # cargo check only (no link, no app)
#   scripts/dev.sh --test              # rerun the workspace tests on change
#   scripts/dev.sh --web               # also rebuild the wasm frontend
#   scripts/dev.sh -- <app args...>    # pass arguments through to the app
#
# The app's own output goes to target/dev-app.log.
#
# Editing flake.nix rebuilds the whole dev shell environment and invalidates
# cargo's cache; avoid it while iterating.

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 1

mode="run"        # run | check | test
profile="debug"
web=0
step=0
app_args=()

while (( $# )); do
  case "$1" in
    --step) step=1 ;;
    --release) profile="release" ;;
    --check) mode="check" ;;
    --test) mode="test" ;;
    --web) web=1 ;;
    --) shift; app_args=("$@"); break ;;
    -h|--help) sed -n '2,25p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
binary="target/$profile/kog"
pidfile="target/dev-app.pid"
app_log="target/dev-app.log"

# --------------------------------------------------------------------- step
#
# One build-and-swap pass. This is what watchexec runs on every settled change.
if (( step )); then
  # watchexec passes the changed paths in the environment, so a frontend edit
  # rebuilds the wasm without needing --web on every run. The generated assets
  # land under crates/kog-server/web, which is a different path, so running the
  # frontend build again cannot retrigger itself.
  rebuild_web=$web
  if [[ "$mode" != "test" ]]; then
    case "${WATCHEXEC_WRITTEN_PATH:-}${WATCHEXEC_CREATED_PATH:-}${WATCHEXEC_RENAMED_PATH:-}${WATCHEXEC_META_CHANGED_PATH:-}" in
      *crates/kog-web/*) rebuild_web=1 ;;
    esac
  fi
  if (( rebuild_web )); then
    if ! crates/kog-web/build.sh; then
      echo "[dev] frontend build failed; keeping the running app" >&2
      exit 1
    fi
  fi

  case "$mode" in
    check) command=(cargo check "${profile_args[@]}" --workspace) ;;
    test) command=(cargo test "${profile_args[@]}" --workspace) ;;
    run) command=(cargo build "${profile_args[@]}") ;;
  esac

  if ! "${command[@]}"; then
    echo "[dev] build failed; keeping the running app" >&2
    exit 1
  fi

  if [[ "$mode" == "run" ]]; then
    # Only now is the old instance retired: the new binary is on disk and
    # built, so the window is gone for a moment rather than for the build.
    if [[ -f "$pidfile" ]]; then
      old="$(cat "$pidfile" 2>/dev/null || true)"
      if [[ -n "$old" ]] && kill -0 "$old" 2>/dev/null; then
        kill "$old" 2>/dev/null || true
        for _ in $(seq 1 30); do
          kill -0 "$old" 2>/dev/null || break
          sleep 0.1
        done
        kill -KILL "$old" 2>/dev/null || true
      fi
      rm -f "$pidfile"
    fi
    # Started in its own session so watchexec stopping the build step cannot
    # take the app down with it.
    setsid nohup "$root/$binary" "${app_args[@]}" >>"$app_log" 2>&1 </dev/null &
    echo $! >"$pidfile"
    echo "[dev] $(date +%T) started $binary (pid $(cat "$pidfile"), log $app_log)"
  fi
  exit 0
fi

# ---------------------------------------------------------------- supervisor

stop_app() {
  if [[ -f "$pidfile" ]]; then
    local old
    old="$(cat "$pidfile" 2>/dev/null || true)"
    if [[ -n "$old" ]]; then
      kill "$old" 2>/dev/null || true
    fi
    rm -f "$pidfile"
  fi
}
trap stop_app EXIT INT TERM

# Only the sources matter. Build output must be ignored explicitly: the web
# crate's target dir churns thousands of files per build, which would restart
# the loop endlessly.
watch_args=(
  --watch src
  --watch crates
  --watch qml
  --watch build.rs
  --watch Cargo.toml
  --ignore target
  --ignore crates/kog-web/target
  --ignore "**/*.tmp"
  --exts rs,qml,toml,json,svg,css,html
  # Let the filesystem settle before acting, so a build's output bursts do not
  # trigger another pass.
  --debounce 3s
)

step_args=(scripts/dev.sh --step)
[[ "$profile" == "release" ]] && step_args+=(--release)
[[ "$mode" != "run" ]] && step_args+=("--$mode")
(( web )) && step_args+=(--web)
if (( ${#app_args[@]} )); then
  step_args+=(--)
  for argument in "${app_args[@]}"; do
    step_args+=("$(printf '%q' "$argument")")
  done
fi

# Debug runs load QML from the source tree, so a QML edit only needs the app
# restarted - no compile of the QML module's C++.
[[ "$profile" == "debug" ]] && export KOG_QML_DIR="$root/qml"

echo "[dev] watching; app output goes to $app_log"
exec watchexec "${watch_args[@]}" --shell=bash -- "${step_args[*]}"

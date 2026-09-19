#!/usr/bin/env bash
# Build the web frontend into the assets kog-server embeds.
#
# Trunk is not used on purpose: it downloads a wasm-bindgen CLI that matches the
# crate, which cannot happen in a sandboxed build. The nix dev shell already
# provides a rustc with the wasm32 target and a matching wasm-bindgen CLI, so
# the two steps are run directly and the build stays hermetic.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
target_dir="${KOG_WEB_TARGET_DIR:-$here/target}"
out_dir="${KOG_WEB_OUT_DIR:-$here/../kog-server/web}"

cd "$here"

# The dev shell exports RUSTFLAGS with -fuse-ld=mold for native links; wasm-ld
# rejects that flag, so clear it for the wasm build. The linker is pinned in
# .cargo/config.toml.
RUSTFLAGS="" cargo build --release --target wasm32-unknown-unknown --target-dir "$target_dir"

wasm="$(find "$target_dir/wasm32-unknown-unknown/release" -maxdepth 1 -name 'kog_web.wasm' -print -quit)"
if [[ -z "$wasm" ]]; then
  echo "the wasm build produced no kog_web.wasm" >&2
  exit 1
fi

mkdir -p "$out_dir"
rm -f "$out_dir"/kog_web.js "$out_dir"/kog_web_bg.wasm "$out_dir"/kog_web.d.ts

wasm-bindgen --target web --no-typescript --out-dir "$out_dir" "$wasm"

cp "$here/index.html" "$out_dir/index.html"
cp "$here/style.css" "$out_dir/style.css"
cp -r "$here/icons" "$out_dir/icons"

echo "web frontend written to $out_dir"

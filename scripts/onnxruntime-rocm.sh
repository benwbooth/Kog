#!/usr/bin/env bash
# Build ONNX Runtime with ROCm (nix/onnxruntime-rocm.nix) and link it into
# Kog's data folder, where Kog looks for it when KOG_ONNXRUNTIME is unset.
# The link is a Nix GC root, so the build stays until the link is removed.
# Takes a while: ONNX Runtime compiles from source (ROCm is downloaded).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}/kog"
mkdir -p "$data"
nix build --impure -f "$root/nix/onnxruntime-rocm.nix" "$@" --out-link "$data/onnxruntime"
echo "ONNX Runtime with ROCm: $data/onnxruntime/lib/libonnxruntime.so"

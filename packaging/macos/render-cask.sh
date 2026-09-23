#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 TAG ARM64_DMG OUTPUT" >&2
  exit 2
fi

tag="$1"
arm_dmg="$2"
output="$3"
version="${tag#v}"
arm_sha="$(shasum -a 256 "$arm_dmg" | awk '{print $1}')"
mkdir -p "$(dirname "$output")"

sed \
  -e "s/@VERSION@/$version/g" \
  -e "s/@TAG@/$tag/g" \
  -e "s/@ARM_SHA@/$arm_sha/g" \
  "$(dirname "$0")/kog.rb.in" > "$output"

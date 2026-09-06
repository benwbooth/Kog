#!/usr/bin/env bash
set -euo pipefail
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$root_dir/packaging/macos/create-dmg.sh"
test_dir=$(mktemp -d)
trap 'rm -rf "$test_dir"' EXIT

# Deterministic command doubles test control flow, not actual macOS image creation.
hdiutil() {
  printf '%s\n' "$*" >> "$test_dir/calls"
  local count
  count=$(wc -l < "$test_dir/calls")
  if (( count <= failures )); then
    printf 'hdiutil: create failed - %s\n' "$error" >&2
    return 7
  fi
  echo 'created: test.dmg'
}
sleep() { printf '%s\n' "$1" >> "$test_dir/delays"; }

for scenario in immediate recovered exhausted permanent; do
  : > "$test_dir/calls"
  : > "$test_dir/delays"
  error='Resource busy'
  case "$scenario" in
    immediate) failures=0; expected_calls=1; expected_status=0; expected_delays='' ;;
    recovered) failures=2; expected_calls=3; expected_status=0; expected_delays=$'5\n10' ;;
    exhausted) failures=9; expected_calls=4; expected_status=7; expected_delays=$'5\n10\n15' ;;
    permanent) failures=9; error='Permission denied'; expected_calls=1; expected_status=7; expected_delays='' ;;
  esac
  status=0
  create_dmg -volname 'Kog test' -srcfolder 'test app' -ov -format UDZO test.dmg \
    > "$test_dir/output" 2>&1 || status=$?
  [[ "$status" == "$expected_status" ]]
  [[ $(wc -l < "$test_dir/calls") -eq "$expected_calls" ]]
  [[ $(< "$test_dir/delays") == "$expected_delays" ]]
  echo "PASS: $scenario"
done

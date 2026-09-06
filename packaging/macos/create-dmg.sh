#!/usr/bin/env bash

# Retry only the transient DiskImages error seen on hosted macOS runners.
# hdiutil's -ov option replaces an incomplete image on the next attempt.
create_dmg() {
  local attempt output status
  for attempt in 1 2 3 4; do
    if output=$(LC_ALL=C hdiutil create "$@" 2>&1); then
      printf '%s\n' "$output"
      return 0
    else
      status=$?
    fi
    printf '%s\n' "$output" >&2
    if [[ "$output" != *"hdiutil: create failed - Resource busy"* ]] || (( attempt == 4 )); then
      return "$status"
    fi
    printf 'DMG creation busy; retrying in %s seconds (attempt %s/4).\n' \
      "$((attempt * 5))" "$((attempt + 1))" >&2
    sleep "$((attempt * 5))"
  done
}

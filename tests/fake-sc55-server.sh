#!/usr/bin/env bash
# Fake kog-sc55-helper --server for Rust lifecycle tests: boots once,
# serves canned headers plus silent PCM per JOB line, exits on EOF.
# Only the JOB framing behavior is real; audio content is zeros.
set -u
LOG="${KOG_SC55_FAKE_LOG:-/dev/null}"
echo "boot $$" >>"$LOG"
echo "READY" >&2
# Header: KOGSC551 magic, version 1, 44100 Hz, 2 channels,
# 4410 total frames, start 0, model "Fake".
HEADER="$(printf '\113\117\107\123\103\065\065\061\001\000\000\000\104\254\000\000\002\000\000\000\072\021\000\000\000\000\000\000\000\000\000\000\000\000\000\000\004\000\000\000\106\141\153\145')"
while IFS= read -r line; do
    line="${line%$'\r'}"
    case "$line" in
        JOB$'\t'*)
            id="$(printf '%s' "$line" | cut -f2)"
            port="$(printf '%s' "$line" | cut -f4)"
            echo "job $id" >>"$LOG"
            exec 3<>"/dev/tcp/127.0.0.1/$port"
            printf '%s' "$HEADER" >&3
            head -c 17640 /dev/zero >&3
            exec 3>&-
            ;;
        *) echo "ignoring: $line" >&2 ;;
    esac
done

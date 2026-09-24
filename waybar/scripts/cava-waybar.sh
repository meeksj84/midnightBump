#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG="$SCRIPT_DIR/../cava-waybar.conf"

bars=("▁" "▂" "▃" "▄" "▅" "▆" "▇" "█")

cava -p "$CONFIG" 2>/dev/null |
while IFS=';' read -ra values; do
    output=""

    for value in "${values[@]}"; do
        [[ "$value" =~ ^[0-9]+$ ]] || continue

        (( value < 0 )) && value=0
        (( value > 7 )) && value=7

        output+="${bars[$value]}"
    done

    printf '%s\n' "$output"
done

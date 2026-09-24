#!/usr/bin/env bash

PLAYER="spotify"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

track="$(
    playerctl --player="$PLAYER" metadata \
        --format '{{artist}} — {{title}}' 2>/dev/null \
    || printf 'Spotify'
)"

status="$(playerctl --player="$PLAYER" status 2>/dev/null || true)"

if [[ "$status" == "Playing" ]]; then
    toggle="󰏤  Pause"
else
    toggle="󰐊  Play"
fi

choice="$(
    printf '%s\n' \
        "󰒮  Previous" \
        "$toggle" \
        "󰒭  Next" \
        "  Open Spotify" |
    rofi -dmenu -i -p "$track"
)"

case "$choice" in
    "󰒮  Previous")
        "$SCRIPT_DIR/previous-track.sh"
	;;
    "󰏤  Pause"|"󰐊  Play")
        playerctl --player="$PLAYER" play-pause
        ;;
    "󰒭  Next")
        playerctl --player="$PLAYER" next
        ;;
    "  Open Spotify")
        setsid -f flatpak run com.spotify.Client >/dev/null 2>&1
        ;;
esac

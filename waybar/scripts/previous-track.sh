#!/usr/bin/env bash

PLAYER="spotify"

position="$(playerctl --player="$PLAYER" position 2>/dev/null || echo 0)"

if awk "BEGIN {exit !($position > 3)}"; then
    # Spotify's first "previous" restarts the current song.
    playerctl --player="$PLAYER" previous
    sleep 0.15
    playerctl --player="$PLAYER" previous
else
    # Near the beginning, one press already goes back a track.
    playerctl --player="$PLAYER" previous
fi

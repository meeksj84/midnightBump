#!/usr/bin/env bash
# Read-only Midnight Bump health check. No sudo, no writes.
set -uo pipefail

ok() { printf '[PASS] %s\n' "$*"; }
warn() { printf '[WARN] %s\n' "$*"; }
check_file() {
    if [[ -e "$1" ]]; then ok "$2"; else warn "$2 missing: $1"; fi
}
check_command() {
    if command -v "$1" >/dev/null 2>&1; then ok "$1"; else warn "$1 not installed"; fi
}

printf '\nMidnight Bump: read-only installation check\n'
printf 'User: %s\n\n' "$(id -un)"
for cmd in hyprctl uwsm waybar kitty awww awww-daemon matugen \
    hyprlock hypridle mako rofi playerctl cava flatpak \
    nmcli blueman-manager wpctl; do
    check_command "$cmd"
done

check_file "$HOME/.config/hypr/hyprland.lua" 'Hyprland Lua config'
check_file "$HOME/.config/hypr/keybindings.lua" 'Hyprland keybindings'
check_file "$HOME/.config/waybar/config.jsonc" 'Waybar layout'
check_file "$HOME/.config/waybar/style.css" 'Waybar theme'
check_file "$HOME/.config/waybar/midnight-bump-colors.css" 'Generated Waybar color palette'
check_file "$HOME/.config/matugen/config.toml" 'Matugen configuration'
check_file "$HOME/.config/systemd/user/midnight-bump-waybar.service" 'Waybar session service'
check_file "$HOME/.config/systemd/user/awww-daemon.service" 'Wallpaper daemon service'

for service in NetworkManager bluetooth sddm; do
    if systemctl is-enabled --quiet "$service.service" 2>/dev/null; then
        ok "$service.service enabled"
    else
        warn "$service.service not enabled"
    fi
done
for unit in midnight-bump-waybar midnight-bump-mako midnight-bump-hypridle awww-daemon midnight-bump-wallpaper; do
    if systemctl --user is-enabled --quiet "$unit.service" 2>/dev/null; then
        ok "$unit.service enabled for Hyprland session"
    else
        warn "$unit.service not enabled (or user systemd unavailable)"
    fi
done

if [[ -n "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]]; then
    ok 'Currently running under Hyprland'
else
    warn 'Not currently in Hyprland: session widgets cannot be checked until login'
fi
if command -v flatpak >/dev/null 2>&1 &&
   flatpak list --user --app --columns=application 2>/dev/null | grep -qx 'com.spotify.Client'; then
    ok 'Spotify Flatpak installed'
else
    warn 'Spotify Flatpak not installed for this user'
fi
if command -v wlogout >/dev/null 2>&1; then
    ok 'wlogout installed'
else
    warn 'wlogout missing: power menu requires AUR installation'
fi

printf '\nReview warnings, then reboot and select Hyprland (uwsm-managed) if needed.\n'

#!/usr/bin/env bash

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

info() {
    printf '\033[1;34m[INFO]\033[0m %s\n' "$*"
}

warn() {
    printf '\033[1;33m[WARN]\033[0m %s\n' "$*" >&2
}

error() {
    printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2
}

backup_path() {
    local path="$1"

    if [[ -L "$path" ]]; then
        rm "$path"
        return
    fi

    if [[ -e "$path" ]]; then
        local timestamp
        timestamp="$(date +%Y%m%d-%H%M%S)"
        local backup="${path}.backup-${timestamp}"

        mv "$path" "$backup"
        info "Backed up $path to $backup"
    fi
}

link_file() {
    local source="$1"
    local target="$2"

    if [[ ! -e "$source" ]]; then
        error "Missing project file: $source"
        return 1
    fi

    mkdir -p "$(dirname "$target")"

    if [[ -L "$target" ]] && [[ "$(readlink -f "$target")" == "$(readlink -f "$source")" ]]; then
        info "Already linked: $target"
        return
    fi

    backup_path "$target"
    ln -s "$source" "$target"
    info "Linked $target -> $source"
}

link_directory() {
    local source="$1"
    local target="$2"

    if [[ ! -d "$source" ]]; then
        error "Missing project directory: $source"
        return 1
    fi

    mkdir -p "$(dirname "$target")"

    if [[ -L "$target" ]] && [[ "$(readlink -f "$target")" == "$(readlink -f "$source")" ]]; then
        info "Already linked: $target"
        return
    fi

    backup_path "$target"
    ln -s "$source" "$target"
    info "Linked $target -> $source"
}

check_command() {
    local command_name="$1"
    local package_hint="$2"

    if ! command -v "$command_name" >/dev/null 2>&1; then
        warn "Missing command '$command_name' (Arch package: $package_hint)"
        MISSING_COMMANDS=1
    fi
}

info "Installing Midnight Bump from: $PROJECT_DIR"

MISSING_COMMANDS=0

check_command hyprctl hyprland
check_command awww awww
check_command awww-daemon awww
check_command matugen matugen
check_command waybar waybar
check_command notify-send libnotify

# Optional commands used by the supplied Waybar configuration.
check_command rofi rofi-wayland
check_command wpctl wireplumber
check_command pavucontrol pavucontrol
check_command wlogout wlogout

if (( MISSING_COMMANDS )); then
    warn "Some commands are missing. The links will still be installed."
    warn "Install the missing packages before using every feature."
fi

mkdir -p \
    "$HOME/.config/hypr/scripts" \
    "$HOME/.config/waybar" \
    "$HOME/.cache"

link_file \
    "$PROJECT_DIR/hypr/scripts/set-wallpaper" \
    "$HOME/.config/hypr/scripts/set-wallpaper"

link_file \
    "$PROJECT_DIR/waybar/config.jsonc" \
    "$HOME/.config/waybar/config.jsonc"

link_file \
    "$PROJECT_DIR/waybar/style.css" \
    "$HOME/.config/waybar/style.css"

# Link the complete Matugen directory so config.toml can use paths relative
# to its own location, including ./templates/waybar-colors.css.
link_directory \
    "$PROJECT_DIR/matugen" \
    "$HOME/.config/matugen"

chmod +x "$PROJECT_DIR/hypr/scripts/set-wallpaper"

info "Generating an initial color palette..."

initial_wallpaper="$(
    find "$PROJECT_DIR/wallpapers" \
        -type f \
        \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' \) \
        -print -quit
)"

if [[ -n "$initial_wallpaper" ]]; then
    mkdir -p "$HOME/.config/waybar"

    matugen image "$initial_wallpaper" \
        --config "$PROJECT_DIR/matugen/config.toml" \
        --source-color-index 0

    info "Generated Waybar colors from: $initial_wallpaper"
else
    warn "No wallpaper was found, so the initial Matugen palette was skipped."
fi

cat <<'EOF'

Midnight Bump files are installed.

Add these commands to your Hyprland startup if they are not already present:

    awww-daemon
    waybar

Keep or add this wallpaper keybind in your Hyprland Lua configuration:

    d(mainMod .. " + W", hl.dsp.exec_cmd("~/.config/hypr/scripts/set-wallpaper"))

Then reload Hyprland:

    hyprctl reload

You can test the complete wallpaper/theme pipeline with:

    ~/.config/hypr/scripts/set-wallpaper

EOF

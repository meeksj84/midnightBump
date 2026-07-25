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
        local backup

        timestamp="$(date +%Y%m%d-%H%M%S)"
        backup="${path}.backup-${timestamp}"

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

    if [[ -L "$target" ]] &&
       [[ "$(readlink -f "$target")" == "$(readlink -f "$source")" ]]; then
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

    if [[ -L "$target" ]] &&
       [[ "$(readlink -f "$target")" == "$(readlink -f "$source")" ]]; then
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
        warn "Missing command '$command_name' (package: $package_hint)"
        MISSING_COMMANDS=1
    fi
}

install_packages() {
    local packages=(
        git
        firefox
        ufw
    )

    local missing_packages=()

    for package in "${packages[@]}"; do
        if ! pacman -Q "$package" >/dev/null 2>&1; then
            missing_packages+=("$package")
        fi
    done

    if (( ${#missing_packages[@]} == 0 )); then
        info "Required base packages are already installed."
        return
    fi

    info "Installing base packages: ${missing_packages[*]}"

    sudo pacman -S --needed "${missing_packages[@]}"
}

configure_firewall() {
    if ! command -v ufw >/dev/null 2>&1; then
        warn "UFW is unavailable; skipped firewall configuration."
        return
    fi

    info "Configuring UFW firewall..."

    sudo ufw default deny incoming
    sudo ufw default allow outgoing

    if sudo systemctl enable --now ufw.service; then
        sudo ufw --force enable
        info "UFW firewall is enabled."
    else
        warn "Could not enable ufw.service."
    fi
}

info "Installing Midnight Bump from: $PROJECT_DIR"

install_packages
configure_firewall

MISSING_COMMANDS=0

check_command hyprctl hyprland
check_command awww awww
check_command awww-daemon awww
check_command matugen matugen
check_command waybar waybar
check_command kitty kitty
check_command hyprlock hyprlock
check_command hypridle hypridle
check_command loginctl systemd
check_command mako mako
check_command makoctl mako
check_command notify-send libnotify

# Optional commands used by the supplied Waybar configuration.
check_command rofi rofi
check_command wpctl wireplumber
check_command pavucontrol pavucontrol
check_command wlogout "wlogout (AUR)"

if (( MISSING_COMMANDS )); then
    warn "Some commands are missing. Configuration links will still be installed."
    warn "Install the missing packages before using every feature."
fi

mkdir -p \
    "$HOME/.config/hypr/scripts" \
    "$HOME/.config/waybar" \
    "$HOME/.config/kitty" \
    "$HOME/.config/mako" \
    "$HOME/.cache"

link_file \
    "$PROJECT_DIR/hypr/scripts/set-wallpaper" \
    "$HOME/.config/hypr/scripts/set-wallpaper"

link_file \
    "$PROJECT_DIR/hypr/hyprlock/hyprlock.conf" \
    "$HOME/.config/hypr/hyprlock.conf"

link_file \
    "$PROJECT_DIR/hypr/hypridle/hypridle.conf" \
    "$HOME/.config/hypr/hypridle.conf"

link_file \
    "$PROJECT_DIR/waybar/config.jsonc" \
    "$HOME/.config/waybar/config.jsonc"

link_file \
    "$PROJECT_DIR/waybar/style.css" \
    "$HOME/.config/waybar/style.css"

link_file \
    "$PROJECT_DIR/kitty/kitty.conf" \
    "$HOME/.config/kitty/kitty.conf"

link_file \
    "$PROJECT_DIR/mako/config" \
    "$HOME/.config/mako/config"

# Link the complete Matugen directory so config.toml can use paths
# relative to its own location.
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
    mkdir -p \
        "$HOME/.config/waybar" \
        "$HOME/.config/kitty"

    if command -v matugen >/dev/null 2>&1; then
        matugen image "$initial_wallpaper" \
            --config "$PROJECT_DIR/matugen/config.toml" \
            --source-color-index 0

        info "Generated Waybar and Kitty colors from: $initial_wallpaper"
    else
        warn "Matugen is unavailable; skipped initial palette generation."
    fi
else
    warn "No wallpaper was found; skipped initial palette generation."
fi

# Enable Hypridle so loginctl lock-session launches Hyprlock.
if command -v hypridle >/dev/null 2>&1; then
    if systemctl --user list-unit-files hypridle.service \
        --no-legend 2>/dev/null |
        grep -q '^hypridle\.service'; then

        if systemctl --user enable --now hypridle.service; then
            info "Enabled Hypridle user service."
        else
            warn "Could not enable Hypridle through systemd."
            warn "Add 'hypridle' to Hyprland autostart instead."
        fi
    else
        warn "Hypridle service was not found."
        warn "Add 'hypridle' to Hyprland autostart."
    fi
fi

if command -v mako >/dev/null 2>&1; then
    if systemctl --user enable --now mako.service; then
        info "Enabled Mako user service."
    else
        warn "Could not enable Mako through systemd."
    fi
fi

POWER_BUTTON_SOURCE="$PROJECT_DIR/systemd/logind.conf.d/50-midnight-bump-power-button.conf"
POWER_BUTTON_TARGET="/etc/systemd/logind.conf.d/50-midnight-bump-power-button.conf"

if [[ -f "$POWER_BUTTON_SOURCE" ]]; then
    if [[ -t 0 ]]; then
        printf '\nInstall the power-button lock configuration? [y/N] '
        read -r install_power_button || install_power_button=""

        if [[ "$install_power_button" =~ ^[Yy]$ ]]; then
            sudo mkdir -p /etc/systemd/logind.conf.d
            sudo install -m 0644 \
                "$POWER_BUTTON_SOURCE" \
                "$POWER_BUTTON_TARGET"

            info "Installed power-button lock configuration."
            warn "Reboot before testing the power button."
        else
            warn "Skipped system power-button configuration."
        fi
    else
        warn "Non-interactive shell detected; skipped power-button configuration."
    fi
else
    warn "Power-button configuration file is missing:"
    warn "$POWER_BUTTON_SOURCE"
fi

cat <<'MESSAGE'

Midnight Bump files are installed.

Hyprland startup should include:

    awww-daemon
    waybar

If Hypridle could not be enabled as a user service, also add:

    hypridle

Keep or add this wallpaper keybind in your Hyprland Lua configuration:

    d(mainMod .. " + W", hl.dsp.exec_cmd("~/.config/hypr/scripts/set-wallpaper"))

Reload Hyprland with:

    hyprctl reload

Test the wallpaper and theme pipeline with:

    ~/.config/hypr/scripts/set-wallpaper

Test the lock path before using the physical power button:

    loginctl lock-session

MESSAGE

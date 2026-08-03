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

validate_project_files() {
    local required_files=(
        "hypr/scripts/set-wallpaper"
        "hypr/keybindings.lua"
        "hypr/hyprlock/hyprlock.conf"
        "hypr/hypridle/hypridle.conf"

        "waybar/config.jsonc"
        "waybar/style.css"

        "kitty/kitty.conf"
        "mako/config"

        "rofi/config.rasi"
        "rofi/midnight-bump.rasi"

        "wlogout/layout"
        "wlogout/style.css"

        "matugen/config.toml"
        "matugen/templates/waybar-colors.css"
        "matugen/templates/kitty-colors.conf"
        "matugen/templates/hyprlock-colors.conf"
        "matugen/templates/mako-colors.conf"
        "matugen/templates/rofi-colors.rasi"
        "matugen/templates/wlogout-colors.css"

        "systemd/logind.conf.d/50-midnight-bump-power-button.conf"
    )

    local missing_files=()
    local relative_path

    for relative_path in "${required_files[@]}"; do
        if [[ ! -f "$PROJECT_DIR/$relative_path" ]]; then
            missing_files+=("$relative_path")
        fi
    done

    if (( ${#missing_files[@]} > 0 )); then
        error "The cloned Midnight Bump repository is incomplete."
        error "Missing required files:"

        printf '  - %s\n' "${missing_files[@]}" >&2

        error "Pull the latest repository changes before running the installer."
        exit 1
    fi

    info "Project file validation passed."
}

install_packages() {
    local packages=(
        base-devel
        git
        firefox
        ufw

        hyprland
        waybar
        kitty
        dolphin

        awww
        matugen

        hyprlock
        hypridle

        mako
        libnotify

        rofi

        wireplumber
        pavucontrol
        brightnessctl
        playerctl

        network-manager-applet
        xdg-desktop-portal-hyprland
        qt6-wayland
        ttf-jetbrains-mono-nerd
    )

    local missing_packages=()
    local package

    for package in "${packages[@]}"; do
        if ! pacman -Q "$package" >/dev/null 2>&1; then
            missing_packages+=("$package")
        fi
    done

    if (( ${#missing_packages[@]} == 0 )); then
        info "Required official packages are already installed."
        return
    fi

    info "Installing official packages: ${missing_packages[*]}"

    sudo pacman -Syu --needed --noconfirm "${missing_packages[@]}"

    info "Official package installation completed."
}

install_paru() {
    if command -v paru >/dev/null 2>&1; then
        info "Paru is already installed."
        return
    fi

    local build_root
    build_root="$(mktemp -d)"

    info "Installing Paru from the AUR..."

    if ! git clone \
        --depth 1 \
        https://aur.archlinux.org/paru.git \
        "$build_root/paru"; then
        rm -rf "$build_root"
        error "Could not clone the Paru AUR repository."
        return 1
    fi

    if ! (
        cd "$build_root/paru"
        makepkg -si --needed --noconfirm
    ); then
        rm -rf "$build_root"
        error "Paru failed to build or install."
        return 1
    fi

    rm -rf "$build_root"

    if ! command -v paru >/dev/null 2>&1; then
        error "Paru installation did not complete successfully."
        return 1
    fi

    info "Paru installation completed."
}

install_aur_packages() {
    local packages=(
        wlogout
    )

    info "Installing Midnight Bump AUR packages..."

    paru -S --needed --noconfirm "${packages[@]}"

    info "AUR package installation completed."
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
validate_project_files

install_packages
install_paru
install_aur_packages
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
check_command rofi rofi
check_command notify-send libnotify

# Optional commands used by the supplied Waybar configuration.
check_command wpctl wireplumber
check_command pavucontrol pavucontrol
check_command paru paru
check_command wlogout wlogout

if (( MISSING_COMMANDS )); then
    warn "Some commands are missing. Configuration links will still be installed."
    warn "Install the missing packages before using every feature."
fi

mkdir -p \
    "$HOME/.config/hypr/scripts" \
    "$HOME/.config/waybar" \
    "$HOME/.config/kitty" \
    "$HOME/.config/mako" \
    "$HOME/.config/rofi" \
    "$HOME/.config/wlogout" \
    "$HOME/.cache"

link_file \
    "$PROJECT_DIR/hypr/scripts/set-wallpaper" \
    "$HOME/.config/hypr/scripts/set-wallpaper"

link_file \
    "$PROJECT_DIR/hypr/keybindings.lua" \
    "$HOME/.config/hypr/keybindings.lua"

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

link_file \
    "$PROJECT_DIR/rofi/config.rasi" \
    "$HOME/.config/rofi/config.rasi"

link_file \
    "$PROJECT_DIR/rofi/midnight-bump.rasi" \
    "$HOME/.config/rofi/midnight-bump.rasi"

link_file \
    "$PROJECT_DIR/wlogout/layout" \
    "$HOME/.config/wlogout/layout"

link_file \
    "$PROJECT_DIR/wlogout/style.css" \
    "$HOME/.config/wlogout/style.css"

HYPRLAND_CONFIG="$HOME/.config/hypr/hyprland.lua"

if [[ -f "$HYPRLAND_CONFIG" ]]; then
    if [[ ! -f "$HYPRLAND_CONFIG.midnight-bump.bak" ]]; then
        cp -a \
            "$HYPRLAND_CONFIG" \
            "$HYPRLAND_CONFIG.midnight-bump.bak"

        info "Backed up Hyprland config to:"
        info "$HYPRLAND_CONFIG.midnight-bump.bak"
    fi

    # Remove the legacy Dunst autostart entry.
    sed -i \
        '/^[[:space:]]*hl\.exec("dunst")[[:space:]]*$/d' \
        "$HYPRLAND_CONFIG"

    if ! grep -Fq 'require("keybindings")' "$HYPRLAND_CONFIG"; then
        printf '\nrequire("keybindings")\n' >> "$HYPRLAND_CONFIG"
        info "Enabled Midnight Bump keybindings."
    else
        info "Midnight Bump keybindings are already loaded."
    fi

    if ! grep -Fq 'hl.exec("awww-daemon")' "$HYPRLAND_CONFIG"; then
        printf 'hl.exec("awww-daemon")\n' >> "$HYPRLAND_CONFIG"
        info "Added awww-daemon to Hyprland startup."
    else
        info "awww-daemon is already in Hyprland startup."
    fi
else
    warn "Hyprland Lua config was not found:"
    warn "$HYPRLAND_CONFIG"
    warn "Could not enable keybindings or wallpaper startup automatically."
fi

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

if command -v waybar >/dev/null 2>&1; then
    if systemctl --user list-unit-files waybar.service \
        --no-legend 2>/dev/null |
        grep -q '^waybar\.service'; then

        if systemctl --user enable --now waybar.service; then
            info "Enabled Waybar user service."
        else
            warn "Could not enable Waybar through systemd."
        fi
    else
        warn "Waybar user service was not found."
    fi
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
    systemctl --user stop mako.service 2>/dev/null || true

    pkill -x dunst 2>/dev/null || true
    pkill -x swaync 2>/dev/null || true
    pkill -x mako 2>/dev/null || true

    systemctl --user reset-failed mako.service 2>/dev/null || true

    if systemctl --user enable --now mako.service; then
        info "Enabled Mako user service."
    else
        warn "Could not enable Mako through systemd."
    fi
fi

if command -v awww-daemon >/dev/null 2>&1 &&
   [[ -n "${WAYLAND_DISPLAY:-}" ]]; then

    if ! pgrep -x awww-daemon >/dev/null 2>&1; then
        nohup awww-daemon >/dev/null 2>&1 &
        sleep 1
    fi

    if pgrep -x awww-daemon >/dev/null 2>&1; then
        if "$HOME/.config/hypr/scripts/set-wallpaper"; then
            info "Started the wallpaper and theme pipeline."
        else
            warn "Could not apply the initial wallpaper."
        fi
    else
        warn "awww-daemon did not start."
    fi
else
    warn "No active Wayland session; wallpaper startup will occur at next Hyprland login."
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

Waybar and the wallpaper engine are configured automatically.

Midnight Bump keybindings are loaded through:

    require("keybindings")

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

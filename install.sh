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
        "packages/official.txt"
        "packages/aur.txt"

        "hypr/scripts/set-wallpaper"
        "hypr/keybindings.lua"
        "hypr/hyprlock/hyprlock.conf"
        "hypr/hypridle/hypridle.conf"

        "waybar/config.jsonc"
        "waybar/style.css"
        "waybar/cava-waybar.conf"
        "waybar/scripts/cava-waybar.sh"
        "waybar/scripts/music-menu.sh"
        "waybar/scripts/previous-track.sh"

        "kitty/kitty.conf"
        "fastfetch/config.jsonc"
        "shell/midnight-bump-prompt.sh"
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

        "systemd/user/awww-daemon.service"
        "systemd/user/midnight-bump-wallpaper.service"
        "systemd/logind.conf.d/50-midnight-bump-power-button.conf"
        "sddm/midnight-bump-theme/Main.qml"
        "sddm/midnight-bump-theme/metadata.desktop"
        "sddm/midnight-bump-theme/Themes/midnight-bump.conf"
        "sddm/midnight-bump-theme/Backgrounds/pixel_sakura.gif"
        "sddm/midnight-bump-theme/Fonts/ARCADECLASSIC.TTF"
        "sddm/config/10-midnight-bump-theme.conf"
        "sddm/config/20-midnight-bump-hidpi.conf"
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
    local package_file="$PROJECT_DIR/packages/official.txt"
    local packages=()
    local missing_packages=()
    local package

    mapfile -t packages < <(
        sed \
            -e 's/[[:space:]]*#.*$//' \
            -e '/^[[:space:]]*$/d' \
            "$package_file"
    )

    if (( ${#packages[@]} == 0 )); then
        warn "Official package manifest is empty:"
        warn "$package_file"
        return
    fi

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
    local package_file="$PROJECT_DIR/packages/aur.txt"
    local packages=()

    mapfile -t packages < <(
        sed \
            -e 's/[[:space:]]*#.*$//' \
            -e '/^[[:space:]]*$/d' \
            "$package_file"
    )

    if (( ${#packages[@]} == 0 )); then
        info "No Midnight Bump AUR packages are configured."
        return
    fi

    info "Installing Midnight Bump AUR packages: ${packages[*]}"

    paru -S --needed --noconfirm "${packages[@]}"

    info "AUR package installation completed."
}

install_spotify_flatpak() {
    if ! command -v flatpak >/dev/null 2>&1; then
        warn "Flatpak is unavailable; Spotify was not installed."
        return
    fi

    info "Configuring Flathub..."

    if ! flatpak remote-add --user --if-not-exists \
        flathub https://flathub.org/repo/flathub.flatpakrepo; then
        warn "Could not configure Flathub."
        return
    fi

    if flatpak list --app --columns=application 2>/dev/null |
        grep -qx 'com.spotify.Client'; then

        info "Spotify Flatpak is already installed."
    else
        info "Installing Spotify Flatpak..."

        if ! flatpak install --user -y flathub com.spotify.Client; then
            warn "Spotify Flatpak installation failed."
        fi
    fi
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
install_spotify_flatpak
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
check_command playerctl playerctl
check_command cava cava
check_command flatpak flatpak
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
    "$HOME/.config/fastfetch" \
    "$HOME/.config/midnight-bump" \
    "$HOME/.config/mako" \
    "$HOME/.config/rofi" \
    "$HOME/.config/wlogout" \
    "$HOME/.config/systemd/user" \
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

link_directory \
    "$PROJECT_DIR/waybar/scripts" \
    "$HOME/.config/waybar/scripts"

link_file \
    "$PROJECT_DIR/kitty/kitty.conf" \
    "$HOME/.config/kitty/kitty.conf"

link_file \
    "$PROJECT_DIR/fastfetch/config.jsonc" \
    "$HOME/.config/fastfetch/config.jsonc"

link_file \
    "$PROJECT_DIR/shell/midnight-bump-prompt.sh" \
    "$HOME/.config/midnight-bump/prompt.sh"

BASH_PROMPT_LINE='source "$HOME/.config/midnight-bump/prompt.sh"'

if ! grep -Fqx "$BASH_PROMPT_LINE" "$HOME/.bashrc" 2>/dev/null; then
    printf '\n%s\n' "$BASH_PROMPT_LINE" >> "$HOME/.bashrc"
    info "Enabled Midnight Bump Bash prompt."
else
    info "Midnight Bump Bash prompt is already enabled."
fi

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
    "$PROJECT_DIR/systemd/user/awww-daemon.service" \
    "$HOME/.config/systemd/user/awww-daemon.service"

link_file \
    "$PROJECT_DIR/systemd/user/midnight-bump-wallpaper.service" \
    "$HOME/.config/systemd/user/midnight-bump-wallpaper.service"

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

    # Remove legacy direct awww-daemon startup entries.
    sed -i \
        '/^[[:space:]]*hl\.exec("awww-daemon")[[:space:]]*$/d' \
        "$HYPRLAND_CONFIG"

    sed -i \
        '/^[[:space:]]*hl\.exec_cmd("\/usr\/bin\/awww-daemon")[[:space:]]*$/d' \
        "$HYPRLAND_CONFIG"

    if ! grep -Fq 'require("keybindings")' "$HYPRLAND_CONFIG"; then
        printf '\nrequire("keybindings")\n' >> "$HYPRLAND_CONFIG"
        info "Enabled Midnight Bump keybindings."
    else
        info "Midnight Bump keybindings are already loaded."
    fi

else
    warn "Hyprland Lua config was not found:"
    warn "$HYPRLAND_CONFIG"
    warn "Could not enable keybindings automatically."
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
        \( \
            -iname '*.jpg' \
            -o -iname '*.jpeg' \
            -o -iname '*.png' \
            -o -iname '*.webp' \
        \) \
        -print \
        -quit
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

# Remove stale enable-links from the pre-UWSM service layout.
rm -f \
    "$HOME/.config/systemd/user/graphical-session.target.wants/awww-daemon.service" \
    "$HOME/.config/systemd/user/graphical-session.target.wants/midnight-bump-wallpaper.service"

if command -v awww-daemon >/dev/null 2>&1; then
    systemctl --user daemon-reload

    if systemctl --user enable --now awww-daemon.service; then
        info "Enabled awww-daemon user service."

        for _ in {1..10}; do
            if pgrep -x awww-daemon >/dev/null 2>&1; then
                break
            fi

            sleep 0.2
        done

        if pgrep -x awww-daemon >/dev/null 2>&1; then
            info "awww-daemon is running."
        else
            warn "awww-daemon service started, but the process was not detected."
        fi
    else
        warn "Could not enable awww-daemon through systemd."
    fi
fi

if [[ -f "$HOME/.config/systemd/user/midnight-bump-wallpaper.service" ]]; then
    systemctl --user daemon-reload

    if systemctl --user enable --now midnight-bump-wallpaper.service; then
        info "Enabled automatic Midnight Bump wallpaper rotation."
    else
        warn "Could not enable automatic wallpaper rotation."
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

SDDM_THEME_SOURCE="$PROJECT_DIR/sddm/midnight-bump-theme"
SDDM_THEME_TARGET="/usr/share/sddm/themes/midnight-bump"

SDDM_THEME_CONFIG_SOURCE="$PROJECT_DIR/sddm/config/10-midnight-bump-theme.conf"
SDDM_THEME_CONFIG_TARGET="/etc/sddm.conf.d/10-midnight-bump-theme.conf"

SDDM_HIDPI_CONFIG_SOURCE="$PROJECT_DIR/sddm/config/20-midnight-bump-hidpi.conf"
SDDM_HIDPI_CONFIG_TARGET="/etc/sddm.conf.d/20-midnight-bump-hidpi.conf"

SDDM_FONT_SOURCE="$PROJECT_DIR/sddm/midnight-bump-theme/Fonts/ARCADECLASSIC.TTF"
SDDM_FONT_TARGET="/usr/share/fonts/ARCADECLASSIC.TTF"

if [[ -d "$SDDM_THEME_SOURCE" ]]; then
    sudo rm -rf "$SDDM_THEME_TARGET"
    sudo mkdir -p "$SDDM_THEME_TARGET"
    sudo cp -a "$SDDM_THEME_SOURCE/." "$SDDM_THEME_TARGET/"
    sudo chown -R root:root "$SDDM_THEME_TARGET"

    info "Installed Midnight Bump SDDM theme."
else
    warn "Midnight Bump SDDM theme source is missing."
fi

if [[ -f "$SDDM_FONT_SOURCE" ]]; then
    sudo install -m 0644 \
        "$SDDM_FONT_SOURCE" \
        "$SDDM_FONT_TARGET"

    if command -v fc-cache >/dev/null 2>&1; then
        sudo fc-cache -f >/dev/null 2>&1 || true
    fi

    info "Installed Midnight Bump SDDM font."
else
    warn "Midnight Bump SDDM font is missing."
fi

if [[ -f "$SDDM_THEME_CONFIG_SOURCE" ]]; then
    sudo mkdir -p /etc/sddm.conf.d

    sudo install -m 0644 \
        "$SDDM_THEME_CONFIG_SOURCE" \
        "$SDDM_THEME_CONFIG_TARGET"

    info "Installed Midnight Bump SDDM theme configuration."
else
    warn "Midnight Bump SDDM theme configuration is missing."
fi

if [[ -f "$SDDM_HIDPI_CONFIG_SOURCE" ]]; then
    sudo mkdir -p /etc/sddm.conf.d

    sudo install -m 0644 \
        "$SDDM_HIDPI_CONFIG_SOURCE" \
        "$SDDM_HIDPI_CONFIG_TARGET"

    info "Installed Midnight Bump SDDM HiDPI configuration."
else
    warn "Midnight Bump SDDM HiDPI configuration is missing."
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

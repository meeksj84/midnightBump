#!/usr/bin/env bash
# Midnight Bump bootstrap for an already-bootable minimal Arch Linux installation.
# Run as a normal user with sudo rights, from a local terminal.
set -euo pipefail

REPO="$(cd -- "$(dirname -- "$0")/.." && pwd -P)"
BACKUP_ROOT="$HOME/.local/state/midnight-bump/backups"
STAMP="$(date +%Y%m%d-%H%M%S)-$$"
DRY_RUN=0
SKIP_AUR=0
SKIP_SPOTIFY=0
SDDM_HIDPI=0

log() { printf '[midnight-bump] %s\n' "$*"; }
warn() { printf '[midnight-bump] WARNING: %s\n' "$*" >&2; }
die() { printf '[midnight-bump] ERROR: %s\n' "$*" >&2; exit 1; }

usage() {
    cat <<'USAGE'
Usage: bash install.sh [options]
  --dry-run       Check prerequisites and print planned changes only
  --skip-aur      Skip third-party paru/wlogout build
  --skip-spotify  Skip Spotify Flatpak installation
  --sddm-hidpi    Apply DESKTOP 2x SDDM greeter scale (not for ordinary laptop)
  --help          Print this help

Requires a bootable Arch installation, working internet, a non-root user,
sudo privileges, and a working pacman keyring/mirror configuration.
The script does not partition disks, install a bootloader, or configure GPUs.
USAGE
}
while (( $# )); do
    case "$1" in
        --dry-run) DRY_RUN=1 ;;
        --skip-aur) SKIP_AUR=1 ;;
        --skip-spotify) SKIP_SPOTIFY=1 ;;
        --sddm-hidpi) SDDM_HIDPI=1 ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; die "Unknown option: $1" ;;
    esac
    shift
done

[[ $EUID -ne 0 ]] || die 'Run as a regular user, not root. sudo is used for system changes.'
[[ -f /etc/arch-release ]] || die 'This installer requires Arch Linux.'
command -v pacman >/dev/null || die 'pacman is missing.'
command -v sudo >/dev/null || die 'sudo is missing. Configure a wheel/sudo user first.'
command -v git >/dev/null || die 'git is missing. Install it before cloning the repo.'
[[ -f "$REPO/packages/official.txt" ]] || die 'Incomplete clone: packages/official.txt is missing.'
[[ -f "$REPO/hypr/hyprland.lua" ]] || die 'Incomplete clone: hypr/hyprland.lua is missing.'
[[ -d "$REPO/sddm/midnight-bump-theme" ]] || die 'Incomplete clone: SDDM theme is missing.'

log "Source: $REPO"
log "Running as user: $(id -un)"
log 'Target: packages, session services, Hyprland Lua, Waybar/Cava, wallpaper, audio, networking, login theme.'
if (( DRY_RUN )); then
    log 'DRY RUN: no packages, files, or services will be changed.'
    log 'Official packages listed in packages/official.txt:'
    sed '/^[[:space:]]*#/d; /^[[:space:]]*$/d' "$REPO/packages/official.txt"
    log 'AUR wlogout/paru: offered with explicit opt-in.'
    log 'Spotify Flatpak: installed unless --skip-spotify.'
    log 'SDDM 2x HiDPI greeter: only with --sddm-hidpi.'
    exit 0
fi

sudo -v
# Avoid pacman's unsupported partial-upgrade state. Let the user review pacman's prompts.
log 'Updating Arch and installing official dependencies (pacman will request confirmation).'
mapfile -t PACKAGES < <(sed -e 's/[[:space:]]*#.*$//' -e '/^[[:space:]]*$/d' "$REPO/packages/official.txt")
(( ${#PACKAGES[@]} > 0 )) || die 'Official package list is empty.'
sudo pacman -Syu --needed -- "${PACKAGES[@]}"

# Install CPU microcode matching this machine, never both vendor packages blindly.
if grep -qm1 'vendor_id[[:space:]]*:[[:space:]]*AuthenticAMD' /proc/cpuinfo; then
    sudo pacman -S --needed amd-ucode
elif grep -qm1 'vendor_id[[:space:]]*:[[:space:]]*GenuineIntel' /proc/cpuinfo; then
    sudo pacman -S --needed intel-ucode
fi

# Make dated local backups, never deleting or overwriting existing user configuration.
backup_target() {
    local target="$1" out
    if [[ -e "$target" || -L "$target" ]]; then
        mkdir -p "$BACKUP_ROOT/$STAMP"
        out="$BACKUP_ROOT/$STAMP/$(printf '%s' "$target" | sed "s|$HOME/||; s|/|__|g")"
        mv -- "$target" "$out"
        log "Backup: $target -> $out"
    fi
}
link_target() {
    local src="$1" dest="$2"
    [[ -e "$src" ]] || die "Missing source: $src"
    mkdir -p "$(dirname -- "$dest")"
    if [[ -L "$dest" && "$(readlink -f -- "$dest")" == "$(readlink -f -- "$src")" ]]; then
        return
    fi
    backup_target "$dest"
    ln -s -- "$src" "$dest"
    log "Linked $dest"
}

# Install a complete starting Hyprland Lua config. The original installer only
# appended keybindings if this file already existed, so a minimal install failed.
link_target "$REPO/hypr/hyprland.lua" "$HOME/.config/hypr/hyprland.lua"
link_target "$REPO/hypr/keybindings.lua" "$HOME/.config/hypr/keybindings.lua"
link_target "$REPO/hypr/scripts/set-wallpaper" "$HOME/.config/hypr/scripts/set-wallpaper"
link_target "$REPO/hypr/hypridle/hypridle.conf" "$HOME/.config/hypr/hypridle.conf"
link_target "$REPO/hypr/hyprlock/hyprlock.conf" "$HOME/.config/hypr/hyprlock.conf"
link_target "$REPO/waybar/config.jsonc" "$HOME/.config/waybar/config.jsonc"
link_target "$REPO/waybar/style.css" "$HOME/.config/waybar/style.css"
link_target "$REPO/waybar/scripts" "$HOME/.config/waybar/scripts"
link_target "$REPO/kitty/kitty.conf" "$HOME/.config/kitty/kitty.conf"
link_target "$REPO/mako/config" "$HOME/.config/mako/config"
link_target "$REPO/rofi/config.rasi" "$HOME/.config/rofi/config.rasi"
link_target "$REPO/rofi/midnight-bump.rasi" "$HOME/.config/rofi/midnight-bump.rasi"
link_target "$REPO/wlogout/layout" "$HOME/.config/wlogout/layout"
link_target "$REPO/wlogout/style.css" "$HOME/.config/wlogout/style.css"
link_target "$REPO/matugen" "$HOME/.config/matugen"
link_target "$REPO/fastfetch/config.jsonc" "$HOME/.config/fastfetch/config.jsonc"
link_target "$REPO/shell/midnight-bump-prompt.sh" "$HOME/.config/midnight-bump/prompt.sh"
link_target "$REPO/scripts/system-maintenance.sh" "$HOME/scripts/system-maintenance.sh"

# Do not add another source line on repeated runs.
PROMPT_LINE='source "$HOME/.config/midnight-bump/prompt.sh"'
touch "$HOME/.bashrc"
if ! grep -Fqx "$PROMPT_LINE" "$HOME/.bashrc"; then
    printf '\n%s\n' "$PROMPT_LINE" >> "$HOME/.bashrc"
fi

for unit in awww-daemon midnight-bump-wallpaper midnight-bump-waybar midnight-bump-mako; do
    link_target "$REPO/systemd/user/$unit.service" "$HOME/.config/systemd/user/$unit.service"
done
systemctl --user daemon-reload || warn 'User systemd bus unavailable; enable user services after login.'
for unit in awww-daemon midnight-bump-wallpaper midnight-bump-waybar midnight-bump-mako; do
    systemctl --user enable "$unit.service" || warn "Could not enable $unit.service; check after login."
done

chmod +x "$REPO/hypr/scripts/set-wallpaper" "$REPO/waybar/scripts/"*.sh \
    "$REPO/scripts/system-maintenance.sh" "$REPO/scripts/pre-upgrade-check.sh"

# Generate a palette now. Avoid starting Wayland services from the installation TTY.
mkdir -p "$HOME/.config/waybar" "$HOME/.config/kitty" "$HOME/.config/hypr" "$HOME/.config/mako"
initial_wallpaper="$(find "$REPO/wallpapers" -type f \( -iname '*.png' -o -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.webp' \) -print -quit)"
if [[ -n "$initial_wallpaper" ]]; then
    matugen image "$initial_wallpaper" --config "$REPO/matugen/config.toml" --source-color-index 0 ||
        warn 'Initial Matugen theme generation failed. Try the wallpaper script after login.'
fi

# Activate essential system services but never replace a pre-existing network stack.
if systemctl is-active --quiet systemd-networkd.service; then
    warn 'systemd-networkd is already active; not enabling NetworkManager alongside it.'
else
    sudo systemctl enable --now NetworkManager.service
fi
sudo systemctl enable --now bluetooth.service
sudo systemctl enable --now ufw.service
sudo ufw default deny incoming
sudo ufw default allow outgoing
if [[ -n "${SSH_CONNECTION:-}" || -n "${SSH_TTY:-}" ]]; then
    warn 'Remote SSH session detected: review UFW rules and enable the firewall manually.'
else
    sudo ufw --force enable
fi

# The supplied SDDM greeter theme includes its QML assets and custom font.
# Do not force the desktop's 2x HiDPI override onto a laptop.
sudo mkdir -p /usr/share/sddm/themes/midnight-bump /etc/sddm.conf.d /usr/share/fonts
sudo cp -a "$REPO/sddm/midnight-bump-theme/." /usr/share/sddm/themes/midnight-bump/
sudo install -m 0644 "$REPO/sddm/midnight-bump-theme/Fonts/ARCADECLASSIC.TTF" /usr/share/fonts/ARCADECLASSIC.TTF
sudo fc-cache -f >/dev/null || true
install_root_config() {
    local source="$1" dest="$2"
    if [[ -f "$dest" ]] && ! cmp -s "$source" "$dest"; then
        sudo cp -a "$dest" "$dest.midnight-bump-backup-$STAMP"
        log "Backed up system file: $dest"
    fi
    sudo install -m 0644 "$source" "$dest"
}
install_root_config "$REPO/sddm/config/10-midnight-bump-theme.conf" /etc/sddm.conf.d/10-midnight-bump-theme.conf
if (( SDDM_HIDPI )); then
    install_root_config "$REPO/sddm/config/20-midnight-bump-hidpi.conf" /etc/sddm.conf.d/20-midnight-bump-hidpi.conf
fi
if [[ -e /etc/systemd/system/display-manager.service ]] &&
   [[ "$(readlink -f /etc/systemd/system/display-manager.service)" != *sddm.service ]]; then
    warn 'Another display manager is configured. Leaving the existing login manager unchanged.'
else
    sudo systemctl enable sddm.service
fi

# AUR builds run arbitrary community PKGBUILD code; require explicit confirmation.
if (( ! SKIP_AUR )) && ! command -v wlogout >/dev/null 2>&1; then
    printf '\nInstall paru and wlogout from the community AUR? Review PKGBUILDs first. [y/N] '
    read -r answer || answer=''
    if [[ "$answer" == y || "$answer" == Y ]]; then
        if ! command -v paru >/dev/null 2>&1; then
            build_dir="$(mktemp -d)"
            git clone https://aur.archlinux.org/paru.git "$build_dir/paru"
            log "Review the paru PKGBUILD in $build_dir/paru before continuing."
            (cd "$build_dir/paru" && makepkg -si)
            rm -rf -- "$build_dir"
        fi
        paru -S --needed wlogout
    else
        warn 'AUR skipped. The Waybar power menu requires wlogout; install it later.'
    fi
fi

if (( ! SKIP_SPOTIFY )); then
    log 'Installing the user-level Spotify Flatpak from Flathub.'
    flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo ||
        warn 'Could not configure Flathub.'
    flatpak install --user -y --noninteractive flathub com.spotify.Client ||
        warn 'Spotify Flatpak install failed; retry after entering Hyprland.'
fi

log 'Installation finished. Reboot, choose "Hyprland (uwsm-managed)" in SDDM, and log in.'
log 'Try SUPER+T (Kitty), SUPER+R (Rofi), SUPER+W (wallpaper).'
log "Backup directory: $BACKUP_ROOT"
log 'For a status check: bash scripts/doctor.sh'

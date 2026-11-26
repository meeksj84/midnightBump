# Midnight Bump

A Hyprland/Waybar desktop for Arch Linux with a neon rainy-night theme, dynamic Matugen colors, Cava/Spotify controls, and a themed SDDM login.

## Fresh minimal Arch Linux bootstrap

Start **after Arch is installed and bootable**. Log into a normal user account with sudo access and working internet. This installer does **not** format disks, install the kernel/bootloader, configure encrypted disks, or select proprietary GPU drivers. If `git` is missing, install it using `sudo pacman -Syu --needed git` first. Configure sudo/wheel if necessary.

Run these commands from a TTY or existing shell:

```bash
mkdir -p ~/Projects
git clone https://github.com/meeksj84/midnightBump.git ~/Projects/midnight-bump
cd ~/Projects/midnight-bump
bash install.sh --dry-run
bash install.sh
```

Git cloning/pulling intentionally **does not execute code**. Explicitly run `bash install.sh` after reviewing the repository. The installer upgrades through pacman (with normal confirmation), installs the full official package manifest, sets up system services, creates a complete Hyprland Lua config, links the repo-managed dotfiles, generates a Matugen color palette, installs the SDDM theme, and optionally sets up Spotify Flatpak and AUR wlogout.

**During installation:** Pacman may ask to confirm packages. The community AUR build is an explicit prompt, and the paru PKGBUILD is shown before building. On the laptop, the SDDM theme does **not** install the desktop-only 2x scaling override. The `--sddm-hidpi` flag is opt-in for a high-density display where that scale is suitable.

After installation, reboot. At SDDM choose **Hyprland (uwsm-managed)**, then log in. You should have a 34px Waybar, the centered Cava + Spotify title, networking/Bluetooth/volume/battery/clock/power widgets, dynamic wallpapers, Kitty, Rofi, Mako notifications, and a lock screen.

Keybindings: `SUPER+T` opens Kitty, `SUPER+R` opens Rofi, `SUPER+E` opens Dolphin, `SUPER+W` changes wallpaper, and `SUPER+M` logs out cleanly with UWSM.

```bash
bash scripts/doctor.sh
```

The doctor script is read-only. It checks core tools, linked configs, service enablement, and Spotify/wlogout installation.

### Installer flags

```bash
bash install.sh --dry-run
bash install.sh --skip-aur
bash install.sh --skip-spotify
bash install.sh --sddm-hidpi
bash install.sh --help
```

Existing user configuration files and symlinks are moved into timestamped backups in `~/.local/state/midnight-bump/backups/`. Before overwriting an existing root SDDM configuration file, the installer also saves a dated sibling backup. Re-running the installer skips links already pointing to the repo. Changes in the repo therefore flow through to the symlinked dotfiles after a later `git pull`, but a pull alone does not reinstall packages.

**Security and compatibility:** The script does not automatically install NVIDIA proprietary drivers or change boot settings. If the laptop has hybrid graphics, the correct kernel/driver configuration still needs verification. AUR builds run third-party packaging scripts and are opt-in. NetworkManager is not enabled if `systemd-networkd` is already active. The firewall is not enabled automatically over a detected SSH connection.

### Recovery

If graphical login fails, switch to a TTY with `Ctrl+Alt+F3` and log in. Inspect:

```bash
bash ~/Projects/midnight-bump/scripts/doctor.sh
journalctl -b -u sddm --no-pager -n 60
systemctl --user --failed
```

If necessary, temporarily disable graphical login using `sudo systemctl disable --now sddm.service`, then troubleshoot from a TTY. Backups are retained in `~/.local/state/midnight-bump/backups/`.

## Project status

Phase 15 Spotify/Cava integration is retained. Phase 16 replaces the Waybar temperature/memory indicators with Bluetooth. Its configuration is staged for laptop testing; Bluetooth hardware/device-state behavior has not yet been confirmed on this fresh installation. See [PROJECT_STATUS.md](PROJECT_STATUS.md) and [TODO.md](TODO.md).

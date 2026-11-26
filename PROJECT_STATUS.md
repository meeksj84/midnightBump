# Midnight Bump Project Status

Last checkpoint: 2026-11-26

## Fresh Arch laptop bootstrap (2026-11-26)

The normal repo entrypoint `install.sh` now runs `scripts/install-minimal-arch.sh` for a bootable minimal Arch environment. The installer installs the official package manifest, links a complete Hyprland Lua config, sets up Waybar/Mako/Hypridle and wallpaper session units, configures networking/Bluetooth/audio, initializes Matugen, and installs the SDDM theme without desktop-only 2x scaling. It offers AUR wlogout with explicit opt-in and installs Spotify Flatpak by default. Backups and a read-only doctor are included. `git clone`/`git pull` alone intentionally never executes scripts.

**Status:** GitHub automated static validation is available; the actual laptop install and graphical session have **not yet been tested**. In particular, GPU drivers, SDDM login behavior, Wi-Fi/BT hardware, and Waybar Bluetooth states require device-side verification before declaring the new-machine rollout complete.

## Active phase

### Phase 16: Waybar Bluetooth

Goal: simplify the Waybar right side and add Bluetooth support while keeping the Phase 15 Spotify/Cava center intact.

Target right side:

`[network] [bluetooth] [volume] [battery] [tray] [clock] [power]`

Planned work:
- BlueZ/Blueman package dependencies have been staged in the installer; validate on the laptop.
- Waybar memory has been replaced with Bluetooth in the repo; verify the live bar.
- Waybar temperature has been removed from the active module list; verify the live bar.
- Preserve network, volume, battery, tray, clock, and power.
- Make Bluetooth click open Blueman.
- Enable `bluetooth.service` during installation.
- Test disconnected, powered-off, and connected-device states.
- Commit and push Phase 16.

## Current project state

- Repo: `meeksj84/midnightBump`
- Branch: `main`
- Standard repo path: `~/Projects/midnight-bump`
- Desktop Git authentication uses SSH.
- Hyprland config is currently clean.
- Waybar is systemd-managed.
- Spotify/Cava integration is reproducible through `install.sh`.
- Fastfetch is installed and uses a Midnight Bump-themed config.
- Bash uses the repo-managed Midnight Bump colored prompt.
- `scripts/pre-upgrade-check.sh` combines the health audit and pre-upgrade backup.
- Pre-upgrade backups exclude Steam, Trash, and Flatpak bulk data.
- `rtkit` and `pacman-contrib` are in the official package manifest.
- `checkupdates`, `paccache`, and `pacdiff` are verified by the installer.
- `scripts/system-maintenance.sh` is repo-managed.
- System maintenance supports:
  - no option: read-only audit
  - `--maintain`: full backup/update/cleanup/scrub workflow
  - `--clean`: alias for `--maintain`
  - `--help`: usage reminder

## Idea inbox

- Theme Dolphin file manager to match Midnight Bump.
- Redesign Rofi to better match Midnight Bump; visual direction is still undecided.

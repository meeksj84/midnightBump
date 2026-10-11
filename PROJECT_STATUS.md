# Midnight Bump Project Status

Last checkpoint: 2026-10-10

## Active phase

### Phase 16: Waybar Bluetooth

Goal: simplify the Waybar right side and add Bluetooth support while keeping the Phase 15 Spotify/Cava center intact.

Target right side:

`[network] [bluetooth] [volume] [battery] [tray] [clock] [power]`

Planned work:
- Add BlueZ/Blueman dependencies to the installer.
- Replace the Waybar memory module with Bluetooth.
- Remove the Waybar temperature module.
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

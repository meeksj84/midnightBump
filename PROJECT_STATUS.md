# Midnight Bump Project Status

Last checkpoint: 2026-10-27

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
- Rust `tools/pre-upgrade-check` implements the health audit, backup and maintenance.
- `scripts/pre-upgrade-check.sh` remains an audit/backup compatibility launcher.
- Pre-upgrade backups exclude Steam, Trash, and Flatpak bulk data.
- `rtkit` and `pacman-contrib` are in the official package manifest.
- `checkupdates`, `paccache`, and `pacdiff` are verified by the installer.
- `scripts/system-maintenance.sh` is a repo-managed compatibility launcher.
- System maintenance supports:
  - no option: read-only audit
  - `--maintain`: full backup/update/cleanup/scrub workflow
  - `--clean`: alias for `--maintain`
  - `--help`: usage reminder

## Maintenance tool migration

- Rust release: v0.1.5; all 47 standalone tests passed on morbo on 2026-10-27.
- The real compact review checked 939 installed packages and completed all online
  inputs, saved its full report, and highlighted pending updates with findings.
- Repository integration adds offline build and legacy launcher tests. Legacy
  defaults remain: pre-upgrade-check audits/backs up; system-maintenance audits.
- Root `install.sh` retains its fuller desktop/SDDM setup and builds/links the CLI.
- Real upgrade, cleanup, scrub and full desktop installation have not been
  exercised by this migration. Phase 16 Waybar work above remains separate.

## Idea inbox

- Theme Dolphin file manager to match Midnight Bump.
- Redesign Rofi to better match Midnight Bump; visual direction is still undecided.

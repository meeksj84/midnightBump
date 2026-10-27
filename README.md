midnightBump

A Hyprland rice inspired by:
- Adult Swim late-night atmosphere
- Cowboy Bebop
- Serial Experiments Lain
- Neon city aesthetics
- Rainy Tokyo nights

Components:
- Hyprland
- Waybar
- Kitty
- Mako
- Matugen
- Hyprpaper

## System audit and maintenance

The `pre-upgrade-check` command is implemented in Rust under
`tools/pre-upgrade-check`. It combines the supplied audit, backup, maintenance
and older installer scripts, and cross-checks installed packages against Arch
security records and public package reports before maintenance package changes.

Build and test from this repository's root with Rust/Cargo 1.70 or newer and a
Linux C linker (`base-devel` on Arch). No Cargo dependencies need downloading:

```bash
cargo test --manifest-path tools/pre-upgrade-check/Cargo.toml --locked --offline
./scripts/build-pre-upgrade-check.sh
./scripts/pre-upgrade-check --help
./scripts/pre-upgrade-check --review --compact \
  --output "$HOME/pre-upgrade-review-$(date +%Y%m%d-%H%M%S).txt"
```

Run as your regular desktop user. `--review` saves the requested report but does
not install packages, create backups or request sudo. Review exit 1 means findings
or incomplete checks; read the summary and report. Cargo's `--offline` controls
building only; the runtime review uses the network.

The original command paths remain available as small compatibility launchers:

| Entry point | Default behavior |
| --- | --- |
| `scripts/pre-upgrade-check` | Audit and backup |
| `scripts/pre-upgrade-check.sh` | Audit and backup, as before |
| `scripts/system-maintenance.sh` | Audit only, as before |
| `scripts/system-maintenance.sh --maintain` / `--clean` | Audit, review, backup, confirmations, upgrade and cleanup |
| `scripts/install-updated.sh` | The older supplied desktop setup, via Rust `--install` |

Arguments, help, exit status and symlinked legacy command paths are retained.
The launchers do not automatically compile or install anything. Rebuild after
changing or pulling Rust source. Their repository path is supplied to the Rust
command; an explicit `--project-dir PATH` can override it.

See [the CLI documentation](tools/pre-upgrade-check/README.md) for all options,
coverage limitations, report saving and backups, and
[validation results](tools/pre-upgrade-check/TESTING.md) for tested behavior.
Build output and generated reports are excluded from Git. Original scripts remain
in `tools/pre-upgrade-check/reference/` for comparison.

## Full desktop installation

The root `install.sh` remains the fuller desktop installer, including Spotify/Cava,
SDDM theme/HiDPI configuration and other setup beyond the older installer that was
ported. It is a separate workflow, not a Rust launcher. It still makes package and
desktop configuration changes, and its existing package-install behavior remains.
Do not rerun it just to migrate or rebuild the maintenance command.

For a fresh desktop installation, make Rust/Cargo available first. `install.sh`
checks for Cargo before package changes, installs the existing official package
manifest (including the C build tools), builds the Rust CLI offline, and links
`~/.local/bin/pre-upgrade-check` plus the existing maintenance command path. It
preserves its other desktop setup steps. Keep `~/.local/bin` in your PATH.

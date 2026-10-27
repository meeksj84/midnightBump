# Source mapping and intentional changes

Source inputs were the four attachments supplied on 2026-10-18. Exact script copies
are kept under `reference/`; `install-updated(1).sh` is named `install-updated.sh` there.
The historical `update.txt` report remains at its existing `scripts/update.txt` path
in the repository; it is not duplicated into the Rust crate.

| Original feature | Rust location / mode |
| --- | --- |
| Host, date, kernel, uptime, desktop, session | `audit.rs`, all audit/backup/maintenance modes |
| Failed system and user services | `audit.rs`; repeated after maintenance |
| Current boot errors and five known platform/firmware exclusions | `audit.rs::known_noise` |
| Kernel warnings, systemd timing, slowest 20 services | `audit.rs` |
| Root/boot usage, mount source/options, FAT masks | `audit.rs` |
| Btrfs usage, device statistics, scrub status | `audit.rs`, read-only commands through sudo as in system-maintenance |
| Pacman database and orphans | `audit.rs` |
| Non-invasive pending versions and new Arch notices review | `review.rs`; RSS/Atom reader in `feed.rs` |
| All installed packages and split-package source mapping | `packages.rs`; `pacman -Q`, `pacman -Qm`, `pacman-conf`, read-only local metadata |
| Installed/pending versions vs security tracker; open package reports | `issues.rs`, bounded HTTP/JSON readers in `http.rs`/`json.rs` |
| Hyprland config/version, Hypridle activity/listener rules | `audit.rs` |
| PipeWire, WirePlumber, Pulse compatibility, default sink, RTKit | `audit.rs` |
| PCI graphics, AMDGPU/DMUB/DMCUB errors, NVIDIA query/Xid errors | `audit.rs` |
| Battery stats and Lenovo charge modes/warnings | `audit.rs` |
| Regulatory database and network errors | `audit.rs` |
| Pass/warn/info counts | `runtime.rs::App` |
| Timestamped workspace and saved report | `backup.rs`, default/backup/maintenance |
| Explicit official and foreign package lists | `backup.rs`: `pacman -Qqen`, `pacman -Qqem` |
| Home configuration archive | Same eight entries as supplied pre-upgrade-check.sh |
| Repository archive and Git state | `backup.rs`; includes .git worktree files as well as directories |
| Archive listing and size summary | `backup.rs` |
| Cache size and journal size audit | `audit.rs`; cache-size uses read-only `sudo du` as in the original |
| Help / maintain / clean options | `cli.rs`; `--clean` remains a full-maintenance alias |
| Warning confirmation / unattended warning cancellation | `maintenance.rs` |
| Pacman update, paccache cleanup, conditional scrub, service recheck | `maintenance.rs` |
| Installer repository validation | `install.rs::REQUIRED`, all 24 required files |
| Official package manifest / missing package installation | `install.rs` |
| Paru bootstrap and AUR manifest installation | `install.rs` |
| UFW defaults, enablement | `install.rs` |
| 17 required/optional command checks | `install.rs` |
| Configuration backups and all 15 links, including Matugen directory | `install.rs::LINKS` |
| Hyprland Lua backup, legacy autostart cleanup, keybindings include | `install.rs` |
| Wallpaper executable, initial palette generation | `install.rs` |
| Waybar, Hypridle, Mako, awww, wallpaper services | `install.rs` |
| Notification process cleanup, stale enable-link removal | `install.rs` |
| awww process wait and power-button prompt | `install.rs` |
| Reload/wallpaper/lock instructions | Installer completion output |
| update.txt | Historical report retained as reference, not executed or treated as current machine state |

## Intentional changes

- One default must be chosen: no arguments retain pre-upgrade-check's audit +
  backup. `--audit` provides system-maintenance's audit-only behavior.
- Installation is explicit (`--install`). No automatic desktop reinstall during
  audits or maintenance. Project location is configurable instead of assuming
  the executable lives inside the Midnight Bump repository.
- Failed diagnostics produce warnings instead of being mistaken for clean checks.
  `checkupdates` code 2 is “no updates”; code 1 is a failed check. Source:
  https://man.archlinux.org/man/checkupdates.8
- An update/cache/scrub command failure stops the workflow and returns 2. It cannot
  print a success message and continue as the old maintenance script sometimes did.
- Btrfs commands run only on Btrfs root. The running-status parser distinguishes
  “running” from “not running.” A started scrub is reported as background work,
  not a completed verification. Source: https://man.archlinux.org/man/btrfs-scrub.8
- Backup directories are private and collision-resistant. Tar creates temporary
  archives, verifies their listing, and renames them only on success. Tar warnings
  that return failure (for example, files changing during archival) block maintenance.
- Empty filtered package lists are accepted when pacman returns 1 without output
  or stderr. Missing home/repository/Git metadata now counts as review warnings.
- Report write failures block maintenance. Git state failure is reported as a
  warning instead of incorrectly printing that recording succeeded.
- Installer backs up existing symlinks as well as regular files. It preserves
  unrelated files at stale unit-link paths and backs up an existing power-button
  configuration with numbered system copies.
- User units are reloaded before enablement. Installer package operations retain
  their actions but omit the old `--noconfirm`, allowing local package review.
- Whole-program execution as root is refused; sudo is used for specific privileged
  operations. This keeps user configuration, backups, and user services associated
  with the logged-in desktop user.
- New `--dry-run`, `--version`, path options, strict argument validation, and tests.
- Version 0.1.2 adds `--review` and a required online/manual review before both
  maintenance and installer changes. `curl` and `checkupdates` are runtime
  prerequisites. No extra Rust crates are required. Download/parse/update-query
  failures block package changes even if a user accepts local health warnings.
- News is displayed in full from the limited RSS feed; published advisories are
  listed with dates/links and possible installed/source/requested name matches.
  Explicit interactive acknowledgement is required even with zero health warnings.
- Version 0.1.3 replaces link-only bug review with automatic retrieval of public
  open Arch packaging reports, matching installed source-package names across
  all pages. General report applicability remains a human assessment; a project
  match is not proof of a version-specific regression.
- Full security JSON is matched by installed package name; `vercmp` assesses
  installed/pending versions against reported fixes. Unknown/no-fix records are
  not cleared, and `Fixed` is never treated as proof the host is already fixed.
  Source: https://security.archlinux.org/issues/all.json
  Comparison semantics: https://man.archlinux.org/man/vercmp.8.en
  Group issue API: https://docs.gitlab.com/api/issues/#list-all-group-issues
  Local metadata: https://gitlab.archlinux.org/pacman/pacman/-/blob/master/lib/libalpm/be_local.c
- The official package page exposes Bug Reports and Security Issues links:
  https://archlinux.org/packages/core/x86_64/linux/
  Feed sources: https://archlinux.org/feeds/news/ and
  https://security.archlinux.org/advisory/feed.atom
- Bounded feed parsing rejects unsupported/malformed responses, empty article
  lists, DTDs and external entities. HTTPS downloads have connect/overall timeouts
  and a 2 MiB cap; notice text has terminal control characters removed. Retrieved
  text never becomes an executable command.
- Security JSON has an 8 MiB bound; each issue page has a 2 MiB bound. All issue
  pages must be retrieved (50-page limit, then fail if more remain). Bad JSON,
  missing page metadata, repeated pages, comparison failures and inventory errors
  block installation; no failure is treated as an empty successful match.
- Informational output is plain text. Reports retain `[PASS]`, `[WARN]`, `[INFO]`
  and status headings; exact line counts/ordering differ because duplicate audits
  are combined and diagnostics are checked more strictly.

No embedded shell scripts, shell `-c` strings, Rust crates, telemetry, automatic
Git commits, or pushes are used. Commands receive separate argument values through
`std::process::Command`: https://doc.rust-lang.org/std/process/struct.Command.html

Version 0.1.1 additionally summarizes kernel firewall lines in the terminal; `--verbose` restores full terminal output, and saved reports always retain full kernel output. See `CHANGELOG.md`.

## Repository entry points

The original scripts under `scripts/` are now compatibility launchers for the Rust
CLI. `pre-upgrade-check.sh` keeps audit/backup as its default; system-maintenance.sh
adds `--audit` when no mode is supplied. `install-updated.sh` selects `--install`.
The new extensionless `scripts/pre-upgrade-check` resolves its checkout through
symlinks and runs its already-built release executable. The build helper is
explicit, offline and locked. No launcher compiles automatically.

The repository root `install.sh` contains additional setup (including SDDM and
Spotify/Cava) beyond the supplied installer, and remains separate. Its integration
changes only add toolchain checking, building and command-link wiring; they do not
replace its full setup with the older Rust installer. It is not used to apply the
maintenance-tool migration on an existing desktop.

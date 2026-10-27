# pre-upgrade-check 0.1.5

Rust audit, backup, maintenance and package review for Midnight Bump. The original
supplied scripts are retained under `reference/` and are never executed by Rust.
The repository root `install.sh` is the separate fuller desktop installer; the
Rust `--install` mode corresponds to the older supplied `install-updated.sh`.

## Build and test from the repository root

```bash
cargo test --manifest-path tools/pre-upgrade-check/Cargo.toml --locked --offline
./scripts/build-pre-upgrade-check.sh
./scripts/pre-upgrade-check --version
./scripts/pre-upgrade-check --help
./scripts/pre-upgrade-check --review --compact \
  --output "$HOME/pre-upgrade-review-$(date +%Y%m%d-%H%M%S).txt"
```

The remaining Cargo examples assume your current directory is
`tools/pre-upgrade-check`. Repository launchers resolve symlinks and supply the
checkout as `--project-dir`; direct binaries keep the default documented below.
Rebuild after Rust source changes. A missing binary returns exit 2 with build
instructions; running a legacy command never silently builds or fetches code.

Use your normal desktop terminal, **without `sudo` before cargo or the program**.
The cache-size and Btrfs audit commands may ask for sudo themselves, as in your maintenance script.
`--audit` creates no backup and performs no package/configuration changes.
`checkupdates` may access the network and maintain its separate temporary database.
An audit/review exit code of **1 means warnings or incomplete online checks**, so
it is not necessarily a program bug. Share the review output before maintenance.
Cargo's `--offline` controls compilation only: the program's online checks still
use the network. `--dry-run` performs no network access or other external commands.

The project requires Rust/Cargo 1.70 or newer and a Linux linker. It uses only the
Rust standard library, so Cargo does not need to fetch any crates. If Cargo is
missing, check your existing Arch Rust/rustup installation before running a system
upgrade solely to install development tools.

## Commands retained and combined

| Command | Behavior |
| --- | --- |
| `pre-upgrade-check` | Full audit plus timestamped backup, matching your original pre-upgrade-check default |
| `pre-upgrade-check --backup` | Explicit alias for that default |
| `pre-upgrade-check --audit` | Combined read-only audit, replacing system-maintenance's no-argument audit |
| `pre-upgrade-check --review` | Cross-check all installed packages against Arch security records and open package reports; no sudo, backup, or package changes |
| `pre-upgrade-check --maintain` | Audit + online review → backup → warning/review confirmations → update → cache cleanup → Btrfs scrub → service checks |
| `pre-upgrade-check --clean` | Preserved alias for `--maintain`; **includes the system update** |
| `pre-upgrade-check --install` | Midnight Bump package/configuration installer from install-updated(1).sh |
| `pre-upgrade-check --help` or `-h` | Full help |
| `pre-upgrade-check --version` or `-V` | Version |
| Add `--compact` | Show progress, health/coverage warnings, news links and the summary; save detailed findings in a report |
| `--output PATH` with review/audit | Create a full private report without creating backups; never overwrite an existing file |
| Add `--dry-run` to any mode | Print the planned actions without executing commands or writing files |
| Add `--verbose` or `-v` | Display all summary groups, kernel firewall entries, full issue/advisory bodies and historical security records meeting recorded fixes |

Optional paths: `--project-dir PATH` and `--backup-root PATH`. Defaults remain
`~/Projects/midnight-bump` and `~/Backups`. Use shell-expanded paths such as
`--project-dir "$HOME/Projects/midnight-bump"`.

The old command paths now use thin compatibility launchers.
The two original scripts had different no-argument behavior. The new program
keeps **pre-upgrade-check's audit + backup default** and exposes the other audit
behavior as `--audit`. Conflicting modes and unknown arguments are rejected.

## Automatic readable summary

Every completed review/audit/backup prints a **PRE-UPGRADE SUMMARY** near the end.
Maintenance and installer modes show it before asking permission to change packages.
No new flag is needed. Dry runs describe the summary without inventing check results.

The summary includes installed and foreign package counts, available repository
updates, security findings needing review, matching open reports, feed counts and
whether all required online checks completed. It lists security findings by package
and distinguishes recorded-fix thresholds, missing fix metadata and foreign builds.
Open reports are grouped by source package with installed split-package names shown.
Arch labels distinguish security reports, bugs/regressions, feature/enhancement/update
requests and other/unclassified reports. Existing warning behavior is unchanged.

The terminal shows up to 12 open-report source groups, putting explicitly labelled
security reports first, then bugs/regressions, unclassified reports and requests.
All security-finding packages are shown. Use `--verbose` for every open-report group.
All saved reports retain every group and the full original findings; the summary
prints the report path. Review/audit modes can save a report with `--output PATH`.
Without `--output`, those modes retain their previous terminal-only behavior.

Missing/failed inputs are **UNKNOWN**, never zero. Incomplete online checks show
**BLOCKED**, with the failing checks listed. Successful retrieval shows **REVIEW
REQUIRED**, even with zero matches. Security counts are package/record matches,
not unique vulnerabilities; open reports are not proof your version is affected.
The summary is a pre-upgrade snapshot, not a post-upgrade rescan. Health warnings,
backup/report failures and the existing confirmation gates still apply.

## Compact output and saved reviews

Use `--compact` to keep progress, health/coverage warnings, Arch News titles/links,
and the final summary on screen. Detailed diagnostics and issue descriptions go
into the report. The normal terminal output remains the default; `--verbose` still
expands it. The two output options cannot be combined.

Review/audit modes require `--output PATH` when using `--compact`, so details are
never discarded. You can also use `--output` without compact mode. The file is
created privately (mode 0600), the parent directory must already exist, and existing
files or symlinks are never overwritten. Choose a new name for each run. Creation
failure stops before the scan; a report-write failure returns exit 2. Dry-run never
creates the requested file.

Backup/maintenance already save reports, so use `--compact` without `--output` for
those modes. The desktop installer keeps its interactive output; it accepts neither
new option. All confirmation prompts, warning counts, exit codes and failure gates
remain in force. Read the saved details and Arch notices before approving changes.

The **PENDING UPDATES WITH MATCHING FINDINGS** section lists installed-to-proposed
version changes for pending packages with unresolved security findings or open
reports. Source matching includes split packages. It distinguishes candidates that
meet a recorded security fix from those still below it; missing fixes and foreign
builds remain unresolved. General reports can be bugs or feature/update requests,
and lack reliable version ranges. A listed update may fix a problem, so the highlight
is not an automatic instruction to postpone it. Missing required inputs produce
UNKNOWN coverage rather than an empty reassuring list. All installed packages
continue to be checked, including packages without pending updates.

## Arch notices and package review

Audit, backup, maintenance, and installer modes now include the online review.
Use `--review` to test it on its own; it does not install anything or request sudo.
It requires existing `curl`, `checkupdates` (from `pacman-contrib`), and
`pacman`, `pacman-conf`, and `vercmp` (provided by Pacman). Missing required tools
are identified in the output. The program will not
automatically install prerequisites to bypass its own review.

Each run inventories **all installed packages, including dependencies**, using
`pacman -Q`; `pacman -Qm` identifies foreign packages. It reads `%BASE%` from each
package's local Pacman metadata, using `pacman-conf DBPath` to locate the database.
This maps split packages such as `linux-headers` to their `linux` source project.
Missing metadata blocks the review; a missing optional `%BASE%` is reported as an
exact-name-only mapping limitation. Nothing in the Pacman database is modified.

It refreshes pending versions with `checkupdates --nocolor`, then cross-checks:

- **Full Arch security tracker:** https://security.archlinux.org/issues/all.json
  Exact installed package names are matched against every returned record.
  `vercmp` compares installed and pending versions with recorded fixed versions,
  including Arch epoch/pkgrel semantics. Findings show the package, installed
  version, pending version, fixed version, severity, tracker status, CVEs and link.
  They distinguish a pending update that meets the recorded fix from one that
  remains below it. Packages without pending updates are checked too.
- **Public open Arch packaging reports:** the GitLab group Issues API is read
  through every page, including archived projects. Installed source-package names
  are matched against report projects. Findings show relevant installed packages,
  versions, title, labels, update date, description and direct issue link. An open
  general bug is a package/source match requiring assessment; the API does not
  supply reliable affected-version ranges. Update requests are shown as info.

The scanner does not equate a tracker status of `Fixed` with your host being
fixed. Installed versions below the recorded fix are still flagged. Records
marked `Not affected` are excluded; unknown/no-fix records remain unresolved.
The tracker's `affected` field is a recorded affected version, **not an established
first vulnerable version**, so older installed versions are not silently cleared.
Foreign builds receive a name-match warning rather than an official-version
verdict. Already-fixed historical matches are counted; `--verbose` lists them.

Matching is performed locally against downloaded public records. The installed
inventory is not sent to Arch. The report scan is bounded to 50 pages of 100
issues; missing/repeated/inconsistent pages or reaching the limit with more
pages remaining makes the review incomplete and blocks package changes.

The existing official news/advisory feed review is retained:

- Arch News: https://archlinux.org/feeds/news/
- Published security advisories: https://security.archlinux.org/advisory/feed.atom

All news entries returned by the feed are displayed with dates, full plain-text
bodies, and article links. General/manual-intervention news is never hidden based
on a package-name filter. Every security-advisory title/date/link is shown; exact
package-name mentions expand the body as a **possible match**, not a finding that
an installed or proposed version is vulnerable. `--verbose` expands all bodies.
Always read the linked full article for its instructions and referenced documents.

Every pending package also includes its old/new version and an Arch package-page
link for further investigation. The installer lists its requested official/AUR
packages and AUR comment/build links. Requested packages that are not installed
are outside the installed-inventory cross-check. Their
candidate versions and new dependencies must be checked in the package-manager
transaction before accepting it.

Feeds contain limited history; the security JSON cross-check covers the full
returned tracker dataset. Neither source establishes exhaustive vulnerability
coverage. Closed/private GitLab reports, upstream-only bugs, AUR comments and
custom-repository issues are outside the automatic bug scan. Older news and
scheduled advisories may be outside the feeds. The full archives remain linked.
A successful download or absence of a package-name match is never an all-clear.
Security notices can advise installing a fix, rather than postponing updates.

Maintenance and installer modes **stop before any package changes** if the update
lookup, inventory, required download, parsing, version comparison or issue
pagination fails. Missing tools, HTTP errors, timeouts, challenge pages, empty
security/news datasets and malformed data are reported as unknown
coverage. There is no bypass flag. Audit/backup modes can still finish and record
warnings; a successful backup does not remove the maintenance block.

After successful checks, maintenance and installer modes always ask:

> Have you reviewed Arch notices, relevant package reports, and required interventions, and want to proceed with package changes? [y/N]

Only an interactive `y` acknowledges the review. Health-warning confirmation is
separate. Refusal/noninteractive input cancels without package changes. Notices
are displayed as text, never executed as commands. Required report-write failures
also block subsequent operations. Backup/maintenance reports include the review
and acknowledgement; audit/review can save the same details with `--output`.
Installer output stays in the terminal.
Long bug descriptions are shortened on screen unless `--verbose` is used;
all saved reports retain their full text.

Pacman refreshes its repository database again when installing. Compare its final
versions/dependencies with the reviewed list and cancel if it introduces changes
that need another review. The program does not pin versions or perform partial
upgrades to hide that difference.

## Build and preview

```bash
cargo build --release --offline
./target/release/pre-upgrade-check --maintain --dry-run
./target/release/pre-upgrade-check --install --project-dir "$HOME/Projects/midnight-bump" --dry-run
```

The release executable is `target/release/pre-upgrade-check`. No `.sh` extension
or Bash wrapper is needed. Rust controls argument parsing, checks, file handling,
logging, backups, and workflow order. Existing Linux tools still perform their
normal jobs (`pacman`, `tar`, `systemctl`, `btrfs`, etc.).

After the audit is reviewed, test an actual backup separately:

```bash
./target/release/pre-upgrade-check --backup
```

Backup mode does **not** install updates. Inspect the printed backup directory and
`pre-upgrade-report.txt`. The backup includes package lists, selected user
configuration, the Midnight Bump repository, and Git status/commit information.
It does not contain all personal files or a complete system/boot image. Symlinks
in home configuration are preserved as links; the repository is archived separately.

Only after local review, `--maintain` runs `sudo pacman -Syu`, `sudo paccache -r`
if available, and starts a Btrfs scrub when `/` is Btrfs and no scrub is running.
It does not remove orphan packages or update AUR packages. The scrub is asynchronous;
check completion later with `sudo btrfs scrub status /`.

`--install` is the full **desktop setup installer**, not installation of this Rust
executable. It requires the complete Midnight Bump checkout and can install
official/AUR packages, set UFW defaults, replace configuration links, restart
notification services, enable user services, and optionally change the power-button
configuration. Package confirmations remain interactive. Preview it before testing.

## Exit codes and logging

| Code | Meaning |
| --- | --- |
| 0 | Completed without warnings, or help/version/dry-run succeeded |
| 1 | Warnings, incomplete audit/review checks, or user cancellation |
| 2 | Invalid arguments, backup/report failure, or failed required operation |

Backups are created in private, unique `system-update-*` directories. Reports
include the audit, backup results, workflow commands, and final result. Kernel firewall
entries are summarized on screen; reports retain the full kernel output. Interactive
package commands inherit the terminal so their prompts work; their full terminal
output is not captured in the report. Audit/review print to the terminal and optionally save with `--output`;
installer output stays in the terminal. The backup completion marker refers only to required backup operations;
check the report for skipped paths and warnings.

## Optional installation into your user PATH

To install a standalone copy of the compiled command in your user PATH, from this
crate directory:

```bash
cargo install --path . --root "$HOME/.local" --offline
"$HOME/.local/bin/pre-upgrade-check" --help
```

Ensure `~/.local/bin` is in your PATH. The program does not commit, push, or pull your Midnight Bump repository. Installer mode can
clone the **separate Paru source** if Paru is missing, matching the original installer.

See `PORTING.md` for the detailed behavior mapping and `TESTING.md` for validation
and remaining local checks.

# 0.1.5 — compact output, saved reviews and pending-update highlights, 2026-10-27

- Add optional --compact output: progress, health/coverage warnings, Arch News
  titles/links and summary stay visible; detailed findings are saved in the report.
- Add --output PATH for review/audit without backups. Create private reports,
  refuse existing files/symlinks and missing parents, and propagate write errors.
  Compact review/audit requires an output file; backup/maintenance use their normal
  reports. Desktop installer keeps full output. Dry-run remains free of writes.
- Highlight pending packages with unresolved security findings or open reports,
  using source names for split packages. Distinguish proposed fixes from versions
  still below recorded thresholds; preserve uncertainty and all-installed coverage.
- Retain existing flags, warning counts, confirmations, failure gates and details.
- 47 tests passed (44 unit/workflow + 3 CLI). No real package changes or Git push.

# 0.1.4 — readable pre-upgrade summary, 2026-10-27

- Add an automatic PRE-UPGRADE SUMMARY to review/audit/backup output and before
  maintenance/installer package-change confirmations. Preserve every existing mode.
- Show installed/foreign package counts, available updates, security findings,
  open-report totals and required online coverage, with packages needing attention.
- Group security matches by package and reason. Group open reports by source
  package, prioritize security and bugs, and distinguish requests using Arch labels.
  Keep all original findings and warning/confirmation behavior unchanged.
- Compact output shows 12 open-report groups; --verbose and saved reports include
  every group. Saved reports also retain the full detailed issue descriptions.
- Use structured totals only after successful checks. Failures show UNKNOWN and
  BLOCKED rather than zero findings; successful checks still require human review.
- Validate with 42 passing tests (39 unit/workflow + 3 CLI), including report
  retention, split-package summaries, classification and incomplete-input cases.
  No real upgrade, cleanup, scrub, desktop installation or Git push performed.

# 0.1.3 — installed-package issue cross-check, 2026-10-21 (America/Los_Angeles)

- Inventory every installed package, including dependencies, and its source
  package from read-only local Pacman metadata. Identify foreign builds.
- Fetch Arch's full security JSON and compare installed/pending versions with
  recorded fixes using `vercmp`. Show severity, CVEs, source status and links.
  Never interpret Arch's `Fixed` status as proof the installed version is fixed;
  unknown/no-fix and foreign-build applicability remain review findings.
- Retrieve all pages of public open Arch packaging reports. Match source-package
  names so split packages participate, even without a pending update. Display
  report titles, labels, descriptions, links and relevant installed/pending versions.
  General reports lack reliable version ranges; a match is not a confirmed regression.
- Block package changes on inventory/comparison/download/parse/pagination failure.
  Retain news feeds, explicit acknowledgement, backups, help and existing modes.
- Keep offline, dependency-free Rust builds. All 39 tests pass; local Arch testing
  and review of actual findings remain necessary. No Git commit or push.

# 0.1.2 — Arch notices and package review, 2026-10-22 UTC

- Add `--review`: pending update versions, Arch News, published security
  advisories, and links for manual package bug-report review. It performs no
  sudo, backup, package installation, cleanup or configuration changes.
- Include that review in audit/backup/maintenance and desktop installer modes.
  All returned news is displayed, including general manual-intervention notices.
  Advisory name matches are hints only; all advisory dates/links remain visible.
- Block maintenance and installer changes on unavailable tools, unsuccessful
  update lookup, network errors, or unsupported/incomplete feed data.
- Always require interactive acknowledgement before maintenance/installer
  changes, even when local health checks have no warnings. Native package-manager
  prompts and the existing health-warning confirmation remain.
- Record review details/acknowledgement in backup/maintenance reports, preserve
  the existing command name and modes, and document limited feed history,
  manual bug-report review, AUR/custom repository gaps and transaction changes.
- Keep Cargo builds dependency-free/offline; curl performs runtime HTTPS fetches.
  Add bounded RSS/Atom parsing with no DTD/entity expansion or remote execution.
- All 31 tests passed. Real Arch feeds were retrieved and parsed successfully;
  real system upgrades/desktop installation remain untested here. No Git push.

# 0.1.1 — local audit fixes, 2026-10-21

Based on the first local run on morbo, which passed all 21 original tests and
completed the audit with 13 passing checks and four warning groups.

- Restore `sudo du -sh /var/cache/pacman/pkg` from the original maintenance
  script. This fixes the cache-size check's permission errors on protected
  `download-*` directories; no directory permissions or package files change.
- Let Cargo capture unit-test output. Simulated updates, firewall changes, and
  deliberately failed operations no longer flood successful test output.
- Summarize `[UFW BLOCK]` kernel entries in the terminal. The supplied run had
  895 of these entries. All other kernel lines remain visible. Add `--verbose`
  (or `-v`) to display every firewall entry; saved backup reports always retain
  the full kernel output.
- Keep the original name, modes, backup behavior, and warning confirmation.

The local machine's other findings are not suppressed or automatically repaired:
four failed TPM-related system units; boot journal errors including mt7921e
resume timeouts; and a Hypridle configuration error recorded earlier in the
current boot. Hypridle being active does not establish whether that old error
has since been fixed. A current service/configuration check is still needed.

The uploaded audit reported no package database errors, no AMDGPU/DMUB/DMCUB
errors, active audio services, and zero Btrfs device-error counters. The scrub
output said `no stats available`, so it does not establish that a scrub completed.
These observations describe that uploaded run, not a new inspection of the host.

No Git push or actual machine maintenance was performed for this revision.

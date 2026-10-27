# Validation — version 0.1.5 and repository integration, 2026-10-27

Built and tested in the development workspace with Rust **1.85.1** and Cargo
**1.85.1** on Linux. Minimum declared Rust version is 1.70; that older toolchain
was not separately tested. Cargo builds remain offline and dependency-free.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo test --offline` | **51 tests passed**, 0 failed (44 unit/workflow + 3 CLI + 4 repository launcher tests) |
| `cargo build --release --offline` | Passed |
| Release executable `--help`, `--version` | Passed |
| Release executable `--maintain --dry-run` | Passed |
| Source-reference byte comparison | Three original scripts preserved exactly; historical report retained at scripts/update.txt |

The existing suite covers command compatibility, dry-run behavior, backups and
archive read-back, report-write failures, configuration preservation, audit logging,
all-installed/source-package matching, security version comparisons, all-page issue
retrieval, online failures and confirmation gates. Privileged commands are simulated;
archive tests use real temporary files and tar. No actual upgrade, cache cleanup,
scrub, firewall update or desktop installation was performed.

Version 0.1.4 added summary classification, compact ordering and full-list retention,
and prevention of false clearance when data are unavailable. Existing security
cases now verify the structured totals for below-fix, missing-fix and foreign-build
findings. Split-package tests verify that reports are counted once and grouped by
source. Review and backup workflows verify that the summary is written into the
report without changing warning counts. Every required online failure case verifies
BLOCKED/UNKNOWN output; missing later pages and failed comparisons cannot leave
apparently complete totals. Summary rendering does not invoke external commands.

Version 0.1.5 tests verify compact review output against a full private report,
unchanged warning counts and command calls, pending split-package highlights,
retained audit diagnostics and visible coverage failures. Existing-file, symlink
and missing-parent output failures stop before scans; existing contents remain
untouched. Output paths with spaces work. Dry-run with compact/output options
performs no commands or writes. A changed installed-version snapshot clears pending
highlights and blocks maintenance. Security cases verify candidate fix thresholds;
report write errors are detected even when terminal details are hidden.

Normal output, all detailed saved findings, warning counts, confirmation gates and
exit semantics are retained. The new summary does not certify upgrade safety or infer reliable
version ranges from free-text bug reports. Label categories describe the report,
not a confirmed defect in the installed package. No real host is represented by
unit-test fixtures.

## Repository integration validation

The four repository launcher tests exercise legacy defaults, argument/exit-code
forwarding, explicit project paths, symlink resolution and checkout paths with
spaces. A missing binary fails without an automatic build. The build helper keeps
Cargo offline/locked, uses the checkout's target directory, and propagates failure.
Shell syntax checks cover the root installer and all launchers. Real release help,
version and dry-run commands verify the launchers against the compiled Rust binary.
Root install.sh changes are limited to toolchain/build wiring, required source paths,
permissions and the user command link; its desktop/SDDM setup remains in place.
The full root installer was not executed.

Morbo's supplied v0.1.5 run passed all 47 standalone tests and completed the live
compact review: 939 installed packages, 42 pending updates, 2,444 security records,
1,930 open reports, 21 security findings and 126 matching package reports. The full
report path was printed, and Git status was clean on main tracking origin/main.
The security findings lacked recorded fixed versions; they were not 21 confirmed
vulnerabilities in the current installed versions. No real maintenance was run.

## Prior user-machine validation

The supplied 2026-10-27 v0.1.3 log from morbo confirms 39 tests passed, a successful
live review of 939 installed packages, 42 pending updates, 2,444 security records
and 1,922 public open package reports. There were 21 security matches needing
review because no fixed version was recorded, and 126 matching open reports.
Those counts describe that run, not a current assessment or the current test fixtures.
The v0.1.3 maintenance dry-run also completed. Actual upgrades, cleanup and scrub
were not exercised. No extra live Arch queries were needed to validate the summary.

## Local test of the repository integration

From the Midnight Bump repository root, run the Cargo test command in the README,
then `./scripts/build-pre-upgrade-check.sh`. Check `--help` and `--version` through
`./scripts/pre-upgrade-check`. Preview `./scripts/system-maintenance.sh --dry-run`
(audit) and `./scripts/system-maintenance.sh --maintain --dry-run` (maintenance).
These previews must make no machine changes. Existing v0.1.5 user-machine review
results remain relevant because the Rust implementation is unchanged by this
integration. Commit the source and launchers after inspecting the local diff;
compiled artifacts and generated reports must remain untracked/ignored.

## First user-machine result (0.1.0)

The user supplied a successful local build/test/audit run from morbo dated
2026-10-21. All 21 original tests passed; the real audit completed with 13 passing
checks and four warning groups. See CHANGELOG.md for findings and fixes.

The subsequent 0.1.1 user-machine audit reported 13 passes and three warning
groups, confirming the privileged cache-size fix. Its backup completed and both
archives passed readability verification; the maintenance dry-run was reviewed.
Actual upgrades, cleanup and scrub were not established by those tests. Version
0.1.3 was subsequently tested on morbo as described above. The supplied attempt to start 0.1.2 actually remained in the old project
after the ZIP was missing; that attempt did not validate the newer review mode.

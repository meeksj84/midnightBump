use std::{env, ffi::OsString, path::PathBuf};

pub const HELP: &str = r#"Midnight Bump — pre-upgrade-check

Usage: pre-upgrade-check [MODE] [OPTIONS]

Modes (choose one):
  (no mode)       Full health audit and timestamped backup
  --backup       Same as the default
  --audit        Health and maintenance audit only; no backup or system changes
  --review       Cross-check installed packages against Arch issues; no sudo
  --maintain     Audit, backup, system update, cache cleanup, and Btrfs scrub
  --clean        Alias for --maintain (includes a system update)
  --install      Install/configure Midnight Bump from its repository

Options:
  --project-dir PATH   Midnight Bump repo (default: ~/Projects/midnight-bump)
  --backup-root PATH   Backup parent (default: ~/Backups)
  --dry-run           Print planned actions; run no external commands or writes
  --compact           Show progress/warnings/summary; retain details in a report
  --output PATH       Save a new private report in --review or --audit mode
  -v, --verbose       Show full issue/advisory text, summary lists and firewall entries
  -h, --help          Show this help
  -V, --version       Show version

Examples:
  pre-upgrade-check --audit
  pre-upgrade-check --review
  pre-upgrade-check --review --compact --output ~/pre-upgrade-review.txt
  pre-upgrade-check
  pre-upgrade-check --maintain --dry-run
  pre-upgrade-check --maintain
  pre-upgrade-check --install --project-dir ~/Projects/midnight-bump --dry-run

Run as your normal desktop user. Privileged operations use sudo when necessary.
Cache-size and Btrfs audit diagnostics may request sudo (read-only commands).
Maintenance requires confirmation for health warnings and stops on backup or
update failure. Both maintenance and installer require curl, checkupdates,
pacman/pacman-conf/vercmp, successful online checks, and explicit interactive
review acknowledgement.
Failed inventory/download/parse/update lookup or incomplete issue pagination
blocks package changes. ALL installed packages (including dependencies) are
matched against the full Arch security tracker and public open packaging reports.
Security fix versions use vercmp; general bug-version applicability needs review.
Closed/private/upstream-only/AUR reports are outside this scan. Retrieval success
or no match does not mean updates are safe.
Orphans are listed, never automatically removed. Scrub runs
in the background, only if / is Btrfs. No Git commit or push is performed.

--install installs official/AUR packages, sets UFW defaults, links desktop
configuration (backing up existing paths), generates colors, enables services,
and optionally installs the power-button configuration. This changes your setup.
Package prompts remain interactive. Review --dry-run before local testing.

Exit codes:
  0  Completed without warnings (online review still needs human assessment)
  1  Warnings, incomplete review-only checks, or user cancellation
  2  Invalid arguments, backup/report failure, or an operation failed

Audit/review checks do not change installed packages/configuration. checkupdates
uses the network and its separate temporary database; curl fetches official
feeds, security records and paginated package reports.
The --offline option to Cargo controls building only, not these runtime checks.
Default/maintenance modes write backups and a report; interactive package-command
output stays in terminal.
Kernel firewall entries are summarized in the terminal unless --verbose is set.
Saved reports always retain all kernel entries collected by the audit.
--output never overwrites files or symlinks; its parent directory must exist.
Report creation/write failures exit 2. With --compact, review/audit modes require
--output so detailed findings are retained. Backup/maintenance use their existing
report. --compact and --verbose cannot be combined. The desktop installer keeps
its full interactive output and does not accept --compact or --output.
An automatic PRE-UPGRADE SUMMARY shows package/update counts, security findings,
open-report groups and online coverage after review/audit/backup, and before
maintenance/installer confirmation. Unknown totals remain UNKNOWN; incomplete
online checks show BLOCKED. Complete checks still require manual review.
The compact summary shows 12 source-package groups, prioritizing security and
bug reports; --verbose and all saved reports retain every group.
PENDING UPDATES WITH MATCHING FINDINGS highlights proposed version changes with
unresolved security findings or open reports, including split packages. It shows
whether a candidate meets or remains below a recorded security fix. General
report matches do not establish that the proposed version is affected.
"#;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Backup,
    Audit,
    Review,
    Maintain,
    Install,
}

#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    pub dry_run: bool,
    pub verbose: bool,
    pub compact: bool,
    pub output: Option<PathBuf>,
    pub project: Option<PathBuf>,
    pub backup_root: Option<PathBuf>,
}
pub enum Action {
    Help,
    Version,
    Run(Options),
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Action, String> {
    let args: Vec<_> = args.into_iter().collect();
    let mut out = Options {
        mode: Mode::Backup,
        dry_run: false,
        verbose: false,
        compact: false,
        output: None,
        project: None,
        backup_root: None,
    };
    let mut selected = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].to_str().ok_or("Option names must be UTF-8")?;
        let mode = match a {
            "--help" | "-h" => return Ok(Action::Help),
            "--version" | "-V" => return Ok(Action::Version),
            "--audit" => Some(Mode::Audit),
            "--review" => Some(Mode::Review),
            "--backup" => Some(Mode::Backup),
            "--maintain" | "--clean" => Some(Mode::Maintain),
            "--install" => Some(Mode::Install),
            "--dry-run" => {
                out.dry_run = true;
                None
            }
            "--verbose" | "-v" => {
                out.verbose = true;
                None
            }
            "--compact" => {
                out.compact = true;
                None
            }
            "--project-dir" | "--backup-root" | "--output" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| format!("{a} requires a path"))?;
                if value.is_empty() || value.to_string_lossy().starts_with('-') {
                    return Err(format!("{a} requires a path"));
                }
                if a == "--output" {
                    if out.output.is_some() {
                        return Err("Specify --output only once".into());
                    }
                    out.output = Some(value.into());
                } else if a == "--project-dir" {
                    out.project = Some(value.into());
                } else {
                    out.backup_root = Some(value.into());
                }
                None
            }
            _ => return Err(format!("Unknown option: {a}")),
        };
        if let Some(mode) = mode {
            if selected {
                return Err("Choose only one mode".into());
            }
            out.mode = mode;
            selected = true;
        }
        i += 1;
    }
    if out.compact && out.verbose {
        return Err("Choose --compact or --verbose, not both".into());
    }
    if out.output.is_some() && !matches!(out.mode, Mode::Review | Mode::Audit) {
        return Err(
            "--output is for --review and --audit; backup/maintenance already save reports".into(),
        );
    }
    if out.compact && out.mode == Mode::Install {
        return Err("--compact is not available for the desktop installer; its interactive details remain visible".into());
    }
    if out.compact && matches!(out.mode, Mode::Review | Mode::Audit) && out.output.is_none() {
        return Err(
            "Use --output PATH with --compact in review/audit mode to retain the full findings"
                .into(),
        );
    }
    Ok(Action::Run(out))
}

#[derive(Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub project: PathBuf,
    pub backup_root: PathBuf,
}
impl Paths {
    pub fn new(opts: &Options) -> Result<Self, String> {
        let home = env::var_os("HOME")
            .filter(|s| !s.is_empty())
            .ok_or("HOME is not set")?;
        let home = PathBuf::from(home);
        if !home.is_absolute() || !home.is_dir() {
            return Err("HOME must be an existing absolute directory".into());
        }
        let absolute = |p: PathBuf| -> Result<PathBuf, String> {
            if p.is_absolute() {
                Ok(p)
            } else {
                env::current_dir()
                    .map(|d| d.join(p))
                    .map_err(|e| e.to_string())
            }
        };
        Ok(Self {
            project: absolute(
                opts.project
                    .clone()
                    .unwrap_or_else(|| home.join("Projects/midnight-bump")),
            )?,
            backup_root: absolute(
                opts.backup_root
                    .clone()
                    .unwrap_or_else(|| home.join("Backups")),
            )?,
            home,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options(a: &[&str]) -> Options {
        match parse(a.iter().map(OsString::from)).unwrap() {
            Action::Run(o) => o,
            _ => panic!(),
        }
    }
    #[test]
    fn legacy_modes_and_default() {
        assert_eq!(options(&[]).mode, Mode::Backup);
        assert_eq!(options(&["--clean"]).mode, Mode::Maintain);
        assert_eq!(options(&["--maintain"]).mode, Mode::Maintain);
        assert!(options(&["--audit", "--verbose"]).verbose);
        assert!(options(&["-v"]).verbose);
        assert!(!options(&[]).verbose);
        assert!(matches!(parse([OsString::from("-h")]), Ok(Action::Help)));
    }
    #[test]
    fn invalid_cli_is_rejected() {
        for a in [
            vec!["--audit", "--install"],
            vec!["--oops"],
            vec!["--project-dir"],
            vec!["--backup-root", "--maintain"],
            vec!["--output"],
            vec!["--output", "report.txt"],
            vec!["--audit", "--compact"],
            vec![
                "--review",
                "--compact",
                "--verbose",
                "--output",
                "report.txt",
            ],
            vec!["--install", "--compact"],
            vec!["--review", "--output", "a.txt", "--output", "b.txt"],
        ] {
            assert!(parse(a.into_iter().map(OsString::from)).is_err());
        }
    }
    #[test]
    fn compact_and_report_options_preserve_mode_semantics() {
        let opts = options(&[
            "--review",
            "--compact",
            "--output",
            "report with spaces.txt",
        ]);
        assert_eq!(opts.mode, Mode::Review);
        assert!(opts.compact);
        assert_eq!(opts.output, Some(PathBuf::from("report with spaces.txt")));
        assert!(options(&["--maintain", "--compact"]).compact);
        assert!(options(&["--clean", "--compact"]).compact);
        assert!(!options(&["--audit", "--output", "report.txt"]).compact);
    }
}

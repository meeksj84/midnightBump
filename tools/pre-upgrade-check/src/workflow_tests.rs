//! Real temporary archives, simulated machine commands. Never uses real sudo/pacman.
use super::*;
use runtime::{Output, Runner};
use std::{
    cell::RefCell,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

struct Fixture {
    root: PathBuf,
    paths: Paths,
}
impl Fixture {
    fn new() -> Self {
        let root = backup::unique_dir(&std::env::temp_dir(), "pre-upgrade-workflow-test").unwrap();
        let home = root.join("home with spaces");
        let project = home.join("Projects/midnight-bump");
        fs::create_dir_all(home.join(".config")).unwrap();
        fs::create_dir_all(project.join(".git")).unwrap();
        fs::write(home.join(".config/example.conf"), "original config\n").unwrap();
        fs::write(home.join(".bashrc"), "# original shell\n").unwrap();
        fs::write(project.join("project.txt"), "original project\n").unwrap();
        let paths = Paths {
            backup_root: home.join("Backups"),
            home,
            project,
        };
        Self { root, paths }
    }
    fn backup_dir(&self) -> PathBuf {
        fs::read_dir(&self.paths.backup_root)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[derive(Default)]
struct Fake {
    calls: RefCell<Vec<String>>,
    fail_archive: bool,
    fail_checkupdates: bool,
    fail_journal: bool,
    kernel_warnings: Option<String>,
    review_error: Option<&'static str>,
    refuse_review: bool,
    installed_output: Option<String>,
    pending_output: Option<String>,
    bug_reports: bool,
}
impl Runner for Fake {
    fn run(&self, p: &str, a: &[OsString], cwd: Option<&Path>, live: bool) -> Output {
        let args: Vec<_> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        let s = args.join(" ");
        self.calls.borrow_mut().push(format!("{p} {s}"));
        let (code, out, err) = match p {
            "id" => (0, "1000\n", ""),
            "date" => (0, "2026-10-18-123456\n", ""),
            "hostnamectl" => (0, "test-host\n", ""),
            "findmnt" if args.iter().any(|a| a == "/boot") => (1, "", ""),
            "findmnt" => (0, "ext4\n", ""),
            "pacman" if s == "-Qqen" => (0, "linux\nbase\n", ""),
            "pacman" if s == "-Q" && self.review_error == Some("inventory") => {
                (1, "", "database unavailable")
            }
            "pacman" if s == "-Q" => (
                0,
                self.installed_output.as_deref().unwrap_or("linux 10-1\n"),
                "",
            ),
            "pacman-conf" => (0, "/test-pacman-db\n", ""),
            "vercmp" if self.review_error == Some("comparison") => (0, "unexpected result", ""),
            "vercmp" => (0, "1\n", ""),
            "pacman" if s == "-Q test-official" => (1, "", ""),
            "pacman" if s == "-Qqem" || s == "-Qtdq" || s == "-Qm" => (1, "", ""),
            "checkupdates" if self.fail_checkupdates => (1, "", "network unavailable"),
            "checkupdates" if self.review_error == Some("updates") => (0, "unexpected output", ""),
            "checkupdates" if self.pending_output.is_some() => {
                (0, self.pending_output.as_deref().unwrap(), "")
            }
            "checkupdates" => (2, "", ""),
            "curl" if self.review_error == Some("network") => (22, "", "HTTP 503"),
            "curl" if self.review_error == Some("html") => (0, "<html>Access challenge</html>", ""),
            "curl"
                if args.last().is_some_and(|s| s == issues::SECURITY_URL)
                    && self.review_error == Some("tracker") =>
            {
                (0, "[{", "")
            }
            "curl" if args.last().is_some_and(|s| s == issues::SECURITY_URL) => {
                (0, issues::TEST_SECURITY, "")
            }
            "curl"
                if args
                    .last()
                    .is_some_and(|s| s.starts_with(issues::BUGS_URL) && s.ends_with("page=2")) =>
            {
                (22, "", "page unavailable")
            }
            "curl"
                if args.last().is_some_and(|s| s.starts_with(issues::BUGS_URL))
                    && (self.bug_reports || self.review_error == Some("page2")) =>
            {
                (0, issues::TEST_BUG, "")
            }
            "curl" if args.last().is_some_and(|s| s.starts_with(issues::BUGS_URL)) => (0, "[]", ""),
            "curl" if args.last().is_some_and(|s| s == review::NEWS_URL) => {
                (0, feed::TEST_NEWS, "")
            }
            "curl" if self.review_error == Some("security") => (28, "", "Timed out"),
            "curl" => (0, feed::TEST_SECURITY, ""),
            "journalctl" if self.fail_journal => (1, "", "permission denied"),
            "journalctl" if s == "-k -b -p warning..alert --no-pager" => {
                (0, self.kernel_warnings.as_deref().unwrap_or(""), "")
            }
            "tar" if self.fail_archive => (2, "", "simulated archive failure"),
            "tar" => return RealRunner.run(p, a, cwd, live),
            "git" => (0, "test git state\n", ""),
            _ => (0, "", ""),
        };
        Output {
            code,
            out: if p == "curl" && code == 0 {
                let pagination = if args.last().is_some_and(|s| s.starts_with(issues::BUGS_URL)) {
                    if self.review_error == Some("pagination") {
                        "x-page: 1\r\nx-per-page: 100\r\n"
                    } else if self.review_error == Some("page2") {
                        "x-page: 1\r\nx-per-page: 100\r\nx-next-page: 2\r\n"
                    } else {
                        "x-page: 1\r\nx-per-page: 100\r\nx-next-page: \r\n"
                    }
                } else {
                    ""
                };
                format!("HTTP/2 200\r\n{pagination}\r\n{out}")
            } else {
                out.into()
            },
            err: err.into(),
        }
    }
    fn have(&self, p: &str) -> bool {
        self.review_error != Some(p)
    }
    fn confirm(&self, question: &str) -> bool {
        self.calls.borrow_mut().push(format!("CONFIRM {question}"));
        !(self.refuse_review && question.starts_with("Have you reviewed"))
    }
    fn read_text(&self, path: &Path) -> Result<String> {
        if self.review_error == Some("metadata") {
            return Err("simulated unreadable package metadata".into());
        }
        let dir = path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy();
        for line in self
            .installed_output
            .as_deref()
            .unwrap_or("linux 10-1\n")
            .lines()
        {
            let (name, version) = line.split_once(' ').unwrap();
            if dir == format!("{name}-{version}") {
                let base = if name == "linux-headers" {
                    "linux"
                } else {
                    name
                };
                return Ok(format!(
                    "%NAME%\n{name}\n\n%VERSION%\n{version}\n\n%BASE%\n{base}\n\n"
                ));
            }
        }
        Err("unexpected test metadata path".into())
    }
}
fn options(mode: Mode) -> Options {
    Options {
        mode,
        dry_run: false,
        verbose: false,
        compact: false,
        output: None,
        project: None,
        backup_root: None,
    }
}
#[test]
fn backup_roundtrip_handles_spaces_and_empty_foreign_package_list() {
    let fixture = Fixture::new();
    let fake = Fake::default();
    let mut app = App::new(&fake);
    assert!(execute(&options(Mode::Backup), &fixture.paths, &mut app).is_ok());
    let d = fixture.backup_dir();
    assert!(d.join("backup-complete.txt").is_file());
    assert_eq!(
        fs::read_to_string(d.join("official-packages.txt")).unwrap(),
        "linux\nbase\n"
    );
    assert_eq!(fs::metadata(d.join("aur-packages.txt")).unwrap().len(), 0);
    assert!(d.join("midnight-bump-git-status.txt").is_file());
    assert!(d.join("midnight-bump-commit.txt").is_file());
    let archive = d.join("home-config.tar.gz");
    let contents = RealRunner.run(
        "tar",
        &[
            "-xOf".into(),
            archive.into_os_string(),
            ".config/example.conf".into(),
        ],
        None,
        false,
    );
    assert!(contents.ok());
    assert_eq!(contents.out, "original config\n");
    let contents = RealRunner.run(
        "tar",
        &[
            "-xOf".into(),
            d.join("midnight-bump.tar.gz").into_os_string(),
            "midnight-bump/project.txt".into(),
        ],
        None,
        false,
    );
    assert!(contents.ok());
    assert_eq!(contents.out, "original project\n");
    let saved = fs::read_to_string(d.join("pre-upgrade-report.txt")).unwrap();
    assert!(saved.contains("PRE-UPGRADE SUMMARY"));
    assert!(saved.contains("Full report:"));
    assert!(saved.find("Backup:").unwrap() < saved.find("PRE-UPGRADE SUMMARY").unwrap());
    assert_eq!(
        fs::read_to_string(fixture.paths.home.join(".config/example.conf")).unwrap(),
        "original config\n"
    );
    assert!(fake
        .calls
        .borrow()
        .iter()
        .all(|s| !s.starts_with("sudo ") || s == "sudo du -sh /var/cache/pacman/pkg"));
}
#[test]
fn archive_failure_blocks_upgrade_and_completion_marker() {
    let fixture = Fixture::new();
    let fake = Fake {
        fail_archive: true,
        ..Fake::default()
    };
    let result = execute(
        &options(Mode::Maintain),
        &fixture.paths,
        &mut App::new(&fake),
    );
    assert!(result.is_err());
    assert!(!fixture.backup_dir().join("backup-complete.txt").exists());
    assert!(fake
        .calls
        .borrow()
        .iter()
        .all(|s| !s.starts_with("sudo ") || s == "sudo du -sh /var/cache/pacman/pkg"));
}
#[test]
fn audit_creates_no_backup_and_invokes_no_mutations() {
    let fixture = Fixture::new();
    let fake = Fake::default();
    execute(&options(Mode::Audit), &fixture.paths, &mut App::new(&fake)).unwrap();
    assert!(!fixture.paths.backup_root.exists());
    assert!(fake.calls.borrow().iter().all(|s| (!s.starts_with("sudo ")
        || s == "sudo du -sh /var/cache/pacman/pkg")
        && !s.starts_with("tar ")));
    assert!(fake
        .calls
        .borrow()
        .iter()
        .any(|s| s == "sudo du -sh /var/cache/pacman/pkg"));
}
#[test]
fn dry_run_of_every_mode_invokes_no_commands_or_writes() {
    let fixture = Fixture::new();
    let fake = Fake::default();
    for mode in [
        Mode::Backup,
        Mode::Audit,
        Mode::Review,
        Mode::Maintain,
        Mode::Install,
    ] {
        let mut opts = options(mode);
        opts.dry_run = true;
        opts.compact = mode != Mode::Install;
        if matches!(mode, Mode::Audit | Mode::Review) {
            opts.output = Some(fixture.root.join("must-not-be-written.txt"));
        }
        assert_eq!(
            execute(&opts, &fixture.paths, &mut App::new(&fake)).unwrap(),
            0
        );
    }
    assert!(fake.calls.borrow().is_empty());
    assert!(!fixture.paths.backup_root.exists());
    assert!(!fixture.root.join("must-not-be-written.txt").exists());
}

#[test]
fn compact_review_saves_full_private_report_and_highlights_pending_split_packages() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let fake = Fake {
        installed_output: Some("linux 10-1\nlinux-headers 10-1\nunrelated 10-1\n".into()),
        pending_output: Some("linux-headers 10-1 -> 11-1\nunrelated 10-1 -> 11-1\n".into()),
        bug_reports: true,
        ..Fake::default()
    };
    let mut opts = options(Mode::Review);
    opts.compact = true;
    let path = fixture.root.join("review with spaces.txt");
    opts.output = Some(path.clone());
    let mut compact = App::new(&fake);
    assert_eq!(execute(&opts, &fixture.paths, &mut compact).unwrap(), 1);
    let terminal = compact.report.terminal.join("\n");
    let full = fs::read_to_string(&path).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(terminal.contains("PRE-UPGRADE SUMMARY"));
    assert!(terminal.contains("linux & boot: manual intervention"));
    assert!(!terminal.contains("Reported after installing version 3."));
    assert!(full.contains("Reported after installing version 3."));
    let highlights = terminal
        .split("PENDING UPDATES WITH MATCHING FINDINGS")
        .nth(1)
        .unwrap()
        .split("Packages with")
        .next()
        .unwrap();
    assert!(highlights.contains("linux-headers: 10-1 -> 11-1"));
    assert!(highlights.contains("1 for source linux"));
    assert!(!highlights.contains("unrelated"));
    assert!(full.contains("unrelated 10-1 -> 11-1"));
    assert!(full.contains("PRE-UPGRADE SUMMARY"));
    assert!(!fixture.paths.backup_root.exists());
    let calls = fake.calls.borrow().clone();
    assert!(calls.iter().all(|s| !s.starts_with("sudo ")));
    fake.calls.borrow_mut().clear();
    let mut normal = App::new(&fake);
    assert_eq!(
        execute(&options(Mode::Review), &fixture.paths, &mut normal).unwrap(),
        1
    );
    assert_eq!(calls, *fake.calls.borrow());
    assert_eq!(normal.warnings, compact.warnings);
    assert!(normal
        .report
        .terminal
        .join("\n")
        .contains("Reported after installing version 3."));
}

#[test]
fn compact_audit_retains_health_diagnostics_and_online_failure_warnings() {
    let fixture = Fixture::new();
    let fake = Fake {
        kernel_warnings: Some("test kernel diagnostic retained in report\n".into()),
        review_error: Some("page2"),
        ..Fake::default()
    };
    let path = fixture.root.join("audit.txt");
    let mut opts = options(Mode::Audit);
    opts.compact = true;
    opts.output = Some(path.clone());
    let mut app = App::new(&fake);
    assert_eq!(execute(&opts, &fixture.paths, &mut app).unwrap(), 1);
    let terminal = app.report.terminal.join("\n");
    let full = fs::read_to_string(path).unwrap();
    assert!(terminal.contains("Package bug cross-check INCOMPLETE"));
    assert!(terminal.contains("Decision: BLOCKED"));
    assert!(terminal.contains("Audit contains warnings"));
    assert!(!terminal.contains("test kernel diagnostic retained in report"));
    assert!(full.contains("test kernel diagnostic retained in report"));
    assert!(full.contains("Decision: BLOCKED"));
    assert!(!terminal.contains("No pending updates matched"));
}

#[test]
fn requested_report_never_overwrites_files_or_symlinks_and_fails_before_scan() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let existing = fixture.root.join("existing.txt");
    fs::write(&existing, "keep this report").unwrap();
    let link = fixture.root.join("link.txt");
    symlink(&existing, &link).unwrap();
    for path in [
        existing.clone(),
        link,
        fixture.root.join("missing-parent/report.txt"),
    ] {
        let fake = Fake::default();
        let mut app = App::new(&fake);
        let mut opts = options(Mode::Review);
        opts.output = Some(path);
        assert!(execute(&opts, &fixture.paths, &mut app).is_err());
        assert_eq!(*fake.calls.borrow(), vec!["id -u"]);
        assert_eq!(fs::read_to_string(&existing).unwrap(), "keep this report");
    }
    assert!(!fixture.root.join("missing-parent").exists());
}

#[test]
fn mismatched_update_inventory_cannot_leave_trusted_pending_highlights() {
    let fake = Fake {
        pending_output: Some("linux 9-1 -> 11-1\n".into()),
        bug_reports: true,
        ..Fake::default()
    };
    let mut app = App::new(&fake);
    assert!(!review::run(&mut app, &[], &[]));
    assert!(app.review_summary.updates.is_empty());
    let summary = app.review_summary.render(app.review_complete, false);
    assert!(summary.contains("Coverage UNKNOWN"));
    assert!(!summary.contains("No pending updates matched"));
    assert!(maintenance::run(&mut app).is_err());
}
#[test]
fn review_only_fetches_notices_without_sudo_backup_or_installation() {
    let fixture = Fixture::new();
    let fake = Fake::default();
    let mut app = App::new(&fake);
    let report = fixture.root.join("online-report.txt");
    app.report.open(&report).unwrap();
    assert_eq!(
        execute(&options(Mode::Review), &fixture.paths, &mut app).unwrap(),
        0
    );
    let saved = fs::read_to_string(report).unwrap();
    assert!(saved.contains("linux & boot: manual intervention"));
    assert!(saved.contains("Cross-checking ALL 1 installed packages"));
    assert!(saved.contains("public open package reports"));
    let summary = saved.split("PRE-UPGRADE SUMMARY").nth(1).unwrap();
    assert!(summary.contains("Installed packages checked:         1"));
    assert!(summary.contains("Available repository updates:       0"));
    assert!(summary.contains("Security findings needing review:   0"));
    assert!(summary.contains("Required online checks:             COMPLETE"));
    assert!(summary.contains("Decision: REVIEW REQUIRED"));
    assert_eq!(app.warnings, 0);
    assert!(app.review_complete);
    assert!(!fixture.paths.backup_root.exists());
    assert!(fake.calls.borrow().iter().all(|s| [
        "id ",
        "date ",
        "checkupdates ",
        "curl ",
        "pacman ",
        "pacman-conf ",
        "vercmp "
    ]
    .iter()
    .any(|prefix| s.starts_with(prefix))));
}
#[test]
fn review_matches_installed_dependencies_and_source_packages_even_without_updates() {
    let fixture = Fixture::new();
    let fake = Fake {
        installed_output: Some("linux 10-1\nlinux-headers 10-1\nunrelated 10-1\n".into()),
        bug_reports: true,
        ..Fake::default()
    };
    let mut app = App::new(&fake);
    let report = fixture.root.join("installed-review.txt");
    app.report.open(&report).unwrap();
    assert_eq!(
        execute(&options(Mode::Review), &fixture.paths, &mut app).unwrap(),
        1
    );
    let text = fs::read_to_string(report).unwrap();
    assert!(text.contains("Cross-checking ALL 3 installed packages"));
    assert!(text.contains("Installed: linux-headers 10-1; pending: none"));
    assert!(!text.contains("Installed: unrelated"));
    assert!(text.contains("Test boot regression"));
    let summary = text.split("PRE-UPGRADE SUMMARY").nth(1).unwrap();
    assert!(summary.contains("linux: 1 report(s)"));
    assert!(summary.contains("Installed: linux, linux-headers"));
    assert!(app.review_complete);
}
#[test]
fn failed_online_inputs_block_all_maintenance_mutations() {
    for failure in [
        "curl",
        "checkupdates",
        "network",
        "html",
        "security",
        "updates",
        "inventory",
        "metadata",
        "tracker",
        "comparison",
        "pagination",
        "page2",
        "pacman-conf",
        "vercmp",
    ] {
        let fake = Fake {
            review_error: Some(failure),
            ..Fake::default()
        };
        let mut app = App::new(&fake);
        assert!(!review::run(&mut app, &[], &[]), "{failure}");
        let summary = app.review_summary.render(app.review_complete, false);
        assert!(summary.contains("Decision: BLOCKED"), "{failure}");
        assert!(summary.contains("UNKNOWN"), "{failure}");
        assert!(!summary.contains("Decision: REVIEW REQUIRED"), "{failure}");
        if ["pagination", "page2"].contains(&failure) {
            assert!(app.review_summary.bugs.is_none());
        }
        if ["tracker", "comparison"].contains(&failure) {
            assert!(app.review_summary.security.is_none());
        }
        assert!(maintenance::run(&mut app).is_err(), "{failure}");
        assert!(
            !fake
                .calls
                .borrow()
                .iter()
                .any(|s| s.starts_with("sudo ") || s.starts_with("CONFIRM ")),
            "{failure}"
        );
    }
    let fake = Fake {
        fail_checkupdates: true,
        ..Fake::default()
    };
    let mut app = App::new(&fake);
    assert!(!review::run(&mut app, &[], &[]));
    assert!(maintenance::run(&mut app).is_err());
}
#[test]
fn healthy_maintenance_still_requires_explicit_online_review_acknowledgement() {
    for refuse_review in [true, false] {
        let fake = Fake {
            refuse_review,
            ..Fake::default()
        };
        let mut app = App::new(&fake);
        assert!(review::run(&mut app, &[], &[]));
        assert_eq!(app.warnings, 0);
        assert_eq!(maintenance::run(&mut app).unwrap(), !refuse_review);
        let calls = fake.calls.borrow();
        let confirmation = calls
            .iter()
            .position(|s| s.starts_with("CONFIRM Have you reviewed"))
            .unwrap();
        let update = calls.iter().position(|s| s == "sudo pacman -Syu");
        if refuse_review {
            assert!(update.is_none());
        } else {
            assert!(confirmation < update.unwrap());
        }
    }
}
#[test]
fn installer_stops_before_package_and_configuration_changes_on_review_failure_or_refusal() {
    for (error, refuse) in [(Some("network"), false), (None, true)] {
        let fixture = Fixture::new();
        for path in install::REQUIRED {
            let path = fixture.paths.project.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "# fixture\n").unwrap();
        }
        fs::write(
            fixture.paths.project.join("packages/official.txt"),
            "test-official\n",
        )
        .unwrap();
        let fake = Fake {
            review_error: error,
            refuse_review: refuse,
            ..Fake::default()
        };
        let result = execute(
            &options(Mode::Install),
            &fixture.paths,
            &mut App::new(&fake),
        );
        if error.is_some() {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), 1);
        }
        assert!(fake.calls.borrow().iter().all(|s| !s.starts_with("sudo ")
            && !s.starts_with("paru ")
            && !s.starts_with("makepkg ")));
        assert!(!fixture
            .paths
            .home
            .join(".config/hypr/hyprland.lua")
            .exists());
        assert!(!fixture.paths.backup_root.exists());
    }
}
#[test]
fn incomplete_installer_repository_fails_before_any_install_command() {
    let fixture = Fixture::new();
    let fake = Fake::default();
    assert!(execute(
        &options(Mode::Install),
        &fixture.paths,
        &mut App::new(&fake)
    )
    .is_err());
    assert_eq!(*fake.calls.borrow(), vec!["id -u"]);
}
#[test]
fn unavailable_updates_and_journals_are_not_clean_checks() {
    let baseline = Fake::default();
    let mut clean = App::new(&baseline);
    audit::run(&mut clean);
    let fake = Fake {
        fail_checkupdates: true,
        fail_journal: true,
        ..Fake::default()
    };
    let mut app = App::new(&fake);
    audit::run(&mut app);
    assert!(app.warnings > clean.warnings);
    assert!(app.passes < clean.passes);
}

#[test]
fn compact_and_verbose_modes_both_keep_full_kernel_report() {
    let fixture = Fixture::new();
    let kernel = "Oct 21 host kernel: mt7921e timeout\nOct 21 host kernel: [UFW BLOCK] first packet\nOct 21 host kernel: [UFW BLOCK] second packet\nOct 21 host kernel: amdgpu error\n";
    let fake = Fake {
        kernel_warnings: Some(kernel.into()),
        ..Fake::default()
    };
    for verbose in [false, true] {
        let mut opts = options(Mode::Audit);
        opts.verbose = verbose;
        let mut app = App::new(&fake);
        let path = fixture.root.join(format!("report-{verbose}.txt"));
        app.report.open(&path).unwrap();
        execute(&opts, &fixture.paths, &mut app).unwrap();
        assert_eq!(app.verbose, verbose);
        let saved = fs::read_to_string(path).unwrap();
        assert!(saved.contains(kernel));
        assert_eq!(
            saved.contains("2 UFW BLOCK kernel entries summarized"),
            !verbose
        );
    }
}
#[test]
fn recursive_backup_destination_is_rejected() {
    let mut fixture = Fixture::new();
    let fake = Fake::default();
    fixture.paths.backup_root = fixture.paths.project.join("Backups");
    assert!(execute(&options(Mode::Backup), &fixture.paths, &mut App::new(&fake)).is_err());
    assert!(fake.calls.borrow().iter().all(|s| !s.starts_with("tar ")));
}

#[test]
fn installer_preserves_files_and_links_and_records_expected_actions() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    for path in install::REQUIRED {
        let path = fixture.paths.project.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "# fixture\n").unwrap();
    }
    fs::write(
        fixture.paths.project.join("packages/official.txt"),
        "test-official # comment\n",
    )
    .unwrap();
    fs::write(fixture.paths.project.join("packages/aur.txt"), "test-aur\n").unwrap();
    fs::create_dir_all(fixture.paths.home.join(".config/waybar")).unwrap();
    fs::write(
        fixture.paths.home.join(".config/waybar/style.css"),
        "original style",
    )
    .unwrap();
    fs::create_dir_all(fixture.paths.home.join(".config/mako")).unwrap();
    symlink(
        "previous-target",
        fixture.paths.home.join(".config/mako/config"),
    )
    .unwrap();
    fs::create_dir_all(fixture.paths.home.join(".config/hypr")).unwrap();
    fs::write(
        fixture.paths.home.join(".config/hypr/hyprland.lua"),
        "hl.exec(\"dunst\")\nmy_setting = 42\n",
    )
    .unwrap();
    let fake = Fake::default();
    execute(
        &options(Mode::Install),
        &fixture.paths,
        &mut App::new(&fake),
    )
    .unwrap();
    for (source, target) in install::LINKS {
        assert_eq!(
            fs::canonicalize(fixture.paths.home.join(target)).unwrap(),
            fs::canonicalize(fixture.paths.project.join(source)).unwrap()
        );
    }
    let backup = fs::read_dir(fixture.paths.home.join(".config/waybar"))
        .unwrap()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("style.css.backup-")
        })
        .unwrap()
        .path();
    assert_eq!(fs::read_to_string(backup).unwrap(), "original style");
    let backup = fs::read_dir(fixture.paths.home.join(".config/mako"))
        .unwrap()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("config.backup-")
        })
        .unwrap()
        .path();
    assert_eq!(
        fs::read_link(backup).unwrap(),
        PathBuf::from("previous-target")
    );
    let lua = fs::read_to_string(fixture.paths.home.join(".config/hypr/hyprland.lua")).unwrap();
    assert!(lua.contains("my_setting = 42"));
    assert!(lua.contains("require(\"keybindings\")"));
    assert!(!lua.contains("dunst"));
    let original = fs::read_to_string(
        fixture
            .paths
            .home
            .join(".config/hypr/hyprland.lua.midnight-bump.bak"),
    )
    .unwrap();
    assert!(original.contains("dunst"));
    let calls = fake.calls.borrow();
    assert!(calls
        .iter()
        .any(|s| s == "sudo pacman -Syu --needed test-official"));
    assert!(calls.iter().any(|s| s == "paru -S --needed test-aur"));
    assert!(calls.iter().any(|s| s == "sudo ufw --force enable"));
    assert!(calls
        .iter()
        .any(|s| s == "systemctl --user enable --now midnight-bump-wallpaper.service"));
    assert!(calls
        .iter()
        .all(|s| !s.starts_with("git push") && !s.starts_with("git commit")));
}

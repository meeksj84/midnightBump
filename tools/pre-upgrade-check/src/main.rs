mod audit;
mod backup;
mod cli;
mod feed;
mod http;
mod install;
mod issues;
mod json;
mod maintenance;
mod packages;
mod review;
mod runtime;
mod summary;
#[cfg(test)]
mod workflow_tests;

use cli::{Action, Mode, Options, Paths};
use runtime::{App, RealRunner, Result};

fn dry_run(opts: &Options, p: &Paths) {
    println!(
        "DRY RUN — no external commands or writes\nMode: {:?}\nProject: {}\nBackup root: {}",
        opts.mode,
        p.project.display(),
        p.backup_root.display()
    );
    if let Some(path) = &opts.output {
        println!(
            "Save the full report to a new private file: {} (never overwrite)",
            path.display()
        );
    }
    if opts.compact {
        println!("Compact terminal output: progress, warnings, news links and final summary; detailed findings stay in the report.");
    }
    println!("Online review: inventory ALL installed packages with pacman -Q and source-package metadata; refresh pending versions with checkupdates --nocolor.\nFetch Arch's full security tracker and compare installed/pending versions using vercmp.\nFetch all pages of public open Arch package reports and match installed source-package names, including split packages.\nFetch Arch News and published advisories. Show matching records, severity/status, versions and source links.\nFailed/missing online inputs or incomplete pagination block package changes. A package match is not proof a general bug affects your version.\nClosed/private/upstream/AUR reports and transaction changes require separate review.");
    println!("Display a compact PRE-UPGRADE SUMMARY with package/update counts, review findings, coverage, and packages needing attention before any package-change confirmation. Keep full details in backup/maintenance reports.");
    if opts.mode == Mode::Review {
        println!("Review only: no backup, sudo, installation, cleanup, or confirmation. Exit after displaying online/manual review information.");
        return;
    }
    if opts.mode == Mode::Install {
        println!("Before any setup/package changes: include requested manifest packages in the review and require explicit interactive acknowledgement.");
        println!("Validate all {} required project files and both package manifests.\nQuery installed packages; install missing official packages: sudo pacman -Syu --needed <packages>\nIf missing, build Paru from https://aur.archlinux.org/paru.git using makepkg -si --needed\nInstall manifest AUR packages: paru -S --needed <packages>\nIf available, set UFW deny incoming/allow outgoing; enable ufw.service and UFW.", install::REQUIRED.len());
        for (s, t) in install::LINKS {
            println!(
                "Back up existing destination and link: {} -> {}",
                p.home.join(t).display(),
                p.project.join(s).display()
            );
        }
        println!("Back up/migrate hyprland.lua (remove legacy Dunst/awww starts, add keybindings require).\nMark set-wallpaper executable; generate initial palette using matugen.\nReload user units; enable Waybar/Hypridle/Mako. Stop Dunst, SwayNC and existing Mako.\nRemove stale wallpaper/awww enable symlinks; enable awww and wallpaper services.\nAsk before installing power-button configuration (back up existing destination).\nPackage-manager prompts remain interactive. No Git commit or push.");
        return;
    }
    println!("Audit: services, boot/kernel logs, boot timing, mounts, package database, orphans,\nupdates, Hyprland/Hypridle, audio/RTKit, AMD/NVIDIA, battery, network/firmware,\ncache size, journal size, and Btrfs usage/device stats/scrub status where applicable.\nCache-size and Btrfs read-only diagnostics use sudo and may prompt for your password.\nOrphans are displayed only. checkupdates may access the network.");
    if opts.mode != Mode::Audit {
        println!("Create unique private backup directory and pre-upgrade-report.txt.\nSave pacman -Qqen and -Qqem package lists.\nArchive existing paths under {}: {}\nArchive repository {}; record Git status and commit; verify archives.\nStop if backup or report writing fails.", p.home.display(), backup::HOME_ITEMS.join(", "), p.project.display());
    }
    if opts.mode == Mode::Maintain {
        println!("Ask before proceeding with warnings; cancel if non-interactive.\nRequire explicit interactive acknowledgement of Arch notices and package bug-report review, even when health checks pass.\nUpdate: sudo pacman -Syu\nOnly after successful update: sudo paccache -r (if available)\nOnly on Btrfs root: check scrub status; sudo btrfs scrub start / unless already running\nCheck failed system/user services. Background scrub completion is checked separately.");
    }
}
fn execute(opts: &Options, paths: &Paths, app: &mut App) -> Result<i32> {
    app.verbose = opts.verbose;
    if opts.dry_run {
        dry_run(opts, paths);
        return Ok(0);
    }
    let uid = app.get("id", &["-u"]);
    if !uid.ok() {
        return Err("Cannot determine user ID".into());
    }
    if uid.out.trim() == "0" {
        return Err("Run as your normal desktop user, without sudo. Privileged operations call sudo themselves.".into());
    }
    if let Some(path) = &opts.output {
        app.report.open(path)?;
    }
    app.report.compact = opts.compact;
    if let Some(path) = app.report.path() {
        let message = format!("Full report: {}", path.display());
        app.important(message);
    }
    if opts.compact {
        app.important("Compact output enabled. Read the full report, including Arch notices, before approving package changes.");
    }
    if opts.mode == Mode::Install {
        if !install::run(app, paths)? {
            return Ok(1);
        }
        return Ok(i32::from(app.warnings > 0));
    }
    if opts.mode == Mode::Review {
        let complete = review::run(app, &[], &[]);
        app.important(if complete { "[REVIEW] Online inputs retrieved. Read the notices and package reports; no installation performed." } else { "[REVIEW] Online checks incomplete; package changes remain blocked." });
        summary::show(app);
        app.report.sync()?;
        return Ok(i32::from(!complete || app.warnings > 0));
    }
    let dir = if opts.mode == Mode::Audit {
        None
    } else {
        Some(backup::workspace(app, paths)?)
    };
    audit::run(app);
    if let Some(dir) = &dir {
        if let Err(error) = backup::run(app, paths, dir) {
            summary::show(app);
            return Err(error);
        }
        app.important(format!("Backup: {}", dir.display()));
    }
    app.report.sync()?;
    summary::show(app);
    app.report.sync()?;
    if opts.mode == Mode::Maintain && !maintenance::run(app)? {
        return Ok(1);
    }
    app.heading("FINAL STATUS");
    match (opts.mode, app.warnings) {
        (Mode::Audit, 0) => {
            app.important("[REVIEW] Health audit passed; online inputs retrieved. Manual notice/package review still required. No backup or system changes performed.")
        }
        (Mode::Audit, _) => {
            app.important("[REVIEW] Audit contains warnings. No backup or system changes performed.")
        }
        (Mode::Backup, 0) => {
            app.important("[REVIEW] Health check passed and backup completed. Manual notice/package review still required before upgrading.")
        }
        (Mode::Backup, _) => {
            app.important("[REVIEW] Backup completed; review warnings before upgrading.")
        }
        (Mode::Maintain, 0) => app
            .important("[READY] Maintenance steps completed. A background scrub may still be running."),
        (Mode::Maintain, _) => {
            app.important("[REVIEW] Maintenance steps completed with warnings; review the report.")
        }
        _ => (),
    }
    app.report.sync()?;
    Ok(i32::from(app.warnings > 0))
}
fn main() {
    let action = match cli::parse(std::env::args_os().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\nRun pre-upgrade-check --help for usage.");
            std::process::exit(2);
        }
    };
    let opts = match action {
        Action::Help => {
            print!("{}", cli::HELP);
            return;
        }
        Action::Version => {
            println!("pre-upgrade-check {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Action::Run(o) => o,
    };
    let paths = match Paths::new(&opts) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mut app = App::new(&RealRunner);
    let code = match execute(&opts, &paths, &mut app) {
        Ok(c) => c,
        Err(e) => {
            app.important(format!("[FAILED] {e}"));
            let _ = app.report.sync();
            2
        }
    };
    std::process::exit(code);
}

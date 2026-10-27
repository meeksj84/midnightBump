use crate::{
    cli::Paths,
    runtime::{args, App, Result},
};
use std::{
    ffi::OsString,
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const HOME_ITEMS: &[&str] = &[
    ".config",
    ".local/bin",
    ".local/share/applications",
    ".local/share/fonts",
    ".local/share/icons",
    ".local/share/themes",
    ".bashrc",
    ".profile",
];
pub fn unique_dir(parent: &Path, prefix: &str) -> Result<PathBuf> {
    for n in 0..1000 {
        let p = parent.join(format!("{prefix}-{}-{n:03}", std::process::id()));
        match DirBuilder::new().mode(0o700).create(&p) {
            Ok(()) => return Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot create {}: {e}", p.display())),
        }
    }
    Err("Could not allocate a unique backup directory".into())
}
pub fn workspace(app: &mut App, paths: &Paths) -> Result<PathBuf> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&paths.backup_root)
        .map_err(|e| format!("Cannot create backup root: {e}"))?;
    let root = fs::canonicalize(&paths.backup_root).map_err(|e| e.to_string())?;
    // Reject a destination inside an archived directory before any tar process starts.
    let mut sources: Vec<_> = HOME_ITEMS.iter().map(|p| paths.home.join(p)).collect();
    sources.push(paths.project.clone());
    for s in sources {
        if let Ok(s) = fs::canonicalize(&s) {
            if root.starts_with(&s) {
                return Err(format!(
                    "Backup root must be outside archived path {}",
                    s.display()
                ));
            }
        }
    }
    let t = app.get("date", &["+%Y-%m-%d-%H%M%S"]);
    let stamp = if t.ok() && !t.out.trim().is_empty() {
        t.out.trim().to_owned()
    } else {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs()
            .to_string()
    };
    let dir = unique_dir(&root, &format!("system-update-{stamp}"))?;
    app.report.open(&dir.join("pre-upgrade-report.txt"))?;
    app.important(format!(
        "Backup workspace: {}\nHealth report: {}",
        dir.display(),
        dir.join("pre-upgrade-report.txt").display()
    ));
    Ok(dir)
}
fn atomic_text(path: &Path, s: &str) -> Result<()> {
    let part = path.with_extension("partial");
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&part)
        .map_err(|e| e.to_string())?;
    f.write_all(s.as_bytes())
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&part, path).map_err(|e| e.to_string())
}
fn package_list(app: &mut App, flag: &str, path: &Path) -> Result<()> {
    let r = app.get("pacman", &[flag]);
    // pacman returns 1 when these filtered queries match no installed packages.
    if r.ok() || r.code == 1 && r.out.is_empty() && r.err.is_empty() {
        atomic_text(path, &r.out)?;
        app.line(format!(
            "[OK] {} saved.",
            path.file_name().unwrap().to_string_lossy()
        ));
        Ok(())
    } else {
        Err(format!("Cannot save package list ({})", r.detail()))
    }
}
fn archive(app: &mut App, base: &Path, items: &[OsString], target: &Path) -> Result<()> {
    let part = target.with_extension("gz.partial");
    let mut a = args(&["-C"]);
    a.push(base.as_os_str().into());
    a.push("-czf".into());
    a.push(part.as_os_str().into());
    a.push("--".into());
    a.extend_from_slice(items);
    let r = app.runner.run("tar", &a, None, false);
    if !r.out.is_empty() {
        app.line(r.out.trim_end());
    }
    if !r.err.is_empty() {
        app.line(r.err.trim_end());
    }
    if !r.ok() {
        let _ = fs::remove_file(&part);
        return Err(format!("Archive failed ({})", r.detail()));
    }
    let r = app.runner.run(
        "tar",
        &["-tzf".into(), part.as_os_str().into()],
        None,
        false,
    );
    if !r.ok() {
        let _ = fs::remove_file(&part);
        return Err(format!("Archive verification failed ({})", r.detail()));
    }
    OpenOptions::new()
        .read(true)
        .open(&part)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&part, target).map_err(|e| e.to_string())?;
    app.line(format!(
        "[OK] {} created and verified.",
        target.file_name().unwrap().to_string_lossy()
    ));
    Ok(())
}
pub fn run(app: &mut App, paths: &Paths, dir: &Path) -> Result<()> {
    app.heading("PRE-UPGRADE BACKUP");
    let mut failures = Vec::new();
    for (flag, file) in [
        ("-Qqen", "official-packages.txt"),
        ("-Qqem", "aur-packages.txt"),
    ] {
        if let Err(e) = package_list(app, flag, &dir.join(file)) {
            app.line(format!("[ERROR] {e}"));
            failures.push(e);
        }
    }
    let items: Vec<_> = HOME_ITEMS
        .iter()
        .filter(|p| fs::symlink_metadata(paths.home.join(p)).is_ok())
        .map(OsString::from)
        .collect();
    if items.is_empty() {
        app.warn("No expected home configuration paths were found.");
    } else if let Err(e) = archive(app, &paths.home, &items, &dir.join("home-config.tar.gz")) {
        app.line(format!("[ERROR] {e}"));
        failures.push(e);
    }
    if paths.project.is_dir() {
        // Canonicalize the repository itself so a symlinked checkout archives its contents.
        let p = fs::canonicalize(&paths.project).map_err(|e| e.to_string())?;
        let parent = p
            .parent()
            .ok_or("Repository cannot be the filesystem root")?;
        let name = p.file_name().ok_or("Invalid repository path")?;
        if let Err(e) = archive(
            app,
            parent,
            &[name.into()],
            &dir.join("midnight-bump.tar.gz"),
        ) {
            app.line(format!("[ERROR] {e}"));
            failures.push(e);
        }
    } else {
        app.warn(format!("{} was not found.", paths.project.display()));
    }
    if paths.project.join(".git").exists() && app.have("git") {
        for (a, file) in [
            (
                vec!["status", "--short", "--branch"],
                "midnight-bump-git-status.txt",
            ),
            (vec!["rev-parse", "HEAD"], "midnight-bump-commit.txt"),
        ] {
            let mut ga = args(&["--no-optional-locks", "-C"]);
            ga.push(paths.project.as_os_str().into());
            ga.extend(args(&a));
            let r = app.runner.run("git", &ga, None, false);
            if r.ok() {
                atomic_text(&dir.join(file), &r.out)?;
            } else {
                app.warn(format!("Could not record {file} ({})", r.detail()));
            }
        }
    } else {
        app.warn("Midnight Bump Git metadata unavailable.");
    }
    app.heading("BACKUP CONTENTS");
    let listing = fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut entries: Vec<_> = listing.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    let mut bytes = 0;
    for e in entries {
        let len = e.metadata().map_err(|e| e.to_string())?.len();
        bytes += len;
        app.line(format!(
            "{:>12} bytes  {}",
            len,
            e.file_name().to_string_lossy()
        ));
    }
    app.line(format!(
        "Total file size (report still open): {bytes} bytes"
    ));
    app.report.sync()?;
    if failures.is_empty() {
        atomic_text(&dir.join("backup-complete.txt"), "Required package-list/archive operations completed. Review pre-upgrade-report.txt for warnings and any skipped paths. This is a configuration/repository backup, not a full system image.\n")?;
        Ok(())
    } else {
        Err(format!(
            "{} backup operation(s) failed. Maintenance cancelled.",
            failures.len()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_private_directories() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir();
        let a = unique_dir(&base, "pre-upgrade-check-test").unwrap();
        let b = unique_dir(&base, "pre-upgrade-check-test").unwrap();
        assert_ne!(a, b);
        assert_eq!(
            fs::metadata(&a).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::remove_dir(a).unwrap();
        fs::remove_dir(b).unwrap();
    }
}

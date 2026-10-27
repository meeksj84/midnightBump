use crate::{
    audit,
    runtime::{App, Result},
};

pub fn run(app: &mut App) -> Result<bool> {
    app.report.sync()?;
    if !app.review_complete {
        return Err("Online review is incomplete. Package changes blocked; resolve the review errors and retry.".into());
    }
    if app.warnings > 0 {
        app.important("Audit/backup completed with warnings. Review the report before continuing.");
        if !app
            .runner
            .confirm("Continue with system maintenance anyway?")
        {
            app.important(
                "Maintenance cancelled. Non-interactive sessions cannot accept warnings.",
            );
            return Ok(false);
        }
    }
    if !crate::review::confirm(app)? {
        return Ok(false);
    }
    app.heading("SYSTEM UPDATE");
    app.live("sudo", &["pacman", "-Syu"])?;
    if app.have("paccache") {
        app.heading("PACMAN CACHE CLEANUP");
        app.live("sudo", &["paccache", "-r"])?;
    } else {
        app.info("paccache not found (pacman-contrib); skipping cache cleanup.");
    }
    let root = app.get("findmnt", &["-n", "-o", "FSTYPE", "--target", "/"]);
    if !root.ok() {
        return Err(format!(
            "Cannot determine root filesystem; scrub skipped ({})",
            root.detail()
        ));
    }
    if root.out.trim() == "btrfs" {
        if !app.have("btrfs") {
            app.warn("Root is Btrfs but btrfs-progs is unavailable; scrub skipped.");
        } else {
            app.heading("BTRFS SCRUB");
            // pacman has already authenticated sudo. -n avoids an invisible password prompt.
            let s = app.get("sudo", &["-n", "btrfs", "scrub", "status", "/"]);
            if !s.ok() {
                return Err(format!(
                    "Cannot read scrub status; no scrub started ({})",
                    s.detail()
                ));
            }
            app.line(s.out.trim_end());
            if audit::scrub_running(&s.out) {
                app.info("A Btrfs scrub is already running.");
            } else {
                app.live("sudo", &["btrfs", "scrub", "start", "/"])?;
                app.info("Btrfs scrub started in the background; completion is not yet verified. Check: sudo btrfs scrub status /");
            }
        }
    } else {
        app.info("Root is not Btrfs; scrub skipped.");
    }
    app.heading("POST-MAINTENANCE CHECK");
    audit::failed_services(app);
    app.important("Maintenance steps complete.");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Output, Runner};
    use std::{cell::RefCell, ffi::OsString, path::Path};
    struct Fake {
        calls: RefCell<Vec<String>>,
        fail_update: bool,
        confirm: bool,
        filesystem: &'static str,
    }
    impl Runner for Fake {
        fn run(&self, p: &str, a: &[OsString], _: Option<&Path>, _: bool) -> Output {
            let s = format!(
                "{p} {}",
                a.iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let fail = self.fail_update && s == "sudo pacman -Syu";
            self.calls.borrow_mut().push(s);
            Output {
                code: if fail { 1 } else { 0 },
                out: if p == "findmnt" {
                    self.filesystem.into()
                } else {
                    String::new()
                },
                err: String::new(),
            }
        }
        fn have(&self, _: &str) -> bool {
            true
        }
        fn confirm(&self, _: &str) -> bool {
            self.confirm
        }
    }
    #[test]
    fn failed_update_stops_cleanup_and_scrub() {
        let f = Fake {
            calls: RefCell::new(vec![]),
            fail_update: true,
            confirm: true,
            filesystem: "btrfs",
        };
        let mut app = App::new(&f);
        app.review_complete = true;
        assert!(run(&mut app).is_err());
        assert_eq!(*f.calls.borrow(), vec!["sudo pacman -Syu"]);
    }
    #[test]
    fn unaccepted_warnings_run_no_commands() {
        let f = Fake {
            calls: RefCell::new(vec![]),
            fail_update: false,
            confirm: false,
            filesystem: "btrfs",
        };
        let mut app = App::new(&f);
        app.review_complete = true;
        app.warnings = 1;
        assert!(!run(&mut app).unwrap());
        assert!(f.calls.borrow().is_empty());
    }
    #[test]
    fn ext4_never_runs_btrfs() {
        let f = Fake {
            calls: RefCell::new(vec![]),
            fail_update: false,
            confirm: true,
            filesystem: "ext4",
        };
        let mut app = App::new(&f);
        app.review_complete = true;
        assert!(run(&mut app).unwrap());
        assert!(f.calls.borrow().iter().all(|s| !s.contains("btrfs")));
    }
}

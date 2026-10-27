use crate::{
    feed, http, issues, packages,
    runtime::{App, Result},
};
use std::collections::{BTreeMap, BTreeSet};

pub const NEWS_URL: &str = "https://archlinux.org/feeds/news/";
pub const SECURITY_URL: &str = "https://security.archlinux.org/advisory/feed.atom";

#[derive(Debug, PartialEq)]
struct Update {
    name: String,
    old: String,
    new: String,
}
fn updates(text: &str) -> Result<Vec<Update>> {
    let mut result = vec![];
    let mut seen = BTreeSet::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() != 4
            || f[2] != "->"
            || !f[0]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
            || line.chars().any(|c| c.is_control() && c != '\t')
            || !packages::version_ok(f[1])
            || !packages::version_ok(f[3])
            || !seen.insert(f[0])
        {
            return Err(
                "Unrecognized checkupdates output; pending package versions are unknown".into(),
            );
        }
        result.push(Update {
            name: f[0].into(),
            old: f[1].into(),
            new: f[3].into(),
        });
    }
    if result.is_empty() {
        return Err("checkupdates returned success without package versions".into());
    }
    Ok(result)
}
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn links(app: &mut App, name: &str, aur: bool) {
    let p = encode(name);
    if aur {
        app.line(format!("  AUR comments/build instructions (manual): https://aur.archlinux.org/packages?K={p}&SeB=n"));
    } else {
        app.line(format!("  Package page -> Bug Reports / Security Issues: https://archlinux.org/packages/?name={p}"));
    }
}
fn fetch(app: &mut App, url: &str, atom: bool) -> Result<Vec<feed::Notice>> {
    let response = http::get(app, url, feed::MAX_BYTES)?;
    feed::parse(&response.body, atom)
}
fn incomplete(app: &mut App, message: impl Into<String>) {
    let message = message.into();
    app.review_summary.errors.push(message.clone());
    app.warn(message);
}
pub fn run(app: &mut App, official: &[String], aur: &[String]) -> bool {
    app.review_complete = false;
    app.review_summary = Default::default();
    app.heading("13. ARCH NEWS AND PACKAGE REVIEW");
    app.line("Online checks are read-only. Notice text is displayed, never executed.");
    let when = app.get("date", &["-u", "+%Y-%m-%d %H:%M:%S UTC"]);
    if when.ok() {
        app.line(format!("Review fetched at: {}", when.out.trim()));
    }
    let mut complete = true;
    let installed = match packages::installed(app) {
        Ok(list) => {
            app.review_summary.installed = Some(list.len());
            app.review_summary.foreign = Some(list.values().filter(|p| p.foreign).count());
            Some(list)
        }
        Err(e) => {
            incomplete(app, format!("Installed-package inventory incomplete: {e}"));
            complete = false;
            None
        }
    };
    let pending = if !app.have("checkupdates") {
        Err("checkupdates (pacman-contrib) is missing; pending versions are unknown".into())
    } else {
        // Refresh the separate database; never sync the real pacman database here.
        let r = app.get("checkupdates", &["--nocolor"]);
        if r.code == 2 && r.out.trim().is_empty() && r.err.trim().is_empty() {
            Ok(vec![])
        } else if r.ok() && r.err.trim().is_empty() {
            updates(&r.out)
        } else {
            Err(format!(
                "Pending-package lookup failed: {}",
                feed::safe_text(&r.detail())
            ))
        }
    };
    let mut names: BTreeSet<String> = official.iter().chain(aur).cloned().collect();
    if let Some(installed) = &installed {
        for (name, pkg) in installed {
            names.insert(name.clone());
            names.insert(pkg.base.clone());
        }
    }
    let mut candidates = BTreeMap::new();
    match pending {
        Ok(list) => {
            app.review_summary.pending = Some(list.len());
            if list.is_empty() {
                app.pass("No pending repository updates detected in this review snapshot.");
            }
            app.info(format!(
                "{} pending repository updates; versions below are a review snapshot.",
                list.len()
            ));
            for u in list {
                app.line(format!("{} {} -> {}", u.name, u.old, u.new));
                links(app, &u.name, false);
                if let Some(installed) = &installed {
                    if installed.get(&u.name).map(|p| &p.version) != Some(&u.old) {
                        incomplete(app, format!("Installed version disagrees with checkupdates for {}; inventory may have changed. Rerun review.",u.name));
                        app.review_summary.pending = None;
                        complete = false;
                    } else if let Some(package) = installed.get(&u.name) {
                        app.review_summary.updates.insert(
                            u.name.clone(),
                            crate::summary::PendingUpdate {
                                installed: u.old.clone(),
                                proposed: u.new.clone(),
                                base: package.base.clone(),
                            },
                        );
                    }
                }
                candidates.insert(u.name.clone(), u.new);
                names.insert(u.name);
            }
        }
        Err(e) => {
            incomplete(app, e);
            complete = false;
        }
    }
    if app.review_summary.pending.is_none() {
        app.review_summary.updates.clear();
    }
    if let Some(installed) = &installed {
        if let Err(e) = issues::security(app, installed, &candidates) {
            incomplete(
                app,
                format!("Security cross-check INCOMPLETE: {e}. Coverage UNKNOWN."),
            );
            complete = false;
        }
        if let Err(e) = issues::bugs(app, installed, &candidates) {
            incomplete(
                app,
                format!("Package bug cross-check INCOMPLETE: {e}. Coverage UNKNOWN."),
            );
            complete = false;
        }
    }
    for (packages, is_aur) in [(official, false), (aur, true)] {
        for name in packages {
            app.line(format!("Installer request: {name} (candidate version/dependencies remain subject to the package-manager transaction)"));
            links(app, name, is_aur);
        }
    }
    for (label, url, atom) in [
        ("ARCH NEWS / MANUAL INTERVENTIONS", NEWS_URL, false),
        ("PUBLISHED SECURITY ADVISORIES", SECURITY_URL, true),
    ] {
        app.heading(label);
        match fetch(app, url, atom) {
            Err(e) => {
                incomplete(app, format!("{label}: {e}. Coverage UNKNOWN."));
                complete = false;
            }
            Ok(items) => {
                if atom {
                    app.review_summary.advisories = Some(items.len());
                } else {
                    app.review_summary.news = Some(items.len());
                }
                app.info(format!("Retrieved {} articles from this feed. Publication dates are shown; feed history is limited.", items.len()));
                for item in items {
                    let headline =
                        format!("\n{}\nPublished: {}\n{}", item.title, item.date, item.link);
                    if atom {
                        app.line(headline);
                    } else {
                        app.important(headline);
                    }
                    let matches: Vec<_> = names
                        .iter()
                        .filter(|name| {
                            let text = format!("{} {}", item.title, item.body).to_ascii_lowercase();
                            text.split(|c: char| {
                                !(c.is_ascii_alphanumeric() || "@._+-".contains(c))
                            })
                            .any(|word| word == name.as_str())
                        })
                        .cloned()
                        .collect();
                    if !matches.is_empty() {
                        app.line(format!("Installed/source/requested package-name match: {} (news/feed text match; assess applicability).", matches.join(", ")));
                    }
                    // News is never filtered by package name: general interventions
                    // and split-package notices can matter without an exact match.
                    if !atom || !matches.is_empty() || app.verbose {
                        app.line(&item.body);
                    } else {
                        app.report.save(&item.body);
                    }
                }
            }
        }
    }
    app.heading("REVIEW COVERAGE AND NEXT STEP");
    app.line("Read full notices and assess manual interventions before approving installation.");
    app.line("Older news: https://archlinux.org/news/");
    app.line("All/scheduled advisories and current issue status: https://security.archlinux.org/advisory and https://security.archlinux.org/");
    app.line("[CROSS-CHECK] Review scope: all installed package names/versions, Arch security records and public open packaging reports. Results and any failed checks are listed above. Split packages use recorded source-package names. Review matched reports before approving changes.");
    app.line("[LIMIT] General bug reports lack reliable affected-version ranges: a match is a review finding, not proof your version is broken. Closed/private/upstream-only reports, AUR comments, foreign builds and new uninstalled dependencies need separate assessment. No match does NOT mean safe.");
    app.line("Pacman refreshes its database at installation: compare its final version/dependency list with this snapshot and cancel if further review is needed.");
    if complete {
        app.info("Online inputs retrieved. Human review is still REQUIRED before package changes.");
    } else {
        app.warn("Online review INCOMPLETE. Package changes are blocked; resolve the errors and rerun --review. No bypass is provided.");
    }
    app.review_complete = complete;
    complete
}

pub fn confirm(app: &mut App) -> Result<bool> {
    app.report.sync()?;
    if !app.review_complete {
        return Err("Online review is incomplete. No package changes started; resolve the errors and rerun --review, then retry this mode.".into());
    }
    let accepted = app.runner.confirm("Have you reviewed Arch notices, relevant package reports, and required interventions, and want to proceed with package changes?");
    app.important(if accepted { "[REVIEW ACKNOWLEDGED] User approved proceeding after the online/manual review." } else { "[CANCELLED] Package review was not acknowledged. No package changes started; interactive confirmation is required." });
    app.report.sync()?;
    Ok(accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_versions_and_url_encoding() {
        assert_eq!(
            updates("linux 7.2-1 -> 7.2-2\n").unwrap(),
            vec![Update {
                name: "linux".into(),
                old: "7.2-1".into(),
                new: "7.2-2".into()
            }]
        );
        for s in [
            "",
            "error: no databases",
            "linux x => y",
            "linux x -> y\nlinux x -> y",
            "linux\u{1b} x -> y",
        ] {
            assert!(updates(s).is_err());
        }
        assert_eq!(encode("libc++"), "libc%2B%2B");
    }
}

//! Structured review totals; never infer success by parsing printed diagnostics.
use crate::runtime::App;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct SecurityPackage {
    pub below_fix: usize,
    pub no_fix: usize,
    pub foreign: usize,
    pub pending_meets_fix: usize,
    pub pending_below_fix: usize,
}
#[derive(Default)]
pub struct SecuritySummary {
    pub records: usize,
    pub matches: usize,
    pub findings: usize,
    pub fixed: usize,
    pub not_affected: usize,
    pub packages: BTreeMap<String, SecurityPackage>,
}
#[derive(Default)]
pub struct BugPackage {
    pub installed: Vec<String>,
    pub security: usize,
    pub bugs: usize,
    pub requests: usize,
    pub other: usize,
}
impl BugPackage {
    pub fn count(&self) -> usize {
        self.security + self.bugs + self.requests + self.other
    }
}
#[derive(Default)]
pub struct BugSummary {
    pub reports: usize,
    pub matched: usize,
    pub packages: BTreeMap<String, BugPackage>,
}
#[derive(Default)]
pub struct PendingUpdate {
    pub installed: String,
    pub proposed: String,
    pub base: String,
}
#[derive(Default)]
pub struct ReviewSummary {
    pub installed: Option<usize>,
    pub foreign: Option<usize>,
    pub pending: Option<usize>,
    pub updates: BTreeMap<String, PendingUpdate>,
    pub security: Option<SecuritySummary>,
    pub bugs: Option<BugSummary>,
    pub news: Option<usize>,
    pub advisories: Option<usize>,
    pub errors: Vec<String>,
}

fn count(value: Option<usize>) -> String {
    value.map_or_else(|| "UNKNOWN".into(), |n| n.to_string())
}
fn row(lines: &mut Vec<String>, label: &str, value: impl std::fmt::Display) {
    lines.push(format!("{label:<35} {value}"));
}

impl ReviewSummary {
    pub fn complete(&self) -> bool {
        self.installed.is_some()
            && self.pending.is_some()
            && self.security.is_some()
            && self.bugs.is_some()
            && self.news.is_some()
            && self.advisories.is_some()
            && self.errors.is_empty()
    }

    pub fn render(&self, complete: bool, expanded: bool) -> String {
        let mut lines = vec![];
        row(
            &mut lines,
            "Installed packages checked:",
            count(self.installed),
        );
        row(
            &mut lines,
            "Foreign packages (manual review):",
            count(self.foreign),
        );
        row(
            &mut lines,
            "Available repository updates:",
            count(self.pending),
        );
        row(
            &mut lines,
            "Security findings needing review:",
            count(self.security.as_ref().map(|s| s.findings)),
        );
        row(
            &mut lines,
            "Matching open package reports:",
            count(self.bugs.as_ref().map(|s| s.matched)),
        );
        row(
            &mut lines,
            "Required online checks:",
            if complete && self.complete() {
                "COMPLETE"
            } else {
                "INCOMPLETE"
            },
        );
        if let Some(s) = &self.security {
            lines.push(format!("Security coverage: {} records; {} package/record matches; {} meeting recorded fixes; {} marked Not affected.", s.records, s.matches, s.fixed, s.not_affected));
        }
        if let Some(b) = &self.bugs {
            lines.push(format!(
                "Open-report coverage: {} reports retrieved across all pages.",
                b.reports
            ));
        }
        lines.push(format!(
            "News / advisory articles retrieved: {} / {}",
            count(self.news),
            count(self.advisories)
        ));
        lines.push(String::new());
        if complete && self.complete() {
            lines.push("Decision: REVIEW REQUIRED — read relevant findings and Arch notices before approving package changes.".into());
        } else {
            lines.push("Decision: BLOCKED — required online checks are incomplete; package changes cannot proceed.".into());
            lines.push("Unavailable totals are UNKNOWN, not zero. Partial results do not establish complete coverage.".into());
            for error in &self.errors {
                lines.push(format!("  - {error}"));
            }
        }
        lines.push(
            "No packages were changed by this review. Counts describe the pre-upgrade snapshot."
                .into(),
        );
        lines.push("\nPENDING UPDATES WITH MATCHING FINDINGS".into());
        let highlight_complete = self.installed.is_some()
            && self.pending.is_some()
            && self.security.is_some()
            && self.bugs.is_some();
        if !highlight_complete {
            lines.push("Coverage UNKNOWN: required inventory, update or issue data are incomplete. Only available matches can be shown.".into());
        }
        let mut highlighted = 0;
        for (name, update) in &self.updates {
            let security = self.security.as_ref().and_then(|s| s.packages.get(name));
            let bugs = self
                .bugs
                .as_ref()
                .and_then(|b| b.packages.get(&update.base));
            if security.is_none() && bugs.is_none() {
                continue;
            }
            highlighted += 1;
            lines.push(format!(
                "  - {name}: {} -> {}",
                update.installed, update.proposed
            ));
            if let Some(s) = security {
                let mut reasons = vec![];
                if s.pending_below_fix > 0 {
                    reasons.push(format!(
                        "proposed version still below {} recorded fix(es)",
                        s.pending_below_fix
                    ));
                }
                if s.pending_meets_fix > 0 {
                    reasons.push(format!(
                        "proposed version meets {} recorded fix(es) for installed-version findings",
                        s.pending_meets_fix
                    ));
                }
                if s.no_fix > 0 {
                    reasons.push(format!(
                        "{} record(s) lack a fixed version; applicability unresolved",
                        s.no_fix
                    ));
                }
                if s.foreign > 0 {
                    reasons.push(format!(
                        "{} foreign-build match(es); manual applicability review",
                        s.foreign
                    ));
                }
                lines.push(format!("    Security: {}", reasons.join("; ")));
            } else if self.security.is_none() {
                lines.push("    Security: UNKNOWN (cross-check incomplete).".into());
            }
            if let Some(b) = bugs {
                lines.push(format!(
                    "    Open reports: {} for source {}; version applicability unconfirmed.",
                    b.count(),
                    update.base
                ));
            } else if self.bugs.is_none() {
                lines.push("    Open reports: UNKNOWN (cross-check incomplete).".into());
            }
        }
        if highlighted == 0 && highlight_complete {
            lines.push("  No pending updates matched unresolved security findings or open reports in this snapshot.".into());
        }
        lines.push("Includes open requests as well as bugs. A listed update is not automatically unsafe; it may provide a security fix.".into());
        if let Some(s) = &self.security {
            if !s.packages.is_empty() {
                lines.push("\nPackages with security findings:".into());
                for (name, p) in &s.packages {
                    let mut reasons = vec![];
                    if p.below_fix > 0 {
                        reasons.push(format!(
                            "{} below recorded fix (installed or pending)",
                            p.below_fix
                        ));
                    }
                    if p.no_fix > 0 {
                        reasons.push(format!("{} with no fixed version recorded", p.no_fix));
                    }
                    if p.foreign > 0 {
                        reasons.push(format!("{} foreign-build matches", p.foreign));
                    }
                    lines.push(format!("  - {name}: {}", reasons.join("; ")));
                }
                lines.push("Counts are package/record matches, not unique CVEs. Missing fixes and foreign builds leave applicability unresolved.".into());
            }
        }
        if let Some(b) = &self.bugs {
            if !b.packages.is_empty() {
                lines.push("\nPackages with open reports (grouped by source package):".into());
                let mut groups: Vec<_> = b.packages.iter().collect();
                // Put explicitly labelled security and bug reports ahead of requests.
                groups.sort_by_key(|(name, p)| {
                    (
                        if p.security > 0 {
                            0
                        } else if p.bugs > 0 {
                            1
                        } else if p.other > 0 {
                            2
                        } else {
                            3
                        },
                        *name,
                    )
                });
                let limit = if expanded { groups.len() } else { 12 };
                for (base, p) in groups.iter().take(limit) {
                    let mut kinds = vec![];
                    if p.security > 0 {
                        kinds.push(format!("{} security", p.security));
                    }
                    if p.bugs > 0 {
                        kinds.push(format!("{} bugs/regressions", p.bugs));
                    }
                    if p.requests > 0 {
                        kinds.push(format!(
                            "{} feature/enhancement/update requests",
                            p.requests
                        ));
                    }
                    if p.other > 0 {
                        kinds.push(format!("{} other/unclassified", p.other));
                    }
                    lines.push(format!(
                        "  - {base}: {} report(s) — {}",
                        p.count(),
                        kinds.join(", ")
                    ));
                    if p.installed != vec![base.to_string()] {
                        lines.push(format!("    Installed: {}", p.installed.join(", ")));
                    }
                }
                if groups.len() > limit {
                    lines.push(format!("  ... {} more source-package groups; full list in the saved report or with --verbose.", groups.len() - limit));
                }
                lines.push("Report categories use Arch's labels. Package matches do not prove your installed or pending version is affected.".into());
            }
        }
        lines.push("\nReview manual-intervention notices and earlier health warnings too. No match is not an all-clear.".into());
        lines.join("\n")
    }
}

pub fn show(app: &mut App) {
    app.heading("PRE-UPGRADE SUMMARY");
    let terminal = app.review_summary.render(app.review_complete, app.verbose);
    let full = app.review_summary.render(app.review_complete, true);
    app.report.display_always(&terminal, &full);
    if let Some(path) = app.report.path() {
        let text = format!("Full report: {}", path.display());
        app.important(text);
    } else {
        app.important("Full findings and source links are above. This mode does not automatically save a report.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_review() -> ReviewSummary {
        ReviewSummary {
            installed: Some(939),
            foreign: Some(3),
            pending: Some(42),
            updates: BTreeMap::new(),
            security: Some(SecuritySummary::default()),
            bugs: Some(BugSummary::default()),
            news: Some(10),
            advisories: Some(15),
            errors: vec![],
        }
    }

    #[test]
    fn unavailable_or_partial_inputs_never_render_an_all_clear() {
        let mut summary = complete_review();
        assert!(summary
            .render(true, false)
            .contains("Decision: REVIEW REQUIRED"));
        summary.security = None;
        let text = summary.render(true, false);
        assert!(text.contains("Security findings needing review:   UNKNOWN"));
        assert!(text.contains("Decision: BLOCKED"));
        assert!(!text.contains("Decision: REVIEW REQUIRED"));
        let text = complete_review().render(false, false);
        assert!(text.contains("Decision: BLOCKED"));
    }

    #[test]
    fn compact_summary_prioritizes_findings_and_full_summary_retains_every_group() {
        let mut summary = complete_review();
        let bugs = summary.bugs.as_mut().unwrap();
        for n in 0..14 {
            let name = format!("package-{n:02}");
            bugs.packages.insert(
                name.clone(),
                BugPackage {
                    installed: vec![name],
                    requests: 1,
                    ..Default::default()
                },
            );
        }
        bugs.packages.insert(
            "z-security".into(),
            BugPackage {
                installed: vec!["z-split".into()],
                security: 1,
                ..Default::default()
            },
        );
        bugs.reports = 15;
        bugs.matched = 15;
        let compact = summary.render(true, false);
        let full = summary.render(true, true);
        assert!(compact.find("z-security:").unwrap() < compact.find("package-00:").unwrap());
        assert!(compact.contains("Installed: z-split"));
        assert!(compact.contains("3 more source-package groups"));
        assert!(!compact.contains("package-13:"));
        assert!(full.contains("package-13:"));
        assert!(!full.contains("more source-package groups"));
        assert!(full.contains("feature/enhancement/update requests"));
    }
}

use crate::{
    feed, http,
    json::{self, Value},
    packages::{self, Installed},
    runtime::{App, Result},
    summary::{BugSummary, SecuritySummary},
};
use std::collections::{BTreeMap, BTreeSet};

pub const SECURITY_URL: &str = "https://security.archlinux.org/issues/all.json";
pub const BUGS_URL: &str =
    "https://gitlab.archlinux.org/api/v4/groups/archlinux%2Fpackaging%2Fpackages/issues";
const PROJECT_PREFIX: &str = "https://gitlab.archlinux.org/archlinux/packaging/packages/";
const MAX_PAGES: usize = 50;
const PER_PAGE: usize = 100;
type Candidates = BTreeMap<String, String>;

fn strings(v: &Value) -> Result<Vec<String>> {
    v.array()?
        .iter()
        .map(|x| x.string().map(String::from))
        .collect()
}
pub struct SecurityRecord {
    id: String,
    packages: Vec<String>,
    status: String,
    severity: String,
    kind: String,
    fixed: Option<String>,
    affected: Option<String>,
    cves: Vec<String>,
}
pub fn parse_security(body: &str) -> Result<Vec<SecurityRecord>> {
    let value = json::parse(body)?;
    let mut ids = BTreeSet::new();
    let mut result = vec![];
    for v in value.array()? {
        let id = v.field("name")?.string()?.to_string();
        if !id
            .strip_prefix("AVG-")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            || !ids.insert(id.clone())
        {
            return Err("Invalid/duplicate security group ID".into());
        }
        let packages = strings(v.field("packages")?)?;
        if packages.is_empty() || packages.iter().any(|s| !packages::name_ok(s)) {
            return Err("Invalid security package list".into());
        }
        let status = v.field("status")?.string()?.to_string();
        if !["Unknown", "Not affected", "Vulnerable", "Fixed", "Testing"].contains(&status.as_str())
        {
            return Err("Unknown security tracker status".into());
        }
        let fixed = v
            .field("fixed")?
            .optional_string()?
            .filter(|s| !s.is_empty())
            .map(String::from);
        if fixed.as_ref().is_some_and(|s| !packages::version_ok(s)) {
            return Err("Invalid reported fixed version".into());
        }
        result.push(SecurityRecord {
            id,
            packages,
            status,
            fixed,
            severity: feed::safe_text(v.field("severity")?.string()?),
            kind: feed::safe_text(v.field("type")?.string()?),
            affected: v.field("affected")?.optional_string()?.map(feed::safe_text),
            cves: strings(v.field("issues")?)?
                .iter()
                .map(|s| feed::safe_text(s))
                .collect(),
        });
    }
    if result.is_empty() {
        return Err("Security tracker returned no records; coverage unknown".into());
    }
    Ok(result)
}
fn compare(
    app: &App,
    a: &str,
    b: &str,
    cache: &mut BTreeMap<(String, String), i32>,
) -> Result<i32> {
    let key = (a.into(), b.into());
    if let Some(value) = cache.get(&key) {
        return Ok(*value);
    }
    let r = app.get("vercmp", &[a, b]);
    if !r.ok() || !r.err.trim().is_empty() {
        return Err(format!("Version comparison failed: {}", r.detail()));
    }
    let value = r
        .out
        .trim()
        .parse::<i32>()
        .map_err(|_| "Invalid vercmp output")?
        .signum();
    cache.insert(key, value);
    Ok(value)
}
pub fn security(app: &mut App, installed: &Installed, pending: &Candidates) -> Result<()> {
    app.review_summary.security = None;
    app.heading("INSTALLED PACKAGES VS ARCH SECURITY TRACKER");
    let response = http::get(app, SECURITY_URL, 8 * 1024 * 1024)?;
    let records = parse_security(&response.body)?;
    assess_security(app, &records, installed, pending)
}
pub fn assess_security(
    app: &mut App,
    records: &[SecurityRecord],
    installed: &Installed,
    pending: &Candidates,
) -> Result<()> {
    // Publish totals only after every required version comparison succeeds.
    app.review_summary.security = None;
    let mut summary = SecuritySummary::default();
    let mut cache = BTreeMap::new();
    let (mut matches, mut findings, mut fixed, mut excluded) = (0, 0, 0, 0);
    for record in records {
        for name in &record.packages {
            let Some(pkg) = installed.get(name) else {
                continue;
            };
            matches += 1;
            if record.status == "Not affected" {
                excluded += 1;
                continue;
            }
            let candidate = pending.get(name);
            let mut candidate_below_fix = None;
            let (current_result, candidate_result, review) = if pkg.foreign {
                (
                    "FOREIGN BUILD: official version thresholds may not apply".into(),
                    "Manual applicability review required".into(),
                    true,
                )
            } else if let Some(fix) = &record.fixed {
                let current_below = compare(app, &pkg.version, fix, &mut cache)? < 0;
                let candidate_below = candidate
                    .map(|v| compare(app, v, fix, &mut cache))
                    .transpose()?
                    .map(|n| n < 0);
                candidate_below_fix = candidate_below;
                let current_result = if current_below {
                    "BELOW reported fixed version"
                } else {
                    "At or above reported fixed version"
                }
                .to_string();
                let candidate_result = match candidate_below {
                    Some(true) => "STILL BELOW reported fixed version",
                    Some(false) => "Meets reported fixed version",
                    None => "No pending update listed",
                }
                .to_string();
                (
                    current_result,
                    candidate_result,
                    current_below || candidate_below == Some(true),
                )
            } else {
                (
                    "NO FIX RECORDED: applicability unresolved".into(),
                    "No fixed-version threshold is available".into(),
                    true,
                )
            };
            if review {
                findings += 1;
                let package = summary.packages.entry(name.clone()).or_default();
                if pkg.foreign {
                    package.foreign += 1;
                } else if record.fixed.is_none() {
                    package.no_fix += 1;
                } else {
                    package.below_fix += 1;
                }
                match candidate_below_fix {
                    Some(true) => package.pending_below_fix += 1,
                    Some(false) => package.pending_meets_fix += 1,
                    None => (),
                }
            } else {
                fixed += 1;
            }
            if review || app.verbose {
                let heading = format!(
                    "{} {}: {} — {} ({}, tracker status: {})",
                    record.id, name, pkg.version, record.kind, record.severity, record.status
                );
                if review {
                    app.warn_detail(heading);
                } else {
                    app.info_detail(heading);
                }
                app.line(format!("  Installed: {} — {current_result}", pkg.version));
                app.line(format!(
                    "  Pending: {} — {candidate_result}",
                    candidate.map_or("none", String::as_str)
                ));
                app.line(format!(
                    "  Recorded affected version: {}; fixed: {}",
                    record.affected.as_deref().unwrap_or("unknown"),
                    record.fixed.as_deref().unwrap_or("not recorded")
                ));
                app.line(format!(
                    "  CVEs: {}\n  https://security.archlinux.org/{}",
                    record.cves.join(", "),
                    record.id
                ));
            }
        }
    }
    app.info(format!("Compared {} security records against {} installed packages: {matches} package/record matches, {findings} requiring review, {fixed} meeting recorded fixes, {excluded} marked 'Not affected' by Arch.",records.len(),installed.len()));
    app.line("Tracker status 'Fixed' describes Arch's package, not necessarily your installed version. 'Affected' is a recorded version, not a reliable introduction boundary. Unknown/no-fix records are not cleared by assuming a newer version is safe.");
    app.line("No matching record is not an all-clear. Security coverage is limited to the current Arch tracker data; foreign builds require manual assessment.");
    summary.records = records.len();
    summary.matches = matches;
    summary.findings = findings;
    summary.fixed = fixed;
    summary.not_affected = excluded;
    app.review_summary.security = Some(summary);
    Ok(())
}

pub struct Bug {
    id: u64,
    base: String,
    title: String,
    link: String,
    labels: Vec<String>,
    updated: String,
    body: String,
}
fn decode(s: &str) -> Result<String> {
    let mut out = vec![];
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == b'%' {
            let bytes = s.get(i + 1..i + 3).ok_or("Invalid URL encoding")?;
            out.push(u8::from_str_radix(bytes, 16).map_err(|_| "Invalid URL encoding")?);
            i += 3;
        } else {
            out.push(s.as_bytes()[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "Invalid URL UTF-8".into())
}
pub fn parse_bugs(body: &str) -> Result<Vec<Bug>> {
    let value = json::parse(body)?;
    let mut result = vec![];
    for item in value.array()? {
        if item.field("state")?.string()? != "opened" {
            return Err("Issue endpoint returned an unexpected state".into());
        }
        let link = item.field("web_url")?.string()?.to_string();
        let rest = link
            .strip_prefix(PROJECT_PREFIX)
            .ok_or("Unexpected issue-project URL")?;
        let (base, tail) = rest.split_once("/-/").ok_or("Missing issue-project path")?;
        let base = decode(base)?;
        let number = tail
            .strip_prefix("issues/")
            .or_else(|| tail.strip_prefix("work_items/"))
            .ok_or("Unrecognized issue URL")?;
        if !packages::name_ok(&base)
            || number.parse::<u64>().ok() != Some(item.field("iid")?.integer()?)
        {
            return Err("Invalid issue source-package/number".into());
        }
        result.push(Bug {
            id: item.field("id")?.integer()?,
            base,
            link,
            title: feed::safe_text(item.field("title")?.string()?),
            labels: strings(item.field("labels")?)?
                .iter()
                .map(|s| feed::safe_text(s))
                .collect(),
            updated: feed::safe_text(item.field("updated_at")?.string()?),
            body: feed::safe_text(
                item.field("description")?
                    .optional_string()?
                    .unwrap_or("No description supplied."),
            ),
        });
    }
    Ok(result)
}
pub fn next_page(response: &http::Response, page: usize, count: usize) -> Result<Option<usize>> {
    let h = &response.headers;
    if h.get("x-page").and_then(|s| s.parse::<usize>().ok()) != Some(page)
        || h.get("x-per-page").and_then(|s| s.parse::<usize>().ok()) != Some(PER_PAGE)
        || count > PER_PAGE
    {
        return Err("Missing/inconsistent issue pagination metadata".into());
    }
    let next = h
        .get("x-next-page")
        .ok_or("Missing issue pagination end marker")?;
    if next.is_empty() {
        return Ok(None);
    }
    let n = next.parse::<usize>().map_err(|_| "Invalid next page")?;
    if n != page + 1 || count == 0 || page >= MAX_PAGES {
        return Err("Issue pagination incomplete or exceeded limit; coverage unknown".into());
    }
    Ok(Some(n))
}
pub fn bugs(app: &mut App, installed: &Installed, pending: &Candidates) -> Result<()> {
    app.review_summary.bugs = None;
    app.heading("INSTALLED PACKAGES VS OPEN ARCH PACKAGE REPORTS");
    let mut all = vec![];
    let mut ids = BTreeSet::new();
    let mut page = 1;
    loop {
        if app.report.compact {
            app.important(format!("Package reports: fetching page {page}..."));
        }
        let url=format!("{BUGS_URL}?state=opened&scope=all&non_archived=false&order_by=created_at&sort=asc&per_page={PER_PAGE}&page={page}");
        let response = http::get(app, &url, 2 * 1024 * 1024)?;
        let items = parse_bugs(&response.body)?;
        let next = next_page(&response, page, items.len())?;
        for item in &items {
            if !ids.insert(item.id) {
                return Err(
                    "Repeated issue across pages; retrieval was inconsistent, rerun review".into(),
                );
            }
        }
        all.extend(items);
        match next {
            Some(p) => page = p,
            None => break,
        }
    }
    show_bugs(app, &all, installed, pending);
    Ok(())
}
pub fn show_bugs(app: &mut App, all: &[Bug], installed: &Installed, pending: &Candidates) {
    let mut matched = 0;
    let mut summary = BugSummary::default();
    for bug in all {
        let packages: Vec<_> = installed
            .iter()
            .filter(|(_, p)| p.base == bug.base)
            .collect();
        if packages.is_empty() {
            continue;
        }
        matched += 1;
        let group = summary.packages.entry(bug.base.clone()).or_default();
        group.installed = packages.iter().map(|(name, _)| (*name).clone()).collect();
        let labelled = |label: &str| bug.labels.iter().any(|s| s == label);
        if labelled("scope::security") {
            group.security += 1;
        } else if labelled("scope::bug") || labelled("scope::regression") {
            group.bugs += 1;
        } else if labelled("scope::feature")
            || labelled("scope::enhancement")
            || labelled("package::out-of-date")
        {
            group.requests += 1;
        } else {
            group.other += 1;
        }
        let title = format!("OPEN REPORT [{}]: {}", bug.base, bug.title);
        if bug.labels.iter().any(|s| s == "package::out-of-date") {
            app.info_detail(title);
        } else {
            app.warn_detail(title);
        }
        for (name, pkg) in packages {
            app.line(format!(
                "  Installed: {name} {}; pending: {}{}",
                pkg.version,
                pending.get(name).map_or("none", String::as_str),
                if pkg.foreign {
                    " (foreign build; applicability uncertain)"
                } else {
                    ""
                }
            ));
        }
        app.line(format!(
            "  Labels: {}\n  Updated: {}\n  {}",
            bug.labels.join(", "),
            bug.updated,
            bug.link
        ));
        let body = format!(
            "Reported description (untrusted text; never executed):\n{}",
            bug.body
        );
        if app.verbose {
            app.line(body);
        } else {
            let excerpt: String = body.chars().take(1200).collect();
            if excerpt.len() < body.len() {
                app.report.display(&format!("{excerpt}\n[Description shortened on screen; full text in saved backup/maintenance report or linked issue.]"),&body);
            } else {
                app.line(body);
            }
        }
        app.line("Package/source match only: issue reports have no reliable affected-version range. Review installed and pending versions against the report.");
    }
    app.info(format!("Retrieved {} public open package reports across all pages; {matched} matched installed package/source names.",all.len()));
    app.line("This includes open regressions, bugs and update requests in Arch's packaging group. Closed/private reports, upstream-only reports and AUR comments are outside this scan. New uninstalled dependencies still require transaction review.");
    summary.reports = all.len();
    summary.matched = matched;
    app.review_summary.bugs = Some(summary);
}

#[cfg(test)]
pub const TEST_SECURITY: &str = r#"[{"name":"AVG-1","packages":["linux"],"status":"Fixed","severity":"High","type":"test issue","affected":"1-1","fixed":"2-1","issues":["CVE-2026-0001"]}]"#;
#[cfg(test)]
pub const TEST_BUG: &str = r#"[{"id":1,"iid":7,"state":"opened","title":"Test boot regression","web_url":"https://gitlab.archlinux.org/archlinux/packaging/packages/linux/-/work_items/7","labels":["scope::regression"],"updated_at":"2026-10-21T00:00:00Z","description":"Reported after installing version 3."}]"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        packages::Package,
        runtime::{Output, Runner},
    };
    use std::{cell::RefCell, ffi::OsString, path::Path};
    #[derive(Default)]
    struct Versions {
        calls: RefCell<Vec<(String, String)>>,
    }
    impl Runner for Versions {
        fn run(&self, p: &str, a: &[OsString], _: Option<&Path>, _: bool) -> Output {
            assert_eq!(p, "vercmp");
            let first = a[0].to_string_lossy().into_owned();
            let second = a[1].to_string_lossy().into_owned();
            self.calls
                .borrow_mut()
                .push((first.clone(), second.clone()));
            let value = match (first.as_str(), second.as_str()) {
                ("1-1", "2-1") | ("1:9.0-1", "2:1.0-1") => "-1",
                ("2-1", "2-1") => "0",
                ("3-1", "2-1") => "1",
                pair => panic!("Unexpected comparison: {pair:?}"),
            };
            Output {
                code: 0,
                out: value.into(),
                err: String::new(),
            }
        }
        fn have(&self, _: &str) -> bool {
            true
        }
        fn confirm(&self, _: &str) -> bool {
            false
        }
    }
    fn installed(version: &str) -> Installed {
        BTreeMap::from([(
            "linux".into(),
            Package {
                version: version.into(),
                base: "linux".into(),
                foreign: false,
            },
        )])
    }
    #[test]
    fn installed_and_pending_versions_use_arch_comparison_and_fixed_is_not_host_status() {
        let records = parse_security(TEST_SECURITY).unwrap();
        for (version, candidate, warnings) in [
            ("1-1", None, 1),
            ("2-1", None, 0),
            ("3-1", None, 0),
            ("1-1", Some("2-1"), 1),
            ("2-1", Some("1-1"), 1),
        ] {
            let fake = Versions::default();
            let mut app = App::new(&fake);
            let pending = candidate
                .map(|v| BTreeMap::from([("linux".into(), v.into())]))
                .unwrap_or_default();
            assess_security(&mut app, &records, &installed(version), &pending).unwrap();
            assert_eq!(app.warnings, warnings, "{version} -> {candidate:?}");
            let summary = app.review_summary.security.as_ref().unwrap();
            assert_eq!(summary.findings, warnings as usize);
            assert_eq!(summary.fixed, usize::from(warnings == 0));
            if warnings > 0 {
                assert_eq!(summary.packages["linux"].below_fix, 1);
                assert_eq!(
                    summary.packages["linux"].pending_meets_fix,
                    usize::from(candidate == Some("2-1"))
                );
                assert_eq!(
                    summary.packages["linux"].pending_below_fix,
                    usize::from(candidate == Some("1-1"))
                );
            }
            assert!(!fake.calls.borrow().is_empty());
        }
        let text = TEST_SECURITY.replace("\"fixed\":\"2-1\"", "\"fixed\":\"2:1.0-1\"");
        let fake = Versions::default();
        let mut app = App::new(&fake);
        assess_security(
            &mut app,
            &parse_security(&text).unwrap(),
            &installed("1:9.0-1"),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(app.warnings, 1);
        assert_eq!(
            *fake.calls.borrow(),
            vec![("1:9.0-1".into(), "2:1.0-1".into())]
        );
    }
    #[test]
    fn unknown_unfixed_foreign_and_uninstalled_packages_are_not_false_clearances() {
        let fake = Versions::default();
        let mut app = App::new(&fake);
        let text = TEST_SECURITY
            .replace("\"fixed\":\"2-1\"", "\"fixed\":null")
            .replace("\"Fixed\"", "\"Unknown\"");
        assess_security(
            &mut app,
            &parse_security(&text).unwrap(),
            &installed("3-1"),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(app.warnings, 1);
        assert!(fake.calls.borrow().is_empty());
        assert_eq!(
            app.review_summary.security.as_ref().unwrap().packages["linux"].no_fix,
            1
        );
        let mut foreign = installed("3-1");
        foreign.get_mut("linux").unwrap().foreign = true;
        assess_security(
            &mut app,
            &parse_security(TEST_SECURITY).unwrap(),
            &foreign,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(app.warnings, 2);
        assert!(fake.calls.borrow().is_empty());
        assert_eq!(
            app.review_summary.security.as_ref().unwrap().packages["linux"].foreign,
            1
        );
        assess_security(
            &mut app,
            &parse_security(TEST_SECURITY).unwrap(),
            &Installed::new(),
            &BTreeMap::new(),
        )
        .unwrap();
        let not_affected = TEST_SECURITY.replace("\"Fixed\"", "\"Not affected\"");
        assess_security(
            &mut app,
            &parse_security(&not_affected).unwrap(),
            &installed("1-1"),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(app.warnings, 2);
        assert!(fake.calls.borrow().is_empty());
    }
    #[test]
    fn record_validation_and_pagination_fail_closed() {
        assert!(parse_security("[]").is_err());
        assert!(parse_security(&TEST_SECURITY[..TEST_SECURITY.len() - 1]).is_err());
        assert!(parse_security(&TEST_SECURITY.replace("\"2-1\"", "\"--help\"")).is_err());
        assert!(parse_bugs(&TEST_BUG.replace(PROJECT_PREFIX, "https://example.com/")).is_err());
        let bugs = parse_bugs(TEST_BUG).unwrap();
        assert_eq!(bugs[0].base, "linux");
        let response = |next: &str| {
            http::parse(
                &format!(
                    "HTTP/2 200\r\nx-page: 1\r\nx-per-page: 100\r\nx-next-page: {next}\r\n\r\n[]"
                ),
                100,
            )
            .unwrap()
        };
        assert_eq!(next_page(&response("2"), 1, 100).unwrap(), Some(2));
        assert_eq!(next_page(&response(""), 1, 0).unwrap(), None);
        assert!(next_page(&response("1"), 1, 100).is_err());
        assert!(next_page(&response("2"), 1, 0).is_err());
    }
    #[test]
    fn all_bug_pages_are_read_and_split_packages_match_without_pending_updates() {
        struct Pages(RefCell<Vec<String>>);
        impl Runner for Pages {
            fn run(&self, p: &str, a: &[OsString], _: Option<&Path>, _: bool) -> Output {
                assert_eq!(p, "curl");
                let url = a.last().unwrap().to_string_lossy().into_owned();
                self.0.borrow_mut().push(url.clone());
                let (page, next, body) = if url.ends_with("page=1") {
                    (1, "2", TEST_BUG.to_string())
                } else if url.ends_with("page=2") {
                    (
                        2,
                        "",
                        TEST_BUG
                            .replace("\"id\":1", "\"id\":2")
                            .replace("packages/linux/", "packages/unrelated/"),
                    )
                } else {
                    panic!("Unexpected page")
                };
                Output { code:0,out:format!("HTTP/2 200\r\nx-page: {page}\r\nx-per-page: 100\r\nx-next-page: {next}\r\n\r\n{body}"),err:String::new() }
            }
            fn have(&self, _: &str) -> bool {
                true
            }
            fn confirm(&self, _: &str) -> bool {
                false
            }
        }
        let fake = Pages(RefCell::new(vec![]));
        let mut app = App::new(&fake);
        let installed = BTreeMap::from([(
            "linux-headers".into(),
            Package {
                version: "3-1".into(),
                base: "linux".into(),
                foreign: false,
            },
        )]);
        bugs(&mut app, &installed, &BTreeMap::new()).unwrap();
        assert_eq!(fake.0.borrow().len(), 2);
        assert_eq!(app.warnings, 1);
        let summary = app.review_summary.bugs.as_ref().unwrap();
        assert_eq!(summary.reports, 2);
        assert_eq!(summary.matched, 1);
        assert_eq!(summary.packages["linux"].installed, vec!["linux-headers"]);
        assert_eq!(summary.packages["linux"].bugs, 1);
    }

    #[test]
    fn summary_categories_preserve_requests_and_prioritize_security_labels() {
        let fake = Versions::default();
        let mut app = App::new(&fake);
        let mut reports = vec![];
        for label in [
            "scope::security\",\"scope::enhancement",
            "scope::regression",
            "scope::feature",
            "scope::enhancement",
            "package::out-of-date",
            "unrecognized",
        ] {
            reports.extend(parse_bugs(&TEST_BUG.replace("scope::regression", label)).unwrap());
        }
        show_bugs(&mut app, &reports, &installed("3-1"), &BTreeMap::new());
        let summary = app.review_summary.bugs.as_ref().unwrap();
        assert_eq!(summary.matched, 6);
        let group = &summary.packages["linux"];
        assert_eq!(
            (group.security, group.bugs, group.requests, group.other),
            (1, 1, 3, 1)
        );
    }
}

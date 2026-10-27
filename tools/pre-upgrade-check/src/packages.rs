use crate::runtime::{App, Result};
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Clone)]
pub struct Package {
    pub version: String,
    pub base: String,
    pub foreign: bool,
}
pub type Installed = BTreeMap<String, Package>;
pub fn name_ok(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(['-', '.'])
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
}
pub fn version_ok(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:+_~-".contains(&b))
}
fn list(text: &str) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 2
            || !name_ok(fields[0])
            || !version_ok(fields[1])
            || result.insert(fields[0].into(), fields[1].into()).is_some()
        {
            return Err("Invalid/duplicate installed-package record".into());
        }
    }
    Ok(result)
}
fn base_from_desc(text: &str, name: &str, version: &str) -> Result<Option<String>> {
    let mut fields = BTreeMap::new();
    for block in text.split("\n\n") {
        let mut lines = block.lines();
        let key = lines.next().unwrap_or("");
        if ["%NAME%", "%VERSION%", "%BASE%"].contains(&key) {
            let value = lines.next().ok_or("Missing package metadata value")?;
            if lines.next().is_some() || fields.insert(key, value).is_some() {
                return Err("Ambiguous local package metadata".into());
            }
        }
    }
    if fields.get("%NAME%") != Some(&name) || fields.get("%VERSION%") != Some(&version) {
        return Err(
            "Local package database changed or did not match pacman -Q; rerun review".into(),
        );
    }
    match fields.get("%BASE%") {
        Some(base) if name_ok(base) => Ok(Some((*base).into())),
        Some(_) => Err("Invalid source-package name".into()),
        None => Ok(None),
    }
}
pub fn installed(app: &mut App) -> Result<Installed> {
    for cmd in ["pacman", "pacman-conf", "vercmp"] {
        if !app.have(cmd) {
            return Err(format!(
                "{cmd} is required for installed-package cross-checking"
            ));
        }
    }
    let r = app.get("pacman", &["-Q"]);
    if !r.ok() || !r.err.trim().is_empty() {
        return Err(format!("Cannot query installed packages: {}", r.detail()));
    }
    let versions = list(&r.out)?;
    if versions.is_empty() {
        return Err("Installed package inventory is empty; coverage unknown".into());
    }
    let r = app.get("pacman", &["-Qm"]);
    let foreign = if r.code == 1 && r.out.trim().is_empty() && r.err.trim().is_empty() {
        BTreeMap::new()
    } else if r.ok() && r.err.trim().is_empty() {
        list(&r.out)?
    } else {
        return Err(format!("Cannot identify foreign packages: {}", r.detail()));
    };
    if foreign.iter().any(|(n, v)| versions.get(n) != Some(v)) {
        return Err("Foreign-package inventory disagrees with pacman -Q; rerun review".into());
    }
    let r = app.get("pacman-conf", &["DBPath"]);
    let db = Path::new(r.out.trim());
    if !r.ok() || !r.err.trim().is_empty() || !db.is_absolute() || r.out.trim().contains('\n') {
        return Err("Cannot determine pacman database path".into());
    }
    let mut result = BTreeMap::new();
    let mut missing_base = Vec::new();
    // libalpm's local database uses <name>-<version>/desc and the %BASE% field.
    // https://gitlab.archlinux.org/pacman/pacman/-/blob/master/lib/libalpm/be_local.c
    for (name, version) in versions {
        let path = db
            .join("local")
            .join(format!("{name}-{version}"))
            .join("desc");
        let desc = app.runner.read_text(&path)?;
        let base = match base_from_desc(&desc, &name, &version)? {
            Some(base) => base,
            None => {
                missing_base.push(name.clone());
                name.clone()
            }
        };
        result.insert(
            name.clone(),
            Package {
                version,
                base,
                foreign: foreign.contains_key(&name),
            },
        );
    }
    app.info(format!("Cross-checking ALL {} installed packages, including dependencies; {} are foreign to the configured sync databases.", result.len(), foreign.len()));
    if !missing_base.is_empty() {
        app.warn(format!(
            "{} packages have no recorded source package; exact-name bug matching only: {}",
            missing_base.len(),
            missing_base.join(", ")
        ));
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_package_metadata_and_inventory_validation() {
        let d = "%NAME%\nlinux-headers\n\n%VERSION%\n2:7.2-1\n\n%BASE%\nlinux\n\n";
        assert_eq!(
            base_from_desc(d, "linux-headers", "2:7.2-1").unwrap(),
            Some("linux".into())
        );
        assert!(base_from_desc(d, "linux-headers", "7.2-1").is_err());
        assert!(list("linux 1\nlinux 2\n").is_err());
        assert!(list("../../foo 1\n").is_err());
        assert!(list("linux --help\n").is_err());
        assert!(version_ok("1:2.0~rc1-3"));
    }
}

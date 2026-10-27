use crate::{
    backup::unique_dir,
    cli::Paths,
    runtime::{args, App, Result},
};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const REQUIRED: &[&str] = &[
    "packages/official.txt",
    "packages/aur.txt",
    "hypr/scripts/set-wallpaper",
    "hypr/keybindings.lua",
    "hypr/hyprlock/hyprlock.conf",
    "hypr/hypridle/hypridle.conf",
    "waybar/config.jsonc",
    "waybar/style.css",
    "kitty/kitty.conf",
    "mako/config",
    "rofi/config.rasi",
    "rofi/midnight-bump.rasi",
    "wlogout/layout",
    "wlogout/style.css",
    "matugen/config.toml",
    "matugen/templates/waybar-colors.css",
    "matugen/templates/kitty-colors.conf",
    "matugen/templates/hyprlock-colors.conf",
    "matugen/templates/mako-colors.conf",
    "matugen/templates/rofi-colors.rasi",
    "matugen/templates/wlogout-colors.css",
    "systemd/user/awww-daemon.service",
    "systemd/user/midnight-bump-wallpaper.service",
    "systemd/logind.conf.d/50-midnight-bump-power-button.conf",
];
pub const LINKS: &[(&str, &str)] = &[
    (
        "hypr/scripts/set-wallpaper",
        ".config/hypr/scripts/set-wallpaper",
    ),
    ("hypr/keybindings.lua", ".config/hypr/keybindings.lua"),
    ("hypr/hyprlock/hyprlock.conf", ".config/hypr/hyprlock.conf"),
    ("hypr/hypridle/hypridle.conf", ".config/hypr/hypridle.conf"),
    ("waybar/config.jsonc", ".config/waybar/config.jsonc"),
    ("waybar/style.css", ".config/waybar/style.css"),
    ("kitty/kitty.conf", ".config/kitty/kitty.conf"),
    ("mako/config", ".config/mako/config"),
    ("rofi/config.rasi", ".config/rofi/config.rasi"),
    ("rofi/midnight-bump.rasi", ".config/rofi/midnight-bump.rasi"),
    ("wlogout/layout", ".config/wlogout/layout"),
    ("wlogout/style.css", ".config/wlogout/style.css"),
    (
        "systemd/user/awww-daemon.service",
        ".config/systemd/user/awww-daemon.service",
    ),
    (
        "systemd/user/midnight-bump-wallpaper.service",
        ".config/systemd/user/midnight-bump-wallpaper.service",
    ),
    ("matugen", ".config/matugen"),
];
pub fn validate(project: &Path) -> Result<()> {
    let missing: Vec<_> = REQUIRED
        .iter()
        .filter(|s| !project.join(s).is_file())
        .copied()
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("Incomplete Midnight Bump repository at {}. Missing:\n  {}\nUse --project-dir to select the complete local checkout.", project.display(), missing.join("\n  ")))
    }
}
fn manifest(path: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = Vec::new();
    for l in text.lines() {
        let p = l.split('#').next().unwrap_or("").trim();
        if p.is_empty() {
            continue;
        }
        if p.starts_with('-')
            || !p
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
        {
            return Err(format!(
                "Invalid package name {p:?} in {} (one name per line)",
                path.display()
            ));
        }
        if !out.iter().any(|s| s == p) {
            out.push(p.to_owned());
        }
    }
    Ok(out)
}
fn backup_path(app: &mut App, target: &Path) -> Result<()> {
    match fs::symlink_metadata(target) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
        Ok(_) => (),
    }
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let mut name = target.as_os_str().to_os_string();
    name.push(format!(".backup-{epoch}-{}", std::process::id()));
    let backup = PathBuf::from(name);
    if fs::symlink_metadata(&backup).is_ok() {
        return Err("Backup path collision".into());
    }
    fs::rename(target, &backup).map_err(|e| format!("Cannot back up {}: {e}", target.display()))?;
    app.info(format!(
        "Backed up {} to {}",
        target.display(),
        backup.display()
    ));
    Ok(())
}
fn link(app: &mut App, source: &Path, target: &Path) -> Result<()> {
    let source = fs::canonicalize(source).map_err(|e| format!("{}: {e}", source.display()))?;
    if fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink())
        && fs::canonicalize(target).ok().as_ref() == Some(&source)
    {
        app.info(format!("Already linked: {}", target.display()));
        return Ok(());
    }
    fs::create_dir_all(target.parent().ok_or("Invalid link destination")?)
        .map_err(|e| e.to_string())?;
    backup_path(app, target)?;
    symlink(&source, target).map_err(|e| format!("Cannot link {}: {e}", target.display()))?;
    app.info(format!(
        "Linked {} -> {}",
        target.display(),
        source.display()
    ));
    Ok(())
}
fn migrate_lua(text: &str) -> String {
    let mut result = text
        .lines()
        .filter(|l| {
            ![
                "hl.exec(\"dunst\")",
                "hl.exec(\"awww-daemon\")",
                "hl.exec_cmd(\"/usr/bin/awww-daemon\")",
            ]
            .contains(&l.trim())
        })
        .collect::<Vec<_>>()
        .join("\n");
    result.push('\n');
    if !result.contains("require(\"keybindings\")") {
        result.push_str("\nrequire(\"keybindings\")\n");
    }
    result
}
fn configure_lua(app: &mut App, paths: &Paths) -> Result<()> {
    let p = paths.home.join(".config/hypr/hyprland.lua");
    if !p.is_file() {
        app.warn(format!(
            "Hyprland Lua config not found: {}; keybindings not enabled automatically.",
            p.display()
        ));
        return Ok(());
    }
    let old = fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let new = migrate_lua(&old);
    if old == new {
        app.info("Hyprland keybindings/autostart configuration is already current.");
        return Ok(());
    }
    let mut backup = p.as_os_str().to_os_string();
    backup.push(".midnight-bump.bak");
    let backup = PathBuf::from(backup);
    // Do not follow an existing backup symlink or overwrite an earlier backup.
    if fs::symlink_metadata(&backup).is_err() {
        fs::copy(&p, &backup).map_err(|e| e.to_string())?;
        app.info(format!("Backed up Hyprland config to {}", backup.display()));
    }
    let actual = fs::canonicalize(&p).map_err(|e| e.to_string())?;
    let permissions = fs::metadata(&actual)
        .map_err(|e| e.to_string())?
        .permissions();
    let tmp_dir = unique_dir(
        actual.parent().ok_or("Invalid Hyprland config path")?,
        ".pre-upgrade-check-lua",
    )?;
    let tmp_file = tmp_dir.join("hyprland.lua");
    let result = (|| {
        fs::write(&tmp_file, new).map_err(|e| e.to_string())?;
        fs::set_permissions(&tmp_file, permissions).map_err(|e| e.to_string())?;
        fs::rename(&tmp_file, &actual).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_dir_all(&tmp_dir);
    result?;
    app.info("Enabled keybindings and removed legacy Dunst/awww autostart lines.");
    Ok(())
}
fn first_wallpaper(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let t = e.file_type().ok()?;
        if t.is_dir() {
            if let Some(p) = first_wallpaper(&e.path()) {
                return Some(p);
            }
        }
        if t.is_file()
            && e.path().extension().is_some_and(|s| {
                ["jpg", "jpeg", "png", "webp"]
                    .contains(&s.to_string_lossy().to_ascii_lowercase().as_str())
            })
        {
            return Some(e.path());
        }
    }
    None
}
fn enable(app: &mut App, unit: &str) {
    if let Err(e) = app.live("systemctl", &["--user", "enable", "--now", unit]) {
        app.warn(e);
    } else {
        app.info(format!("Enabled {unit}."));
    }
}
pub fn run(app: &mut App, paths: &Paths) -> Result<bool> {
    validate(&paths.project)?;
    let official = manifest(&paths.project.join("packages/official.txt"))?;
    let aur = manifest(&paths.project.join("packages/aur.txt"))?;
    app.heading("INSTALL MIDNIGHT BUMP");
    app.info(format!("Installing from {}", paths.project.display()));
    if !app.have("pacman") || !app.have("sudo") {
        return Err("The installer requires pacman and sudo on Arch Linux".into());
    }
    let mut review_aur = aur.clone();
    if !app.have("paru") && !review_aur.iter().any(|p| p == "paru") {
        review_aur.push("paru".into());
    }
    crate::review::run(app, &official, &review_aur);
    crate::summary::show(app);
    if !crate::review::confirm(app)? {
        return Ok(false);
    }
    let missing: Vec<_> = official
        .iter()
        .filter(|p| !app.get("pacman", &["-Q", p]).ok())
        .map(OsString::from)
        .collect();
    if !missing.is_empty() {
        let mut a = args(&["pacman", "-Syu", "--needed"]);
        a.extend(missing);
        app.required("sudo", &a, None, true)?;
    } else {
        app.info("Required official packages are already installed (or the manifest is empty).");
    }
    if !app.have("paru") {
        if !app.have("git") || !app.have("makepkg") {
            return Err("Building Paru requires git and makepkg (base-devel).".into());
        }
        let build = unique_dir(&std::env::temp_dir(), "pre-upgrade-check-paru")?;
        let repo = build.join("paru");
        let r = (|| {
            let mut a = args(&[
                "clone",
                "--depth",
                "1",
                "https://aur.archlinux.org/paru.git",
            ]);
            a.push(repo.as_os_str().into());
            app.required("git", &a, None, true)?;
            app.required("makepkg", &args(&["-si", "--needed"]), Some(&repo), true)
        })();
        if let Err(e) = fs::remove_dir_all(&build) {
            app.warn(format!(
                "Temporary Paru build remains at {}: {e}",
                build.display()
            ));
        }
        r?;
        if !app.have("paru") {
            return Err("Paru installation did not complete successfully".into());
        }
    }
    if !aur.is_empty() {
        let mut a = args(&["-S", "--needed"]);
        a.extend(aur.iter().map(OsString::from));
        app.required("paru", &a, None, true)?;
    } else {
        app.info("No Midnight Bump AUR packages configured.");
    }
    if app.have("ufw") {
        app.live("sudo", &["ufw", "default", "deny", "incoming"])?;
        app.live("sudo", &["ufw", "default", "allow", "outgoing"])?;
        app.live("sudo", &["systemctl", "enable", "--now", "ufw.service"])?;
        app.live("sudo", &["ufw", "--force", "enable"])?;
    } else {
        app.warn("UFW unavailable; firewall configuration skipped.");
    }
    for (command, package) in [
        ("hyprctl", "hyprland"),
        ("awww", "awww"),
        ("awww-daemon", "awww"),
        ("matugen", "matugen"),
        ("waybar", "waybar"),
        ("kitty", "kitty"),
        ("hyprlock", "hyprlock"),
        ("hypridle", "hypridle"),
        ("loginctl", "systemd"),
        ("mako", "mako"),
        ("makoctl", "mako"),
        ("rofi", "rofi"),
        ("notify-send", "libnotify"),
        ("wpctl", "wireplumber"),
        ("pavucontrol", "pavucontrol"),
        ("paru", "paru"),
        ("wlogout", "wlogout"),
    ] {
        if !app.have(command) {
            app.warn(format!("Missing command '{command}' (package: {package}); configuration links will still be installed."));
        }
    }
    fs::create_dir_all(paths.home.join(".cache")).map_err(|e| e.to_string())?;
    for (source, target) in LINKS {
        link(app, &paths.project.join(source), &paths.home.join(target))?;
    }
    configure_lua(app, paths)?;
    let script = paths.project.join("hypr/scripts/set-wallpaper");
    let mode = fs::metadata(&script)
        .map_err(|e| e.to_string())?
        .permissions()
        .mode();
    fs::set_permissions(&script, fs::Permissions::from_mode(mode | 0o111))
        .map_err(|e| e.to_string())?;
    if let Some(wallpaper) = first_wallpaper(&paths.project.join("wallpapers")) {
        if app.have("matugen") {
            let a = vec![
                "image".into(),
                wallpaper.into_os_string(),
                "--config".into(),
                paths.project.join("matugen/config.toml").into_os_string(),
                "--source-color-index".into(),
                "0".into(),
            ];
            app.required("matugen", &a, None, true)?;
        } else {
            app.warn("Matugen unavailable; initial palette generation skipped.");
        }
    } else {
        app.warn("No wallpaper found; initial palette generation skipped.");
    }

    // Reload once after writing units, before attempting to enable any user service.
    app.live("systemctl", &["--user", "daemon-reload"])?;
    for (cmd, unit) in [
        ("waybar", "waybar.service"),
        ("hypridle", "hypridle.service"),
    ] {
        if app.have(cmd) {
            if app.get("systemctl", &["--user", "cat", unit]).ok() {
                enable(app, unit);
            } else {
                app.warn(format!(
                    "{unit} not found; install its unit or configure {cmd} autostart."
                ));
            }
        }
    }
    if app.have("mako") {
        let _ = app.get("systemctl", &["--user", "stop", "mako.service"]);
        for process in ["dunst", "swaync", "mako"] {
            let _ = app.get("pkill", &["-x", process]);
        }
        let _ = app.get("systemctl", &["--user", "reset-failed", "mako.service"]);
        enable(app, "mako.service");
    }
    for unit in ["awww-daemon.service", "midnight-bump-wallpaper.service"] {
        let path = paths
            .home
            .join(".config/systemd/user/graphical-session.target.wants")
            .join(unit);
        if let Ok(m) = fs::symlink_metadata(&path) {
            if m.file_type().is_symlink() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            } else {
                app.warn(format!("Preserved non-symlink at {}", path.display()));
            }
        }
    }
    app.live("systemctl", &["--user", "daemon-reload"])?;
    if app.have("awww-daemon") {
        enable(app, "awww-daemon.service");
        for _ in 0..10 {
            if app.get("pgrep", &["-x", "awww-daemon"]).ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if app.get("pgrep", &["-x", "awww-daemon"]).ok() {
            app.info("awww-daemon is running.");
        } else {
            app.warn("awww-daemon process not detected.");
        }
    }
    enable(app, "midnight-bump-wallpaper.service");
    let source = paths
        .project
        .join("systemd/logind.conf.d/50-midnight-bump-power-button.conf");
    if app
        .runner
        .confirm("Install the power-button lock configuration?")
    {
        app.live("sudo", &["mkdir", "-p", "/etc/systemd/logind.conf.d"])?;
        let mut a = args(&["install", "--backup=numbered", "-m", "0644"]);
        a.push(source.into_os_string());
        a.push("/etc/systemd/logind.conf.d/50-midnight-bump-power-button.conf".into());
        app.required("sudo", &a, None, true)?;
        app.info("Power-button configuration installed. Reboot before testing the power button.");
    } else {
        app.info("Power-button configuration skipped (declined or non-interactive).");
    }
    app.line(
        r#"
Midnight Bump files are installed.
Keybindings are loaded through: require("keybindings")
If hypridle.service could not be enabled, add hypridle to session autostart.
Wallpaper keybind: d(mainMod .. " + W", hl.dsp.exec_cmd("~/.config/hypr/scripts/set-wallpaper"))
Reload Hyprland: hyprctl reload
Test wallpaper: ~/.config/hypr/scripts/set-wallpaper
Test lock path: loginctl lock-session
"#,
    );
    app.summary();
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lua_edit_is_idempotent_and_preserves_other_lines() {
        let original = "-- mine\nhl.exec(\"dunst\")\n  hl.exec(\"awww-daemon\")\nhl.exec_cmd(\"/usr/bin/awww-daemon\")\nsetting = 42\n";
        let updated = migrate_lua(original);
        assert!(updated.contains("-- mine\nsetting = 42"));
        assert_eq!(updated.matches("require(\"keybindings\")").count(), 1);
        assert_eq!(migrate_lua(&updated), updated);
        assert!(!updated.contains("hl.exec"));
    }
}

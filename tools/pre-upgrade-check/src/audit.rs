use crate::runtime::App;
use std::{env, fs, path::Path};

pub fn known_noise(s: &str) -> bool {
    s.contains("virt/tdx: TDX not supported by the host platform")
        || s.contains(
            "ACPI BIOS Error (bug): Could not resolve symbol [\\_SB.PCI0.PB2], AE_NOT_FOUND",
        )
        || s.contains("ACPI Error: AE_NOT_FOUND, During name lookup/catalog")
        || (s.contains("lenovo_wmi_gamezone ") && s.contains("platform_profile probe failed"))
        || s.contains("nl80211: kernel reports: multicast RX registrations are not supported")
}
pub fn scrub_running(s: &str) -> bool {
    s.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        let l = l.trim();
        l.strip_prefix("status:")
            .map(|v| v.trim() == "running")
            .unwrap_or(false)
            || l.starts_with("scrub started at ") && l.ends_with(" running")
    })
}
fn tail_matching(s: &str, n: usize, pred: impl Fn(&str) -> bool) -> String {
    let lines: Vec<_> = s.lines().filter(|s| pred(s)).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}
pub fn failed_services(app: &mut App) {
    for (title, args) in [
        (
            "system",
            vec!["--failed", "--no-legend", "--plain", "--no-pager"],
        ),
        (
            "user",
            vec!["--user", "--failed", "--no-legend", "--plain", "--no-pager"],
        ),
    ] {
        if let Some(s) = app.inspect("systemctl", &args) {
            if s.trim().is_empty() {
                app.pass(format!("No failed {title} services."));
            } else {
                app.warn(format!("Failed {title} services detected:"));
                app.line(s.trim_end());
            }
        }
    }
}
fn service(app: &mut App, name: &str, user: bool) {
    let mut a = vec!["is-active", "--quiet", name];
    if user {
        a.insert(0, "--user");
    }
    let r = app.get("systemctl", &a);
    if r.ok() {
        app.pass(format!("{name} is active."));
    } else {
        app.warn(format!(
            "{name} is not active or could not be checked ({}).",
            r.detail()
        ));
    }
}
fn compact_kernel(text: &str) -> (String, usize) {
    let mut count = 0;
    let lines: Vec<_> = text
        .lines()
        .filter(|line| {
            if line.contains("kernel: [UFW BLOCK]") {
                count += 1;
                false
            } else {
                true
            }
        })
        .collect();
    (lines.join("\n"), count)
}
fn kernel_warnings(app: &mut App) {
    if let Some(text) = app.inspect(
        "journalctl",
        &["-k", "-b", "-p", "warning..alert", "--no-pager"],
    ) {
        if app.verbose {
            app.line(text.trim_end());
            return;
        }
        let (compact, count) = compact_kernel(&text);
        app.report.display(&compact, text.trim_end());
        if count > 0 {
            app.info(format!("{count} UFW BLOCK kernel entries summarized. Use --verbose to display every entry; saved backup reports retain them all."));
        }
    }
}
pub fn run(app: &mut App) {
    app.heading("MIDNIGHT BUMP SYSTEM HEALTH CHECK");
    let host = app.get("hostnamectl", &["--static"]);
    let host = if host.ok() && !host.out.trim().is_empty() {
        host.out
    } else {
        fs::read_to_string("/etc/hostname").unwrap_or_else(|_| app.get("uname", &["-n"]).out)
    };
    app.line(format!("Host:       {}", host.trim()));
    for (label, p, a) in [
        ("Date", "date", vec![]),
        ("Kernel", "uname", vec!["-r"]),
        ("Uptime", "uptime", vec!["-p"]),
    ] {
        let r = app.get(p, &a);
        app.line(format!(
            "{label}: {}",
            if r.ok() { r.out.trim() } else { "unavailable" }
        ));
    }
    for (label, var) in [
        ("Desktop", "XDG_CURRENT_DESKTOP"),
        ("Session", "XDG_SESSION_TYPE"),
    ] {
        app.line(format!(
            "{label}: {}",
            env::var(var).unwrap_or_else(|_| "unknown".into())
        ));
    }
    app.heading("1. FAILED SERVICES");
    failed_services(app);

    app.heading("2. CURRENT BOOT ERRORS");
    if let Some(s) = app.inspect("journalctl", &["-b", "-p", "err..alert", "--no-pager"]) {
        let noise = tail_matching(&s, usize::MAX, known_noise);
        let errors = tail_matching(&s, usize::MAX, |l| {
            !known_noise(l)
                && !l.trim().is_empty()
                && l.trim() != "-- No entries --"
                && !l.trim_end().ends_with(" kernel:")
        });
        if errors.is_empty() {
            app.pass("No actionable error-or-higher journal entries detected.");
        } else {
            app.warn("Unrecognized error-or-higher journal entries:");
            app.line(errors);
        }
        if !noise.is_empty() {
            app.info("Known platform/firmware messages (not counted as warnings):");
            app.line(noise);
        }
    }
    app.heading("3. KERNEL WARNINGS");
    kernel_warnings(app);
    app.heading("4. BOOT PERFORMANCE");
    if app.have("systemd-analyze") {
        app.show("systemd-analyze", &[]);
        if let Some(s) = app.inspect("systemd-analyze", &["blame"]) {
            app.line("Slowest services:");
            app.line(s.lines().take(20).collect::<Vec<_>>().join("\n"));
        }
    } else {
        app.info("systemd-analyze not available.");
    }

    app.heading("5. STORAGE");
    app.show("df", &["-hT", "/", "/boot"]);
    if app.have("findmnt") {
        let source = app.get("findmnt", &["-n", "-o", "SOURCE", "--mountpoint", "/boot"]);
        if source.ok() && !source.out.trim().is_empty() {
            app.info(format!("/boot source: {}", source.out.trim()));
            if let Some(opts) =
                app.inspect("findmnt", &["-n", "-o", "OPTIONS", "--mountpoint", "/boot"])
            {
                app.line(format!("Options: {}", opts.trim()));
                if opts.contains("fmask=0077") && opts.contains("dmask=0077") {
                    app.pass("/boot uses restrictive FAT masks (0077/0077).");
                } else if let Some(t) =
                    app.inspect("findmnt", &["-n", "-o", "FSTYPE", "--mountpoint", "/boot"])
                {
                    if t.trim() == "vfat" {
                        app.warn("/boot is FAT without fmask=0077,dmask=0077.");
                    }
                }
            }
        } else if source.code == 1 && source.err.is_empty() {
            app.info("No separate /boot mount detected.");
        } else {
            app.warn(format!(
                "Could not inspect /boot mount ({})",
                source.detail()
            ));
        }
    } else {
        app.warn("findmnt not available; mount checks skipped.");
    }

    app.heading("6. ARCH PACKAGE HEALTH");
    if app.have("pacman") {
        app.line("Pacman database check:");
        if let Some(s) = app.inspect("pacman", &["-Dk"]) {
            app.line(s.trim_end());
            app.pass("Pacman database check completed.");
        }
        let r = app.get("pacman", &["-Qtdq"]);
        if r.code == 0 && !r.out.trim().is_empty() {
            app.info("Orphan packages (never automatically removed):");
            app.line(r.out.trim_end());
        } else if (r.code == 0 || r.code == 1) && r.out.trim().is_empty() && r.err.trim().is_empty()
        {
            app.pass("No orphan packages.");
        } else {
            app.warn(format!("Could not query orphan packages ({})", r.detail()));
        }
    } else {
        app.warn("pacman unavailable; this program targets Arch Linux.");
    }

    app.heading("7. HYPRLAND");
    if app.have("hyprctl")
        && env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|s| !s.is_empty())
    {
        if let Some(s) = app.inspect("hyprctl", &["configerrors"]) {
            if s.trim().is_empty() || s.trim().eq_ignore_ascii_case("ok") {
                app.pass("Hyprland reports no configuration errors.");
            } else {
                app.warn("Hyprland configuration errors:");
                app.line(s.trim_end());
            }
        }
        if let Some(s) = app.inspect("hyprctl", &["version"]) {
            app.line(s.lines().take(12).collect::<Vec<_>>().join("\n"));
        }
    } else {
        app.info("Hyprland IPC unavailable in this shell/session.");
    }
    if app
        .get("systemctl", &["--user", "cat", "hypridle.service"])
        .ok()
    {
        service(app, "hypridle.service", true);
        if let Some(s) = app.inspect(
            "journalctl",
            &["--user", "-b", "-u", "hypridle.service", "--no-pager"],
        ) {
            let recent = tail_matching(&s, 1, |l| {
                l.contains("No rules configured")
                    || l.contains("Config has errors")
                    || rule_count(l).is_some()
            });
            if !recent.is_empty() {
                app.line(&recent);
                if recent.contains("No rules configured")
                    || recent.contains("Config has errors")
                    || rule_count(&recent) == Some(0)
                {
                    app.warn("Hypridle has reported configuration problems this boot.");
                } else {
                    app.pass("Hypridle has active listener rules.");
                }
            }
        }
    }

    app.heading("8. AUDIO / REALTIME");
    service(app, "pipewire.service", true);
    service(app, "wireplumber.service", true);
    if app.get("pacman", &["-Q", "pipewire-pulse"]).ok() {
        if app
            .get(
                "systemctl",
                &["--user", "is-active", "--quiet", "pipewire-pulse.socket"],
            )
            .ok()
            || app
                .get(
                    "systemctl",
                    &["--user", "is-active", "--quiet", "pipewire-pulse.service"],
                )
                .ok()
        {
            app.pass("PipeWire PulseAudio compatibility layer is available.");
        } else {
            app.warn("pipewire-pulse is installed but its user socket/service is not active.");
        }
    } else {
        app.warn("pipewire-pulse is not installed or its package could not be queried.");
    }
    if app.have("wpctl") {
        if let Some(s) = app.inspect("wpctl", &["status"]) {
            let sink = s
                .split("Sinks:")
                .nth(1)
                .and_then(|s| s.split("Sources:").next())
                .and_then(|s| {
                    s.lines()
                        .find_map(|l| l.split_once('*').map(|(_, r)| r.trim()))
                });
            if let Some(sink) = sink {
                app.info(format!("Default audio sink: {sink}"));
            } else {
                app.info("No active default audio sink detected by wpctl.");
            }
        }
    }
    if app.get("pacman", &["-Q", "rtkit"]).ok() {
        app.pass("rtkit package is installed.");
        if app
            .get(
                "systemctl",
                &["is-active", "--quiet", "rtkit-daemon.service"],
            )
            .ok()
        {
            app.pass("rtkit-daemon is active.");
        } else {
            app.info("rtkit-daemon inactive/unavailable; it may start on demand through D-Bus.");
        }
    } else {
        app.warn("rtkit not installed or its package could not be queried.");
    }
    if let Some(s) = app.inspect(
        "journalctl",
        &["--user", "-b", "--since", "-10 minutes", "--no-pager"],
    ) {
        let s = tail_matching(&s, 10, |l| {
            let l = l.to_lowercase();
            l.contains("rtkit error") || l.contains("realtimekit") && l.contains("serviceunknown")
        });
        if s.is_empty() {
            app.pass("No RTKit ServiceUnknown errors in the last 10 minutes of this boot.");
        } else {
            app.warn("Recent RTKit-related errors:");
            app.line(s);
        }
    }

    app.heading("9. GRAPHICS");
    if app.have("lspci") {
        if let Some(s) = app.inspect("lspci", &["-k"]) {
            let mut left = 0;
            for l in s.lines() {
                if ["VGA", "3D", "Display"].iter().any(|p| l.contains(p)) {
                    left = 5;
                }
                if left > 0 {
                    app.line(l);
                    left -= 1;
                }
            }
        }
    }
    let kernel = app.inspect("journalctl", &["-k", "-b", "--no-pager"]);
    if let Some(s) = &kernel {
        let errors = tail_matching(s, usize::MAX, |l| {
            let l = l.to_ascii_lowercase();
            (l.contains("amdgpu") || l.contains("dmcub")) && l.contains("error")
                || l.contains("error getting dmub")
                || l.contains("dpia_query") && l.contains("fail")
        });
        if errors.is_empty() {
            app.pass("No AMDGPU/DMUB/DMCUB errors detected.");
        } else {
            app.warn("AMDGPU errors detected:");
            app.line(errors);
        }
    }
    if app.have("nvidia-smi") {
        if app.get("nvidia-smi", &[]).ok() {
            app.pass("NVIDIA driver responds to nvidia-smi.");
            app.show(
                "nvidia-smi",
                &[
                    "--query-gpu=name,driver_version,temperature.gpu,power.draw",
                    "--format=csv,noheader",
                ],
            );
        } else {
            app.warn("nvidia-smi is installed but could not query the GPU.");
        }
        if let Some(s) = &kernel {
            let errors = tail_matching(s, usize::MAX, |l| {
                let l = l.to_ascii_lowercase();
                l.contains("nvrm: xid") || l.contains("gpu has fallen off the bus")
            });
            if errors.is_empty() {
                app.pass("No NVIDIA Xid/fallen-off-bus errors detected.");
            } else {
                app.warn("Serious NVIDIA errors detected:");
                app.line(errors);
            }
        }
    }

    app.heading("10. LAPTOP / BATTERY");
    let mut battery_found = false;
    let mut standard = false;
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        for e in entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("BAT") && e.path().is_dir())
        {
            battery_found = true;
            app.line(format!("Battery: {}", e.file_name().to_string_lossy()));
            for f in [
                "status",
                "capacity",
                "cycle_count",
                "energy_full",
                "energy_full_design",
                "charge_types",
            ] {
                if let Ok(s) = fs::read_to_string(e.path().join(f)) {
                    app.line(format!("  {f:20} {}", s.trim()));
                    if f == "charge_types" {
                        if s.contains("[Standard]") {
                            standard = true;
                            app.pass("Battery charge mode is Standard.");
                        } else if s.contains("[Long_Life]") {
                            app.info("Battery charge mode is Long_Life.");
                        } else if s.contains("[Fast]") {
                            app.info("Battery charge mode is Fast.");
                        }
                    }
                }
            }
        }
    }
    if !battery_found {
        app.info("No battery detected; treating this as a desktop system.");
    }
    if let Some(s) = &kernel {
        let errors = tail_matching(s, 5, |l| l.contains("unexpected charge_types"));
        if !errors.is_empty() {
            if standard {
                app.info("Lenovo charge-mode warnings occurred earlier this boot; current mode is Standard.");
            } else {
                app.warn("Lenovo charge-mode warnings this boot:");
                app.line(errors);
            }
        }
    }
    app.heading("11. NETWORK / FIRMWARE");
    if Path::new("/usr/lib/firmware/regulatory.db").is_file() {
        app.pass("Wireless regulatory database is present.");
    } else {
        app.warn("/usr/lib/firmware/regulatory.db is missing.");
    }
    if let Some(s) = app.inspect("journalctl", &["-b", "--no-pager"]) {
        let errors = tail_matching(&s, 10, |l| {
            (l.contains("NetworkManager") || l.contains("wpa_supplicant")) && l.contains("error")
        });
        if !errors.is_empty() {
            app.info("Recent network messages containing 'error':");
            app.line(errors);
        }
    }

    app.heading("12. MAINTENANCE AUDIT");
    app.line("Pacman cache:");
    app.show("sudo", &["du", "-sh", "/var/cache/pacman/pkg"]);
    app.line("Journal size:");
    app.show("journalctl", &["--disk-usage"]);
    if let Some(t) = app.inspect("findmnt", &["-n", "-o", "FSTYPE", "--target", "/"]) {
        if t.trim() == "btrfs" {
            if app.have("btrfs") {
                app.line("Btrfs usage:");
                app.show("sudo", &["btrfs", "filesystem", "usage", "/"]);
                app.line("Btrfs device stats:");
                if let Some(s) = app.inspect("sudo", &["btrfs", "device", "stats", "/"]) {
                    app.line(s.trim_end());
                    if s.lines().any(|l| {
                        l.split_whitespace()
                            .last()
                            .and_then(|v| v.parse::<u64>().ok())
                            .is_some_and(|v| v > 0)
                    }) {
                        app.warn(
                            "Btrfs device error counters are nonzero; review before upgrading.",
                        );
                    }
                }
                app.line("Btrfs scrub status:");
                app.show("sudo", &["btrfs", "scrub", "status", "/"]);
            } else {
                app.warn("Root is Btrfs but btrfs-progs is unavailable.");
            }
        } else {
            app.info("Root filesystem is not Btrfs; Btrfs checks skipped.");
        }
    }
    crate::review::run(app, &[], &[]);
    app.summary();
}
fn rule_count(s: &str) -> Option<u32> {
    let rest = s.split("found ").nth(1)?;
    let (n, tail) = rest.split_once(' ')?;
    if tail.starts_with("rules") {
        n.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn firewall_summary_preserves_all_other_diagnostics() {
        let input = "Oct 21 host kernel: mt7921e timeout\nOct 21 host kernel: [UFW BLOCK] packet one\nOct 21 host kernel: [UFW BLOCK] packet two\nOct 21 host kernel: amdgpu error\nOct 21 host app: error mentioning [UFW BLOCK]\n";
        let (compact, count) = compact_kernel(input);
        assert_eq!(count, 2);
        assert!(compact.contains("mt7921e timeout"));
        assert!(compact.contains("amdgpu error"));
        assert!(compact.contains("app: error mentioning [UFW BLOCK]"));
        assert!(!compact.contains("packet one"));
    }
    #[test]
    fn noise_filter_is_narrow() {
        assert!(known_noise(
            "kernel: virt/tdx: TDX not supported by the host platform"
        ));
        assert!(!known_noise("kernel: amdgpu Error getting DMUB"));
        assert!(!known_noise(
            "kernel: ACPI BIOS Error (bug): some other failure"
        ));
    }
    #[test]
    fn scrub_status_does_not_confuse_not_running() {
        assert!(scrub_running("UUID: abc\nStatus: running\n"));
        assert!(!scrub_running("Status: finished\n"));
        assert!(!scrub_running("scrub status: not running\n"));
    }
}

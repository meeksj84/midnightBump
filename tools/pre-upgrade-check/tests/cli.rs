use std::process::Command;

#[test]
fn help_and_version_work_without_home() {
    for a in ["--help", "-h", "--version", "-V"] {
        let r = Command::new(env!("CARGO_BIN_EXE_pre-upgrade-check"))
            .arg(a)
            .env_remove("HOME")
            .output()
            .unwrap();
        assert!(r.status.success());
        assert!(String::from_utf8_lossy(&r.stdout).contains("pre-upgrade-check"));
    }
}
#[test]
fn bad_options_exit_two_before_work() {
    for a in [
        vec!["--unknown"],
        vec!["--maintain", "--audit"],
        vec!["--project-dir"],
        vec!["--review", "--compact"],
        vec!["--audit", "--output"],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_pre-upgrade-check"))
            .args(a)
            .output()
            .unwrap();
        assert_eq!(r.status.code(), Some(2));
        assert!(r.stdout.is_empty());
    }
}
#[test]
fn all_modes_can_be_previewed_as_any_user() {
    for mode in [
        "--audit",
        "--review",
        "--backup",
        "--maintain",
        "--clean",
        "--install",
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_pre-upgrade-check"))
            .args([mode, "--dry-run"])
            .output()
            .unwrap();
        assert!(r.status.success());
        assert!(String::from_utf8_lossy(&r.stdout).contains("DRY RUN"));
    }
    let output = std::env::temp_dir().join(format!(
        "pre-upgrade-dry-run-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for mode in ["--review", "--audit"] {
        let r = Command::new(env!("CARGO_BIN_EXE_pre-upgrade-check"))
            .args([mode, "--compact", "--dry-run", "--output"])
            .arg(&output)
            .output()
            .unwrap();
        assert!(r.status.success());
        assert!(String::from_utf8_lossy(&r.stdout).contains("Save the full report"));
        assert!(!output.exists());
    }
}

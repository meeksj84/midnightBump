//! Exercise the repository compatibility commands without real machine operations.
use std::{
    fs,
    os::unix::fs::{symlink, DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(with_binary: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "pre-upgrade launcher test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for name in [
            "pre-upgrade-check",
            "pre-upgrade-check.sh",
            "system-maintenance.sh",
            "install-updated.sh",
            "build-pre-upgrade-check.sh",
        ] {
            let path = root.join("scripts").join(name);
            write_executable(
                &path,
                &fs::read_to_string(source.join("scripts").join(name)).unwrap(),
            );
        }
        if with_binary {
            write_executable(
                &root.join("tools/pre-upgrade-check/target/release/pre-upgrade-check"),
                "#!/bin/bash\nprintf '%s\\0' \"$@\"\nexit 37\n",
            );
        }
        Self(root)
    }
    fn run(&self, name: &str, args: &[&str]) -> Output {
        Command::new(self.0.join("scripts").join(name))
            .args(args)
            .output()
            .unwrap()
    }
    fn check(&self, name: &str, args: &[&str], expected: &[&str]) {
        let out = self.run(name, args);
        assert_eq!(
            out.status.code(),
            Some(37),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let mut all = vec![
            "--project-dir".to_string(),
            self.0.to_string_lossy().into_owned(),
        ];
        all.extend(expected.iter().map(|s| s.to_string()));
        assert_eq!(out.stdout, nul_args(&all));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn write_executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
fn nul_args(args: &[String]) -> Vec<u8> {
    args.iter()
        .flat_map(|s| s.bytes().chain(std::iter::once(0)))
        .collect()
}

#[test]
fn legacy_modes_forward_arguments_and_exit_status() {
    let f = Fixture::new(true);
    f.check("pre-upgrade-check", &[], &[]);
    f.check("pre-upgrade-check.sh", &[], &[]);
    f.check("system-maintenance.sh", &[], &["--audit"]);
    for mode in ["--maintain", "--clean", "--review"] {
        f.check(
            "system-maintenance.sh",
            &[mode, "--dry-run"],
            &[mode, "--dry-run"],
        );
    }
    f.check(
        "system-maintenance.sh",
        &["--compact", "--output", "report with spaces.txt"],
        &["--audit", "--compact", "--output", "report with spaces.txt"],
    );
    f.check(
        "pre-upgrade-check.sh",
        &["--project-dir", "/another checkout", "--dry-run"],
        &["--project-dir", "/another checkout", "--dry-run"],
    );
    f.check(
        "install-updated.sh",
        &["--dry-run"],
        &["--install", "--dry-run"],
    );
}

#[test]
fn symlinked_entrypoints_resolve_checkout_with_spaces() {
    let f = Fixture::new(true);
    let aliases = f.0.join("user bin");
    fs::create_dir(&aliases).unwrap();
    for name in ["pre-upgrade-check", "system-maintenance.sh"] {
        let link = aliases.join(name);
        symlink(f.0.join("scripts").join(name), &link).unwrap();
        let out = Command::new(link).arg("--review").output().unwrap();
        assert_eq!(out.status.code(), Some(37));
        assert_eq!(
            out.stdout,
            nul_args(&[
                "--project-dir".into(),
                f.0.to_string_lossy().into_owned(),
                "--review".into()
            ])
        );
    }
}

#[test]
fn missing_binary_fails_without_building() {
    let f = Fixture::new(false);
    for name in [
        "pre-upgrade-check",
        "pre-upgrade-check.sh",
        "system-maintenance.sh",
        "install-updated.sh",
    ] {
        let out = f.run(name, &["--dry-run"]);
        assert_eq!(out.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&out.stderr).contains("build-pre-upgrade-check.sh"));
        assert!(!f.0.join("tools/pre-upgrade-check/target").exists());
    }
}

#[test]
fn build_helper_keeps_offline_paths_and_propagates_failure() {
    let f = Fixture::new(false);
    let fake_bin = f.0.join("fake bin");
    write_executable(
        &fake_bin.join("cargo"),
        "#!/bin/bash\nprintf '%s\\0' \"$@\" > \"$TRACE\"\nexit \"$FAKE_CARGO_EXIT\"\n",
    );
    let trace = f.0.join("cargo args");
    let command = || {
        let mut cmd = Command::new(f.0.join("scripts/build-pre-upgrade-check.sh"));
        cmd.env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        cmd.env("TRACE", &trace);
        cmd
    };
    for status in [0, 23] {
        let out = command()
            .env("FAKE_CARGO_EXIT", status.to_string())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(status));
        let crate_dir = f.0.join("tools/pre-upgrade-check");
        assert_eq!(
            fs::read(&trace).unwrap(),
            nul_args(&[
                "build".into(),
                "--manifest-path".into(),
                crate_dir.join("Cargo.toml").to_string_lossy().into_owned(),
                "--release".into(),
                "--locked".into(),
                "--offline".into(),
                "--target-dir".into(),
                crate_dir.join("target").to_string_lossy().into_owned()
            ])
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).contains("Built pre-upgrade-check"),
            status == 0
        );
    }
    fs::remove_file(&trace).unwrap();
    assert!(command().arg("--help").output().unwrap().status.success());
    assert_eq!(
        command().arg("--unknown").output().unwrap().status.code(),
        Some(2)
    );
    assert!(!trace.exists());
}

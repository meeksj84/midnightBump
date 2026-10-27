use std::{
    env,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, IsTerminal, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub type Result<T> = std::result::Result<T, String>;
pub fn args(a: &[&str]) -> Vec<OsString> {
    a.iter().map(OsString::from).collect()
}

#[derive(Clone, Debug)]
pub struct Output {
    pub code: i32,
    pub out: String,
    pub err: String,
}
impl Output {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    pub fn detail(&self) -> String {
        format!(
            "exit {}{}{}",
            self.code,
            if self.err.is_empty() { "" } else { ": " },
            self.err.trim()
        )
    }
}
pub trait Runner {
    fn run(&self, program: &str, args: &[OsString], cwd: Option<&Path>, live: bool) -> Output;
    fn have(&self, program: &str) -> bool;
    fn confirm(&self, question: &str) -> bool;
    fn read_text(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
    }
}
pub struct RealRunner;
impl Runner for RealRunner {
    fn run(&self, program: &str, args: &[OsString], cwd: Option<&Path>, live: bool) -> Output {
        let mut c = Command::new(program);
        c.args(args)
            .env("LC_ALL", "C")
            .env("SYSTEMD_COLORS", "0")
            .env("SYSTEMD_PAGER", "cat");
        if let Some(d) = cwd {
            c.current_dir(d);
        }
        if live {
            match c.status() {
                Ok(s) => Output {
                    code: s.code().unwrap_or(128),
                    out: String::new(),
                    err: String::new(),
                },
                Err(e) => Output {
                    code: 127,
                    out: String::new(),
                    err: e.to_string(),
                },
            }
        } else {
            match c.stdin(Stdio::null()).output() {
                Ok(s) => Output {
                    code: s.status.code().unwrap_or(128),
                    out: String::from_utf8_lossy(&s.stdout).into_owned(),
                    err: String::from_utf8_lossy(&s.stderr).into_owned(),
                },
                Err(e) => Output {
                    code: 127,
                    out: String::new(),
                    err: e.to_string(),
                },
            }
        }
    }
    fn have(&self, program: &str) -> bool {
        use std::os::unix::fs::PermissionsExt;
        env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|p| {
            std::fs::metadata(p.join(program))
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        })
    }
    fn confirm(&self, question: &str) -> bool {
        if !io::stdin().is_terminal() {
            return false;
        }
        print!("{question} [y/N] ");
        if io::stdout().flush().is_err() {
            return false;
        }
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
    }
}

#[derive(Default)]
pub struct Report {
    file: Option<File>,
    path: Option<PathBuf>,
    pub failed: bool,
    pub compact: bool,
    #[cfg(test)]
    pub terminal: Vec<String>,
}
impl Report {
    pub fn open(&mut self, path: &Path) -> Result<()> {
        self.file = Some(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| format!("Cannot create report {}: {e}", path.display()))?,
        );
        self.path = Some(path.to_owned());
        Ok(())
    }
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn line(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        self.display(text, text);
    }
    pub fn display(&mut self, terminal: &str, full_report: &str) {
        self.write(terminal, full_report, !self.compact);
    }
    pub fn important(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        self.display_always(text, text);
    }
    pub fn display_always(&mut self, terminal: &str, full_report: &str) {
        self.write(terminal, full_report, true);
    }
    pub fn save(&mut self, full_report: &str) {
        self.write("", full_report, false);
    }
    fn write(&mut self, terminal: &str, full_report: &str, visible: bool) {
        // Use libtest's captured printing during unit tests. Direct stdout writes
        // bypass capture and make simulated commands look like real operations.
        #[cfg(test)]
        if visible {
            self.terminal.push(terminal.to_owned());
            println!("{terminal}");
        }
        // A closed stdout pipe must not conceal a failure to persist the report.
        #[cfg(not(test))]
        if visible {
            let _ = writeln!(io::stdout(), "{terminal}");
        }
        if let Some(f) = &mut self.file {
            if let Err(e) = writeln!(f, "{full_report}") {
                if !self.failed {
                    eprintln!("Cannot write report: {e}");
                }
                self.failed = true;
            }
        }
    }
    pub fn sync(&mut self) -> Result<()> {
        if let Some(f) = &mut self.file {
            if let Err(e) = f.sync_all() {
                self.failed = true;
                return Err(format!("Cannot save report: {e}"));
            }
        }
        if self.failed {
            Err("Report could not be saved completely".into())
        } else {
            Ok(())
        }
    }
}

pub struct App<'a> {
    pub runner: &'a dyn Runner,
    pub report: Report,
    pub passes: u32,
    pub warnings: u32,
    pub notes: u32,
    pub verbose: bool,
    pub review_complete: bool,
    pub review_summary: crate::summary::ReviewSummary,
}
impl<'a> App<'a> {
    pub fn new(runner: &'a dyn Runner) -> Self {
        Self {
            runner,
            report: Report::default(),
            passes: 0,
            warnings: 0,
            notes: 0,
            verbose: false,
            review_complete: false,
            review_summary: crate::summary::ReviewSummary::default(),
        }
    }
    pub fn line(&mut self, s: impl AsRef<str>) {
        self.report.line(s);
    }
    pub fn important(&mut self, s: impl AsRef<str>) {
        self.report.important(s);
    }
    pub fn heading(&mut self, s: &str) {
        let full = format!("\n============================================================\n{s}\n============================================================");
        let terminal = if self.report.compact {
            format!("\n{s}")
        } else {
            full.clone()
        };
        self.report.display_always(&terminal, &full);
    }
    pub fn pass(&mut self, s: impl AsRef<str>) {
        self.passes += 1;
        self.important(format!("[PASS] {}", s.as_ref()));
    }
    pub fn warn(&mut self, s: impl AsRef<str>) {
        self.warnings += 1;
        self.important(format!("[WARN] {}", s.as_ref()));
    }
    pub fn warn_detail(&mut self, s: impl AsRef<str>) {
        self.warnings += 1;
        self.line(format!("[WARN] {}", s.as_ref()));
    }
    pub fn info(&mut self, s: impl AsRef<str>) {
        self.notes += 1;
        self.important(format!("[INFO] {}", s.as_ref()));
    }
    pub fn info_detail(&mut self, s: impl AsRef<str>) {
        self.notes += 1;
        self.line(format!("[INFO] {}", s.as_ref()));
    }
    pub fn get(&self, p: &str, a: &[&str]) -> Output {
        self.runner.run(p, &args(a), None, false)
    }
    pub fn have(&self, p: &str) -> bool {
        self.runner.have(p)
    }
    pub fn inspect(&mut self, p: &str, a: &[&str]) -> Option<String> {
        let r = self.get(p, a);
        if r.ok() {
            if !r.err.trim().is_empty() {
                self.warn(format!("{p}: {}", r.err.trim()));
            }
            Some(r.out)
        } else {
            self.warn(format!(
                "Could not run {p} {} ({})",
                a.join(" "),
                r.detail()
            ));
            None
        }
    }
    pub fn show(&mut self, p: &str, a: &[&str]) {
        if let Some(s) = self.inspect(p, a) {
            if !s.trim().is_empty() {
                self.line(s.trim_end());
            }
        }
    }
    pub fn required(
        &mut self,
        p: &str,
        a: &[OsString],
        cwd: Option<&Path>,
        live: bool,
    ) -> Result<()> {
        self.report.sync()?;
        self.important(format!(
            "[RUN] {p} {}",
            a.iter()
                .map(|s| format!("{:?}", s))
                .collect::<Vec<_>>()
                .join(" ")
        ));
        // Persist the command before executing a privileged or interactive operation.
        self.report.sync()?;
        let r = self.runner.run(p, a, cwd, live);
        if !r.out.is_empty() {
            self.important(r.out.trim_end());
        }
        if !r.err.is_empty() {
            self.important(r.err.trim_end());
        }
        if r.ok() {
            Ok(())
        } else {
            Err(format!("{p} failed ({})", r.detail()))
        }
    }
    pub fn live(&mut self, p: &str, a: &[&str]) -> Result<()> {
        self.required(p, &args(a), None, true)
    }
    pub fn summary(&mut self) {
        self.heading("SUMMARY");
        self.important(format!(
            "[PASS] {} checks\n[WARN] {} checks\n[INFO] {} notes",
            self.passes, self.warnings, self.notes
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_write_failure_is_detected() {
        for compact in [false, true] {
            let mut report = Report {
                file: Some(OpenOptions::new().write(true).open("/dev/full").unwrap()),
                path: None,
                failed: false,
                compact,
                ..Report::default()
            };
            report.line("Must detect a full filesystem");
            assert!(report.failed);
            assert!(report.sync().is_err());
        }
    }
}

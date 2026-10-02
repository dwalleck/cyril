//! The repository's check command, run once by `/review` after gather and
//! before the workflow starts. Never a crtool subcommand, so the review policy
//! can never allowlist an arbitrary command.

use crate::run::{ReviewRun, read_manifest, write, write_json};
use crate::{Result, ReviewError, io_error};
use std::future::Future;
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::task::JoinHandle;

/// How long a stopped command (or a pipe a grandchild still holds) may take
/// to finish before crtool keeps what it has and moves on.
const GRACE: Duration = Duration::from_secs(2);
const MAX_MATCHED_LINES: usize = 200;
const TAIL_LINES: usize = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckOutcome {
    Clean,
    /// `exit_code` is `None` only if the platform reported no code at all.
    Failed {
        exit_code: Option<i64>,
    },
    TimedOut,
    Cancelled,
}

impl CheckOutcome {
    /// The status text the report and the manifest carry.
    pub fn status(&self) -> String {
        match self {
            Self::Clean => "clean".to_owned(),
            Self::Failed {
                exit_code: Some(code),
            } => format!("FAILED (exit {code})"),
            Self::Failed { exit_code: None } => "FAILED (exit unknown)".to_owned(),
            Self::TimedOut => "TIMED OUT".to_owned(),
            Self::Cancelled => "CANCELLED".to_owned(),
        }
    }
}

#[derive(Debug)]
pub struct CheckResult {
    outcome: CheckOutcome,
    cleanup_error: Option<String>,
    summary: String,
}

impl CheckResult {
    pub fn outcome(&self) -> CheckOutcome {
        self.outcome
    }

    /// Set when the command could not be stopped, or its output could not be
    /// read to the end. The collected output was still written.
    pub fn cleanup_error(&self) -> Option<&str> {
        self.cleanup_error.as_deref()
    }

    /// One line for the operator, like crtool's `diagnostics` line.
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// Run `command` once in the repository root, then write
/// `facts/diagnostics-raw.txt`, `facts/diagnostics.txt` and the manifest's
/// `facts.diagnostics*` keys. `cancel` resolving stops the command.
pub async fn run_check(
    run: &ReviewRun,
    command: &str,
    timeout: Duration,
    cancel: impl Future<Output = ()>,
) -> Result<CheckResult> {
    let mut manifest = read_manifest(run)?;
    let mut process = tokio::process::Command::from(build_command(command)?);
    process
        .current_dir(run.workspace())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let started = Instant::now();
    let mut child = process.spawn().map_err(|source| ReviewError::CannotStart {
        command: command.to_owned(),
        source,
    })?;
    let stdout = Collector::start(child.stdout.take());
    let stderr = Collector::start(child.stderr.take());

    let mut problems = Vec::new();
    tokio::pin!(cancel);
    let stopped = tokio::select! {
        status = child.wait() => {
            let status = status
                .map_err(|source| io_error("wait for check command", run.workspace(), source))?;
            Stopped::Exited(match exit_code(status) {
                Some(0) => CheckOutcome::Clean,
                exit_code => CheckOutcome::Failed { exit_code },
            })
        }
        () = tokio::time::sleep(timeout) => Stopped::Killed(CheckOutcome::TimedOut),
        () = &mut cancel => Stopped::Killed(CheckOutcome::Cancelled),
    };
    let outcome = match stopped {
        Stopped::Exited(outcome) => outcome,
        Stopped::Killed(outcome) => {
            if let Err(error) = child.start_kill() {
                problems.push(format!("could not stop the check command: {error}"));
            } else if tokio::time::timeout(GRACE, child.wait()).await.is_err() {
                problems.push("the check command did not exit after it was killed".to_owned());
            }
            outcome
        }
    };
    let took = started.elapsed();
    let (out, out_complete) = stdout.finish().await;
    let (err, err_complete) = stderr.finish().await;
    if !(out_complete && err_complete) {
        problems.push("output written after the check stopped was not captured".to_owned());
    }
    let cleanup_error = (!problems.is_empty()).then(|| problems.join("; "));
    if let Some(problem) = &cleanup_error {
        tracing::warn!(command, problem, "check command cleanup");
    }

    let mut raw = out;
    raw.push(b'\n');
    raw.extend_from_slice(&err);
    let text = String::from_utf8_lossy(&raw);
    write(&run.path("facts/diagnostics-raw.txt"), text.as_bytes())?;

    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let changed: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    // Checkers print native paths (`crates\x\a.rs:12:5` on Windows); git's use `/`.
    let mine: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| {
            let line = line.replace('\\', "/");
            changed.iter().any(|path| line.contains(path))
        })
        .collect();
    let status = outcome.status();
    let head: String = manifest.head.chars().take(12).collect();
    let mut report = vec![
        format!("command: {command}"),
        format!(
            "result: {status} in {:.0}s, on HEAD {head}",
            took.as_secs_f64()
        ),
    ];
    if let Some(problem) = &cleanup_error {
        report.push(format!("cleanup: {problem}"));
    }
    report.push(format!(
        "{} output line(s) mention a changed file{}",
        mine.len(),
        if mine.is_empty() { "." } else { ":" }
    ));
    report.extend(
        mine.iter()
            .take(MAX_MATCHED_LINES)
            .map(|line| (*line).to_owned()),
    );
    report.push(String::new());
    report.push("last lines of output:".to_owned());
    report.extend(
        lines[lines.len().saturating_sub(TAIL_LINES)..]
            .iter()
            .map(|line| (*line).to_owned()),
    );
    write(
        &run.path("facts/diagnostics.txt"),
        format!("{}\n", report.join("\n")).as_bytes(),
    )?;

    manifest.update_facts(|facts| {
        facts.insert("diagnostics".to_owned(), "facts/diagnostics.txt".into());
        facts.insert("diagnostics_status".to_owned(), status.clone().into());
    });
    write_json(&run.path("manifest.json"), &manifest)?;
    Ok(CheckResult {
        outcome,
        cleanup_error,
        summary: format!(
            "diagnostics: {status} in {:.0}s; {} line(s) on changed files -> facts/diagnostics.txt",
            took.as_secs_f64(),
            mine.len()
        ),
    })
}

/// Why waiting for the command ended.
enum Stopped {
    Exited(CheckOutcome),
    /// Timed out or cancelled: the command must be killed.
    Killed(CheckOutcome),
}

/// POSIX: split like a shell, run without one. Windows: hand the string to
/// `CreateProcess` unchanged, as the platform's own quoting rules expect.
fn build_command(command: &str) -> Result<std::process::Command> {
    let invalid = |message| ReviewError::InvalidCommand {
        command: command.to_owned(),
        message,
    };
    #[cfg(not(windows))]
    let mut process = {
        let words = shlex::split(command).ok_or_else(|| invalid("unbalanced quotes"))?;
        let (program, args) = words
            .split_first()
            .ok_or_else(|| invalid("empty command"))?;
        let mut process = std::process::Command::new(program);
        process.args(args);
        process
    };
    #[cfg(windows)]
    let mut process = {
        use std::os::windows::process::CommandExt;
        let (program, args) = split_windows(command).ok_or_else(|| invalid("empty command"))?;
        let mut process = std::process::Command::new(program);
        if !args.is_empty() {
            process.raw_arg(args);
        }
        process
    };
    // An empty toolchain variable is never a setting, only a broken
    // environment: cargo refuses to start on CARGO_TARGET_DIR="".
    for (key, value) in std::env::vars_os() {
        let toolchain = key
            .to_str()
            .is_some_and(|key| key.starts_with("CARGO_") || key.starts_with("RUST"));
        if toolchain && value.is_empty() {
            process.env_remove(&key);
        }
    }
    Ok(process)
}

/// The program and the untouched rest of a Windows command line, split the
/// way `CreateProcess` finds the program.
#[cfg(any(windows, test))]
fn split_windows(command: &str) -> Option<(&str, &str)> {
    let command = command.trim_start();
    let (program, rest) = match command.strip_prefix('"') {
        Some(quoted) => {
            let end = quoted.find('"').unwrap_or(quoted.len());
            (&quoted[..end], quoted.get(end + 1..).unwrap_or(""))
        }
        None => command.split_at(command.find(char::is_whitespace).unwrap_or(command.len())),
    };
    (!program.is_empty()).then_some((program, rest.trim_start()))
}

#[cfg(unix)]
fn exit_code(status: std::process::ExitStatus) -> Option<i64> {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .map(i64::from)
        .or_else(|| status.signal().map(|signal| -i64::from(signal)))
}

#[cfg(not(unix))]
fn exit_code(status: std::process::ExitStatus) -> Option<i64> {
    status.code().map(i64::from)
}

/// Reads one output stream to the end in its own task, keeping every byte it
/// gets even if the stream never ends.
struct Collector {
    bytes: Arc<Mutex<Vec<u8>>>,
    task: Option<JoinHandle<()>>,
}

impl Collector {
    fn start(stream: Option<impl AsyncRead + Unpin + Send + 'static>) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let task = stream.map(|mut stream| {
            let bytes = Arc::clone(&bytes);
            tokio::spawn(async move {
                let mut chunk = [0u8; 8192];
                loop {
                    match stream.read(&mut chunk).await {
                        Ok(0) => break,
                        Ok(count) => bytes
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .extend_from_slice(&chunk[..count]),
                        Err(error) => {
                            tracing::warn!(%error, "reading check command output failed");
                            break;
                        }
                    }
                }
            })
        });
        Self { bytes, task }
    }

    /// The bytes read, and whether the stream reached its end within `GRACE`.
    async fn finish(self) -> (Vec<u8>, bool) {
        let complete = match self.task {
            None => true,
            Some(mut task) => match tokio::time::timeout(GRACE, &mut task).await {
                Ok(_) => true,
                Err(_) => {
                    task.abort();
                    false
                }
            },
        };
        let bytes = std::mem::take(&mut *self.bytes.lock().unwrap_or_else(PoisonError::into_inner));
        (bytes, complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_program_is_the_first_token_or_the_quoted_prefix() {
        assert_eq!(
            split_windows("cargo check --all"),
            Some(("cargo", "check --all"))
        );
        assert_eq!(
            split_windows(r#""C:\Program Files\x\x.exe" -a "b c""#),
            Some((r"C:\Program Files\x\x.exe", r#"-a "b c""#))
        );
        assert_eq!(split_windows("  just"), Some(("just", "")));
        assert_eq!(split_windows("   "), None);
    }

    #[test]
    fn status_text_matches_crtool() {
        assert_eq!(CheckOutcome::Clean.status(), "clean");
        assert_eq!(
            CheckOutcome::Failed {
                exit_code: Some(101)
            }
            .status(),
            "FAILED (exit 101)"
        );
        assert_eq!(
            CheckOutcome::Failed { exit_code: None }.status(),
            "FAILED (exit unknown)"
        );
        assert_eq!(CheckOutcome::TimedOut.status(), "TIMED OUT");
        assert_eq!(CheckOutcome::Cancelled.status(), "CANCELLED");
    }
}

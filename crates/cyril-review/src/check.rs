//! The repository's check command, run once by `/review` after gather and
//! before the workflow starts. Never a crtool subcommand, so the review policy
//! can never allowlist an arbitrary command.

use crate::run::{ReviewRun, read_manifest, write, write_json};
use crate::{Result, ReviewError, io_error};
use std::future::Future;
use std::path::Path;
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
    let execution = execute_check(run.workspace(), command, timeout, cancel).await?;
    let changed: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let artifacts = render_diagnostics(command, &manifest.head, &changed, &execution);
    if let Some(problem) = &artifacts.result.cleanup_error {
        tracing::warn!(command, problem, "check command cleanup");
    }
    write(
        &run.path("facts/diagnostics-raw.txt"),
        artifacts.raw.as_bytes(),
    )?;
    write(
        &run.path("facts/diagnostics.txt"),
        artifacts.report.as_bytes(),
    )?;
    manifest.update_facts(|facts| {
        facts.insert("diagnostics".to_owned(), "facts/diagnostics.txt".into());
        facts.insert(
            "diagnostics_status".to_owned(),
            artifacts.result.outcome.status().into(),
        );
    });
    write_json(&run.path("manifest.json"), &manifest)?;
    Ok(artifacts.result)
}

struct CapturedStream {
    bytes: Vec<u8>,
    end: CaptureEnd,
}

#[derive(Debug)]
enum CaptureEnd {
    Eof,
    DrainDeadline,
    ReadFailed(std::io::Error),
    TaskFailed(String),
}

impl CaptureEnd {
    fn from_task(result: Result<std::io::Result<()>, tokio::task::JoinError>) -> Self {
        match result {
            Ok(Ok(())) => Self::Eof,
            Ok(Err(error)) => Self::ReadFailed(error),
            Err(error) if error.is_cancelled() => Self::DrainDeadline,
            Err(error) => Self::TaskFailed(error.to_string()),
        }
    }
}

struct CheckExecution {
    outcome: CheckOutcome,
    stdout: CapturedStream,
    stderr: CapturedStream,
    elapsed: Duration,
    cleanup_errors: Vec<String>,
}

async fn execute_check(
    workspace: &Path,
    command: &str,
    timeout: Duration,
    cancel: impl Future<Output = ()>,
) -> Result<CheckExecution> {
    let mut process = tokio::process::Command::from(build_command(command)?);
    process
        .current_dir(workspace)
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
                .map_err(|source| io_error("wait for check command", workspace, source))?;
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
            } else {
                match tokio::time::timeout(GRACE, child.wait()).await {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        problems.push(format!("could not reap the check command: {error}"))
                    }
                    Err(_) => problems
                        .push("the check command did not exit after it was killed".to_owned()),
                }
            }
            outcome
        }
    };
    let elapsed = started.elapsed();
    let deadline = tokio::time::Instant::now() + GRACE;
    let (stdout, stderr) = tokio::join!(stdout.finish(deadline), stderr.finish(deadline));
    Ok(CheckExecution {
        outcome,
        stdout,
        stderr,
        elapsed,
        cleanup_errors: problems,
    })
}

struct DiagnosticArtifacts {
    raw: String,
    report: String,
    result: CheckResult,
}

fn render_diagnostics(
    command: &str,
    head: &str,
    changed: &[&str],
    execution: &CheckExecution,
) -> DiagnosticArtifacts {
    let mut problems = execution.cleanup_errors.clone();
    let mut abandoned = false;
    for (name, stream) in [("stdout", &execution.stdout), ("stderr", &execution.stderr)] {
        match &stream.end {
            CaptureEnd::Eof => {}
            CaptureEnd::DrainDeadline => abandoned = true,
            CaptureEnd::ReadFailed(error) => {
                problems.push(format!("could not read {name}: {error}"))
            }
            CaptureEnd::TaskFailed(error) => {
                problems.push(format!("{name} reader failed: {error}"))
            }
        }
    }
    if abandoned {
        problems.push("output written after the check stopped was not captured".to_owned());
    }
    let cleanup_error = (!problems.is_empty()).then(|| problems.join("; "));
    let mut raw = execution.stdout.bytes.clone();
    raw.push(b'\n');
    raw.extend_from_slice(&execution.stderr.bytes);
    let text = String::from_utf8_lossy(&raw);

    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
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
    let status = execution.outcome.status();
    let head: String = head.chars().take(12).collect();
    let mut report = vec![
        format!("command: {command}"),
        format!(
            "result: {status} in {:.0}s, on HEAD {head}",
            execution.elapsed.as_secs_f64()
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
    let matched = mine.len();
    DiagnosticArtifacts {
        raw: text.into_owned(),
        report: format!("{}\n", report.join("\n")),
        result: CheckResult {
            outcome: execution.outcome,
            cleanup_error,
            summary: format!(
                "diagnostics: {status} in {:.0}s; {} line(s) on changed files -> facts/diagnostics.txt",
                execution.elapsed.as_secs_f64(),
                matched
            ),
        },
    }
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
    task: Option<JoinHandle<std::io::Result<()>>>,
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
                        Ok(0) => return Ok(()),
                        Ok(count) => bytes
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .extend_from_slice(&chunk[..count]),
                        Err(error) => return Err(error),
                    }
                }
            })
        });
        Self { bytes, task }
    }

    /// Stop the reader before taking its buffer, including on the deadline.
    async fn finish(mut self, deadline: tokio::time::Instant) -> CapturedStream {
        let end = match self.task.as_mut() {
            None => CaptureEnd::Eof,
            Some(task) => match tokio::time::timeout_at(deadline, &mut *task).await {
                Ok(result) => CaptureEnd::from_task(result),
                Err(_) => {
                    task.abort();
                    CaptureEnd::from_task(task.await)
                }
            },
        };
        let bytes = std::mem::take(&mut *self.bytes.lock().unwrap_or_else(PoisonError::into_inner));
        CapturedStream { bytes, end }
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        // Also stop readers if the caller drops execution or child.wait fails.
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingReader(bool);

    impl AsyncRead for FailingReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            if self.0 {
                std::task::Poll::Ready(Err(std::io::Error::other("capture failed")))
            } else {
                self.0 = true;
                buffer.put_slice(b"retained");
                std::task::Poll::Ready(Ok(()))
            }
        }
    }

    #[tokio::test]
    async fn read_failure_keeps_bytes_and_reports_incomplete_capture() {
        let captured = Collector::start(Some(FailingReader(false)))
            .finish(tokio::time::Instant::now() + GRACE)
            .await;
        assert_eq!(captured.bytes, b"retained");
        assert!(matches!(captured.end, CaptureEnd::ReadFailed(_)));
    }

    #[tokio::test]
    async fn deadline_keeps_output_from_both_open_streams() -> std::io::Result<()> {
        use tokio::io::AsyncWriteExt;
        let (mut first_writer, first_reader) = tokio::io::duplex(64);
        let (mut second_writer, second_reader) = tokio::io::duplex(64);
        first_writer.write_all(b"first").await?;
        second_writer.write_all(b"second").await?;
        let first = Collector::start(Some(first_reader));
        let second = Collector::start(Some(second_reader));
        let deadline = tokio::time::Instant::now() + Duration::from_millis(25);
        let (first, second) = tokio::join!(first.finish(deadline), second.finish(deadline));
        assert_eq!(first.bytes, b"first");
        assert_eq!(second.bytes, b"second");
        assert!(matches!(first.end, CaptureEnd::DrainDeadline));
        assert!(matches!(second.end, CaptureEnd::DrainDeadline));
        // Both readers have terminated, even though the writers remain alive.
        assert!(first_writer.write_all(b"later").await.is_err());
        assert!(second_writer.write_all(b"later").await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn eof_is_complete_and_task_failure_is_not() {
        let captured = Collector::start(Some(&b"complete"[..]))
            .finish(tokio::time::Instant::now() + GRACE)
            .await;
        assert_eq!(captured.bytes, b"complete");
        assert!(matches!(captured.end, CaptureEnd::Eof));
        let task = tokio::spawn(async { panic!("reader failed") });
        let collector = Collector {
            bytes: Arc::new(Mutex::new(b"before panic".to_vec())),
            task: Some(task),
        };
        let captured = collector.finish(tokio::time::Instant::now() + GRACE).await;
        assert_eq!(captured.bytes, b"before panic");
        assert!(matches!(captured.end, CaptureEnd::TaskFailed(_)));
    }

    #[test]
    fn diagnostics_render_capture_failure_separately_from_clean_exit() {
        let execution = CheckExecution {
            outcome: CheckOutcome::Clean,
            stdout: CapturedStream {
                bytes: b"src\\lib.rs:7 warning\n".to_vec(),
                end: CaptureEnd::ReadFailed(std::io::Error::other("broken pipe")),
            },
            stderr: CapturedStream {
                bytes: b"last line\n".to_vec(),
                end: CaptureEnd::Eof,
            },
            elapsed: Duration::from_secs(3),
            cleanup_errors: Vec::new(),
        };
        let artifacts =
            render_diagnostics("check", "1234567890123456", &["src/lib.rs"], &execution);
        assert_eq!(artifacts.raw, "src\\lib.rs:7 warning\n\nlast line\n");
        assert!(
            artifacts
                .report
                .contains("result: clean in 3s, on HEAD 123456789012")
        );
        assert!(
            artifacts
                .report
                .contains("1 output line(s) mention a changed file:")
        );
        assert!(
            artifacts
                .report
                .contains("cleanup: could not read stdout: broken pipe")
        );
        assert!(artifacts.result.cleanup_error().is_some());
    }

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

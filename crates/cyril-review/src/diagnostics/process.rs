use super::{Cancellation, DiagnosticsOutcome};
use crate::{DiagnosticsError, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(10);
const REAP_DEADLINE: Duration = Duration::from_secs(1);
static CAPTURE_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn capture(
    mut command: Command,
    text: &str,
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<(DiagnosticsOutcome, Vec<u8>)> {
    let (stdout_path, stdout) = create_capture()?;
    let (stderr_path, stderr) = match create_capture() {
        Ok(capture) => capture,
        Err(error) => {
            drop(stdout);
            return finish(Err(error), &[stdout_path]);
        }
    };
    command.stdout(stdout).stderr(stderr);
    let spawned = command.spawn();
    // Command retains its Stdio owners after spawn; release them before polling.
    drop(command);
    let paths = [stdout_path, stderr_path];
    let result = match spawned {
        Ok(mut child) => observe(&mut child, timeout, cancel)
            .and_then(|outcome| snapshot(&paths).map(|bytes| (outcome, bytes))),
        Err(source) => Err(DiagnosticsError::CannotStart {
            command: text.to_owned(),
            source,
        }
        .into()),
    };
    finish(result, &paths)
}

fn create_capture() -> Result<(PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    loop {
        let id = CAPTURE_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("cyril-review-{}-{id}.capture", std::process::id()));
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(lifecycle("create capture", source)),
        }
    }
}

fn finish<T>(result: Result<T>, paths: &[PathBuf]) -> Result<T> {
    let mut failure = None;
    for path in paths {
        if let Err(source) = fs::remove_file(path) {
            match &mut failure {
                None => failure = Some((format!("remove capture {}", path.display()), source)),
                Some((operation, _)) => operation.push_str(&format!(
                    "; also failed removing {}: {source}",
                    path.display()
                )),
            }
        }
    }
    if let Some((mut operation, source)) = failure {
        if let Err(error) = result {
            operation.push_str(&format!(" after {error}"));
        }
        return Err(lifecycle(&operation, source));
    }
    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    Exit(ExitStatus),
    Cancelled,
    TimedOut,
}

fn terminal_decision(
    status: Option<ExitStatus>,
    cancelled: bool,
    elapsed: Duration,
    timeout: Duration,
) -> Option<Terminal> {
    if let Some(status) = status {
        Some(Terminal::Exit(status))
    } else if cancelled {
        Some(Terminal::Cancelled)
    } else if elapsed >= timeout {
        Some(Terminal::TimedOut)
    } else {
        None
    }
}

fn observe(
    child: &mut Child,
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<DiagnosticsOutcome> {
    let started = Instant::now();
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(source) => {
                let error = lifecycle("poll child", source);
                return match terminate(child) {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(lifecycle(
                        &format!("poll child failed: {error}; cleanup also failed"),
                        io::Error::other(cleanup),
                    )),
                };
            }
        };
        match terminal_decision(status, cancel.is_cancelled(), started.elapsed(), timeout) {
            Some(Terminal::Exit(status)) => return classify(status),
            Some(Terminal::Cancelled) => {
                terminate(child)?;
                return Ok(DiagnosticsOutcome::Cancelled);
            }
            Some(Terminal::TimedOut) => {
                terminate(child)?;
                return Ok(DiagnosticsOutcome::TimedOut);
            }
            None => thread::sleep(POLL),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ReapDecision {
    Reaped,
    Waiting,
    Expired,
}

fn reap_decision(reaped: bool, elapsed: Duration) -> ReapDecision {
    if reaped {
        ReapDecision::Reaped
    } else if elapsed >= REAP_DEADLINE {
        ReapDecision::Expired
    } else {
        ReapDecision::Waiting
    }
}

fn terminate(child: &mut Child) -> Result<()> {
    let started = Instant::now();
    let mut kill_error = match child.kill() {
        Ok(()) => None,
        Err(source) => {
            // Natural exit can race kill; an already-reaped child needs no kill.
            if child
                .try_wait()
                .map_err(|error| lifecycle("reap after failed kill", error))?
                .is_some()
            {
                return Ok(());
            }
            Some(source)
        }
    };
    loop {
        let status = child
            .try_wait()
            .map_err(|source| lifecycle("reap child", source))?;
        match reap_decision(status.is_some(), started.elapsed()) {
            ReapDecision::Reaped => {
                return match kill_error.take() {
                    Some(source) => Err(lifecycle("kill child", source)),
                    None => Ok(()),
                };
            }
            ReapDecision::Expired => {
                return Err(match kill_error.take() {
                    Some(source) => {
                        lifecycle("kill failed; child not reaped within one second", source)
                    }
                    None => lifecycle(
                        "reap child",
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "child not reaped within one second",
                        ),
                    ),
                });
            }
            ReapDecision::Waiting => thread::sleep(POLL),
        }
    }
}

fn classify(status: ExitStatus) -> Result<DiagnosticsOutcome> {
    if status.success() {
        return Ok(DiagnosticsOutcome::Clean);
    }
    #[cfg(unix)]
    let code = {
        use std::os::unix::process::ExitStatusExt;
        match (status.code(), status.signal()) {
            (Some(code), _) => i64::from(code),
            (_, Some(signal)) => -i64::from(signal),
            _ => {
                return Err(lifecycle(
                    "classify child exit",
                    io::Error::other("missing exit status"),
                ));
            }
        }
    };
    #[cfg(windows)]
    let code = i64::from(status.code().ok_or_else(|| {
        lifecycle(
            "classify child exit",
            io::Error::other("missing exit status"),
        )
    })? as u32);
    Ok(DiagnosticsOutcome::Failed { exit_code: code })
}

fn snapshot(paths: &[PathBuf; 2]) -> Result<Vec<u8>> {
    let mut stdout = open_reader(&paths[0])?;
    let mut stderr = open_reader(&paths[1])?;
    // Sample BOTH independent readers before consuming either stream.
    let stdout_length = stdout
        .metadata()
        .map_err(|source| lifecycle("sample stdout", source))?
        .len();
    let stderr_length = stderr
        .metadata()
        .map_err(|source| lifecycle("sample stderr", source))?
        .len();
    let mut bytes = Vec::new();
    bounded_read(&mut stdout, stdout_length, &mut bytes)?;
    bytes.push(b'\n');
    bounded_read(&mut stderr, stderr_length, &mut bytes)?;
    Ok(bytes)
}

fn open_reader(path: &Path) -> Result<File> {
    File::open(path).map_err(|source| lifecycle("open capture reader", source))
}

fn bounded_read(reader: &mut File, length: u64, bytes: &mut Vec<u8>) -> Result<()> {
    reader
        .take(length)
        .read_to_end(bytes)
        .map_err(|source| lifecycle("read capture snapshot", source))?;
    Ok(())
}

fn lifecycle(operation: &str, source: io::Error) -> crate::ReviewError {
    DiagnosticsError::Lifecycle {
        operation: operation.to_owned(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[cfg(unix)]
    #[test]
    fn capture_files_do_not_expose_child_output_to_other_users() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let (path, file) = create_capture()?;
        let permissions = file
            .metadata()
            .map(|metadata| metadata.permissions().mode());
        drop(file);
        finish(Ok(()), &[path])?;
        let permissions = permissions.map_err(|source| lifecycle("inspect capture", source))?;
        assert_eq!(
            permissions & 0o077,
            0,
            "temporary diagnostics output must not be readable or writable by other users"
        );
        Ok(())
    }

    #[test]
    fn bounded_read_excludes_bytes_appended_after_length_sample()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut capture = tempfile::NamedTempFile::new()?;
        capture.write_all(b"known prefix\0\xff")?;
        capture.flush()?;
        let sampled = capture.as_file().metadata()?.len();
        capture.write_all(b"late bytes must not be retained")?;
        capture.flush()?;
        let mut reader = File::open(capture.path())?;
        let mut bytes = Vec::new();
        bounded_read(&mut reader, sampled, &mut bytes)?;
        assert_eq!(bytes, b"known prefix\0\xff");
        Ok(())
    }

    #[test]
    fn terminal_observations_prioritize_exit_then_live_cancel_then_timeout() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        let status = ExitStatus::from_raw(0);
        let timeout = Duration::from_secs(2);
        for cancelled in [false, true] {
            for elapsed in [Duration::ZERO, timeout, timeout + POLL] {
                assert_eq!(
                    terminal_decision(Some(status), cancelled, elapsed, timeout),
                    Some(Terminal::Exit(status))
                );
            }
        }
        for elapsed in [Duration::ZERO, timeout, timeout + POLL] {
            assert_eq!(
                terminal_decision(None, true, elapsed, timeout),
                Some(Terminal::Cancelled)
            );
        }
        assert_eq!(
            terminal_decision(None, false, timeout - Duration::from_nanos(1), timeout),
            None
        );
        assert_eq!(
            terminal_decision(None, false, timeout, timeout),
            Some(Terminal::TimedOut)
        );
        assert_eq!(
            terminal_decision(None, false, timeout + POLL, timeout),
            Some(Terminal::TimedOut)
        );
        assert_eq!(
            terminal_decision(None, false, Duration::ZERO, Duration::ZERO),
            Some(Terminal::TimedOut)
        );
    }

    #[test]
    fn reap_deadline_is_exact_and_an_observed_exit_still_wins() {
        // Contract input, independent of the production deadline constant.
        let deadline = Duration::from_secs(1);
        assert_eq!(
            reap_decision(false, deadline - Duration::from_nanos(1)),
            ReapDecision::Waiting
        );
        assert_eq!(reap_decision(false, deadline), ReapDecision::Expired);
        assert_eq!(reap_decision(false, deadline + POLL), ReapDecision::Expired);
        for elapsed in [Duration::ZERO, deadline, deadline + POLL] {
            assert_eq!(reap_decision(true, elapsed), ReapDecision::Reaped);
        }
    }

    #[test]
    fn cleanup_failure_is_typed_and_does_not_skip_the_other_capture()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let blocked = directory.path().join("not-a-file");
        fs::create_dir(&blocked)?;
        let removable = directory.path().join("owned-capture");
        fs::write(&removable, b"capture")?;
        let error = finish(Ok(()), &[blocked.clone(), removable.clone()])
            .expect_err("directory cannot be removed as a capture file");
        match error {
            crate::ReviewError::Diagnostics(DiagnosticsError::Lifecycle { operation, .. }) => {
                assert!(operation.contains(&blocked.display().to_string()));
            }
            other => panic!("cleanup error was not lifecycle: {other:?}"),
        }
        assert!(blocked.is_dir());
        assert!(
            !removable.exists(),
            "cleanup skipped second path after first error"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_status_preserves_the_negative_signal_number() -> Result<()> {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            classify(ExitStatus::from_raw(9))?,
            DiagnosticsOutcome::Failed { exit_code: -9 }
        );
        assert_eq!(
            classify(ExitStatus::from_raw(7 << 8))?,
            DiagnosticsOutcome::Failed { exit_code: 7 }
        );
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_status_preserves_all_unsigned_exit_bits() -> Result<()> {
        use std::os::windows::process::ExitStatusExt;
        assert_eq!(
            classify(ExitStatus::from_raw(0xffff_ffff))?,
            DiagnosticsOutcome::Failed {
                exit_code: 4_294_967_295
            }
        );
        Ok(())
    }
}

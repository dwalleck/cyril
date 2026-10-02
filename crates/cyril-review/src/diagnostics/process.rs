use super::{Cancellation, DiagnosticsOutcome};
use crate::{DiagnosticsError, Result};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, ReadBuf};

const POLL: Duration = Duration::from_millis(10);
const REAP_DEADLINE: Duration = Duration::from_secs(1);
const DRAIN_DEADLINE: Duration = Duration::from_secs(1);
const READ_QUANTUM: usize = 16 * 1024;

#[cfg(not(windows))]
type Command = tokio::process::Command;
#[cfg(windows)]
type Command = subprocess::Exec;
#[cfg(not(windows))]
type Child = tokio::process::Child;
#[cfg(windows)]
type Child = subprocess::Job;

struct OwnedChild {
    inner: Child,
    cleanup_attempted: bool,
}

impl OwnedChild {
    fn poll(&mut self) -> io::Result<Option<i64>> {
        #[cfg(not(windows))]
        {
            use std::os::unix::process::ExitStatusExt;
            self.inner
                .try_wait()?
                .map(|status| {
                    status
                        .code()
                        .map(i64::from)
                        .or_else(|| status.signal().map(|signal| -i64::from(signal)))
                        .ok_or_else(|| io::Error::other("missing child exit status"))
                })
                .transpose()
        }
        #[cfg(windows)]
        {
            self.inner
                .wait_timeout(Duration::ZERO)?
                .map(|status| {
                    status
                        .code()
                        .map(i64::from)
                        .ok_or_else(|| io::Error::other("missing child exit status"))
                })
                .transpose()
        }
    }

    fn kill(&mut self) -> io::Result<()> {
        #[cfg(not(windows))]
        {
            self.inner.start_kill()
        }
        #[cfg(windows)]
        {
            self.inner.kill()
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Jobs are detached at launch: neither errors nor future-drop may wait.
        match self.poll() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "polling dropped diagnostics child failed"),
        }
        if let Err(error) = self.kill() {
            tracing::warn!(%error, "killing dropped diagnostics child failed");
        }
    }
}

/// Terminal evidence of one check run.
#[derive(Debug)]
pub(super) struct Capture {
    pub(super) outcome: DiagnosticsOutcome,
    /// stdout, a newline, then stderr.
    pub(super) bytes: Vec<u8>,
    /// Both streams reached EOF before the final-drain deadline.
    pub(super) complete: bool,
    /// Killing or reaping a timed-out/cancelled child failed. The bytes above
    /// are still the child's real output and must be reported, not dropped.
    pub(super) cleanup_failure: Option<crate::ReviewError>,
}

pub(super) async fn capture(
    command: Command,
    text: &str,
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<Capture> {
    let (mut child, streams) = spawn(command, text)?;
    let result = match streams {
        Ok((mut stdout, mut stderr)) => {
            observe(&mut child, &mut stdout, &mut stderr, timeout, cancel).await
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(captured) => Ok(captured),
        Err(error) if child.cleanup_attempted => Err(error),
        Err(error) => match terminate(&mut child).await {
            Ok(()) => Err(error),
            Err(cleanup) => Err(lifecycle(
                &format!("{error}; direct-child cleanup also failed"),
                io::Error::other(cleanup),
            )),
        },
    }
}

#[cfg(not(windows))]
fn spawn(
    mut command: Command,
    text: &str,
) -> Result<(
    OwnedChild,
    Result<(tokio::process::ChildStdout, tokio::process::ChildStderr)>,
)> {
    use std::process::Stdio;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|source| cannot_start(text, source))?;
    let streams = match (child.stdout.take(), child.stderr.take()) {
        (Some(stdout), Some(stderr)) => Ok((stdout, stderr)),
        _ => Err(lifecycle(
            "take capture pipes",
            io::Error::other("missing pipe"),
        )),
    };
    Ok((
        OwnedChild {
            inner: child,
            cleanup_attempted: false,
        },
        streams,
    ))
}

#[cfg(windows)]
type Reader = interprocess::os::windows::named_pipe::tokio::RecvPipeStream<
    interprocess::os::windows::named_pipe::pipe_mode::Bytes,
>;

#[cfg(windows)]
fn spawn(command: Command, text: &str) -> Result<(OwnedChild, Result<(Reader, Reader)>)> {
    use subprocess::Redirection;
    static LAUNCH: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LAUNCH
        .lock()
        .map_err(|error| lifecycle("lock capture launch", io::Error::other(error.to_string())))?;
    let job = command
        .stdin(Redirection::Null)
        .stdout(Redirection::Pipe)
        .stderr(Redirection::Pipe)
        .detached()
        .start()
        .map_err(|source| cannot_start(text, source))?;
    let mut child = OwnedChild {
        inner: job,
        cleanup_attempted: false,
    };
    let streams = pipe(child.inner.stdout.take())
        .and_then(|stdout| pipe(child.inner.stderr.take()).map(|stderr| (stdout, stderr)));
    Ok((child, streams))
}

#[cfg(windows)]
fn pipe(file: Option<std::fs::File>) -> Result<Reader> {
    let file =
        file.ok_or_else(|| lifecycle("take capture pipe", io::Error::other("missing pipe")))?;
    Reader::try_from(std::os::windows::io::OwnedHandle::from(file))
        .map_err(|error| lifecycle("register capture pipe", io::Error::from(error)))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    Exit(i64),
    Cancelled,
    TimedOut,
}

fn terminal_decision(
    status: Option<i64>,
    cancelled: bool,
    elapsed: Duration,
    timeout: Duration,
) -> Option<Terminal> {
    if let Some(code) = status {
        Some(Terminal::Exit(code))
    } else if cancelled {
        Some(Terminal::Cancelled)
    } else if elapsed >= timeout {
        Some(Terminal::TimedOut)
    } else {
        None
    }
}

async fn observe(
    child: &mut OwnedChild,
    stdout: &mut (impl AsyncRead + Unpin),
    stderr: &mut (impl AsyncRead + Unpin),
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<Capture> {
    let started = Instant::now();
    let mut terminal = None;
    let mut cleanup_failure = None;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let (mut out_eof, mut err_eof) = (false, false);
    let mut buffer = [0; READ_QUANTUM];
    loop {
        if terminal.is_none() {
            let status = child
                .poll()
                .map_err(|source| lifecycle("poll child", source))?;
            if let Some(decision) =
                terminal_decision(status, cancel.is_cancelled(), started.elapsed(), timeout)
            {
                let outcome = match decision {
                    Terminal::Exit(0) => DiagnosticsOutcome::Clean,
                    Terminal::Exit(exit_code) => DiagnosticsOutcome::Failed { exit_code },
                    Terminal::Cancelled | Terminal::TimedOut => {
                        if let Err(error) = terminate(child).await {
                            tracing::warn!(%error, "diagnostics child cleanup failed; keeping captured output");
                            cleanup_failure = Some(error);
                        }
                        if decision == Terminal::Cancelled {
                            DiagnosticsOutcome::Cancelled
                        } else {
                            DiagnosticsOutcome::TimedOut
                        }
                    }
                };
                terminal = Some((outcome, Instant::now()));
            }
        }
        let mut quantum = POLL;
        if let Some((outcome, ended)) = terminal {
            if drain_finished(out_eof, err_eof, ended.elapsed()) {
                out.try_reserve(1 + err.len())
                    .map_err(|error| lifecycle("combine capture", io::Error::other(error)))?;
                out.push(b'\n');
                out.extend_from_slice(&err);
                return Ok(Capture {
                    outcome,
                    bytes: out,
                    complete: out_eof && err_eof,
                    cleanup_failure,
                });
            }
            quantum = quantum.min(DRAIN_DEADLINE.saturating_sub(ended.elapsed()));
        }
        // Poll each stream once per turn; a cold stream never holds up a ready
        // one. Bytes are retained before yielding or cancelling this future.
        let reads = std::future::poll_fn(|cx| -> Poll<Result<()>> {
            let a = read_chunk(stdout, &mut out, &mut buffer, out_eof, cx);
            let b = read_chunk(stderr, &mut err, &mut buffer, err_eof, cx);
            let ready = a.is_ready() || b.is_ready();
            if let Poll::Ready(eof) = a {
                out_eof = eof?;
            }
            if let Poll::Ready(eof) = b {
                err_eof = eof?;
            }
            if ready {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        });
        tokio::select! {
            biased;
            () = tokio::time::sleep(quantum) => {}
            result = reads => result?,
        }
        tokio::task::yield_now().await;
    }
}

fn read_chunk(
    reader: &mut (impl AsyncRead + Unpin),
    bytes: &mut Vec<u8>,
    buffer: &mut [u8],
    eof: bool,
    cx: &mut Context<'_>,
) -> Poll<Result<bool>> {
    if eof {
        return Poll::Pending;
    }
    let mut buffer = ReadBuf::new(buffer);
    match Pin::new(reader).poll_read(cx, &mut buffer) {
        Poll::Ready(Ok(())) => {
            let collected = buffer.filled();
            bytes
                .try_reserve(collected.len())
                .map_err(|error| lifecycle("grow capture", io::Error::other(error)))?;
            bytes.extend_from_slice(collected);
            Poll::Ready(Ok(collected.is_empty()))
        }
        Poll::Ready(Err(source)) => Poll::Ready(Err(lifecycle("read capture", source))),
        Poll::Pending => Poll::Pending,
    }
}

fn drain_finished(stdout_eof: bool, stderr_eof: bool, elapsed: Duration) -> bool {
    (stdout_eof && stderr_eof) || elapsed >= DRAIN_DEADLINE
}

fn reap_deadline_reached(elapsed: Duration) -> bool {
    elapsed >= REAP_DEADLINE
}

async fn terminate(child: &mut OwnedChild) -> Result<()> {
    child.cleanup_attempted = true;
    let started = Instant::now();
    let mut poll_error = match child.poll() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => None,
        Err(source) => Some(source),
    };
    let kill = child.kill();
    let mut live_after_kill = false;
    loop {
        match child.poll() {
            Ok(Some(_)) => {
                if let Some(source) = poll_error {
                    return Err(lifecycle("poll child during cleanup", source));
                }
                if live_after_kill {
                    kill.map_err(|source| lifecycle("kill child", source))?;
                }
                // An immediately observed exit can race a failed kill.
                return Ok(());
            }
            Ok(None) => live_after_kill = true,
            Err(source) => {
                poll_error.get_or_insert(source);
            }
        }
        if reap_deadline_reached(started.elapsed()) {
            return Err(lifecycle(
                "reap child within one second",
                poll_error.or_else(|| kill.err()).unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "direct child not reaped")
                }),
            ));
        }
        tokio::time::sleep(POLL.min(REAP_DEADLINE.saturating_sub(started.elapsed()))).await;
    }
}

fn cannot_start(command: &str, source: io::Error) -> crate::ReviewError {
    DiagnosticsError::CannotStart {
        command: command.to_owned(),
        source,
    }
    .into()
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

    #[test]
    fn drain_finishes_at_both_eof_or_exactly_one_second() {
        let deadline = Duration::from_secs(1);
        for (stdout_eof, stderr_eof) in [(false, false), (true, false), (false, true)] {
            assert!(!drain_finished(stdout_eof, stderr_eof, Duration::ZERO));
            assert!(!drain_finished(
                stdout_eof,
                stderr_eof,
                deadline - Duration::from_nanos(1)
            ));
            assert!(drain_finished(stdout_eof, stderr_eof, deadline));
            assert!(drain_finished(
                stdout_eof,
                stderr_eof,
                deadline + Duration::from_nanos(1)
            ));
        }
        for elapsed in [Duration::ZERO, deadline, deadline + Duration::from_nanos(1)] {
            assert!(drain_finished(true, true, elapsed));
        }
    }

    #[test]
    fn reap_deadline_is_exactly_one_second() {
        let deadline = Duration::from_secs(1);
        assert!(!reap_deadline_reached(Duration::ZERO));
        assert!(!reap_deadline_reached(deadline - Duration::from_nanos(1)));
        assert!(reap_deadline_reached(deadline));
        assert!(reap_deadline_reached(deadline + Duration::from_nanos(1)));
    }

    #[test]
    fn terminal_observations_prioritize_exit_then_live_cancel_then_timeout() {
        let timeout = Duration::from_secs(2);
        for code in [0, 7, -9, 4_294_967_295] {
            for cancelled in [false, true] {
                for elapsed in [Duration::ZERO, timeout, timeout + POLL] {
                    assert_eq!(
                        terminal_decision(Some(code), cancelled, elapsed, timeout),
                        Some(Terminal::Exit(code))
                    );
                }
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
        for elapsed in [timeout, timeout + POLL] {
            assert_eq!(
                terminal_decision(None, false, elapsed, timeout),
                Some(Terminal::TimedOut)
            );
        }
        assert_eq!(
            terminal_decision(None, false, Duration::ZERO, Duration::ZERO),
            Some(Terminal::TimedOut)
        );
    }

    #[tokio::test]
    async fn bounded_reads_retain_binary_bytes_across_quantum_and_eof() -> Result<()> {
        let expected: Vec<u8> = (0..READ_QUANTUM + 17).map(|i| (i % 256) as u8).collect();
        let mut reader = expected.as_slice();
        let mut bytes = Vec::new();
        let mut buffer = [0; READ_QUANTUM];
        assert!(
            !std::future::poll_fn(|cx| {
                read_chunk(&mut reader, &mut bytes, &mut buffer, false, cx)
            })
            .await?
        );
        assert_eq!(bytes, expected[..READ_QUANTUM]);
        assert!(
            !std::future::poll_fn(|cx| {
                read_chunk(&mut reader, &mut bytes, &mut buffer, false, cx)
            })
            .await?
        );
        assert_eq!(bytes, expected);
        assert!(
            std::future::poll_fn(|cx| {
                read_chunk(&mut reader, &mut bytes, &mut buffer, false, cx)
            })
            .await?
        );
        assert_eq!(bytes, expected);
        Ok(())
    }

    #[tokio::test]
    async fn pending_stream_is_not_eof_and_read_failure_is_lifecycle() -> Result<()> {
        struct Broken;
        impl AsyncRead for Broken {
            fn poll_read(
                self: std::pin::Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
                _: &mut tokio::io::ReadBuf<'_>,
            ) -> std::task::Poll<io::Result<()>> {
                std::task::Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "denied",
                )))
            }
        }
        let (mut reader, _writer) = tokio::io::duplex(1);
        let mut bytes = b"prior bytes".to_vec();
        let mut buffer = [0; READ_QUANTUM];
        let pending =
            std::future::poll_fn(|cx| read_chunk(&mut reader, &mut bytes, &mut buffer, false, cx));
        assert!(tokio::time::timeout(Duration::ZERO, pending).await.is_err());
        let result =
            std::future::poll_fn(|cx| read_chunk(&mut Broken, &mut bytes, &mut buffer, false, cx))
                .await;
        match result {
            Err(crate::ReviewError::Diagnostics(DiagnosticsError::Lifecycle {
                source, ..
            })) => {
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected capture lifecycle failure, got {other:?}"),
        }
        assert_eq!(bytes, b"prior bytes");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_signal_exit_remains_negative() -> Result<()> {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "kill -TERM $$"]);
        let Capture {
            outcome,
            bytes,
            complete,
            cleanup_failure,
        } = capture(
            command,
            "/bin/sh -c 'kill -TERM $$'",
            Duration::from_secs(2),
            &Cancellation::default(),
        )
        .await?;
        assert_eq!(outcome, DiagnosticsOutcome::Failed { exit_code: -15 });
        assert_eq!(bytes, b"\n");
        assert!(complete);
        assert!(cleanup_failure.is_none());
        Ok(())
    }
}

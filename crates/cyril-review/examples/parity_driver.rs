//! Deterministic verification consumer for the public review leaf seams.
//!
//! This example is intentionally not installed as a Cyril command.  It gives
//! the differential harness a real Rust caller while keeping the production
//! CLI on `SystemReviewClock`.

use std::cell::Cell;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cyril_review::{
    Cancellation, DiagnosticsError, DiagnosticsOptions, DiagnosticsOutcome, Result, ReviewClock,
    ReviewError, ReviewRun, StepOutput, SystemReviewClock, diagnostics, facts, gather,
};

struct FixedReviewClock {
    gathered_at: String,
    elapsed: Duration,
}

impl ReviewClock for FixedReviewClock {
    fn gathered_at_utc(&self) -> Result<String> {
        Ok(self.gathered_at.clone())
    }

    fn diagnostics_elapsed(&self, _started: Instant) -> Duration {
        self.elapsed
    }
}

struct DriverError {
    code: i32,
    message: String,
    crtool_prefix: bool,
}

impl DriverError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            code: 2,
            message: message.into(),
            crtool_prefix: false,
        }
    }

    fn review(error: ReviewError) -> Self {
        Self {
            code: error.exit_code(),
            message: error.to_string(),
            crtool_prefix: true,
        }
    }
}

struct Arguments {
    operation: String,
    workspace: PathBuf,
    run_dir: PathBuf,
    target: Option<String>,
    scope: Option<String>,
    gathered_at: Option<String>,
}

fn parse_arguments() -> std::result::Result<Arguments, DriverError> {
    let mut values = env::args().skip(1);
    let operation = values
        .next()
        .ok_or_else(|| DriverError::usage("missing operation (gather or facts)"))?;
    if operation != "gather" && operation != "facts" {
        return Err(DriverError::usage(format!(
            "unsupported operation: {operation}"
        )));
    }

    let mut workspace = None;
    let mut run_dir = None;
    let mut target = None;
    let mut scope = None;
    let mut gathered_at = None;
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| DriverError::usage(format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--workspace" => workspace = Some(PathBuf::from(value)),
            "--rundir" => run_dir = Some(PathBuf::from(value)),
            "--target" => target = Some(value),
            "--scope" => scope = Some(value),
            "--gathered-at" => gathered_at = Some(value),
            _ => return Err(DriverError::usage(format!("unknown argument: {flag}"))),
        }
    }

    let workspace = workspace.ok_or_else(|| DriverError::usage("missing --workspace"))?;
    let run_dir = run_dir.ok_or_else(|| DriverError::usage("missing --rundir"))?;
    if operation == "gather" && (target.is_none() || scope.is_none() || gathered_at.is_none()) {
        return Err(DriverError::usage(
            "gather requires --target, --scope, and --gathered-at",
        ));
    }
    if operation == "facts" && (target.is_some() || scope.is_some() || gathered_at.is_some()) {
        return Err(DriverError::usage("facts does not accept gather arguments"));
    }
    Ok(Arguments {
        operation,
        workspace,
        run_dir,
        target,
        scope,
        gathered_at,
    })
}

fn execute(arguments: Arguments) -> std::result::Result<StepOutput, DriverError> {
    let run =
        ReviewRun::new(arguments.workspace, arguments.run_dir).map_err(DriverError::review)?;
    match arguments.operation.as_str() {
        "gather" => {
            let clock = FixedReviewClock {
                elapsed: Duration::ZERO,
                gathered_at: arguments
                    .gathered_at
                    .ok_or_else(|| DriverError::usage("missing fixed gathered-at value"))?,
            };
            let target = arguments
                .target
                .ok_or_else(|| DriverError::usage("missing target"))?;
            let scope = arguments
                .scope
                .ok_or_else(|| DriverError::usage("missing scope"))?;
            gather(&run, &target, &scope, &clock).map_err(DriverError::review)
        }
        "facts" => facts(&run).map_err(DriverError::review),
        operation => Err(DriverError::usage(format!(
            "unsupported operation: {operation}"
        ))),
    }
}

fn report(error: DriverError) -> ! {
    let mut stderr = io::stderr().lock();
    let prefix = if error.crtool_prefix {
        "crtool"
    } else {
        "parity_driver"
    };
    let write_result =
        writeln!(stderr, "{prefix}: error: {}", error.message).and_then(|_| stderr.flush());
    let code = if write_result.is_ok() { error.code } else { 2 };
    std::process::exit(code);
}

fn emit(output: StepOutput) -> std::result::Result<(), DriverError> {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(output.stdout())
        .and_then(|_| stdout.flush())
        .map_err(|error| DriverError {
            code: 2,
            message: format!("could not write stdout: {error}"),
            crtool_prefix: false,
        })
}

fn main() {
    match env::args().nth(1).as_deref() {
        Some("fixture") => match fixture_main() {
            Ok(code) => std::process::exit(code),
            Err(error) => report(error),
        },
        Some("diagnostics") => {
            if let Err(error) = diagnostics_driver() {
                report(error);
            }
            return;
        }
        _ => {}
    }
    let arguments = match parse_arguments() {
        Ok(arguments) => arguments,
        Err(error) => report(error),
    };
    let output = match execute(arguments) {
        Ok(output) => output,
        Err(error) => report(error),
    };
    if let Err(error) = emit(output) {
        report(error);
    }
}

// Native fixture modes stay in this explicitly built verification example.
fn fixture_error(error: impl std::fmt::Display) -> DriverError {
    DriverError::usage(format!("native fixture: {error}"))
}

fn write_receipt(
    path: &std::path::Path,
    value: &serde_json::Value,
) -> std::result::Result<(), DriverError> {
    let bytes = serde_json::to_vec(value).map_err(fixture_error)?;
    std::fs::write(path, bytes).map_err(fixture_error)
}

#[cfg(unix)]
fn native_units(value: &std::ffi::OsStr) -> Vec<u32> {
    use std::os::unix::ffi::OsStrExt;
    value
        .as_bytes()
        .iter()
        .map(|byte| u32::from(*byte))
        .collect()
}

#[cfg(windows)]
fn native_units(value: &std::ffi::OsStr) -> Vec<u32> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().map(u32::from).collect()
}

fn handshake(root: &std::path::Path, name: &str) -> std::result::Result<(), DriverError> {
    std::fs::write(root.join(name), b"ready").map_err(fixture_error)
}

fn serve_fixture(
    root: &std::path::Path,
    role: &str,
    append: bool,
) -> std::result::Result<(), DriverError> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let request = root.join(format!("{role}.challenge"));
    let response = root.join(format!("{role}.response"));
    let release = root.join(format!("{role}.release"));
    let mut previous = Vec::new();
    while !release.exists() {
        if Instant::now() >= deadline {
            return Err(fixture_error(format!("{role} release deadline")));
        }
        match std::fs::read(&request) {
            Ok(value) if value != previous => {
                std::fs::write(&response, &value).map_err(fixture_error)?;
                previous = value;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(fixture_error(error)),
        }
        if append && root.join("holder.append").exists() {
            io::stderr()
                .write_all(b"LATE-HOLDER\n")
                .map_err(fixture_error)?;
            io::stderr().flush().map_err(fixture_error)?;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if role == "holder" {
        io::stdout()
            .write_all(b"released-out")
            .map_err(fixture_error)?;
        io::stderr()
            .write_all(b"released-err")
            .map_err(fixture_error)?;
        io::stdout().flush().map_err(fixture_error)?;
        io::stderr().flush().map_err(fixture_error)?;
    }
    handshake(root, &format!("{role}.done"))
}

fn fixture_main() -> std::result::Result<i32, DriverError> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 5 || args[1] != "--root" || args[3] != "--kind" {
        return Err(fixture_error(
            "expected fixture --root PATH --kind KIND [-- ARGS]",
        ));
    }
    let root = PathBuf::from(&args[2]);
    let role = &args[4];
    std::fs::write(
        root.join(format!("{role}.pid")),
        std::process::id().to_string(),
    )
    .map_err(fixture_error)?;
    if role == "sentinel" || role == "holder" {
        if role == "holder" {
            io::stdout()
                .write_all(b"holder-out")
                .map_err(fixture_error)?;
            io::stderr()
                .write_all(b"holder-err")
                .map_err(fixture_error)?;
            io::stdout().flush().map_err(fixture_error)?;
            io::stderr().flush().map_err(fixture_error)?;
        }
        handshake(&root, &format!("{role}.ready"))?;
        serve_fixture(&root, role, role == "holder")?;
        return Ok(0);
    }
    let user_args = match args.get(5) {
        Some(marker) if marker == "--" => args[6..].to_vec(),
        None => Vec::new(),
        _ => return Err(fixture_error("fixture user arguments require --")),
    };
    let environment: Vec<_> = env::vars_os()
        .map(|(key, value)| serde_json::json!({"name": native_units(&key), "value": native_units(&value)}))
        .collect();
    let cwd = env::current_dir().map_err(fixture_error)?;
    write_receipt(
        &root.join("child.receipt"),
        &serde_json::json!({
            "argv": user_args, "cwd": native_units(cwd.as_os_str()), "env": environment,
            "pid": std::process::id()
        }),
    )?;
    let launches = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("launches"));
    writeln!(launches.map_err(fixture_error)?, "{}", std::process::id()).map_err(fixture_error)?;
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("config.json")).map_err(fixture_error)?)
            .map_err(fixture_error)?;
    io::stdout()
        .write_all(&std::fs::read(root.join("stdout.bin")).map_err(fixture_error)?)
        .map_err(fixture_error)?;
    io::stderr()
        .write_all(&std::fs::read(root.join("stderr.bin")).map_err(fixture_error)?)
        .map_err(fixture_error)?;
    io::stdout().flush().map_err(fixture_error)?;
    io::stderr().flush().map_err(fixture_error)?;
    if role == "exit-holder" || role == "live-holder" {
        let mut holder = std::process::Command::new(env::current_exe().map_err(fixture_error)?)
            .args(["fixture", "--root"])
            .arg(&root)
            .args(["--kind", "holder"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(fixture_error)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.join("holder.ready").exists() {
            if Instant::now() >= deadline {
                holder.kill().map_err(fixture_error)?;
                holder.wait().map_err(fixture_error)?;
                return Err(fixture_error("holder never became ready"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        // Dropping a std Child does not kill it. The harness owns its release.
        drop(holder);
    }
    handshake(&root, "child.ready")?;
    if role == "wait" || role == "live-holder" {
        serve_fixture(&root, "child", false)?;
    } else {
        let millis = config
            .get("sleep_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        std::thread::sleep(Duration::from_millis(millis));
    }
    handshake(&root, "child.exiting")?;
    let code = config
        .get("exit_code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    i32::try_from(code).map_err(fixture_error)
}

struct DiagnosticsClock {
    fixed: Option<Duration>,
    cancel_after_exit: bool,
    cancel: Cancellation,
    elapsed: Cell<Option<Duration>>,
}

impl ReviewClock for DiagnosticsClock {
    fn gathered_at_utc(&self) -> Result<String> {
        SystemReviewClock.gathered_at_utc()
    }

    fn diagnostics_elapsed(&self, started: Instant) -> Duration {
        if self.cancel_after_exit {
            self.cancel.cancel();
        }
        let elapsed = self
            .fixed
            .unwrap_or_else(|| SystemReviewClock.diagnostics_elapsed(started));
        self.elapsed.set(Some(elapsed));
        elapsed
    }
}

fn diagnostics_driver() -> std::result::Result<(), DriverError> {
    let mut flags = std::collections::BTreeMap::new();
    let mut args = env::args().skip(2);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| DriverError::usage(format!("missing value for {flag}")))?;
        if !matches!(
            flag.as_str(),
            "--workspace"
                | "--rundir"
                | "--command"
                | "--receipt"
                | "--clock"
                | "--elapsed-seconds"
                | "--timeout-seconds"
                | "--cancel"
                | "--cancel-ready"
        ) {
            return Err(DriverError::usage(format!(
                "unknown diagnostics argument: {flag}"
            )));
        }
        if flags.insert(flag.clone(), value).is_some() {
            return Err(DriverError::usage(format!(
                "duplicate diagnostics argument: {flag}"
            )));
        }
    }
    let required = |flag: &str| {
        flags
            .get(flag)
            .cloned()
            .ok_or_else(|| DriverError::usage(format!("missing {flag}")))
    };
    let workspace = required("--workspace")?;
    let run_dir = required("--rundir")?;
    let command = required("--command")?;
    let receipt = PathBuf::from(required("--receipt")?);
    let duration = |flag: &str| -> std::result::Result<Duration, DriverError> {
        let value: f64 = required(flag)?.parse().map_err(fixture_error)?;
        Duration::try_from_secs_f64(value).map_err(fixture_error)
    };
    let fixed = match required("--clock")?.as_str() {
        "fixed" => Some(duration("--elapsed-seconds")?),
        "system" if !flags.contains_key("--elapsed-seconds") => None,
        _ => {
            return Err(DriverError::usage(
                "--clock requires fixed with --elapsed-seconds or system without it",
            ));
        }
    };
    let options = DiagnosticsOptions {
        timeout: match flags.get("--timeout-seconds").map(String::as_str) {
            Some("default") | None => DiagnosticsOptions::default().timeout,
            Some(_) => duration("--timeout-seconds")?,
        },
    };
    let cancel = Cancellation::default();
    let mode = flags.get("--cancel").map_or("none", String::as_str);
    if !matches!(mode, "none" | "pre" | "live" | "after-exit") {
        return Err(DriverError::usage("unsupported cancellation mode"));
    }
    if mode == "pre" {
        cancel.cancel();
    }
    let clock = DiagnosticsClock {
        fixed,
        cancel_after_exit: mode == "after-exit",
        cancel: cancel.clone(),
        elapsed: Cell::new(None),
    };
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker = if mode == "live" {
        let ready = PathBuf::from(required("--cancel-ready")?);
        let signal = cancel.clone();
        let stop = stop.clone();
        Some(std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !stop.load(std::sync::atomic::Ordering::Acquire) {
                if ready.exists() {
                    signal.cancel();
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(fixture_error("live cancellation handshake deadline"));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            if signal.is_cancelled() {
                Ok(())
            } else {
                Err(fixture_error(
                    "operation returned before live cancellation handshake",
                ))
            }
        }))
    } else {
        None
    };
    let invocation = ReviewRun::new(workspace, run_dir)
        .and_then(|run| diagnostics(&run, &command, &options, &cancel, &clock));
    stop.store(true, std::sync::atomic::Ordering::Release);
    let cancellation_result = match worker {
        Some(worker) => worker
            .join()
            .map_err(|_| fixture_error("cancellation worker panicked"))?,
        None => Ok(()),
    };
    match invocation {
        Ok(result) => {
            let (outcome, exit_code) = match result.outcome() {
                DiagnosticsOutcome::Clean => ("Clean", Some(0)),
                DiagnosticsOutcome::Failed { exit_code } => ("Failed", Some(exit_code)),
                DiagnosticsOutcome::TimedOut => ("TimedOut", None),
                DiagnosticsOutcome::Cancelled => ("Cancelled", None),
            };
            let elapsed = clock.elapsed.get();
            write_receipt(
                &receipt,
                &serde_json::json!({
                    "outcome": outcome, "exit_code": exit_code, "started": result.started(),
                    "cancelled": cancel.is_cancelled(), "elapsed_seconds": elapsed.map(|value| value.as_secs_f64()),
                    "timeout_seconds": options.timeout.as_secs_f64(), "clock": if fixed.is_some() { "fixed" } else { "system" }
                }),
            )?;
            cancellation_result?;
            io::stdout()
                .write_all(result.output().stdout())
                .map_err(fixture_error)?;
            io::stdout().flush().map_err(fixture_error)
        }
        Err(error) => {
            let kind = match &error {
                ReviewError::Diagnostics(DiagnosticsError::InvalidCommand { .. }) => {
                    "InvalidCommand"
                }
                ReviewError::Diagnostics(DiagnosticsError::CannotStart { .. }) => "CannotStart",
                ReviewError::Diagnostics(DiagnosticsError::Lifecycle { .. }) => "Lifecycle",
                ReviewError::MissingStamp => "MissingStamp",
                ReviewError::StampMismatch { .. } => "StampMismatch",
                ReviewError::InvalidManifest { .. } => "InvalidManifest",
                ReviewError::Io { .. } => "Io",
                ReviewError::Json { .. } => "Json",
                _ => "Other",
            };
            write_receipt(
                &receipt,
                &serde_json::json!({"error_kind": kind, "message": error.to_string()}),
            )?;
            cancellation_result?;
            Err(DriverError::review(error))
        }
    }
}

//! Deterministic verification consumer for the public slice-A leaf seam.
//!
//! This example is intentionally not installed as a Cyril command.  It gives
//! the differential harness a real Rust caller while keeping the production
//! CLI on `SystemReviewClock`.

use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

use cyril_review::{Result, ReviewClock, ReviewError, ReviewRun, StepOutput, facts, gather};

struct FixedReviewClock {
    gathered_at: String,
}

impl ReviewClock for FixedReviewClock {
    fn gathered_at_utc(&self) -> Result<String> {
        Ok(self.gathered_at.clone())
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

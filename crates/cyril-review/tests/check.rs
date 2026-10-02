//! The check command: outcomes, output capture, and stopping it.

use cyril_review::{CheckOutcome, CheckResult, ReviewError, ReviewRun, gather, run_check};
use std::error::Error;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// A gathered run over a one-commit repository with `src/lib.rs` changed.
fn gathered() -> TestResult<(tempfile::TempDir, ReviewRun)> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    fs::create_dir_all(repo.join("src"))?;
    let config = tree.path().join("gitconfig");
    fs::write(&config, "")?;
    let git = |args: &[&str]| -> TestResult {
        let status = Command::new("git")
            .args(args)
            .current_dir(&repo)
            .env("GIT_CONFIG_GLOBAL", &config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Check")
            .env("GIT_AUTHOR_EMAIL", "check@example.invalid")
            .env("GIT_COMMITTER_NAME", "Check")
            .env("GIT_COMMITTER_EMAIL", "check@example.invalid")
            .status()?;
        if !status.success() {
            return Err(format!("git {args:?} failed").into());
        }
        Ok(())
    };
    git(&["init", "-q", "-b", "main"])?;
    // Windows git ships core.autocrlf=true; its CRLF warnings would show up
    // in the check output as extra lines that mention the changed file.
    git(&["config", "core.autocrlf", "false"])?;
    fs::write(repo.join("src/lib.rs"), "pub fn one() {}\n")?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "--no-verify", "-m", "base"])?;
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn one() {}\npub fn two() {}\n",
    )?;
    let run = ReviewRun::new(&repo, tree.path().join("run"))?;
    gather(&run, "HEAD", "")?;
    Ok((tree, run))
}

fn check(
    run: &ReviewRun,
    command: &str,
    timeout: Duration,
    cancel: impl Future<Output = ()>,
) -> cyril_review::Result<CheckResult> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| ReviewError::Io {
            operation: "build runtime",
            path: run.dir().to_path_buf(),
            source,
        })?;
    runtime.block_on(run_check(run, command, timeout, cancel))
}

fn report(run: &ReviewRun) -> TestResult<String> {
    Ok(fs::read_to_string(run.dir().join("facts/diagnostics.txt"))?)
}

fn raw(run: &ReviewRun) -> TestResult<String> {
    Ok(fs::read_to_string(
        run.dir().join("facts/diagnostics-raw.txt"),
    )?)
}

fn manifest_status(run: &ReviewRun) -> TestResult<String> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(run.dir().join("manifest.json"))?)?;
    let status = manifest["facts"]["diagnostics_status"]
        .as_str()
        .ok_or("manifest has no facts.diagnostics_status")?;
    Ok(status.to_owned())
}

/// A command that prints `early` and then runs for 30 seconds.
fn slow_command() -> &'static str {
    if cfg!(windows) {
        "powershell -NoProfile -Command \"Write-Output early; Start-Sleep -Seconds 30\""
    } else {
        "sh -c 'echo early; sleep 30'"
    }
}

const LONG: Duration = Duration::from_secs(120);

#[test]
fn clean_check_reports_lines_that_mention_changed_files() -> TestResult {
    let (_tree, run) = gathered()?;
    let result = check(
        &run,
        "git diff --name-only HEAD",
        LONG,
        std::future::pending(),
    )?;
    assert_eq!(result.outcome(), CheckOutcome::Clean);
    assert_eq!(result.cleanup_error(), None);
    assert!(result.summary().starts_with("diagnostics: clean in "));
    let report = report(&run)?;
    assert!(report.starts_with("command: git diff --name-only HEAD\nresult: clean in "));
    assert!(report.contains("1 output line(s) mention a changed file:\nsrc/lib.rs\n"));
    assert!(report.ends_with("last lines of output:\nsrc/lib.rs\n"));
    assert_eq!(manifest_status(&run)?, "clean");
    Ok(())
}

#[test]
fn failing_check_is_data_with_its_exit_code() -> TestResult {
    let (_tree, run) = gathered()?;
    let result = check(
        &run,
        "git rev-parse --verify no-such-ref",
        LONG,
        std::future::pending(),
    )?;
    assert_eq!(
        result.outcome(),
        CheckOutcome::Failed {
            exit_code: Some(128)
        }
    );
    assert_eq!(manifest_status(&run)?, "FAILED (exit 128)");
    assert!(raw(&run)?.contains("fatal"));
    Ok(())
}

#[test]
fn a_command_that_cannot_start_is_an_error_and_writes_nothing() -> TestResult {
    let (_tree, run) = gathered()?;
    let error = check(
        &run,
        "cyril-no-such-program-x",
        LONG,
        std::future::pending(),
    )
    .err()
    .ok_or("a missing program must not run")?;
    assert!(matches!(error, ReviewError::CannotStart { .. }), "{error}");
    assert_eq!(error.exit_code(), 2);
    assert!(!run.dir().join("facts/diagnostics.txt").exists());
    Ok(())
}

#[test]
fn an_empty_command_is_refused() -> TestResult {
    let (_tree, run) = gathered()?;
    let error = check(&run, "   ", LONG, std::future::pending())
        .err()
        .ok_or("an empty command must be refused")?;
    assert!(
        matches!(error, ReviewError::InvalidCommand { .. }),
        "{error}"
    );
    Ok(())
}

#[test]
fn timeout_kills_the_command_and_keeps_its_output() -> TestResult {
    let (_tree, run) = gathered()?;
    let started = Instant::now();
    let result = check(
        &run,
        slow_command(),
        Duration::from_secs(3),
        std::future::pending(),
    )?;
    assert_eq!(result.outcome(), CheckOutcome::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
    assert!(raw(&run)?.contains("early"));
    assert!(report(&run)?.contains("result: TIMED OUT in "));
    assert_eq!(manifest_status(&run)?, "TIMED OUT");
    Ok(())
}

#[test]
fn cancellation_kills_the_command_and_keeps_its_output() -> TestResult {
    let (_tree, run) = gathered()?;
    let started = Instant::now();
    let result = check(
        &run,
        slow_command(),
        LONG,
        // Built lazily: a tokio timer must be created inside the runtime.
        async { tokio::time::sleep(Duration::from_secs(3)).await },
    )?;
    assert_eq!(result.outcome(), CheckOutcome::Cancelled);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
    assert!(raw(&run)?.contains("early"));
    assert_eq!(manifest_status(&run)?, "CANCELLED");
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_grandchild_holding_the_pipe_cannot_hang_the_review() -> TestResult {
    let (_tree, run) = gathered()?;
    let started = Instant::now();
    let result = check(
        &run,
        "sh -c 'sleep 30 & echo started'",
        LONG,
        std::future::pending(),
    )?;
    assert_eq!(result.outcome(), CheckOutcome::Clean);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "took {:?}",
        started.elapsed()
    );
    assert!(
        result
            .cleanup_error()
            .is_some_and(|problem| problem.contains("not captured"))
    );
    assert!(raw(&run)?.contains("started"));
    assert!(report(&run)?.contains("\ncleanup: "));
    Ok(())
}

#[test]
fn an_unstamped_run_is_refused() -> TestResult {
    let (_tree, run) = gathered()?;
    let path = run.dir().join("manifest.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    manifest["crtool_version"] = serde_json::json!("0.0.0-other");
    fs::write(&path, serde_json::to_vec(&manifest)?)?;
    let error = check(&run, "git --version", LONG, std::future::pending())
        .err()
        .ok_or("a foreign stamp must be refused")?;
    assert!(
        matches!(error, ReviewError::StampMismatch { .. }),
        "{error}"
    );
    assert_eq!(error.exit_code(), 2);
    let facts = cyril_review::facts(&run)
        .err()
        .ok_or("facts must refuse too")?;
    assert!(
        matches!(facts, ReviewError::StampMismatch { .. }),
        "{facts}"
    );
    assert!(Path::new(&path).exists());
    Ok(())
}

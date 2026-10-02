//! The hidden `cyril crtool` subcommand as the review recipe runs it.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn git(repo: &Path, config: &Path, args: &[&str]) -> TestResult {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Cli")
        .env("GIT_AUTHOR_EMAIL", "cli@example.invalid")
        .env("GIT_COMMITTER_NAME", "Cli")
        .env("GIT_COMMITTER_EMAIL", "cli@example.invalid")
        .status()?;
    if !status.success() {
        return Err(format!("git {args:?} failed").into());
    }
    Ok(())
}

fn cyril(cwd: &Path, args: &[&str]) -> TestResult<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_cyril"))
        .args(args)
        .current_dir(cwd)
        .output()?)
}

#[test]
fn crtool_gathers_refuses_and_reports_empty_diffs_with_crtool_exit_codes() -> TestResult {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    fs::create_dir_all(repo.join("src"))?;
    let config = tree.path().join("gitconfig");
    fs::write(&config, "")?;
    git(&repo, &config, &["init", "-q", "-b", "main"])?;
    fs::write(repo.join("src/lib.rs"), "pub fn one() {}\n")?;
    git(&repo, &config, &["add", "-A"])?;
    git(
        &repo,
        &config,
        &["commit", "-q", "--no-verify", "-m", "base"],
    )?;

    let empty_run = tree.path().join("empty-run");
    let empty = cyril(
        &repo,
        &[
            "crtool",
            "gather",
            &empty_run.to_string_lossy(),
            "HEAD",
            ".",
        ],
    )?;
    assert_eq!(empty.status.code(), Some(3), "{empty:?}");
    assert!(String::from_utf8_lossy(&empty.stderr).starts_with("crtool: error: empty diff"));

    fs::write(
        repo.join("src/lib.rs"),
        "pub fn one() {}\npub fn two() {}\n",
    )?;
    let run = tree.path().join("run");
    let run = run.to_string_lossy();
    let gathered = cyril(&repo, &["crtool", "gather", &run, "HEAD", "src"])?;
    assert_eq!(gathered.status.code(), Some(0), "{gathered:?}");
    assert!(gathered.stderr.is_empty());
    let stdout = String::from_utf8(gathered.stdout)?;
    assert!(stdout.starts_with("facts: 1 changed symbols"), "{stdout}");
    let last = stdout.lines().last().ok_or("no stdout")?;
    assert!(last.starts_with("gathered 1 files, "), "{stdout}");
    assert!(last.ends_with(" patch bytes, target=HEAD"), "{stdout}");

    let again = cyril(&repo, &["crtool", "gather", &run, "HEAD", "src"])?;
    assert_eq!(
        String::from_utf8(again.stdout)?,
        "already gathered: 1 files, target=HEAD\n"
    );

    let other = cyril(&repo, &["crtool", "gather", &run, "HEAD", "docs"])?;
    assert_eq!(other.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&other.stderr).contains("already holds a different gathered run")
    );

    let subdirectory = cyril(&repo.join("src"), &["crtool", "facts", &run])?;
    assert_eq!(subdirectory.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&subdirectory.stderr).contains("must run from the repository root")
    );

    let facts = cyril(&repo, &["crtool", "facts", &run])?;
    assert_eq!(facts.status.code(), Some(0), "{facts:?}");
    Ok(())
}

#[test]
fn crtool_is_absent_from_help() -> TestResult {
    let tree = tempfile::tempdir()?;
    let help = cyril(tree.path(), &["--help"])?;
    assert!(help.status.success());
    assert!(!String::from_utf8_lossy(&help.stdout).contains("crtool"));
    Ok(())
}

//! `touched_files`: the count `/review` shows before consent.

use cyril_review::{ReviewError, ReviewRun, base_branches, touched_files};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

type TestResult = Result<(), Box<dyn Error>>;

fn git(repo: &Path, config: &Path, args: &[&str]) -> TestResult {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Probe")
        .env("GIT_AUTHOR_EMAIL", "probe@example.invalid")
        .env("GIT_COMMITTER_NAME", "Probe")
        .env("GIT_COMMITTER_EMAIL", "probe@example.invalid")
        .output()?;
    if !output.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(())
}

#[test]
fn counts_the_auto_diff_and_refuses_below_the_root() -> TestResult {
    let tree = tempfile::tempdir()?;
    let config = tree.path().join("gitconfig");
    fs::write(&config, "")?;
    let repo = tree.path().join("repo");
    fs::create_dir_all(repo.join("src"))?;
    fs::write(repo.join("src/a.rs"), "fn a() {}\n")?;
    fs::write(repo.join("b.md"), "b\n")?;
    git(&repo, &config, &["init", "-q", "-b", "main"])?;
    git(&repo, &config, &["add", "-A"])?;
    git(&repo, &config, &["commit", "-q", "-m", "first"])?;
    let run = ReviewRun::new(&repo, repo.join(".code-review/r"))?;

    // Clean and at main: auto diffs the merge-base against the worktree.
    assert_eq!(touched_files(&run, "auto", "")?, 0);

    fs::write(repo.join("src/a.rs"), "fn a() { todo!() }\n")?;
    fs::write(repo.join("b.md"), "b, changed\n")?;
    assert_eq!(touched_files(&run, "auto", "")?, 2);
    assert_eq!(touched_files(&run, "auto", "src")?, 1);

    let below = ReviewRun::new(repo.join("src"), repo.join(".code-review/r"))?;
    assert!(matches!(
        touched_files(&below, "auto", ""),
        Err(ReviewError::NotRepositoryRoot { .. })
    ));
    Ok(())
}

#[test]
fn base_branches_leave_out_the_checked_out_branch() -> TestResult {
    let tree = tempfile::tempdir()?;
    let config = tree.path().join("gitconfig");
    fs::write(&config, "")?;
    let repo = tree.path().join("repo");
    fs::create_dir_all(&repo)?;
    fs::write(repo.join("a.txt"), "a\n")?;
    git(&repo, &config, &["init", "-q", "-b", "main"])?;
    git(&repo, &config, &["add", "-A"])?;
    git(&repo, &config, &["commit", "-q", "-m", "first"])?;
    git(&repo, &config, &["branch", "release"])?;
    git(&repo, &config, &["checkout", "-q", "-b", "feature"])?;
    let run = ReviewRun::new(&repo, repo.join(".code-review/r"))?;
    assert_eq!(base_branches(&run)?, ["main", "release"]);
    Ok(())
}

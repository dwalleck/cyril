//! git, always as an explicit-argv subprocess run from the repository root.

use crate::{Result, ReviewError};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// `git diff` with every output-shaping setting pinned, so the patches the
/// models read never depend on the user's diff configuration.
pub(crate) const DIFF: [&str; 11] = [
    "diff",
    "--no-renames",
    "--no-relative",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--submodule=short",
    "--src-prefix=a/",
    "--dst-prefix=b/",
    "--diff-algorithm=myers",
    "--indent-heuristic",
];

/// Pins `diff.context` for the calls that print patches. Never pass it with
/// `--numstat`, `--name-status` or `--name-only`: `-U` implies `--patch`,
/// which appends patch text to their records.
pub(crate) const PATCH_CONTEXT: &str = "-U3";

/// Run git; a non-zero exit is an error carrying git's stderr.
pub(crate) fn git<A: AsRef<OsStr>>(workspace: &Path, args: &[A]) -> Result<Vec<u8>> {
    let output = git_output(workspace, args)?;
    if !output.status.success() {
        return Err(failure(args, &output));
    }
    Ok(output.stdout)
}

pub(crate) fn git_text<A: AsRef<OsStr>>(workspace: &Path, args: &[A]) -> Result<String> {
    Ok(String::from_utf8_lossy(&git(workspace, args)?).into_owned())
}

/// Run git and return its output whatever the exit status.
pub(crate) fn git_output<A: AsRef<OsStr>>(workspace: &Path, args: &[A]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| ReviewError::GitSpawn { source })
}

pub(crate) fn failure<A: AsRef<OsStr>>(args: &[A], output: &Output) -> ReviewError {
    ReviewError::Git {
        args: args
            .iter()
            .map(|arg| arg.as_ref().to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
}

/// Whether `revision` names a commit.
pub(crate) fn is_commit(workspace: &Path, revision: &str) -> Result<bool> {
    let spec = format!("{revision}^{{commit}}");
    Ok(
        git_output(workspace, &["rev-parse", "--verify", "--quiet", &spec])?
            .status
            .success(),
    )
}

pub(crate) fn commit_id(workspace: &Path, revision: &str) -> Result<String> {
    Ok(
        git_text(workspace, &["rev-parse", &format!("{revision}^{{commit}}")])?
            .trim()
            .to_owned(),
    )
}

/// Split NUL-terminated `-z` output into UTF-8 records.
pub(crate) fn nul_records(bytes: &[u8]) -> Result<Vec<&str>> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            std::str::from_utf8(record).map_err(|_| ReviewError::NonUtf8Path {
                name: String::from_utf8_lossy(record).into_owned(),
            })
        })
        .collect()
}

/// Refuse to run anywhere but the repository root: every path crtool writes
/// and reads is repository-root-relative.
pub(crate) fn require_root(workspace: &Path) -> Result<()> {
    let cdup = git_text(workspace, &["rev-parse", "--show-cdup"])?;
    let cdup = cdup.trim();
    if cdup.is_empty() {
        Ok(())
    } else {
        Err(ReviewError::NotRepositoryRoot {
            cdup: cdup.to_owned(),
        })
    }
}

/// A revision that git would read as an option is refused outright.
pub(crate) fn admit_target(target: &str) -> Result<()> {
    if target.starts_with('-') {
        return Err(ReviewError::InvalidTarget {
            target: target.to_owned(),
        });
    }
    Ok(())
}

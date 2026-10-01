use crate::run::ReviewRun;
use crate::{Result, ReviewError};
use git2::{Diff, DiffOptions, Oid, Patch, Repository, RevparseMode, StatusOptions};
use std::ffi::OsStr;
use std::process::{Command, Output};

pub(crate) fn operation(context: impl Into<String>, error: impl std::fmt::Display) -> ReviewError {
    ReviewError::GitOperation {
        operation: context.into(),
        message: error.to_string(),
    }
}

pub(crate) fn admit_target(target: &str) -> Result<()> {
    if target.starts_with('-') {
        return Err(ReviewError::InvalidTarget {
            target: target.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn repository(run: &ReviewRun) -> Result<Repository> {
    Repository::discover(run.workspace()).map_err(|error| {
        operation(
            format!("discover repository at {}", run.workspace().display()),
            error,
        )
    })
}

pub(crate) fn raw_path_arg(bytes: &[u8]) -> Result<&OsStr> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(bytes).map(OsStr::new).map_err(|error| {
            operation(
                format!(
                    "represent Git filename {:?}",
                    String::from_utf8_lossy(bytes)
                ),
                error,
            )
        })
    }
}

fn run_allow_failure<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<Output> {
    Command::new("git")
        .args(args)
        .current_dir(run.workspace())
        .output()
        .map_err(|source| ReviewError::GitSpawn {
            args: render_args(args),
            source,
        })
}

fn required_output<A: AsRef<OsStr>>(args: &[A], output: Output) -> Result<Vec<u8>> {
    if !output.status.success() {
        return Err(ReviewError::GitFailure {
            args: render_args(args),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

pub(crate) fn scoped_patch(run: &ReviewRun, target: &str, scope: &[String]) -> Result<Vec<u8>> {
    admit_target(target)?;
    // Git owns caller pathspecs and userdiff headers; libgit2 owns the typed patch model.
    let mut args = vec![
        "diff",
        "--no-renames",
        "--no-relative",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--binary",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        target,
        "--",
    ];
    args.extend(scope.iter().map(String::as_str));
    required_output(&args, run_allow_failure(run, &args)?)
}

pub(crate) fn grep<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<Vec<u8>> {
    let output = run_allow_failure(run, args)?;
    match output.status.code() {
        Some(0) | Some(1) => Ok(output.stdout),
        _ => required_output(args, output),
    }
}

pub(crate) fn commit_id(repo: &Repository, revision: &str) -> Result<Oid> {
    repo.revparse_single(revision)
        .and_then(|object| object.peel_to_commit())
        .map(|commit| commit.id())
        .map_err(|error| operation(format!("resolve commit {revision:?}"), error))
}

pub(crate) fn resolve_target(
    repo: &Repository,
    requested: &str,
) -> Result<(String, Option<String>)> {
    admit_target(requested)?;
    if requested != "auto" {
        return Ok((requested.to_owned(), None));
    }
    let mut base = None;
    for candidate in ["@{upstream}", "main", "master"] {
        match repo
            .revparse_single(candidate)
            .and_then(|object| object.peel_to_commit())
        {
            Ok(commit) => {
                base = Some((candidate, commit.id()));
                break;
            }
            Err(error)
                if matches!(
                    error.code(),
                    git2::ErrorCode::NotFound | git2::ErrorCode::UnbornBranch
                ) => {}
            // Detached HEAD has no branch upstream.
            Err(error)
                if candidate == "@{upstream}" && error.code() == git2::ErrorCode::InvalidSpec => {}
            Err(error) => return Err(operation(format!("resolve auto base {candidate:?}"), error)),
        }
    }
    let statuses = repo
        .statuses(Some(
            StatusOptions::new()
                .include_untracked(false)
                .no_refresh(true),
        ))
        .map_err(|error| operation("inspect tracked worktree status", error))?;
    let dirty = !statuses.is_empty();
    let Some((base, base_id)) = base else {
        let target = if dirty { "HEAD" } else { "HEAD~1" };
        return Ok((
            target.to_owned(),
            Some(format!("no upstream/main: fell back to {target}")),
        ));
    };
    let head = commit_id(repo, "HEAD")?;
    let merge_base = repo
        .merge_base(base_id, head)
        .map_err(|error| operation(format!("merge-base {base} HEAD"), error))?;
    let range = format!("{base}...HEAD");
    let empty = target_diff(repo, &range)?.deltas().len() == 0;
    if dirty || empty {
        let merge_base = merge_base.to_string();
        let warning = format!(
            "working tree included (dirty={dirty}, empty_range={empty}); diffing against merge-base {}",
            &merge_base[..12],
        );
        return Ok((merge_base, Some(warning)));
    }
    Ok((range, None))
}

pub(crate) fn target_diff<'a>(repo: &'a Repository, target: &str) -> Result<Diff<'a>> {
    admit_target(target)?;
    let result = (|| {
        let spec = repo.revparse(target)?;
        let from = spec
            .from()
            .ok_or_else(|| git2::Error::from_str("missing revision start"))?;
        let mut options = DiffOptions::new();
        if spec.mode().contains(RevparseMode::RANGE) {
            let to = spec
                .to()
                .ok_or_else(|| git2::Error::from_str("missing revision end"))?;
            let to_tree = to.peel_to_tree()?;
            let from_tree = if spec.mode().contains(RevparseMode::MERGE_BASE) {
                let base =
                    repo.merge_base(from.peel_to_commit()?.id(), to.peel_to_commit()?.id())?;
                repo.find_commit(base)?.tree()?
            } else {
                from.peel_to_tree()?
            };
            repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut options))
        } else {
            repo.diff_tree_to_workdir_with_index(Some(&from.peel_to_tree()?), Some(&mut options))
        }
    })();
    result.map_err(|error| operation(format!("diff target {target:?}"), error))
}

pub(crate) fn parse_patch(bytes: &[u8], source: &str) -> Result<Diff<'static>> {
    // libgit2 needs a mail terminator for header-only entries, even at EOF.
    let lines = bytes.split_inclusive(|byte| *byte == b'\n');
    let records = lines
        .clone()
        .filter(|line| line.starts_with(b"diff --git "))
        .count();
    let mut framed = Vec::with_capacity(bytes.len() + (records + 1) * 4);
    let mut synthetic = std::collections::HashSet::with_capacity(records + 1);
    for line in lines {
        if line.starts_with(b"diff --git ") && !framed.is_empty() {
            synthetic.insert(framed.len() as i64);
            framed.extend_from_slice(b"-- \n");
        }
        framed.extend_from_slice(line);
    }
    synthetic.insert(framed.len() as i64);
    framed.extend_from_slice(b"-- \n");
    let diff = Diff::from_buffer(&framed)
        .map_err(|error| operation(format!("decode enveloped Git patch {source:?}"), error))?;
    // A truncated hunk must not consume the synthetic trailer as a deletion.
    diff.foreach(
        &mut |_, _| true,
        None,
        None,
        Some(&mut |_, _, line| !synthetic.contains(&line.content_offset())),
    )
    .map_err(|error| {
        let message = if error.code() == git2::ErrorCode::User {
            "incomplete source hunk consumes record envelope".to_owned()
        } else {
            error.to_string()
        };
        operation(format!("validate Git patch envelope {source:?}"), message)
    })?;
    Ok(diff)
}

pub(crate) fn patch<'a>(diff: &'a Diff<'_>, index: usize) -> Result<Patch<'a>> {
    Patch::from_diff(diff, index)
        .map_err(|error| operation(format!("read patch delta {index}"), error))?
        .ok_or_else(|| operation(format!("read patch delta {index}"), "missing patch"))
}

pub(crate) fn delta_path<'a>(delta: &git2::DiffDelta<'a>) -> Result<&'a [u8]> {
    delta
        .new_file()
        .path_bytes()
        .or_else(|| delta.old_file().path_bytes())
        .ok_or_else(|| operation("read delta path", "missing path"))
}

pub(crate) fn delta_status(delta: git2::Delta) -> Result<&'static str> {
    match delta {
        git2::Delta::Added => Ok("A"),
        git2::Delta::Deleted => Ok("D"),
        git2::Delta::Modified => Ok("M"),
        git2::Delta::Typechange => Ok("T"),
        _ => Err(operation(
            "read delta status",
            format!("unsupported status {delta:?}"),
        )),
    }
}

pub(crate) fn head_side(target: &str) -> Option<&str> {
    for separator in ["...", ".."] {
        if let Some(index) = target.find(separator) {
            let side = &target[index + separator.len()..];
            return Some(if side.is_empty() { "HEAD" } else { side });
        }
    }
    None
}

fn render_args<A: AsRef<OsStr>>(args: &[A]) -> String {
    let mut rendered = String::new();
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            rendered.push(' ');
        }
        rendered.push_str(&arg.as_ref().to_string_lossy());
    }
    rendered
}

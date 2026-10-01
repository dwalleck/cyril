use crate::run::ReviewRun;
use crate::{Result, ReviewError};
use std::ffi::OsStr;
use std::process::{Command, Output};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

pub(crate) fn raw_path_arg(bytes: &[u8]) -> Result<&OsStr> {
    #[cfg(unix)]
    {
        Ok(OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        let value = std::str::from_utf8(bytes).map_err(|error| ReviewError::GitFailure {
            args: "diff pathspec".to_owned(),
            message: format!(
                "Git filename {:?} is not valid UTF-8 on this platform: {error}",
                String::from_utf8_lossy(bytes)
            ),
        })?;
        Ok(OsStr::new(value))
    }
}

pub(crate) fn run_allow_failure<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<Output> {
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

pub(crate) fn bytes<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<Vec<u8>> {
    required_output(args, run_allow_failure(run, args)?)
}

pub(crate) fn grep<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<Vec<u8>> {
    let output = run_allow_failure(run, args)?;
    match output.status.code() {
        Some(0) | Some(1) => Ok(output.stdout),
        _ => required_output(args, output),
    }
}

pub(crate) fn text<A: AsRef<OsStr>>(run: &ReviewRun, args: &[A]) -> Result<String> {
    Ok(String::from_utf8_lossy(&bytes(run, args)?).into_owned())
}

pub(crate) fn rev_ok(run: &ReviewRun, revision: &str) -> Result<bool> {
    let args = vec![
        "rev-parse".to_owned(),
        "--verify".to_owned(),
        "--quiet".to_owned(),
        format!("{revision}^{{commit}}"),
    ];
    Ok(run_allow_failure(run, &args)?.status.success())
}

pub(crate) fn resolve_target(run: &ReviewRun, requested: &str) -> Result<(String, Option<String>)> {
    if requested != "auto" {
        return Ok((requested.to_owned(), None));
    }
    let mut base = None;
    for candidate in ["@{upstream}", "main", "master"] {
        if rev_ok(run, candidate)? {
            base = Some(candidate.to_owned());
            break;
        }
    }
    let dirty_args = vec![
        "status".to_owned(),
        "--porcelain".to_owned(),
        "--untracked-files=no".to_owned(),
    ];
    let dirty = !text(run, &dirty_args)?.trim().is_empty();
    let Some(base) = base else {
        let target = if dirty { "HEAD" } else { "HEAD~1" };
        return Ok((
            target.to_owned(),
            Some(format!("no upstream/main: fell back to {target}")),
        ));
    };
    let range = format!("{base}...HEAD");
    let range_args = vec!["diff".to_owned(), "--name-only".to_owned(), range.clone()];
    let empty = text(run, &range_args)?.trim().is_empty();
    if dirty || empty {
        let merge_args = vec!["merge-base".to_owned(), base.clone(), "HEAD".to_owned()];
        let merge_base = text(run, &merge_args)?.trim().to_owned();
        let prefix = merge_base.chars().take(12).collect::<String>();
        return Ok((
            merge_base,
            Some(format!(
                "working tree included (dirty={dirty}, empty_range={empty}); diffing against merge-base {prefix}",
            )),
        ));
    }
    Ok((range, None))
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

pub(crate) fn render_args<A: AsRef<OsStr>>(args: &[A]) -> String {
    let mut rendered = String::new();
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            rendered.push(' ');
        }
        rendered.push_str(&arg.as_ref().to_string_lossy());
    }
    rendered
}

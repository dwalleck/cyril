use crate::git::{self, DIFF, PATCH_CONTEXT};
use crate::run::{Manifest, ManifestFile, ReviewRun, VERSION, read_manifest, write, write_json};
use crate::{Result, ReviewError, facts, io_error};
use std::collections::HashMap;
use std::fs;

const RUN_DIRS: [&str; 6] = [
    "patches",
    "candidates",
    "deduped",
    "queues",
    "verdicts",
    "facts",
];

/// `crtool gather <rundir> <target> [<scope>]`: write the diff, per-file
/// patches and manifest, then the facts. Returns the step's stdout.
pub fn gather(run: &ReviewRun, requested_target: &str, scope: &str) -> Result<String> {
    git::admit_target(requested_target)?;
    git::require_root(run)?;
    let scope = split_scope(scope);
    let manifest_path = run.path("manifest.json");
    if manifest_path.exists() {
        // `/review` gathers before the workflow starts, so the recipe's setup
        // step normally finds its work done.
        let mut prior = read_manifest(run)?;
        if prior.requested_target != requested_target || prior.scope != scope {
            return Err(ReviewError::ExistingRun {
                path: run.dir().to_path_buf(),
            });
        }
        let mut out = String::new();
        if !run.path("facts/symbols.json").exists() {
            out += &facts::build_facts(run, &mut prior)?;
        }
        out += &format!(
            "already gathered: {} files, target={}\n",
            prior.total_files, prior.target
        );
        return Ok(out);
    }
    for name in RUN_DIRS {
        let path = run.path(name);
        fs::create_dir_all(&path).map_err(|source| io_error("create directory", path, source))?;
    }

    let (target, note) = resolve_target(run, requested_target)?;
    git::admit_target(&target)?;
    let mut warnings: Vec<String> = note.into_iter().collect();
    let head = git::commit_id(run, "HEAD")?;
    let mut matches_head = true;
    if let Some(side) = head_side(&target) {
        matches_head = git::commit_id(run, side)? == head;
        if !matches_head {
            warnings.push(format!(
                "working tree (HEAD) is not at the diff's head ({side}); source files may not match the patches"
            ));
        }
    }

    let full = git::git(run, &diff_args(&target, &[PATCH_CONTEXT], &scope))?;
    if full.iter().all(u8::is_ascii_whitespace) {
        return Err(ReviewError::EmptyDiff { target, scope });
    }
    write(&run.path("diff.patch"), &full)?;

    let name_status = git::git(run, &diff_args(&target, &["--name-status", "-z"], &scope))?;
    let mut status: HashMap<&str, &str> = HashMap::new();
    for pair in git::nul_records(&name_status)?.chunks_exact(2) {
        status.insert(pair[1], pair[0]);
    }
    let numstat = git::git(run, &diff_args(&target, &["--numstat", "-z"], &scope))?;
    let mut files = Vec::new();
    for record in git::nul_records(&numstat)? {
        let mut fields = record.splitn(3, '\t');
        let (Some(insertions), Some(deletions), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let index = files.len() + 1;
        let patch_name = format!("patches/{index:03}.patch");
        let patch = git::git(
            run,
            &diff_args(&target, &[PATCH_CONTEXT], &[format!(":(literal){path}")]),
        )?;
        write(&run.path(&patch_name), &patch)?;
        let binary = insertions == "-";
        files.push(ManifestFile {
            index: index as u64,
            path: path.to_owned(),
            status: status.get(path).copied().unwrap_or("?").to_owned(),
            insertions: count(binary, insertions),
            deletions: count(binary, deletions),
            binary,
            patch: patch_name,
            patch_bytes: patch.len() as u64,
        });
    }
    let changed: String = files
        .iter()
        .map(|file| format!("{}\n", file.path))
        .collect();
    write(&run.path("changed-files.txt"), changed.as_bytes())?;

    let mut manifest = Manifest {
        requested_target: requested_target.to_owned(),
        target,
        scope,
        head,
        worktree_matches_diff_head: matches_head,
        gathered_at: gathered_at(),
        crtool_version: Some(VERSION.to_owned()),
        total_files: files.len() as u64,
        total_patch_bytes: full.len() as u64,
        warnings,
        files,
        rest: Default::default(),
    };
    write_json(&manifest_path, &manifest)?;
    let mut out = facts::build_facts(run, &mut manifest)?;
    out += &format!(
        "gathered {} files, {} patch bytes, target={}",
        manifest.total_files, manifest.total_patch_bytes, manifest.target
    );
    if !manifest.warnings.is_empty() {
        out += &format!(" | WARNINGS: {}", manifest.warnings.join("; "));
    }
    out.push('\n');
    Ok(out)
}

/// How many files the diff `gather` would write touches: what `/review`
/// shows before consent. Refuses anywhere but the repository root.
pub fn touched_files(run: &ReviewRun, requested_target: &str, scope: &str) -> Result<usize> {
    git::admit_target(requested_target)?;
    git::require_root(run)?;
    let (target, _) = resolve_target(run, requested_target)?;
    git::admit_target(&target)?;
    let names = git::git(
        run,
        &diff_args(&target, &["--name-only", "-z"], &split_scope(scope)),
    )?;
    Ok(git::nul_records(&names)?.len())
}

/// Branches `/review` offers as a base: local and remote-tracking, without
/// the checked-out branch and the symbolic `<remote>/HEAD` entries.
pub fn base_branches(run: &ReviewRun) -> Result<Vec<String>> {
    let current = git::git_output(run, &["symbolic-ref", "--short", "-q", "HEAD"])?;
    // Exit 1 means a detached HEAD: no branch to leave out.
    let current = String::from_utf8_lossy(&current.stdout).trim().to_owned();
    let refs = git::git_text(
        run,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    Ok(refs
        .lines()
        .filter(|name| !name.is_empty() && *name != current && !name.ends_with("/HEAD"))
        .map(str::to_owned)
        .collect())
}

/// Scope as the recipe passes it: one string of space-separated pathspecs.
pub(crate) fn split_scope(scope: &str) -> Vec<String> {
    let parts: Vec<String> = scope.split_whitespace().map(str::to_owned).collect();
    if parts.is_empty() {
        vec![".".to_owned()]
    } else {
        parts
    }
}

fn diff_args(target: &str, extra: &[&str], pathspecs: &[String]) -> Vec<String> {
    let mut args: Vec<String> = DIFF.iter().map(|arg| (*arg).to_owned()).collect();
    args.extend(extra.iter().map(|arg| (*arg).to_owned()));
    args.push(target.to_owned());
    args.push("--".to_owned());
    args.extend(pathspecs.iter().cloned());
    args
}

fn count(binary: bool, field: &str) -> Option<u64> {
    if binary {
        return None;
    }
    let parsed = field.parse().ok();
    if parsed.is_none() {
        tracing::warn!(field, "unparseable git numstat count");
    }
    parsed
}

/// Pick the diff when the caller says `auto` (phase 0 of the review prompt).
fn resolve_target(run: &ReviewRun, requested: &str) -> Result<(String, Option<String>)> {
    if requested != "auto" {
        return Ok((requested.to_owned(), None));
    }
    let mut base = None;
    for candidate in ["@{upstream}", "main", "master"] {
        if git::is_commit(run, candidate)? {
            base = Some(candidate);
            break;
        }
    }
    let dirty = !git::git_text(run, &["status", "--porcelain", "--untracked-files=no"])?
        .trim()
        .is_empty();
    let Some(base) = base else {
        let target = if dirty { "HEAD" } else { "HEAD~1" };
        return Ok((
            target.to_owned(),
            Some(format!("no upstream/main: fell back to {target}")),
        ));
    };
    let range = format!("{base}...HEAD");
    let empty = git::git_text(run, &diff_args(&range, &["--name-only"], &[]))?
        .trim()
        .is_empty();
    if dirty || empty {
        let merge_base = git::git_text(run, &["merge-base", base, "HEAD"])?
            .trim()
            .to_owned();
        let short: String = merge_base.chars().take(12).collect();
        let warning = format!(
            "working tree included (dirty={}, empty_range={}); diffing against merge-base {short}",
            py_bool(dirty),
            py_bool(empty)
        );
        return Ok((merge_base, Some(warning)));
    }
    Ok((range, None))
}

/// The revision the diff ends at, or `None` when it ends at the working tree.
fn head_side(target: &str) -> Option<&str> {
    ["...", ".."].into_iter().find_map(|separator| {
        target
            .split_once(separator)
            .map(|(_, side)| if side.is_empty() { "HEAD" } else { side })
    })
}

/// Python's spelling of a bool, which the warnings have always used.
fn py_bool(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Now, in UTC, to the second: `2026-10-02T01:49:29+00:00`.
fn gathered_at() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_side_reads_ranges_and_defaults_an_open_end_to_head() {
        assert_eq!(head_side("main...feature"), Some("feature"));
        assert_eq!(head_side("a..b"), Some("b"));
        assert_eq!(head_side("main..."), Some("HEAD"));
        assert_eq!(head_side("HEAD~1"), None);
    }

    #[test]
    fn empty_scope_means_everything() {
        assert_eq!(split_scope(""), ["."]);
        assert_eq!(split_scope(" src  docs "), ["src", "docs"]);
    }

    #[test]
    fn timestamp_has_python_isoformat_shape() {
        let stamp = gathered_at();
        assert_eq!(stamp.len(), "2026-10-02T01:49:29+00:00".len());
        assert!(stamp.ends_with("+00:00"));
    }
}

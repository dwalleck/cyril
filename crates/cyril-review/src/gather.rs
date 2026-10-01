use crate::clock::ReviewClock;
use crate::facts;
use crate::git;
use crate::run::{
    Manifest, ManifestFile, ReviewRun, ensure_layout, facts_dir, manifest_path, package_version,
    read_manifest, write_binary, write_json, write_text,
};
use crate::{Result, ReviewError, StepOutput};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;

/// Gather a scoped Git diff and build its source facts.
pub fn gather(
    run: &ReviewRun,
    target: &str,
    scope: &str,
    clock: &dyn ReviewClock,
) -> Result<StepOutput> {
    let requested_scope = split_scope(scope);
    let manifest_file = manifest_path(run);
    if manifest_file.exists() {
        let mut prior = read_manifest(run)?;
        if prior.requested_target != target || prior.scope != requested_scope {
            return Err(ReviewError::ExistingRun {
                path: run.directory().to_path_buf(),
            });
        }
        let mut stdout = String::new();
        let symbols = facts_dir(run).join("symbols.json");
        if !symbols.exists() {
            stdout.push_str(&facts::build_facts(run, &mut prior)?);
        }
        stdout.push_str(&format!(
            "already gathered: {} files, target={}\n",
            prior.total_files, prior.target
        ));
        return Ok(StepOutput::from_text(stdout));
    }

    ensure_layout(run)?;
    let (resolved_target, target_warning) = git::resolve_target(run, target)?;
    let mut warnings = Vec::new();
    if let Some(note) = target_warning {
        warnings.push(note);
    }

    let matches_head = match git::head_side(&resolved_target) {
        Some(head) => {
            let head_args = vec!["rev-parse".to_owned(), head.to_owned() + "^{commit}"];
            let head_value = git::text(run, &head_args)?.trim().to_owned();
            let current_args = vec!["rev-parse".to_owned(), "HEAD".to_owned()];
            let current_value = git::text(run, &current_args)?.trim().to_owned();
            let matches = head_value == current_value;
            if !matches {
                warnings.push(format!(
                    "working tree (HEAD) is not at the diff's head ({head}); source files may not match the patches"
                ));
            }
            matches
        }
        None => true,
    };

    let full = git::bytes(run, &diff_args(&resolved_target, &requested_scope, &[]))?;
    if full.iter().all(u8::is_ascii_whitespace) {
        return Err(ReviewError::EmptyDiff {
            target: resolved_target,
            scope: requested_scope,
        });
    }
    write_binary(&run.directory().join("diff.patch"), &full)?;

    let status_args = diff_args(&resolved_target, &requested_scope, &["--name-status", "-z"]);
    let status_bytes = git::bytes(run, &status_args)?;
    let status = parse_status(&status_bytes);

    let numstat_args = diff_args(&resolved_target, &requested_scope, &["--numstat", "-z"]);
    let numstat_bytes = git::bytes(run, &numstat_args)?;
    let records = parse_numstat(&numstat_bytes)?;
    let mut files = Vec::with_capacity(records.len());
    let mut changed_files = String::new();
    for (index, (insertions, deletions, path, binary)) in records.into_iter().enumerate() {
        let path_arg = git::raw_path_arg(path)?;
        let patch_args = diff_args(&resolved_target, std::slice::from_ref(&path_arg), &[]);
        let patch = git::bytes(run, &patch_args)?;
        let patch_rel = format!("patches/{:03}.patch", index + 1);
        write_binary(&run.directory().join(&patch_rel), &patch)?;
        let (display_path, raw_path_bytes) = match String::from_utf8_lossy(path) {
            Cow::Borrowed(display_path) => (display_path.to_owned(), None),
            Cow::Owned(display_path) => (display_path, Some(path.to_vec())),
        };
        changed_files.push_str(&display_path);
        changed_files.push('\n');

        let status_value =
            String::from_utf8_lossy(status.get(path).copied().unwrap_or(b"?")).into_owned();
        files.push(ManifestFile {
            index: (index + 1) as u64,
            path: display_path,
            raw_path_bytes,
            status: status_value,
            insertions: (!binary).then_some(insertions),
            deletions: (!binary).then_some(deletions),
            binary,
            patch: patch_rel,
            patch_bytes: patch.len() as u64,
            metadata: Default::default(),
        });
    }
    let total_files = files.len() as u64;
    write_text(&run.directory().join("changed-files.txt"), &changed_files)?;

    let head_args = vec!["rev-parse".to_owned(), "HEAD".to_owned()];
    let head = git::text(run, &head_args)?.trim().to_owned();
    let gathered_at = clock.gathered_at_utc()?;
    let mut manifest_value = Manifest {
        requested_target: target.to_owned(),
        target: resolved_target,
        scope: requested_scope,
        head,
        worktree_matches_diff_head: matches_head,
        gathered_at,
        crtool_version: package_version().to_owned(),
        total_files,
        total_patch_bytes: full.len() as u64,
        warnings,
        files,
        metadata: Default::default(),
    };
    write_json(&manifest_file, &manifest_value)?;

    let facts_line = facts::build_facts(run, &mut manifest_value)?;
    let warning_suffix = if manifest_value.warnings.is_empty() {
        String::new()
    } else {
        format!(" | WARNINGS: {}", manifest_value.warnings.join("; "))
    };
    let gathered_line = format!(
        "gathered {total_files} files, {} patch bytes, target={}{warning_suffix}\n",
        full.len(),
        manifest_value.target,
    );
    Ok(StepOutput::from_text(facts_line + &gathered_line))
}

pub(crate) fn split_scope(scope: &str) -> Vec<String> {
    let mut parts = scope
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts.push(".".to_owned());
    }
    parts
}

pub(crate) fn diff_args<'a, S: AsRef<OsStr>>(
    target: &'a str,
    scope: &'a [S],
    options: &[&'a str],
) -> Vec<&'a OsStr> {
    let mut args = Vec::with_capacity(4 + options.len() + scope.len());
    args.push(OsStr::new("diff"));
    args.push(OsStr::new("--no-renames"));
    args.extend(options.iter().map(|option| OsStr::new(*option)));
    args.push(OsStr::new(target));
    args.push(OsStr::new("--"));
    args.extend(scope.iter().map(AsRef::as_ref));
    args
}

fn parse_status(bytes: &[u8]) -> HashMap<&[u8], &[u8]> {
    let mut status = HashMap::new();
    let mut fields = bytes.split(|byte| *byte == 0);
    while let Some(kind) = fields.next() {
        if kind.is_empty() {
            break;
        }
        let Some(path) = fields.next() else {
            break;
        };
        status.insert(path, kind);
    }
    status
}

type NumstatRecord<'a> = (u64, u64, &'a [u8], bool);

fn parse_numstat(bytes: &[u8]) -> Result<Vec<NumstatRecord<'_>>> {
    let mut records = Vec::new();
    for record in bytes.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let insertions = fields.next().ok_or_else(|| ReviewError::GitFailure {
            args: "diff --no-renames --numstat -z".to_owned(),
            message: "missing insertion count".to_owned(),
        })?;
        let deletions = fields.next().ok_or_else(|| ReviewError::GitFailure {
            args: "diff --no-renames --numstat -z".to_owned(),
            message: "missing deletion count".to_owned(),
        })?;
        let path = fields.next().ok_or_else(|| ReviewError::GitFailure {
            args: "diff --no-renames --numstat -z".to_owned(),
            message: "missing path".to_owned(),
        })?;
        let binary = insertions == b"-";
        let (insertions, deletions) = if binary {
            (0, 0)
        } else {
            let ins = String::from_utf8_lossy(insertions)
                .parse::<u64>()
                .map_err(|_| ReviewError::GitFailure {
                    args: "diff --no-renames --numstat -z".to_owned(),
                    message: format!(
                        "invalid insertion count {:?}",
                        String::from_utf8_lossy(insertions)
                    ),
                })?;
            let del = String::from_utf8_lossy(deletions)
                .parse::<u64>()
                .map_err(|_| ReviewError::GitFailure {
                    args: "diff --no-renames --numstat -z".to_owned(),
                    message: format!(
                        "invalid deletion count {:?}",
                        String::from_utf8_lossy(deletions)
                    ),
                })?;
            (ins, del)
        };
        records.push((insertions, deletions, path, binary));
    }
    Ok(records)
}

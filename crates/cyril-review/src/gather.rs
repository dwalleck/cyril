use crate::clock::ReviewClock;
use crate::facts;
use crate::git;
use crate::run::{
    Manifest, ManifestFile, ReviewRun, ensure_layout, facts_dir, manifest_path, package_version,
    read_manifest, write_binary, write_json, write_text,
};
use crate::{Result, ReviewError, StepOutput};
use std::borrow::Cow;

/// Gather a scoped Git diff and build its source facts.
pub fn gather(
    run: &ReviewRun,
    target: &str,
    scope: &str,
    clock: &dyn ReviewClock,
) -> Result<StepOutput> {
    git::admit_target(target)?;
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
    let repo = git::repository(run)?;
    let (resolved_target, target_warning) = git::resolve_target(&repo, target)?;
    let head_id = git::commit_id(&repo, "HEAD")?;
    let mut warnings = Vec::new();
    if let Some(note) = target_warning {
        warnings.push(note);
    }

    let matches_head = match git::head_side(&resolved_target) {
        Some(head) => {
            let matches = git::commit_id(&repo, head)? == head_id;
            if !matches {
                warnings.push(format!(
                    "working tree (HEAD) is not at the diff's head ({head}); source files may not match the patches"
                ));
            }
            matches
        }
        None => true,
    };

    let full = git::scoped_patch(run, &resolved_target, &requested_scope)?;
    if full.iter().all(u8::is_ascii_whitespace) {
        return Err(ReviewError::EmptyDiff {
            target: resolved_target,
            scope: requested_scope,
        });
    }
    write_binary(&run.directory().join("diff.patch"), &full)?;

    let diff = git::parse_patch(&full, &resolved_target)?;
    let mut files = Vec::with_capacity(diff.deltas().len());
    let mut changed_files = String::new();
    let mut deltas = diff.deltas().enumerate().peekable();
    while let Some((delta_index, delta)) = deltas.next() {
        let path = git::delta_path(&delta)?;
        let mut status = git::delta_status(delta.status())?;
        let mut patch = git::patch(&diff, delta_index)?;
        let (_, mut insertions, mut deletions) = patch
            .line_stats()
            .map_err(|error| git::operation("read per-file patch statistics", error))?;
        let mut binary = patch.delta().flags().is_binary();
        let first_bytes = patch
            .to_buf()
            .map_err(|error| git::operation("serialize per-file patch", error))?;
        let mut bytes = Cow::Borrowed(&first_bytes[..]);
        // Git represents a typechange as adjacent deletion/addition patches.
        if let Some((next_index, next)) = deltas.peek()
            && git::delta_path(next)? == path
        {
            if !matches!(
                (delta.status(), next.status()),
                (git2::Delta::Deleted, git2::Delta::Added)
                    | (git2::Delta::Added, git2::Delta::Deleted)
            ) {
                return Err(git::operation(
                    "combine file deltas",
                    "unexpected duplicate path",
                ));
            }
            let mut next_patch = git::patch(&diff, *next_index)?;
            let (_, added, removed) = next_patch
                .line_stats()
                .map_err(|error| git::operation("read typechange statistics", error))?;
            insertions += added;
            deletions += removed;
            binary |= next_patch.delta().flags().is_binary();
            let next_bytes = next_patch
                .to_buf()
                .map_err(|error| git::operation("serialize typechange patch", error))?;
            let mut combined = Vec::with_capacity(first_bytes.len() + next_bytes.len());
            combined.extend_from_slice(&first_bytes);
            combined.extend_from_slice(&next_bytes);
            bytes = Cow::Owned(combined);
            status = "T";
            deltas.next();
        }
        let index = files.len() + 1;
        let patch_rel = format!("patches/{index:03}.patch");
        write_binary(&run.directory().join(&patch_rel), &bytes)?;
        let (display_path, raw_path_bytes) = match String::from_utf8_lossy(path) {
            Cow::Borrowed(display_path) => (display_path.to_owned(), None),
            Cow::Owned(display_path) => (display_path, Some(path.to_vec())),
        };
        changed_files.push_str(&display_path);
        changed_files.push('\n');

        files.push(ManifestFile {
            index: index as u64,
            path: display_path,
            raw_path_bytes,
            status: status.to_owned(),
            insertions: (!binary).then_some(insertions as u64),
            deletions: (!binary).then_some(deletions as u64),
            binary,
            patch: patch_rel,
            patch_bytes: bytes.len() as u64,
            metadata: Default::default(),
        });
    }
    let total_files = files.len() as u64;
    write_text(&run.directory().join("changed-files.txt"), &changed_files)?;

    let gathered_at = clock.gathered_at_utc()?;
    let mut manifest_value = Manifest {
        requested_target: target.to_owned(),
        target: resolved_target,
        scope: requested_scope,
        head: head_id.to_string(),
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

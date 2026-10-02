//! `/review resume`: which persisted run to continue. A run directory is only
//! a candidate; whether its run can continue is the agent's persisted
//! workflow status, never the presence of `run.json` or a timestamp.

use super::launch::RUNS_DIR;
use super::run_record::{RunRecord, RunRecordError};
use crate::types::{WorkflowId, WorkflowRunStatus};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// One run directory and what its `run.json` says.
#[derive(Debug)]
pub struct Candidate {
    /// The directory's name, as `read_dir` reported it.
    pub name: String,
    pub dir: PathBuf,
    pub record: Result<RunRecord, RunRecordError>,
}

/// The run to continue.
#[derive(Debug)]
pub struct Chosen {
    /// The run directory's name.
    pub name: String,
    pub dir: PathBuf,
    pub record: RunRecord,
    pub workflow_id: WorkflowId,
    pub status: WorkflowRunStatus,
    /// Newer directories passed over because their `run.json` could not be
    /// read: said aloud, never treated as finished runs.
    pub unreadable: Vec<String>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[error("review resume: {0}")]
pub struct ResumeError(String);

fn refuse(message: impl Into<String>) -> ResumeError {
    ResumeError(message.into())
}

/// Every run directory under `<workspace>/.code-review`, newest first (the
/// directory names start with their UTC creation time).
pub fn scan(workspace: &Path) -> io::Result<Vec<Candidate>> {
    let runs = workspace.join(RUNS_DIR);
    let entries = match fs::read_dir(&runs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push((
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            ));
        }
    }
    dirs.sort();
    dirs.reverse();
    Ok(dirs
        .into_iter()
        .map(|(name, dir)| Candidate {
            name,
            record: RunRecord::read(&dir),
            dir,
        })
        .collect())
}

/// Pick the run to continue. `selector` is a run directory name or path, or
/// a workflow id; without one, the newest run the agent reports as failed or
/// paused. `statuses` is the agent's persisted status of every run it knows.
pub fn choose(
    candidates: Vec<Candidate>,
    selector: Option<&str>,
    statuses: &HashMap<WorkflowId, WorkflowRunStatus>,
) -> Result<Chosen, ResumeError> {
    match selector {
        Some(selector) => {
            let candidate = candidates
                .into_iter()
                .find(|candidate| matches(candidate, selector))
                .ok_or_else(|| refuse(format!("no run under {RUNS_DIR}/ matches {selector:?}")))?;
            let record = candidate
                .record
                .map_err(|error| refuse(error.to_string()))?;
            let workflow_id = workflow_id(&record)?;
            let status = resumable(&workflow_id, statuses.get(&workflow_id))?;
            Ok(Chosen {
                name: candidate.name,
                dir: candidate.dir,
                record,
                workflow_id,
                status,
                unreadable: Vec::new(),
            })
        }
        None => {
            let mut unreadable = Vec::new();
            for candidate in candidates {
                let name = candidate.name.clone();
                let record = match candidate.record {
                    Ok(record) => record,
                    // A directory without run.json never got a workflow.
                    Err(RunRecordError::Missing { .. }) => continue,
                    Err(_) => {
                        unreadable.push(name);
                        continue;
                    }
                };
                let Ok(workflow_id) = workflow_id(&record) else {
                    unreadable.push(name);
                    continue;
                };
                if let Some(status @ (WorkflowRunStatus::Failed | WorkflowRunStatus::Paused)) =
                    statuses.get(&workflow_id).copied()
                {
                    return Ok(Chosen {
                        name,
                        dir: candidate.dir,
                        record,
                        workflow_id,
                        status,
                        unreadable,
                    });
                }
            }
            let mut message = format!("no failed or paused review run under {RUNS_DIR}/");
            if !unreadable.is_empty() {
                message.push_str(&format!(
                    " (unreadable run.json in {})",
                    unreadable.join(", ")
                ));
            }
            Err(refuse(message))
        }
    }
}

/// A run continues only with the cyril that started it: its steps call the
/// stored crtool prefix, and its facts were written by that version.
pub fn compatible(
    record: &RunRecord,
    crtool_prefix: &str,
    cyril_version: &str,
) -> Result<(), ResumeError> {
    if record.crtool_prefix != crtool_prefix {
        return Err(refuse(format!(
            "run was started by a different cyril binary ({}); start a new /review",
            record.crtool_prefix
        )));
    }
    if record.cyril_version != cyril_version {
        return Err(refuse(format!(
            "run was started by cyril {} and this is cyril {cyril_version}; start a new /review",
            record.cyril_version
        )));
    }
    Ok(())
}

/// A selector is a workflow id, a run directory name, or a path to one in
/// any ordinary spelling (`./`, a trailing separator, absolute).
fn matches(candidate: &Candidate, selector: &str) -> bool {
    let path: PathBuf = Path::new(selector)
        .components()
        .filter(|component| !matches!(component, std::path::Component::CurDir))
        .collect();
    candidate.name == selector
        || path.as_os_str() == candidate.name.as_str()
        || (path.components().count() > 1 && candidate.dir.ends_with(&path))
        || candidate
            .record
            .as_ref()
            .is_ok_and(|record| record.workflow_id == selector)
}

fn workflow_id(record: &RunRecord) -> Result<WorkflowId, ResumeError> {
    WorkflowId::try_from(record.workflow_id.clone())
        .map_err(|error| refuse(format!("run.json names an invalid workflow id: {error}")))
}

fn resumable(
    workflow_id: &WorkflowId,
    status: Option<&WorkflowRunStatus>,
) -> Result<WorkflowRunStatus, ResumeError> {
    match status {
        Some(status @ (WorkflowRunStatus::Failed | WorkflowRunStatus::Paused)) => Ok(*status),
        Some(WorkflowRunStatus::Running) => Err(refuse(format!(
            "{workflow_id} is still running — /workflow status {workflow_id}"
        ))),
        Some(status) => Err(refuse(format!(
            "{workflow_id} is {status}; there is nothing to resume"
        ))),
        None => Err(refuse(format!("the agent does not know run {workflow_id}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> RunRecord {
        RunRecord {
            workflow_id: id.to_owned(),
            crtool_prefix: "\"/bin/cyril\" crtool".to_owned(),
            cyril_version: "0.2.0".to_owned(),
            target: "auto".to_owned(),
            scope: vec![".".to_owned()],
        }
    }

    fn id(text: &str) -> WorkflowId {
        WorkflowId::try_from(text.to_owned()).expect("valid id")
    }

    /// Runs newest first: `c` (completed), `corrupt`, `b` (failed), `a`
    /// (paused), plus a directory that never got a workflow.
    fn workspace() -> (tempfile::TempDir, HashMap<WorkflowId, WorkflowRunStatus>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let runs = dir.path().join(RUNS_DIR);
        for (name, run) in [
            ("20261001-090000-aaaa", Some("wf_a")),
            ("20261001-100000-bbbb", Some("wf_b")),
            ("20261001-110000-dddd", None),
            ("20261001-120000-cccc", Some("wf_c")),
        ] {
            fs::create_dir_all(runs.join(name)).expect("run dir");
            if let Some(run) = run {
                record(run).write(&runs.join(name)).expect("record");
            }
        }
        fs::create_dir_all(runs.join("20261001-115000-eeee")).expect("corrupt dir");
        fs::write(runs.join("20261001-115000-eeee/run.json"), "{").expect("corrupt");
        let statuses = HashMap::from([
            (id("wf_a"), WorkflowRunStatus::Paused),
            (id("wf_b"), WorkflowRunStatus::Failed),
            (id("wf_c"), WorkflowRunStatus::Completed),
        ]);
        (dir, statuses)
    }

    #[test]
    fn no_selector_takes_the_newest_failed_or_paused_run() {
        let (dir, statuses) = workspace();
        let chosen = choose(scan(dir.path()).expect("scan"), None, &statuses).expect("a run");
        assert_eq!(chosen.workflow_id, id("wf_b"));
        assert_eq!(chosen.status, WorkflowRunStatus::Failed);
        assert_eq!(chosen.unreadable, ["20261001-115000-eeee"]);
    }

    #[test]
    fn a_selector_names_a_directory_path_or_workflow() {
        let (dir, statuses) = workspace();
        for selector in [
            "20261001-090000-aaaa",
            "20261001-090000-aaaa/",
            "wf_a",
            ".code-review/20261001-090000-aaaa",
            "./.code-review/20261001-090000-aaaa/",
        ] {
            let chosen = choose(scan(dir.path()).expect("scan"), Some(selector), &statuses)
                .unwrap_or_else(|error| panic!("{selector}: {error}"));
            assert_eq!(chosen.workflow_id, id("wf_a"), "{selector}");
        }
    }

    #[test]
    fn refusals_name_the_reason() {
        let (dir, statuses) = workspace();
        let refusal = |selector: &str| {
            choose(scan(dir.path()).expect("scan"), Some(selector), &statuses)
                .expect_err(selector)
                .to_string()
        };
        assert_eq!(
            refusal("wf_c"),
            "review resume: wf_c is completed; there is nothing to resume"
        );
        assert!(refusal("20261001-110000-dddd").contains("is missing"));
        assert!(refusal("20261001-115000-eeee").contains("is corrupt"));
        assert_eq!(
            refusal("nope"),
            "review resume: no run under .code-review/ matches \"nope\""
        );
        let empty = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            choose(scan(empty.path()).expect("scan"), None, &statuses)
                .expect_err("nothing")
                .to_string(),
            "review resume: no failed or paused review run under .code-review/"
        );
    }

    #[test]
    fn only_the_same_cyril_continues_a_run() {
        let stored = record("wf_a");
        assert!(compatible(&stored, "\"/bin/cyril\" crtool", "0.2.0").is_ok());
        assert_eq!(
            compatible(&stored, "\"/opt/cyril\" crtool", "0.2.0")
                .expect_err("moved")
                .to_string(),
            "review resume: run was started by a different cyril binary (\"/bin/cyril\" crtool); start a new /review"
        );
        assert_eq!(
            compatible(&stored, "\"/bin/cyril\" crtool", "0.3.0")
                .expect_err("upgraded")
                .to_string(),
            "review resume: run was started by cyril 0.2.0 and this is cyril 0.3.0; start a new /review"
        );
    }
}

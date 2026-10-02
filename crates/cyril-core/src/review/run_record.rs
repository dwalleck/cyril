//! `<run_dir>/run.json`: the identity of a review run, written once the
//! workflow exists and before it is invoked, read again by `/review resume`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const RUN_RECORD: &str = "run.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunRecord {
    pub workflow_id: String,
    pub crtool_prefix: String,
    pub cyril_version: String,
    pub target: String,
    pub scope: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RunRecordError {
    #[error("{} is missing", .path.display())]
    Missing { path: PathBuf },
    #[error("{} is corrupt: {source}", .path.display())]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot access {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl RunRecord {
    /// Write `run.json` atomically (temp file + rename).
    pub fn write(&self, run_dir: &Path) -> Result<(), RunRecordError> {
        let path = run_dir.join(RUN_RECORD);
        let io_error = |source| RunRecordError::Io {
            path: path.clone(),
            source,
        };
        let mut text =
            serde_json::to_string_pretty(self).map_err(|source| RunRecordError::Corrupt {
                path: path.clone(),
                source,
            })?;
        text.push('\n');
        let temporary = run_dir.join(format!("{RUN_RECORD}.tmp"));
        fs::write(&temporary, text).map_err(io_error)?;
        fs::rename(&temporary, &path).map_err(io_error)
    }

    pub fn read(run_dir: &Path) -> Result<Self, RunRecordError> {
        let path = run_dir.join(RUN_RECORD);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(RunRecordError::Missing { path });
            }
            Err(source) => return Err(RunRecordError::Io { path, source }),
        };
        serde_json::from_slice(&bytes).map_err(|source| RunRecordError::Corrupt { path, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_round_trips_and_missing_differs_from_corrupt() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(matches!(
            RunRecord::read(dir.path()),
            Err(RunRecordError::Missing { .. })
        ));
        let record = RunRecord {
            workflow_id: "wf-1".to_owned(),
            crtool_prefix: "\"/bin/cyril\" crtool".to_owned(),
            cyril_version: "0.2.0".to_owned(),
            target: "main...HEAD".to_owned(),
            scope: vec![".".to_owned()],
        };
        record.write(dir.path()).expect("write");
        assert_eq!(RunRecord::read(dir.path()).expect("read"), record);
        fs::write(dir.path().join(RUN_RECORD), b"{\"workflow_id\": ").expect("corrupt it");
        assert!(matches!(
            RunRecord::read(dir.path()),
            Err(RunRecordError::Corrupt { .. })
        ));
    }
}

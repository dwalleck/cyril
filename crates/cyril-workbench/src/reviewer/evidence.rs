use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use super::{ReviewError, ReviewInput, ReviewLimits};
use serde::Serialize;
use tokio::sync::oneshot;

/// Keeps evidence alive even when the caller's executor drops the drive future.
pub(super) struct EvidenceCleanup {
    tree: Option<EvidenceTree>,
    pub completion: oneshot::Receiver<()>,
    completed: Option<bool>,
}

impl EvidenceCleanup {
    pub fn new(tree: EvidenceTree, completion: oneshot::Receiver<()>) -> Self {
        Self {
            tree: Some(tree),
            completion,
            completed: None,
        }
    }

    pub fn resolved(&mut self, completed: bool) {
        self.completed = Some(completed);
    }

    pub async fn close(mut self) -> Result<(), ()> {
        if self.completed != Some(true) {
            return Err(());
        }
        let tree = self.tree.take().ok_or_else(|| {
            tracing::warn!("review evidence owner missing during cleanup");
        })?;
        let root = tree.root().to_owned();
        match tokio::task::spawn_blocking(move || {
            let root = tree.root().to_owned();
            tree.close().map_err(|error| {
                // Log inside the blocking owner even if its async waiter is gone.
                tracing::warn!(root = %root.display(), kind = ?error.kind(), "review tree cleanup failed");
            })
        }).await {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!(root = %root.display(), "review tree cleanup task failed");
                Err(())
            }
        }
    }
}

impl Drop for EvidenceCleanup {
    fn drop(&mut self) {
        let Some(tree) = self.tree.take() else { return };
        // Persist BEFORE spawning: a failed thread spawn drops its closure.
        // Neither that drop nor a lost completion may remove a live child's cwd.
        let root = tree.root.keep();
        if self.completed == Some(false) {
            tracing::warn!(root = %root.display(), "core completion lost; retaining review tree");
            return;
        }
        let retained = root.clone();
        let (_, empty) = oneshot::channel();
        let completion = std::mem::replace(&mut self.completion, empty);
        let completed = self.completed;
        if let Err(error) = std::thread::Builder::new()
            .name("review-evidence-cleanup".into())
            .spawn(move || {
                if completed != Some(true) && completion.blocking_recv().is_err() {
                    tracing::warn!(root = %root.display(), "core completion lost; retaining review tree");
                    return;
                }
                if let Err(error) = fs::remove_dir_all(&root) {
                    tracing::warn!(root = %root.display(), kind = ?error.kind(), "review tree cleanup failed");
                }
            })
        {
            tracing::warn!(root = %retained.display(), kind = ?error.kind(), "cleanup waiter unavailable; retaining review tree");
        }
    }
}

pub(super) struct EvidenceTree {
    root: tempfile::TempDir,
    pub cwd: PathBuf,
    pub evidence: PathBuf,
}

#[derive(Serialize)]
struct ManifestEntry<'a> {
    file: String,
    source_label: &'a str,
    bytes: usize,
}

pub(super) fn validate(input: &ReviewInput, limits: &ReviewLimits) -> Result<(), ReviewError> {
    if input.documents.is_empty() || input.documents.len() > limits.documents {
        return Err(ReviewError::InvalidInput(
            "document count outside configured bounds",
        ));
    }
    if input.instruction.trim().is_empty() || input.instruction.len() > limits.instruction_bytes {
        return Err(ReviewError::InvalidInput("instruction empty or too large"));
    }
    let mut instruction_bytes = input.instruction.len();
    for instruction in &input.follow_up_instructions {
        instruction_bytes = instruction_bytes
            .checked_add(instruction.len())
            .ok_or(ReviewError::InvalidInput("instruction byte count overflow"))?;
        if instruction.trim().is_empty() || instruction_bytes > limits.instruction_bytes {
            return Err(ReviewError::InvalidInput("continuation empty or too large"));
        }
    }
    let mut bytes = 0usize;
    for document in &input.documents {
        if document.source_label.trim().is_empty()
            || document.source_label.len() > limits.label_bytes
        {
            return Err(ReviewError::InvalidInput("source label empty or too large"));
        }
        bytes = bytes
            .checked_add(document.text.len())
            .ok_or(ReviewError::InvalidInput("evidence byte count overflow"))?;
        if bytes > limits.evidence_bytes {
            return Err(ReviewError::InvalidInput("evidence byte limit exceeded"));
        }
    }
    Ok(())
}

impl EvidenceTree {
    pub fn stage(parent: &Path, input: &ReviewInput) -> Result<Self, ReviewError> {
        // TempDir creates a new unpredictable private directory. The trusted parent
        // is not an evidence source; every child name here is generated internally.
        let root = tempfile::Builder::new()
            .prefix("cyril-review-")
            .tempdir_in(parent)?;
        let cwd = root.path().join("workspace");
        let evidence = cwd.join("evidence");
        fs::create_dir_all(&evidence)?;
        let mut manifest = Vec::with_capacity(input.documents.len());
        for (index, document) in input.documents.iter().enumerate() {
            let file = format!("document-{index:04}.txt");
            write_new(&evidence.join(&file), document.text.as_bytes())?;
            manifest.push(ManifestEntry {
                file,
                source_label: &document.source_label,
                bytes: document.text.len(),
            });
        }
        let manifest_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(evidence.join("manifest.json"))?;
        let mut writer = BufWriter::new(manifest_file);
        serde_json::to_writer(&mut writer, &manifest).map_err(|error| {
            ReviewError::Serialization {
                document: "evidence manifest",
                diagnostic: super::ReviewDiagnostic::new(error.to_string()),
            }
        })?;
        writer.flush()?;
        // Transient same-host evidence needs completed writes, not crash durability.
        Ok(Self {
            root,
            cwd,
            evidence,
        })
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn close(self) -> Result<(), std::io::Error> {
        self.root.close()
    }
}

pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ReviewError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn staged(parent: &Path) -> Result<EvidenceTree, ReviewError> {
        EvidenceTree::stage(
            parent,
            &ReviewInput {
                instruction: "inspect".into(),
                documents: vec![crate::reviewer::EvidenceDocument {
                    source_label: "source".into(),
                    text: "private".into(),
                }],
                follow_up_instructions: vec![],
            },
        )
    }

    #[test]
    fn unpolled_cleanup_guard_retains_tree_until_completion()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let tree = staged(parent.path())?;
        let root = tree.root().to_owned();
        let (completed, completion) = oneshot::channel();
        drop(EvidenceCleanup::new(tree, completion));
        assert!(root.join("workspace/evidence/manifest.json").is_file());
        completed
            .send(())
            .map_err(|_| "cleanup lost its completion receiver")?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while root.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.exists(), "cleanup did not follow completion");
        Ok(())
    }

    #[test]
    fn lost_core_completion_retains_evidence_instead_of_guessing_child_exit()
    -> Result<(), Box<dyn std::error::Error>> {
        let parent = tempfile::tempdir()?;
        let tree = staged(parent.path())?;
        let root = tree.root().to_owned();
        let (completed, completion) = oneshot::channel();
        let mut cleanup = EvidenceCleanup::new(tree, completion);
        drop(completed);
        cleanup.resolved(false);
        drop(cleanup);
        assert_eq!(
            fs::read_to_string(root.join("workspace/evidence/document-0000.txt"))?,
            "private"
        );
        Ok(())
    }
}

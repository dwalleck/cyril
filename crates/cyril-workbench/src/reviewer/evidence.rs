use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use super::{ReviewError, ReviewInput, ReviewLimits};
use serde::Serialize;

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
        serde_json::to_writer(&mut writer, &manifest)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
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
    file.sync_all()?;
    Ok(())
}

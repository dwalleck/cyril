use crate::{Result, ReviewError, io_error, json_error};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Manifest {
    pub(crate) requested_target: String,
    pub(crate) target: String,
    pub(crate) scope: Vec<String>,
    pub(crate) head: String,
    pub(crate) worktree_matches_diff_head: bool,
    pub(crate) gathered_at: String,
    pub(crate) crtool_version: String,
    pub(crate) total_files: u64,
    pub(crate) total_patch_bytes: u64,
    pub(crate) warnings: Vec<String>,
    pub(crate) files: Vec<ManifestFile>,
    #[serde(flatten)]
    pub(crate) metadata: Map<String, Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ManifestFile {
    pub(crate) index: u64,
    pub(crate) path: String,
    #[serde(
        rename = "_raw_path_bytes",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) raw_path_bytes: Option<Vec<u8>>,
    pub(crate) status: String,
    // A binary count may be null, but omission is invalid durable input.
    #[serde(deserialize_with = "Option::<u64>::deserialize")]
    pub(crate) insertions: Option<u64>,
    #[serde(deserialize_with = "Option::<u64>::deserialize")]
    pub(crate) deletions: Option<u64>,
    pub(crate) binary: bool,
    pub(crate) patch: String,
    pub(crate) patch_bytes: u64,
    #[serde(flatten)]
    pub(crate) metadata: Map<String, Value>,
}

impl ManifestFile {
    pub(crate) fn path_bytes(&self) -> &[u8] {
        self.raw_path_bytes
            .as_deref()
            .unwrap_or(self.path.as_bytes())
    }
}

/// Absolute lexical anchors for one review run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewRun {
    workspace: PathBuf,
    directory: PathBuf,
}

impl ReviewRun {
    pub fn new(workspace: impl Into<PathBuf>, directory: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self {
            workspace: lexical_absolute(workspace.into())?,
            directory: lexical_absolute(directory.into())?,
        })
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

pub(crate) fn lexical_absolute(path: PathBuf) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path
    } else {
        let cwd =
            env::current_dir().map_err(|source| io_error("read current directory", ".", source))?;
        cwd.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            component @ Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

pub(crate) fn package_version() -> &'static str {
    VERSION
}

pub(crate) fn manifest_path(run: &ReviewRun) -> PathBuf {
    run.directory.join("manifest.json")
}

pub(crate) fn facts_dir(run: &ReviewRun) -> PathBuf {
    run.directory.join("facts")
}

pub(crate) fn ensure_layout(run: &ReviewRun) -> Result<()> {
    fs::create_dir_all(&run.directory)
        .map_err(|source| io_error("create run directory", run.directory.as_path(), source))?;
    for name in [
        "patches",
        "candidates",
        "deduped",
        "queues",
        "verdicts",
        "facts",
    ] {
        let path = run.directory.join(name);
        fs::create_dir_all(&path)
            .map_err(|source| io_error("create run directory", path, source))?;
    }
    Ok(())
}

pub(crate) fn write_binary(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create output directory", parent, source))?;
    }
    fs::write(path, bytes).map_err(|source| io_error("write file", path, source))
}

pub(crate) fn read_binary(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| io_error("read file", path, source))
}

pub(crate) fn write_text(path: &Path, text: &str) -> Result<()> {
    write_binary(path, text.as_bytes())
}

pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut encoded =
        serde_json::to_string_pretty(value).map_err(|source| json_error(path, source))?;
    encoded.push('\n');
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create output directory", parent, source))?;
    }
    // Unique per writer: concurrent steps must never rename each other's temp file.
    static TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, encoded)
        .map_err(|source| io_error("write temporary JSON", temporary.as_path(), source))?;
    fs::rename(&temporary, path).map_err(|source| {
        if let Err(error) = fs::remove_file(&temporary) {
            tracing::warn!(path = %temporary.display(), %error, "removing orphaned temporary JSON failed");
        }
        io_error("replace JSON", path, source)
    })
}

/// Read-modify-write the manifest under an exclusive run lock, so a long step
/// (diagnostics) merges its keys into the current manifest instead of writing
/// back the copy it read at launch over keys other steps added meanwhile.
pub(crate) fn update_manifest<T>(
    run: &ReviewRun,
    change: impl FnOnce(&mut Manifest) -> Result<T>,
) -> Result<T> {
    let lock_path = run.directory.join("manifest.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|source| io_error("open manifest lock", lock_path.as_path(), source))?;
    lock.lock()
        .map_err(|source| io_error("lock manifest", lock_path.as_path(), source))?;
    let mut manifest = read_manifest(run)?;
    let value = change(&mut manifest)?;
    write_json(&manifest_path(run), &manifest)?;
    // Dropping the handle releases the lock.
    Ok(value)
}

pub(crate) fn read_json(path: &Path) -> Result<Value> {
    let bytes = read_binary(path)?;
    serde_json::from_slice(&bytes).map_err(|source| json_error(path, source))
}

pub(crate) fn read_manifest(run: &ReviewRun) -> Result<Manifest> {
    let path = manifest_path(run);
    let value = read_json(&path)?;
    validate_stamp(&value)?;
    let manifest = serde_json::from_value::<Manifest>(value).map_err(|source| {
        ReviewError::InvalidManifest {
            path: path.clone(),
            message: source.to_string(),
        }
    })?;
    validate_raw_paths(&manifest.files, &path)?;
    Ok(manifest)
}

pub(crate) fn facts_metadata<'a>(
    run: &ReviewRun,
    manifest: &'a mut Manifest,
) -> Result<&'a mut Map<String, Value>> {
    if !manifest.metadata.contains_key("facts") {
        manifest
            .metadata
            .insert("facts".to_owned(), Value::Object(Map::new()));
    }
    manifest
        .metadata
        .get_mut("facts")
        .and_then(|facts| {
            if facts.is_null() {
                *facts = Value::Object(Map::new());
            }
            facts.as_object_mut()
        })
        .ok_or_else(|| ReviewError::InvalidManifest {
            path: manifest_path(run),
            message: "facts is not an object".to_owned(),
        })
}

fn validate_raw_paths(files: &[ManifestFile], path: &Path) -> Result<()> {
    for file in files {
        let Some(raw_path_bytes) = file.raw_path_bytes.as_deref() else {
            continue;
        };
        if String::from_utf8_lossy(raw_path_bytes).as_ref() != file.path.as_str() {
            return Err(ReviewError::InvalidManifest {
                path: path.to_path_buf(),
                message: format!(
                    "files[{}]._raw_path_bytes does not decode to path {:?}",
                    file.index, file.path
                ),
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_stamp(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| ReviewError::InvalidManifest {
            path: PathBuf::from("manifest.json"),
            message: "top-level value is not an object".to_owned(),
        })?;
    let Some(stamp) = object.get("crtool_version") else {
        return Err(ReviewError::MissingStamp);
    };
    let Some(found) = stamp.as_str() else {
        return Err(ReviewError::StampMismatch {
            run: stamp.to_string(),
            current: VERSION.to_owned(),
        });
    };
    if found.is_empty() {
        return Err(ReviewError::MissingStamp);
    }
    if found != VERSION {
        return Err(ReviewError::StampMismatch {
            run: stamp.to_string(),
            current: VERSION.to_owned(),
        });
    }
    Ok(())
}

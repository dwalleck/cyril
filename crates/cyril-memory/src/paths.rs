//! Canonical, private filesystem locations owned by the memory runtime.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::permissions;

const MEMORY_STORE_NAME: &str = "memory.sqlite3";
const KNOWLEDGE_STORE_NAME: &str = "knowledge.sqlite3";
const LOCK_NAME: &str = "memory-runtime.lock";

/// A failure while resolving or preparing the private memory data root.
#[derive(Debug, Error)]
pub enum PathError {
    /// An explicitly supplied override was not absolute.
    #[error("memory data-root override is not absolute: {path}")]
    RelativeOverride { path: PathBuf },
    /// A required environment variable was not present.
    #[error("required environment variable `{variable}` is missing")]
    MissingEnvironment { variable: String },
    /// A required environment variable contained a relative path.
    #[error("environment variable `{variable}` is relative: {path}")]
    RelativeEnvironment { variable: String, path: PathBuf },
    /// Metadata inspection failed before the root could be prepared.
    #[error("could not inspect memory data root `{path}`: {source}")]
    Inspect { path: PathBuf, source: io::Error },
    /// The requested root is a symlink.
    #[error("memory data root is a symlink: {path}")]
    Symlink { path: PathBuf },
    /// The requested root exists but is not a directory.
    #[error("memory data root is not a directory: {path}")]
    NotDirectory { path: PathBuf },
    /// Creating the requested root failed.
    #[error("could not create memory data root `{path}`: {source}")]
    Create { path: PathBuf, source: io::Error },
    /// Applying private permissions to the requested root failed.
    #[error("could not protect memory data root `{path}`: {source}")]
    Tighten { path: PathBuf, source: io::Error },
    /// Canonicalizing the prepared root failed.
    #[error("could not canonicalize memory data root `{path}`: {source}")]
    Canonicalize { path: PathBuf, source: io::Error },
    /// The host platform has no implemented private-directory semantics.
    #[error("private memory-directory paths are unsupported on this platform")]
    UnsupportedPlatform,
}

/// Canonical filesystem paths used by the memory runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryPaths {
    data_root: PathBuf,
    memory_store_path: PathBuf,
    knowledge_store_path: PathBuf,
    lock_path: PathBuf,
}

impl MemoryPaths {
    /// Resolve, create, protect, and canonicalize the memory data root.
    ///
    /// An override must be absolute. With no override, the platform's data
    /// directory environment variables are resolved without ever falling back
    /// to the process working directory.
    pub fn prepare(data_root: Option<&Path>) -> Result<Self, PathError> {
        let root = match data_root {
            Some(path) if !path.is_absolute() => {
                return Err(PathError::RelativeOverride {
                    path: path.to_path_buf(),
                });
            }
            Some(path) => path.to_path_buf(),
            None => resolve_default_from(|variable| env::var_os(variable))?,
        };

        prepare_root(root)
    }

    /// Return the canonical private data root.
    pub fn data_root(&self) -> &Path {
        &self.data_root
    }

    /// Return the canonical path of the memory SQLite store.
    pub fn memory_store_path(&self) -> &Path {
        &self.memory_store_path
    }

    /// Return the canonical path of the knowledge SQLite store.
    pub fn knowledge_store_path(&self) -> &Path {
        &self.knowledge_store_path
    }

    /// Return the canonical path of the runtime ownership lock.
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

fn prepare_root(root: PathBuf) -> Result<MemoryPaths, PathError> {
    match fs::symlink_metadata(&root) {
        Ok(metadata) => validate_root_metadata(&root, &metadata)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(&root).map_err(|source| PathError::Create {
                path: root.clone(),
                source,
            })?;
            let metadata = fs::symlink_metadata(&root).map_err(|source| PathError::Inspect {
                path: root.clone(),
                source,
            })?;
            validate_root_metadata(&root, &metadata)?;
        }
        Err(source) => {
            return Err(PathError::Inspect { path: root, source });
        }
    }

    permissions::tighten_directory(&root).map_err(|source| PathError::Tighten {
        path: root.clone(),
        source,
    })?;

    let canonical_root = root
        .canonicalize()
        .map_err(|source| PathError::Canonicalize { path: root, source })?;

    Ok(MemoryPaths {
        memory_store_path: canonical_root.join(MEMORY_STORE_NAME),
        knowledge_store_path: canonical_root.join(KNOWLEDGE_STORE_NAME),
        lock_path: canonical_root.join(LOCK_NAME),
        data_root: canonical_root,
    })
}

fn validate_root_metadata(path: &Path, metadata: &fs::Metadata) -> Result<(), PathError> {
    if metadata.file_type().is_symlink() {
        return Err(PathError::Symlink {
            path: path.to_path_buf(),
        });
    }
    if !metadata.is_dir() {
        return Err(PathError::NotDirectory {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

/// Resolve a platform default from an injected environment lookup.
///
/// Keeping this lookup separate from `std::env` lets tests exercise missing,
/// relative, Unicode, and spaced environment values without mutating process
/// global state.
fn resolve_default_from<F>(lookup: F) -> Result<PathBuf, PathError>
where
    F: Fn(&str) -> Option<OsString>,
{
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg_data_home) = lookup("XDG_DATA_HOME") {
            let xdg_path = PathBuf::from(xdg_data_home);
            if xdg_path.is_absolute() {
                return Ok(xdg_path.join("cyril"));
            }
        }
        let home = required_absolute_environment(&lookup, "HOME")?;
        Ok(home.join(".local").join("share").join("cyril"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = required_absolute_environment(&lookup, "HOME")?;
        Ok(home
            .join("Library")
            .join("Application Support")
            .join("Cyril"))
    }

    #[cfg(target_os = "windows")]
    {
        let local_app_data = required_absolute_environment(&lookup, "LOCALAPPDATA")?;
        Ok(local_app_data.join("Cyril"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = lookup;
        Err(PathError::UnsupportedPlatform)
    }
}

fn required_absolute_environment<F>(lookup: &F, variable: &str) -> Result<PathBuf, PathError>
where
    F: Fn(&str) -> Option<OsString>,
{
    let value = lookup(variable).ok_or_else(|| PathError::MissingEnvironment {
        variable: variable.to_owned(),
    })?;
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(PathError::RelativeEnvironment {
            variable: variable.to_owned(),
            path,
        });
    }
    Ok(path)
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::{MemoryPaths, PathError, resolve_default_from};
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn env(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let values: HashMap<_, _> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), OsString::from(*value)))
            .collect();
        move |name| values.get(name).cloned()
    }

    #[test]
    fn resolves_override_and_derives_children_from_canonical_root() {
        let parent = tempfile::tempdir().expect("tempdir");
        let requested = parent.path().join("spaced ü root");
        let paths = MemoryPaths::prepare(Some(&requested)).expect("prepare");
        let canonical = requested.canonicalize().expect("canonical");

        assert_eq!(paths.data_root(), canonical.as_path());
        assert_eq!(paths.memory_store_path(), canonical.join("memory.sqlite3"));
        assert_eq!(
            paths.knowledge_store_path(),
            canonical.join("knowledge.sqlite3")
        );
        assert_eq!(paths.lock_path(), canonical.join("memory-runtime.lock"));
    }

    #[test]
    fn rejects_relative_override_without_touching_filesystem() {
        let error = MemoryPaths::prepare(Some(Path::new("relative memory")))
            .expect_err("relative override");
        assert!(
            matches!(error, PathError::RelativeOverride { path } if path.as_path() == Path::new("relative memory"))
        );
    }

    #[test]
    fn rejects_existing_file() {
        let parent = tempfile::tempdir().expect("tempdir");
        let file = parent.path().join("not a directory");
        fs::write(&file, b"file").expect("file");

        let error = MemoryPaths::prepare(Some(&file)).expect_err("file must be rejected");
        assert!(matches!(error, PathError::NotDirectory { path } if path == file));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_existing_symlink() {
        let parent = tempfile::tempdir().expect("tempdir");
        let target = parent.path().join("target");
        let link = parent.path().join("link");
        fs::create_dir(&target).expect("target");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let error = MemoryPaths::prepare(Some(&link)).expect_err("symlink must be rejected");
        assert!(matches!(error, PathError::Symlink { path } if path == link));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_linux_default_from_absolute_xdg() {
        let path = resolve_default_from(env(&[
            ("XDG_DATA_HOME", "/tmp/ü data"),
            ("HOME", "relative"),
        ]))
        .expect("absolute XDG");
        assert_eq!(path, PathBuf::from("/tmp/ü data/cyril"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn resolves_linux_default_from_home_when_xdg_missing_or_relative() {
        let missing = resolve_default_from(env(&[("HOME", "/home/space user")])).expect("HOME");
        assert_eq!(
            missing,
            PathBuf::from("/home/space user/.local/share/cyril")
        );

        let relative =
            resolve_default_from(env(&[("XDG_DATA_HOME", "relative"), ("HOME", "/home/u")]))
                .expect("HOME fallback");
        assert_eq!(relative, PathBuf::from("/home/u/.local/share/cyril"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reports_missing_and_relative_home() {
        let missing = resolve_default_from(env(&[])).expect_err("missing HOME");
        assert!(
            matches!(missing, PathError::MissingEnvironment { variable } if variable == "HOME")
        );

        let relative =
            resolve_default_from(env(&[("HOME", "relative")])).expect_err("relative HOME");
        assert!(
            matches!(relative, PathError::RelativeEnvironment { variable, path } if variable == "HOME" && path.as_path() == Path::new("relative"))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolves_macos_default_without_process_environment_mutation() {
        let path = resolve_default_from(env(&[("HOME", "/Users/space user")])).expect("HOME");
        assert_eq!(
            path,
            PathBuf::from("/Users/space user/Library/Application Support/Cyril")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn resolves_windows_default_without_process_environment_mutation() {
        let path = resolve_default_from(env(&[(
            "LOCALAPPDATA",
            r"C:\Users\space user\AppData\Local",
        )]))
        .expect("LOCALAPPDATA");
        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\space user\AppData\Local\Cyril")
        );
    }
}

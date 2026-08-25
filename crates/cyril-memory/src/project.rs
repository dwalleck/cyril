use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Opaque stable identity for one canonical local project.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectId([u8; 32]);

impl ProjectId {
    pub(crate) const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ProjectId({self})")
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

/// One canonical workspace bound to its stable project identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectScope {
    project_id: ProjectId,
    display_path: PathBuf,
}

impl ProjectScope {
    /// Resolve a workspace once without consulting process cwd.
    pub fn resolve(workspace: &Path) -> Result<Self, ProjectError> {
        let display_path =
            workspace
                .canonicalize()
                .map_err(|source| ProjectError::Canonicalize {
                    path: workspace.to_path_buf(),
                    source,
                })?;
        let identity_path =
            find_git_common_dir(&display_path)?.unwrap_or_else(|| display_path.clone());
        let project_id = ProjectId::from_bytes(hash_identity(&identity_path));
        Ok(Self {
            project_id,
            display_path,
        })
    }
    pub(crate) fn from_wire(project_id: &str, display_path: &str) -> Result<Self, ProjectError> {
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(project_id, &mut bytes)
            .map_err(|_| ProjectError::InvalidBoundScope)?;
        let display_path = PathBuf::from(display_path);
        if display_path.as_os_str().is_empty()
            || !display_path.is_absolute()
            || display_path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Err(ProjectError::InvalidBoundScope);
        }
        Ok(Self {
            project_id: ProjectId::from_bytes(bytes),
            display_path,
        })
    }

    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    pub fn display_path(&self) -> &Path {
        &self.display_path
    }
}

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("could not canonicalize project workspace {path}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not inspect Git metadata at {path}")]
    InspectGit {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Git metadata file {path} is invalid")]
    InvalidGitFile { path: PathBuf },
    #[error("bound project identity is invalid")]
    InvalidBoundScope,
}

fn find_git_common_dir(workspace: &Path) -> Result<Option<PathBuf>, ProjectError> {
    for ancestor in workspace.ancestors() {
        let marker = ancestor.join(".git");
        let metadata = match fs::metadata(&marker) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(ProjectError::InspectGit {
                    path: marker,
                    source,
                });
            }
        };
        if metadata.is_dir() {
            return canonicalize_git_path(&marker).map(Some);
        }
        if metadata.is_file() {
            return resolve_git_file(&marker).map(Some);
        }
        return Err(ProjectError::InvalidGitFile { path: marker });
    }
    Ok(None)
}

fn resolve_git_file(marker: &Path) -> Result<PathBuf, ProjectError> {
    let contents = fs::read_to_string(marker).map_err(|source| ProjectError::InspectGit {
        path: marker.to_path_buf(),
        source,
    })?;
    let Some(raw_git_dir) = contents.trim().strip_prefix("gitdir: ") else {
        return Err(ProjectError::InvalidGitFile {
            path: marker.to_path_buf(),
        });
    };
    if raw_git_dir.is_empty() {
        return Err(ProjectError::InvalidGitFile {
            path: marker.to_path_buf(),
        });
    }
    let parent = marker
        .parent()
        .ok_or_else(|| ProjectError::InvalidGitFile {
            path: marker.to_path_buf(),
        })?;
    let git_dir = resolve_relative(parent, Path::new(raw_git_dir));
    let git_dir = canonicalize_git_path(&git_dir)?;
    let common_marker = git_dir.join("commondir");
    match fs::read_to_string(&common_marker) {
        Ok(common) => {
            let common = common.trim();
            if common.is_empty() {
                return Err(ProjectError::InvalidGitFile {
                    path: common_marker,
                });
            }
            canonicalize_git_path(&resolve_relative(&git_dir, Path::new(common)))
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(git_dir),
        Err(source) => Err(ProjectError::InspectGit {
            path: common_marker,
            source,
        }),
    }
}

fn resolve_relative(base: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        value.to_path_buf()
    } else {
        base.join(value)
    }
}

fn canonicalize_git_path(path: &Path) -> Result<PathBuf, ProjectError> {
    path.canonicalize()
        .map_err(|source| ProjectError::InspectGit {
            path: path.to_path_buf(),
            source,
        })
}

fn hash_identity(path: &Path) -> [u8; 32] {
    let mut hasher = Sha256::new();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(b"unix\0");
        hasher.update(path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        hasher.update(b"windows\0");
        for unit in path.as_os_str().encode_wide() {
            hasher.update(unit.to_le_bytes());
        }
    }
    hasher.finalize().into()
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn project_identity_matches_git_common_dir_and_keeps_display_paths() {
        let root = tempdir().expect("root");
        let primary = root.path().join("project with space");
        let linked = root.path().join("linked ünicode");
        let common = primary.join(".git");
        let linked_git_dir = common.join("worktrees").join("linked");
        let nested = linked.join("nested");
        fs::create_dir_all(&linked_git_dir).expect("linked git dir");
        fs::create_dir_all(&nested).expect("nested workspace");
        fs::write(
            linked.join(".git"),
            "gitdir: ../project with space/.git/worktrees/linked\n",
        )
        .expect("gitdir marker");
        fs::write(linked_git_dir.join("commondir"), "../..\n").expect("common marker");

        let primary_scope = ProjectScope::resolve(&primary).expect("primary scope");
        let linked_scope = ProjectScope::resolve(&linked).expect("linked scope");
        let nested_scope = ProjectScope::resolve(&nested).expect("nested scope");

        assert_eq!(primary_scope.project_id(), linked_scope.project_id());
        assert_eq!(linked_scope.project_id(), nested_scope.project_id());
        assert_eq!(
            primary_scope.display_path(),
            primary.canonicalize().expect("canonical primary")
        );
        assert_eq!(
            linked_scope.display_path(),
            linked.canonicalize().expect("canonical linked")
        );
        assert_ne!(primary_scope.display_path(), linked_scope.display_path());
        let debug = format!("{:?}", primary_scope.project_id());
        assert!(!debug.contains(primary.to_string_lossy().as_ref()));
    }

    #[test]
    fn non_git_workspace_uses_its_canonical_identity() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).expect("workspace");
        let scope = ProjectScope::resolve(&workspace).expect("scope");
        let same = ProjectScope::resolve(&workspace.join(".")).expect("same scope");
        assert_eq!(scope.project_id(), same.project_id());
        assert_eq!(
            scope.display_path(),
            workspace.canonicalize().expect("canonical")
        );
    }

    #[test]
    fn bound_scope_does_not_retarget_when_git_metadata_changes() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspace");
        let first_git = root.path().join("first.git");
        let second_git = root.path().join("second.git");
        fs::create_dir(&workspace).expect("workspace");
        fs::create_dir(&first_git).expect("first git dir");
        fs::create_dir(&second_git).expect("second git dir");
        fs::write(workspace.join(".git"), "gitdir: ../first.git\n").expect("first binding");
        let first = ProjectScope::resolve(&workspace).expect("first scope");
        let bound = ProjectScope::from_wire(
            &first.project_id().to_string(),
            first.display_path().to_str().expect("UTF-8 fixture"),
        )
        .expect("bound scope");

        fs::write(workspace.join(".git"), "gitdir: ../second.git\n").expect("retarget binding");
        let retargeted = ProjectScope::resolve(&workspace).expect("retargeted scope");
        assert_ne!(first.project_id(), retargeted.project_id());
        assert_eq!(bound, first);
    }

    #[test]
    fn malformed_git_file_is_an_error_not_non_git_fallback() {
        let root = tempdir().expect("root");
        fs::write(root.path().join(".git"), "not a gitdir marker").expect("marker");
        assert!(matches!(
            ProjectScope::resolve(root.path()),
            Err(ProjectError::InvalidGitFile { .. })
        ));
    }
}

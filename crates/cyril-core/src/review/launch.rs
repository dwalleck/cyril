//! The blocking half of starting a `/review`: everything between the
//! operator's consent and `_kiro/workflow/new`. The App runs these on
//! `spawn_blocking`; nothing here touches the event loop.

use super::CrtoolPrefix;
use super::config::{ConfigError, ReviewConfig};
use super::inputs::{InputProblem, recipe_inputs};
use cyril_review::{ASSETS, AssetKind, ReviewError, ReviewRun, gather, touched_files};
use serde_json::{Map, Value};
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

/// The repository-local directory every run directory lives under.
pub const RUNS_DIR: &str = ".code-review";
/// Run-directory name attempts before giving up on a collision.
const NAME_ATTEMPTS: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("run /review from the repo root ({})", .root.display())]
    NotRoot { root: PathBuf },
    #[error(
        "this workspace defines agents with reserved review names: {} — rename them to run /review",
        .paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(", ")
    )]
    ReservedAgents { paths: Vec<PathBuf> },
    #[error("no home directory to install the review workflow into (HOME / USERPROFILE)")]
    NoHome,
    #[error("cannot {action} {}: {source}", .path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("gather failed in {}: {source}", .run_dir.display())]
    Gather {
        run_dir: PathBuf,
        #[source]
        source: ReviewError,
    },
    #[error(transparent)]
    Review(#[from] ReviewError),
    #[error(transparent)]
    Inputs(#[from] InputProblem),
    #[error(transparent)]
    Config(#[from] ConfigError),
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> LaunchError {
    let path = path.to_path_buf();
    move |source| LaunchError::Io {
        action,
        path,
        source,
    }
}

/// What the operator is asked to consent to.
#[derive(Clone, Debug)]
pub struct LaunchRequest {
    pub workspace: PathBuf,
    pub target: String,
    pub scope: Vec<String>,
    pub crtool: CrtoolPrefix,
    /// Where Kiro looks for global workflows and agents: Node's home
    /// directory, see [`node_home`].
    pub home: Option<PathBuf>,
    /// The `[review]` settings the operator consented to.
    pub config: ReviewConfig,
}

/// A run ready for `_kiro/workflow/new`.
#[derive(Clone, Debug)]
pub struct ReadyRun {
    pub run_dir: PathBuf,
    /// The installed recipe's absolute path, sent as `workflowPath` so no
    /// workspace recipe name can shadow it.
    pub recipe: PathBuf,
    /// Agent files were written: KAS loads them from its file watcher a
    /// moment later, so the workflow must not be created straight away.
    pub agents_changed: bool,
    pub inputs: Map<String, Value>,
    pub files: usize,
}

#[derive(Debug)]
pub enum Prepared {
    /// The diff is empty: nothing was installed or written.
    Nothing,
    Ready(ReadyRun),
}

/// Files the request's diff touches; refuses outside the repository root.
pub fn probe(workspace: &Path, target: &str, scope: &[String]) -> Result<usize, LaunchError> {
    touched_files(workspace, target, &scope.join(" ")).map_err(|error| root_error(workspace, error))
}

/// The refusal outside the root names the root in the operator's terms.
fn root_error(workspace: &Path, error: ReviewError) -> LaunchError {
    match error {
        ReviewError::NotRepositoryRoot { cdup } => LaunchError::NotRoot {
            root: up(workspace, &cdup),
        },
        other => other.into(),
    }
}

/// `workspace` with git's `--show-cdup` (`../../`) applied lexically, so the
/// root reads in the same form as the directory the user launched from.
fn up(workspace: &Path, cdup: &str) -> PathBuf {
    let mut root = workspace.to_path_buf();
    for part in Path::new(cdup).components() {
        match part {
            Component::ParentDir => {
                root.pop();
            }
            Component::CurDir => {}
            other => root.push(other),
        }
    }
    root
}

/// Branches the form offers as a base for a `base...HEAD` review.
pub fn base_branches(workspace: &Path) -> Result<Vec<String>, LaunchError> {
    Ok(cyril_review::base_branches(workspace)?)
}

/// What a target's diff touches across the whole repository, and which
/// tracked files have uncommitted changes: the form derives its path groups,
/// counts and notes from these without asking git again.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Touched {
    pub files: Vec<String>,
    pub uncommitted: Vec<String>,
}

pub fn touched(workspace: &Path, target: &str) -> Result<Touched, LaunchError> {
    let files = cyril_review::changed_paths(workspace, target, ".")
        .map_err(|error| root_error(workspace, error))?;
    Ok(Touched {
        files,
        uncommitted: cyril_review::uncommitted_paths(workspace)?,
    })
}

/// Recheck the diff, then install the assets, create the run directory and
/// gather. An empty diff stops before any write.
pub fn prepare(request: &LaunchRequest) -> Result<Prepared, LaunchError> {
    let LaunchRequest {
        workspace,
        target,
        scope,
        crtool,
        home,
        config,
    } = request;
    let files = probe(workspace, target, scope)?;
    if files == 0 {
        return Ok(Prepared::Nothing);
    }
    // `context_file` is read now, not when the form opened.
    let context = config.context_text(workspace)?;
    // The inputs are checked before anything is written; the run directory's
    // own name is only digits, `-` and hex.
    recipe_inputs(
        &workspace.join(RUNS_DIR),
        target,
        scope,
        context.as_deref(),
        crtool,
    )?;
    let installed = install_checked(workspace, home.as_deref())?;
    let run_dir = create_run_dir(workspace)?;
    let run = ReviewRun::new(workspace, &run_dir)?;
    gather(&run, target, &scope.join(" ")).map_err(|source| LaunchError::Gather {
        run_dir: run_dir.clone(),
        source,
    })?;
    let inputs = recipe_inputs(&run_dir, target, scope, context.as_deref(), crtool)?;
    Ok(Prepared::Ready(ReadyRun {
        run_dir,
        recipe: installed.recipe,
        agents_changed: installed.agents_changed,
        inputs,
        files,
    }))
}

/// Refuse workspace agents with the review's reserved names, then install
/// the assets into `home`: the one install step every launch and resume runs.
pub fn install_checked(workspace: &Path, home: Option<&Path>) -> Result<Installed, LaunchError> {
    let reserved = reserved_agents(workspace)?;
    if !reserved.is_empty() {
        return Err(LaunchError::ReservedAgents { paths: reserved });
    }
    install_assets(home.ok_or(LaunchError::NoHome)?)
}

/// Node's `os.homedir()`, which is where KAS looks for `~/.kiro`.
pub fn node_home() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// Workspace agent files that would shadow a review agent.
pub fn reserved_agents(workspace: &Path) -> Result<Vec<PathBuf>, LaunchError> {
    let agents = workspace.join(".kiro").join("agents");
    let mut found = Vec::new();
    for asset in ASSETS
        .iter()
        .filter(|asset| asset.kind() == AssetKind::Agent)
    {
        for extension in ["md", "json"] {
            let path = agents.join(format!("{}.{extension}", asset.name()));
            match fs::symlink_metadata(&path) {
                Ok(_) => found.push(path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error("check", &path)(error)),
            }
        }
    }
    Ok(found)
}

/// What [`install_assets`] did.
#[derive(Debug)]
pub struct Installed {
    pub recipe: PathBuf,
    pub agents_changed: bool,
}

/// Write each asset under `<home>/.kiro/{workflows,agents}` whose bytes
/// differ, and return the recipe's path.
pub fn install_assets(home: &Path) -> Result<Installed, LaunchError> {
    let kiro = home.join(".kiro");
    let mut recipe = None;
    let mut agents_changed = false;
    for asset in ASSETS {
        let dir = kiro.join(match asset.kind() {
            AssetKind::Workflow => "workflows",
            AssetKind::Agent => "agents",
        });
        let path = dir.join(asset.file_name());
        if asset.kind() == AssetKind::Workflow {
            recipe = Some(path.clone());
        }
        match fs::read(&path) {
            Ok(bytes) if bytes == asset.contents().as_bytes() => continue,
            Ok(_) => tracing::info!(path = %path.display(), "review: overwriting a changed asset"),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                tracing::info!(path = %path.display(), "review: installing asset");
            }
            Err(error) => return Err(io_error("read", &path)(error)),
        }
        fs::create_dir_all(&dir).map_err(io_error("create", &dir))?;
        let temporary = dir.join(format!(".{}.tmp", asset.file_name()));
        fs::write(&temporary, asset.contents()).map_err(io_error("write", &temporary))?;
        fs::rename(&temporary, &path).map_err(io_error("install", &path))?;
        agents_changed |= asset.kind() == AssetKind::Agent;
    }
    let recipe = recipe.ok_or_else(|| LaunchError::Io {
        action: "find",
        path: kiro.join("workflows"),
        source: io::Error::new(io::ErrorKind::NotFound, "no embedded recipe"),
    })?;
    Ok(Installed {
        recipe,
        agents_changed,
    })
}

/// A new `.code-review/<YYYYMMDD-HHMMSS>-<4hex>/`, under a `.code-review`
/// that ignores itself (`.gitignore` = `*`); the repository's own ignore
/// files are never touched.
pub fn create_run_dir(workspace: &Path) -> Result<PathBuf, LaunchError> {
    let base = workspace.join(RUNS_DIR);
    match fs::create_dir(&base) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error("create", &base)(error)),
    }
    // Created when missing (another tool may have made the directory), never
    // rewritten: an existing ignore file is the user's.
    let ignore = base.join(".gitignore");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ignore)
    {
        Ok(mut file) => file.write_all(b"*\n").map_err(io_error("write", &ignore))?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error("create", &ignore)(error)),
    }
    let now = time::OffsetDateTime::now_utc();
    let stamp = format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let mut last = None;
    for _ in 0..NAME_ATTEMPTS {
        let suffix = std::hash::RandomState::new().build_hasher().finish() & 0xffff;
        let dir = base.join(format!("{stamp}-{suffix:04x}"));
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last = Some(error),
            Err(error) => return Err(io_error("create", &dir)(error)),
        }
    }
    Err(LaunchError::Io {
        action: "create a unique run directory in",
        path: base,
        source: last.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)),
    })
}

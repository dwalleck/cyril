//! The blocking half of starting a `/review`: everything between the
//! operator's consent and `_kiro/workflow/new`. The App runs these on
//! `spawn_blocking`; nothing here touches the event loop.

use super::CrtoolPrefix;
use super::inputs::{InputProblem, recipe_inputs};
use cyril_review::{ASSETS, AssetKind, ReviewError, ReviewRun, gather, touched_files};
use serde_json::{Map, Value};
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};

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
}

/// A run ready for `_kiro/workflow/new`.
#[derive(Clone, Debug)]
pub struct ReadyRun {
    pub run_dir: PathBuf,
    /// The installed recipe's absolute path, sent as `workflowPath` so no
    /// workspace recipe name can shadow it.
    pub recipe: PathBuf,
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
    let run = ReviewRun::new(workspace, workspace.join(RUNS_DIR))?;
    match touched_files(&run, target, &scope.join(" ")) {
        Err(ReviewError::NotRepositoryRoot { cdup }) => {
            let root = workspace.join(cdup);
            Err(LaunchError::NotRoot {
                root: root.canonicalize().unwrap_or(root),
            })
        }
        other => Ok(other?),
    }
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
    } = request;
    let files = probe(workspace, target, scope)?;
    if files == 0 {
        return Ok(Prepared::Nothing);
    }
    // The inputs are checked before anything is written; the run directory's
    // own name is only digits, `-` and hex.
    recipe_inputs(&workspace.join(RUNS_DIR), target, scope, None, crtool)?;
    let reserved = reserved_agents(workspace)?;
    if !reserved.is_empty() {
        return Err(LaunchError::ReservedAgents { paths: reserved });
    }
    let recipe = install_assets(home.as_deref().ok_or(LaunchError::NoHome)?)?;
    let run_dir = create_run_dir(workspace)?;
    let run = ReviewRun::new(workspace, &run_dir)?;
    gather(&run, target, &scope.join(" ")).map_err(|source| LaunchError::Gather {
        run_dir: run_dir.clone(),
        source,
    })?;
    let inputs = recipe_inputs(&run_dir, target, scope, None, crtool)?;
    Ok(Prepared::Ready(ReadyRun {
        run_dir,
        recipe,
        inputs,
        files,
    }))
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

/// Write each asset under `<home>/.kiro/{workflows,agents}` whose bytes
/// differ, and return the recipe's path.
pub fn install_assets(home: &Path) -> Result<PathBuf, LaunchError> {
    let kiro = home.join(".kiro");
    let mut recipe = None;
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
    }
    recipe.ok_or_else(|| LaunchError::Io {
        action: "find",
        path: kiro.join("workflows"),
        source: io::Error::new(io::ErrorKind::NotFound, "no embedded recipe"),
    })
}

/// A new `.code-review/<YYYYMMDD-HHMMSS>-<4hex>/`. Creating `.code-review`
/// also writes its self-ignoring `.gitignore`; the repository's own ignore
/// files are never touched.
pub fn create_run_dir(workspace: &Path) -> Result<PathBuf, LaunchError> {
    let base = workspace.join(RUNS_DIR);
    match fs::create_dir(&base) {
        Ok(()) => {
            let ignore = base.join(".gitignore");
            fs::write(&ignore, "*\n").map_err(io_error("write", &ignore))?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error("create", &base)(error)),
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

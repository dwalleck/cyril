//! `/review`'s blocking launch steps against a real repository and a fake
//! home: what lands on disk, and what an empty diff or a refusal leaves alone.

use cyril_core::review::launch::{LaunchError, LaunchRequest, Prepared, RUNS_DIR, prepare, probe};
use cyril_core::review::{CrtoolPrefix, ShellDialect};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type TestResult = Result<(), Box<dyn Error>>;

struct Fixture {
    _tree: tempfile::TempDir,
    repo: PathBuf,
    home: PathBuf,
}

fn git(repo: &Path, args: &[&str]) -> TestResult {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", repo.join("../gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Launch")
        .env("GIT_AUTHOR_EMAIL", "launch@example.invalid")
        .env("GIT_COMMITTER_NAME", "Launch")
        .env("GIT_COMMITTER_EMAIL", "launch@example.invalid")
        .output()?;
    if !output.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    Ok(())
}

/// A committed repository on `main`, clean.
fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let root = tree.path().to_path_buf();
    fs::write(root.join("gitconfig"), "")?;
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("src"))?;
    fs::write(repo.join("src/a.rs"), "fn a() {}\n")?;
    git(&repo, &["init", "-q", "-b", "main"])?;
    git(&repo, &["config", "core.autocrlf", "false"])?;
    git(&repo, &["add", "-A"])?;
    git(&repo, &["commit", "-q", "-m", "first"])?;
    let home = root.join("home");
    fs::create_dir_all(&home)?;
    Ok(Fixture {
        _tree: tree,
        repo,
        home,
    })
}

fn request(fixture: &Fixture) -> Result<LaunchRequest, Box<dyn Error>> {
    let exe = if cfg!(windows) {
        "C:/bin/cyril.exe"
    } else {
        "/bin/cyril"
    };
    Ok(LaunchRequest {
        workspace: fixture.repo.clone(),
        target: "auto".to_owned(),
        scope: vec![".".to_owned()],
        crtool: CrtoolPrefix::from_executable(Path::new(exe), ShellDialect::Posix)?,
        home: Some(fixture.home.clone()),
    })
}

#[test]
fn an_empty_diff_writes_nothing() -> TestResult {
    let fixture = fixture()?;
    assert!(matches!(prepare(&request(&fixture)?)?, Prepared::Nothing));
    assert!(!fixture.repo.join(RUNS_DIR).exists());
    assert!(!fixture.home.join(".kiro").exists());
    Ok(())
}

#[test]
fn a_ready_run_installs_assets_gathers_and_ignores_itself() -> TestResult {
    let fixture = fixture()?;
    fs::write(fixture.repo.join("src/a.rs"), "fn a() { todo!() }\n")?;
    // A stale agent is replaced with the embedded bytes.
    let agents = fixture.home.join(".kiro/agents");
    fs::create_dir_all(&agents)?;
    fs::write(agents.join("cyril-review-clerk.md"), "stale")?;

    let Prepared::Ready(run) = prepare(&request(&fixture)?)? else {
        return Err("a changed file must produce a run".into());
    };
    assert_eq!(run.files, 1);
    assert!(run.agents_changed, "a fresh home gets every agent");
    assert_eq!(
        run.recipe,
        fixture
            .home
            .join(".kiro/workflows/cyril-review.workflow.json")
    );
    for asset in cyril_review::ASSETS {
        let dir = match asset.kind() {
            cyril_review::AssetKind::Workflow => "workflows",
            cyril_review::AssetKind::Agent => "agents",
        };
        let installed =
            fs::read_to_string(fixture.home.join(".kiro").join(dir).join(asset.file_name()))?;
        assert_eq!(installed, asset.contents(), "{}", asset.name());
    }

    let runs = fixture.repo.join(RUNS_DIR);
    assert_eq!(fs::read_to_string(runs.join(".gitignore"))?, "*\n");
    assert!(
        !fixture.repo.join(".gitignore").exists(),
        "the repo's own ignore file is untouched"
    );
    assert_eq!(run.run_dir.parent(), Some(runs.as_path()));
    let name = run
        .run_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("run dir name")?;
    let shape: Vec<usize> = name.split('-').map(str::len).collect();
    assert_eq!(shape, [8, 6, 4], "YYYYMMDD-HHMMSS-hex: {name}");
    assert!(run.run_dir.join("manifest.json").is_file(), "gather ran");
    assert_eq!(
        run.inputs["rundir"],
        run.run_dir.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(run.inputs["scope"], ".");

    // A second run gets its own directory and leaves the ignore file alone.
    fs::write(runs.join(".gitignore"), "*\n# kept\n")?;
    let Prepared::Ready(second) = prepare(&request(&fixture)?)? else {
        return Err("the diff is still there".into());
    };
    assert_ne!(second.run_dir, run.run_dir);
    assert!(
        !second.agents_changed,
        "identical bytes are left alone, so KAS has nothing to reload"
    );
    assert_eq!(fs::read_to_string(runs.join(".gitignore"))?, "*\n# kept\n");
    Ok(())
}

#[test]
fn reserved_agent_names_refuse_before_any_install() -> TestResult {
    let fixture = fixture()?;
    fs::write(fixture.repo.join("src/a.rs"), "fn a() { todo!() }\n")?;
    let agents = fixture.repo.join(".kiro/agents");
    fs::create_dir_all(&agents)?;
    fs::write(agents.join("cyril-review-finder.json"), "{}")?;
    match prepare(&request(&fixture)?) {
        Err(LaunchError::ReservedAgents { paths }) => {
            assert_eq!(paths, [agents.join("cyril-review-finder.json")]);
        }
        other => return Err(format!("expected a reserved-name refusal, got {other:?}").into()),
    }
    assert!(!fixture.home.join(".kiro").exists());
    assert!(!fixture.repo.join(RUNS_DIR).exists());
    Ok(())
}

#[test]
fn below_the_root_names_the_root() -> TestResult {
    let fixture = fixture()?;
    match probe(&fixture.repo.join("src"), "auto", &[".".to_owned()]) {
        Err(error @ LaunchError::NotRoot { .. }) => assert_eq!(
            error.to_string(),
            format!(
                "run /review from the repo root ({})",
                fixture.repo.display()
            )
        ),
        other => return Err(format!("expected NotRoot, got {other:?}").into()),
    }
    Ok(())
}

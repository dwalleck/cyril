mod app;
mod memory_runtime;

use std::fmt;
use std::path::{Component, Path, PathBuf};

use clap::Parser;
use cyril_core::types::AgentEngine;

#[derive(Parser)]
#[command(
    name = "cyril",
    about = "Polished TUI for the Agent Client Protocol ecosystem"
)]
struct Cli {
    /// Working directory
    #[arg(short = 'd', long = "cwd")]
    cwd: Option<PathBuf>,

    /// Send a one-shot prompt
    #[arg(long)]
    prompt: Option<String>,

    /// Command line for the ACP agent. First value is the program; remaining
    /// values are arguments. Defaults to `kiro-cli acp`.
    #[arg(
        long = "agent-command",
        num_args = 1..,
        default_values_t = vec!["kiro-cli".to_string(), "acp".to_string()],
    )]
    agent_command: Vec<String>,

    /// Which Kiro engine to drive: `v2` (default) or `kas` (`v3` is accepted
    /// as an alias for `kas`). Overrides `[agent] engine` in config.
    #[arg(long = "agent-engine")]
    agent_engine: Option<AgentEngine>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    setup_logging();

    let cwd = startup_cwd(cli.cwd)?;

    let config_report = cyril_memory::load_config_report(&config_dir().join("config.toml"));
    let memory_config = config_report.memory().clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let config = config_report.ordinary().clone();

    // Spawn bridge
    let agent_command = cyril_core::types::AgentCommand::try_from_argv(cli.agent_command)?;
    // The `--agent-engine` flag overrides `[agent] engine` in config; config
    // defaults to v2 (KAS-0, ADR-0002).
    let agent_engine = cli.agent_engine.unwrap_or(config.agent.engine);
    // KAS spawn shape (KAS-1): `[agent] kas_spawn` (free | wrapper); free default.
    let bridge = cyril_core::protocol::bridge::spawn_bridge(
        agent_command,
        cyril_core::protocol::bridge::SpawnConfig {
            engine: agent_engine,
            kas_spawn: config.agent.kas_spawn,
            shell: config.agent.shell,
            present_as: config.agent.present_as,
            kas_hooks: config.agent.kas_hooks,
            // Deliberately not TOML-exposed (cyril-14ou design, negative
            // space #5): the 30s default is probe-derived with >3x margin and
            // v1 ships no tuning surface for it. Unlike the fields above,
            // which mirror `[agent]` config keys, this one is a constant.
            stall_threshold: cyril_core::protocol::bridge::DEFAULT_STALL_THRESHOLD,
        },
        cwd.clone(),
    )?;

    // Bound outside the async block: `cli` is already partially moved (cwd,
    // agent_command), and an async block would capture the whole struct.
    let oneshot_prompt = cli.prompt;
    let usage_log = cyril_core::usage::UsageLog::open(&config_dir().join("usage.sqlite3"))?;

    // Build and run TUI.

    rt.block_on(async {
        // Who answers `/hooks` depends on which side owns a hook registry
        // (cyril-gk17) — resolved once here, from the same two values the
        // bridge was spawned with.
        let hooks_source = cyril_core::commands::HooksCommandSource::resolve(
            agent_engine,
            config.agent.kas_hooks,
            cwd.clone(),
        );
        // The native /workflow family exists exactly when the engine is KAS
        // (cyril-0qe6, ADR-0011) — no mode axis, unlike hooks.
        let workflow_source =
            cyril_core::commands::WorkflowCommandSource::resolve(agent_engine, cwd.clone());
        let mut app = app::App::new(
            bridge,
            &config.ui,
            cwd.clone(),
            hooks_source,
            workflow_source,
            usage_log,
            agent_engine,
        );

        // The memory companion starts only once every fallible startup step
        // above has passed: an early `?` return would drop the handle, whose
        // `Drop` is abort-only, and SIGKILL the child with its socket file
        // left behind. From here on every exit path goes through
        // `App::shutdown_memory_runtime`.
        let memory_runtime = memory_runtime::MemoryRuntimeHandle::start(memory_config);
        let project_binding = memory_runtime.bind_project(&cwd);
        app.set_memory_runtime(memory_runtime, project_binding);

        // Create initial session; a parsed `--prompt` rides along and is
        // submitted once the session is ready (cyril-0ffy).
        app.create_initial_session(cwd, oneshot_prompt).await;

        // Initialize terminal
        let mut terminal = ratatui::init();
        if let Err(e) =
            crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)
        {
            ratatui::restore();
            app.shutdown_memory_runtime().await;
            return Err(Box::new(cyril_core::Error::with_source(
                cyril_core::ErrorKind::Transport {
                    detail: "failed to enable bracketed paste".into(),
                },
                e,
            )) as Box<dyn std::error::Error>);
        }

        // Mouse capture follows `ui.mouse_capture` (cyril-nd4h). Derived from
        // App's state, NOT read from the config again: two independent reads is
        // exactly how this flag and the terminal drift apart, which shows up as
        // a first Ctrl+M press that appears to do nothing. The restore path
        // below disables unconditionally, which is a no-op when never enabled.
        //
        // Non-fatal on purpose, matching the runtime Ctrl+M handler: a terminal
        // that refuses mouse capture should not stop cyril from starting. `?`
        // here would also abandon the terminal mid-setup — past `ratatui::init()`
        // and with bracketed paste already on — skipping the restore below.
        // Rolling the flag back keeps UiState in agreement with the terminal.
        if app.mouse_captured()
            && let Err(e) =
                crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)
        {
            tracing::warn!(error = %e, "failed to enable mouse capture; continuing without it");
            app.set_mouse_captured(false);
        }

        let result = app.run(&mut terminal).await;

        // Restore terminal
        if let Err(e) = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste,
        ) {
            tracing::warn!(error = %e, "failed to disable mouse capture / bracketed paste");
        }
        ratatui::restore();
        // `run` shuts the companion down on a clean quit; an error return
        // from it must not skip that.
        app.shutdown_memory_runtime().await;

        if let Err(ref e) = result {
            eprintln!("Error: {e}");
        }

        result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    })?;

    Ok(())
}

/// Why the startup workspace could not be bound. Carries the path: this is
/// the first thing a user sees after a bad `--cwd`, and `Os { code: 2 }`
/// alone does not say which path.
#[derive(thiserror::Error)]
enum StartupCwdError {
    #[error("workspace path {path} could not be resolved: {source}")]
    Resolve {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workspace path {path} is not a directory")]
    NotADirectory { path: PathBuf },
}

// `main` returns this through `Box<dyn Error>`, which the runtime prints via
// `Debug`; delegating to `Display` keeps the message a sentence.
impl fmt::Debug for StartupCwdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

/// Resolve the workspace to an absolute, lexically normalized path that is a
/// directory. Deliberately NOT `canonicalize`: that yields a `\\?\C:\…`
/// verbatim path on Windows that a native `kiro-cli.exe` and `cmd.exe`-based
/// hooks do not accept, and rewrites a symlinked workspace to its target,
/// which splits the usage log's per-project history. Project *identity*
/// (`ProjectScope`) canonicalizes on its own.
fn startup_cwd(configured: Option<PathBuf>) -> Result<PathBuf, StartupCwdError> {
    let requested = match configured {
        Some(cwd) => cwd,
        None => std::env::current_dir().map_err(|source| StartupCwdError::Resolve {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let absolute = std::path::absolute(&requested).map_err(|source| StartupCwdError::Resolve {
        path: requested.clone(),
        source,
    })?;
    let path = normalize_lexically(&absolute);
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(path),
        Ok(_) => Err(StartupCwdError::NotADirectory { path }),
        Err(source) => Err(StartupCwdError::Resolve { path, source }),
    }
}

/// Drop `.` components and fold `..` onto the preceding component, the way a
/// shell's logical `cd` does, without touching the filesystem.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // `..` above the root stays at the root; `pop` reports that
                // there was nothing to remove and that is the desired result.
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

fn setup_logging() {
    let log_dir = config_dir();
    // Ensure config directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Warning: could not create log directory: {e}");
        return;
    }

    let log_path = log_dir.join("cyril.log");

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .json()
            .init();
    }
}

fn config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("cyril")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".config").join("cyril")
    } else {
        PathBuf::from(".cyril")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    // Slice 5 (D7 parse table): no flag -> None (config supplies the default);
    // `--agent-engine kas` -> Some(Kas); an unknown value is REJECTED at parse
    // time, never silently defaulted.
    #[test]
    fn cli_agent_engine_flag() {
        let none = Cli::try_parse_from(["cyril"]).expect("parses with no engine flag");
        assert_eq!(none.agent_engine, None);

        let kas = Cli::try_parse_from(["cyril", "--agent-engine", "kas"])
            .expect("parses --agent-engine kas");
        assert_eq!(kas.agent_engine, Some(AgentEngine::Kas));

        assert!(
            Cli::try_parse_from(["cyril", "--agent-engine", "bogus"]).is_err(),
            "an unknown engine value is rejected, not silently defaulted"
        );
    }

    #[test]
    fn configured_startup_cwd_is_absolute_normalized_and_names_the_path_on_error() {
        let directory = tempfile::tempdir().expect("temporary workspace");
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).expect("create nested workspace");
        let alias = nested.join("..").join("nested").join(".");
        let resolved = startup_cwd(Some(alias)).expect("normalized startup workspace");
        // Lexical only: `..`/`.` fold away, but the tempdir itself is NOT
        // rewritten to its canonical target (no `\\?\`, no symlink chasing).
        assert_eq!(resolved, nested);
        assert!(resolved.is_absolute());

        let missing = directory.path().join("missing");
        let error = startup_cwd(Some(missing.clone())).expect_err("missing workspace");
        assert!(matches!(&error, StartupCwdError::Resolve { path, .. } if path == &missing));
        assert!(
            format!("{error:?}").contains(&missing.display().to_string()),
            "{error:?}"
        );

        let file = directory.path().join("file.txt");
        std::fs::write(&file, "not a directory").expect("file fixture");
        let error = startup_cwd(Some(file.clone())).expect_err("file is not a workspace");
        assert!(matches!(&error, StartupCwdError::NotADirectory { path } if path == &file));
        assert_eq!(
            error.to_string(),
            format!("workspace path {} is not a directory", file.display())
        );

        // `.` resolves against the process cwd without touching the filesystem
        // for anything but the final directory check.
        let here = startup_cwd(Some(PathBuf::from("."))).expect("process cwd");
        assert_eq!(here, std::env::current_dir().expect("current directory"));
    }

    #[test]
    fn lexical_normalization_keeps_the_root_and_drops_dots() {
        let root = Path::new("/").join("work");
        assert_eq!(
            normalize_lexically(&root.join(".").join("a").join("..").join("b")),
            root.join("b")
        );
        assert_eq!(
            normalize_lexically(&Path::new("/").join("..").join("x")),
            Path::new("/").join("x")
        );
    }
}

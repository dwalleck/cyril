use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::Subcommand;
use cyril_review::ReviewRun;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Internal mechanical review operations.
    #[command(hide = true)]
    Crtool {
        #[command(subcommand)]
        step: Step,
    },
}

#[derive(Subcommand)]
pub(crate) enum Step {
    /// Gather a diff and its review facts.
    Gather {
        rundir: PathBuf,
        #[arg(allow_hyphen_values = true)]
        target: String,
        #[arg(default_value = "", allow_hyphen_values = true)]
        scope: String,
    },
    /// Rebuild facts for a stamped run.
    Facts { rundir: PathBuf },
}

impl Step {
    /// The run directory exactly as the caller spelled it.
    fn rundir(&self) -> &Path {
        match self {
            Self::Gather { rundir, .. } | Self::Facts { rundir } => rundir,
        }
    }
}

/// Anchor a caller-supplied run directory to the selected workspace: a
/// relative path resolves beneath it and an absolute path is preserved.
fn anchored(workspace: &Path, rundir: &Path) -> PathBuf {
    if rundir.is_absolute() {
        rundir.to_path_buf()
    } else {
        workspace.join(rundir)
    }
}

impl Command {
    /// Runs before ordinary startup; owns no runtime, agent, or terminal state.
    ///
    /// `configured_cwd` is the parsed `--cwd`, moved here by early dispatch.
    /// The ordinary `startup_cwd` resolution and directory validation select
    /// the workspace; relative run directories anchor beneath it while
    /// absolute ones are preserved, so gather and facts share one mapping.
    /// A bad workspace is refused before any run artifact exists, and
    /// `ReviewRun`'s own anchoring stays unchanged.
    pub(crate) fn run(self, configured_cwd: Option<PathBuf>) -> i32 {
        let Self::Crtool { step } = self;
        let workspace = match crate::startup_cwd(configured_cwd) {
            Ok(workspace) => workspace,
            Err(error) => return report_error(&error, 2),
        };
        let rundir = anchored(&workspace, step.rundir());
        let result = match step {
            Step::Gather { target, scope, .. } => ReviewRun::new(&workspace, rundir)
                .and_then(|run| cyril_review::gather(&run, &target, &scope)),
            Step::Facts { .. } => {
                ReviewRun::new(&workspace, rundir).and_then(|run| cyril_review::facts(&run))
            }
        };
        match result {
            Ok(output) => write_output(&output),
            Err(error) => report_error(&error, error.exit_code()),
        }
    }
}

fn write_output(output: &str) -> i32 {
    match io::stdout().lock().write_all(output.as_bytes()) {
        Ok(()) => 0,
        Err(error) => report_error(&error, 2),
    }
}

fn report_error(error: &dyn std::fmt::Display, exit_code: i32) -> i32 {
    match writeln!(io::stderr().lock(), "crtool: error: {error}") {
        Ok(()) => exit_code,
        // A broken diagnostic stream cannot report its own failure; expose it
        // through the ordinary I/O failure exit, never through empty-diff exit3.
        Err(_) => 2,
    }
}

use std::io::{self, Write};
use std::path::PathBuf;

use clap::Subcommand;
use cyril_review::{ReviewRun, StepOutput, SystemReviewClock};

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
        target: String,
        #[arg(default_value = "")]
        scope: String,
    },
    /// Rebuild facts for a stamped run.
    Facts { rundir: PathBuf },
}

impl Command {
    /// Runs before ordinary startup; owns no runtime, agent, or terminal state.
    pub(crate) fn run(self) -> i32 {
        let Self::Crtool { step } = self;
        let result = match step {
            Step::Gather {
                rundir,
                target,
                scope,
            } => ReviewRun::new(".", rundir)
                .and_then(|run| cyril_review::gather(&run, &target, &scope, &SystemReviewClock)),
            Step::Facts { rundir } => {
                ReviewRun::new(".", rundir).and_then(|run| cyril_review::facts(&run))
            }
        };
        match result {
            Ok(output) => write_output(&output),
            Err(error) => report_error(&error, error.exit_code()),
        }
    }
}

fn write_output(output: &StepOutput) -> i32 {
    match io::stdout().lock().write_all(output.stdout()) {
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

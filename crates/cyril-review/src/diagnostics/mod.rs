mod command;
mod process;

use crate::run::{facts_dir, manifest_path, read_manifest, write_binary, write_json, write_text};
use crate::{Result, ReviewError, ReviewRun, StepOutput};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Execution timeout for one blocking check.
#[derive(Clone, Copy, Debug)]
pub struct DiagnosticsOptions {
    pub timeout: Duration,
}

impl Default for DiagnosticsOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(1_800),
        }
    }
}

/// Caller-owned cancellation signal for a single diagnostics operation.
#[derive(Clone, Debug, Default)]
pub struct Cancellation {
    cancelled: Arc<AtomicBool>,
}

impl Cancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticsOutcome {
    Clean,
    Failed { exit_code: i64 },
    TimedOut,
    Cancelled,
}

#[derive(Debug)]
pub struct DiagnosticsResult {
    outcome: DiagnosticsOutcome,
    started: bool,
    output: StepOutput,
}

impl DiagnosticsResult {
    pub fn outcome(&self) -> DiagnosticsOutcome {
        self.outcome
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn output(&self) -> &StepOutput {
        &self.output
    }
}

/// Execute one native check, capturing and reporting its terminal evidence.
pub fn diagnostics(
    run: &ReviewRun,
    command: &str,
    options: &DiagnosticsOptions,
    cancel: &Cancellation,
    clock: &dyn crate::ReviewClock,
) -> Result<DiagnosticsResult> {
    if cancel.is_cancelled() {
        return Ok(DiagnosticsResult {
            outcome: DiagnosticsOutcome::Cancelled,
            started: false,
            output: StepOutput::default(),
        });
    }
    let mut manifest = read_manifest(run)?;
    // Match facts' existing missing/null/object policy, before any child or write.
    let facts = manifest
        .metadata
        .entry("facts".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if facts.is_null() {
        *facts = Value::Object(Map::new());
    }
    let facts = facts
        .as_object_mut()
        .ok_or_else(|| ReviewError::InvalidManifest {
            path: manifest_path(run),
            message: "facts is not an object".to_owned(),
        })?;
    let prepared = command::prepare(command, run.workspace())?;
    // A cancellation observed during validation is still a prelaunch cancellation.
    if cancel.is_cancelled() {
        return Ok(DiagnosticsResult {
            outcome: DiagnosticsOutcome::Cancelled,
            started: false,
            output: StepOutput::default(),
        });
    }
    let started = Instant::now();
    let (outcome, raw) = process::capture(prepared, command, options.timeout, cancel)?;
    let took = format!("{:.0}", clock.diagnostics_elapsed(started).as_secs_f64());
    let status = match outcome {
        DiagnosticsOutcome::Clean => "clean".to_owned(),
        DiagnosticsOutcome::Failed { exit_code } => format!("FAILED (exit {exit_code})"),
        DiagnosticsOutcome::TimedOut => "TIMED OUT".to_owned(),
        DiagnosticsOutcome::Cancelled => "CANCELLED".to_owned(),
    };
    let changed: Vec<_> = manifest
        .files
        .iter()
        .map(|file| slash_normalized(&file.path))
        .collect();
    let decoded = String::from_utf8_lossy(&raw);
    let lines = filter_lines(&decoded, &changed);
    let matches = lines.total;
    let head_end = manifest
        .head
        .char_indices()
        .nth(12)
        .map_or(manifest.head.len(), |(index, _)| index);
    let mut report = format!(
        "command: {command}\nresult: {status} in {took}s, on HEAD {}\n{matches} output line(s) mention a changed file\n",
        &manifest.head[..head_end],
    );
    for line in lines.matches {
        report.push_str(line);
        report.push('\n');
    }
    report.push_str("\nlast lines of output:\n");
    for line in lines.tail {
        report.push_str(line);
        report.push('\n');
    }
    let directory = facts_dir(run);
    write_binary(&directory.join("diagnostics-raw.txt"), &raw)?;
    write_text(&directory.join("diagnostics.txt"), &report)?;
    facts.insert(
        "diagnostics".to_owned(),
        Value::from("facts/diagnostics.txt"),
    );
    facts.insert(
        "diagnostics_status".to_owned(),
        Value::from(status.as_str()),
    );
    write_json(&manifest_path(run), &manifest)?;
    Ok(DiagnosticsResult {
        outcome,
        started: true,
        output: StepOutput::from_text(format!(
            "diagnostics: {status} in {took}s; {matches} line(s) on changed files -> facts/diagnostics.txt\n"
        )),
    })
}

fn slash_normalized(text: &str) -> Cow<'_, str> {
    if text.contains('\\') {
        Cow::Owned(text.replace('\\', "/"))
    } else {
        Cow::Borrowed(text)
    }
}

struct FilteredLines<'a> {
    matches: Vec<&'a str>,
    tail: VecDeque<&'a str>,
    total: usize,
}

fn filter_lines<'a>(text: &'a str, changed: &[Cow<'_, str>]) -> FilteredLines<'a> {
    let mut lines = FilteredLines {
        matches: Vec::with_capacity(200),
        tail: VecDeque::with_capacity(15),
        total: 0,
    };
    // Python splitlines boundaries, including CRLF and Unicode separators.
    for line in text.split([
        '\n', '\r', '\x0b', '\x0c', '\x1c', '\x1d', '\x1e', '\u{85}', '\u{2028}', '\u{2029}',
    ]) {
        if line.trim().is_empty() {
            continue;
        }
        let normalized = slash_normalized(line);
        if changed
            .iter()
            .any(|path| normalized.contains(path.as_ref()))
        {
            lines.total += 1;
            if lines.matches.len() < 200 {
                lines.matches.push(line);
            }
        }
        if lines.tail.len() == 15 {
            lines.tail.pop_front();
        }
        lines.tail.push_back(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonempty_unicode_lines_and_native_slashes_preserve_original_evidence() {
        let text = "\r\n  \u{2028}src\\lib.rs: one\r\nsrc/lib.rs: two\u{85}last\u{2029}";
        let lines = filter_lines(text, &[Cow::Borrowed("src/lib.rs")]);
        assert_eq!(lines.total, 2);
        assert_eq!(lines.matches, ["src\\lib.rs: one", "src/lib.rs: two"]);
        assert_eq!(
            lines.tail.into_iter().collect::<Vec<_>>(),
            ["src\\lib.rs: one", "src/lib.rs: two", "last"]
        );
    }
}

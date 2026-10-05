//! The one chat system message a review run ends with. Width truncation is
//! the renderer's job, not this module's.

use cyril_review::{Finding, FindingsError, Verdict};
use std::path::Path;

/// Findings listed in the message; the rest are counted.
const SHOWN: usize = 10;
/// Denial reasons quoted in the message; all of them are in `denied.log`.
const DENIALS_SHOWN: usize = 3;

/// How the run ended, from its `run_complete` status.
#[derive(Debug)]
pub enum RunEnding {
    Completed(Result<Vec<Finding>, FindingsError>),
    Failed,
    Aborted,
    /// Still armed; `/workflow resume <id>` continues it.
    Paused {
        workflow_id: String,
    },
}

/// The completion message for a run ending in `run_dir`.
pub fn summary(ending: &RunEnding, run_dir: &Path, denials: &[String]) -> String {
    let dir = run_dir.display();
    let findings = match ending {
        RunEnding::Failed => return format!("review failed — {dir}"),
        RunEnding::Aborted => return format!("review aborted — {dir}"),
        RunEnding::Paused { workflow_id } => {
            return format!("review paused — /workflow resume {workflow_id} continues it ({dir})");
        }
        RunEnding::Completed(Err(error)) => {
            return format!("review completed, but {error} — {dir}");
        }
        RunEnding::Completed(Ok(findings)) => findings,
    };
    let mut lines = vec![headline(findings)];
    lines.extend(findings.iter().take(SHOWN).map(|finding| {
        format!(
            "[{}] {} {}",
            finding.verdict,
            finding.summary.as_deref().unwrap_or("(no summary)"),
            location(finding)
        )
    }));
    if findings.len() > SHOWN {
        lines.push(format!(
            "...and {} more in report.md",
            findings.len() - SHOWN
        ));
    }
    lines.push(format!("report: {}", run_dir.join("report.md").display()));
    if !denials.is_empty() {
        let shown: Vec<&str> = denials
            .iter()
            .take(DENIALS_SHOWN)
            .map(String::as_str)
            .collect();
        lines.push(format!(
            "policy denied {} request(s): {}",
            denials.len(),
            shown.join("; ")
        ));
    }
    lines.join("\n")
}

fn headline(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "review complete: no findings".to_owned();
    }
    let counts: Vec<String> = Verdict::REPORT_ORDER
        .iter()
        .filter(|verdict| **verdict != Verdict::Refuted)
        .filter_map(|verdict| {
            let count = findings
                .iter()
                .filter(|finding| finding.verdict == *verdict)
                .count();
            (count > 0).then(|| format!("{count} {verdict}"))
        })
        .collect();
    format!(
        "review complete: {} finding(s) — {}",
        findings.len(),
        counts.join(", ")
    )
}

fn location(finding: &Finding) -> String {
    match (&finding.file, finding.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        (None, _) => "?".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(number: usize, verdict: Verdict) -> Finding {
        Finding {
            id: format!("C{number:02}"),
            file: Some(format!("src/f{number}.rs")),
            line: Some(number as i64),
            summary: Some(format!("defect {number}")),
            failure_scenario: None,
            verdict,
            angles: vec!["a-line-scan".to_owned()],
            comment: None,
            duplicate_of: None,
        }
    }

    #[test]
    fn completed_lists_ten_counts_the_rest_and_quotes_three_denials() {
        let findings: Vec<Finding> = (1..=12)
            .map(|n| {
                finding(
                    n,
                    if n % 2 == 0 {
                        Verdict::Confirmed
                    } else {
                        Verdict::Plausible
                    },
                )
            })
            .collect();
        let denials: Vec<String> = (1..=4).map(|n| format!("reason {n}")).collect();
        let text = summary(
            &RunEnding::Completed(Ok(findings)),
            Path::new("/w/.code-review/r"),
            &denials,
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "review complete: 12 finding(s) — 6 CONFIRMED, 6 PLAUSIBLE"
        );
        assert_eq!(lines[1], "[PLAUSIBLE] defect 1 src/f1.rs:1");
        assert_eq!(lines[11], "...and 2 more in report.md");
        assert_eq!(
            lines[12],
            format!(
                "report: {}",
                Path::new("/w/.code-review/r").join("report.md").display()
            )
        );
        assert_eq!(
            lines[13],
            "policy denied 4 request(s): reason 1; reason 2; reason 3"
        );
    }

    #[test]
    fn other_endings_are_one_line() {
        let dir = Path::new("/w/.code-review/r");
        assert_eq!(
            summary(&RunEnding::Failed, dir, &[]),
            format!("review failed — {}", dir.display())
        );
        assert_eq!(
            summary(&RunEnding::Aborted, dir, &[]),
            format!("review aborted — {}", dir.display())
        );
        let paused = RunEnding::Paused {
            workflow_id: "wf-1".to_owned(),
        };
        assert_eq!(
            summary(&paused, dir, &[]),
            format!(
                "review paused — /workflow resume wf-1 continues it ({})",
                dir.display()
            )
        );
        let empty = summary(&RunEnding::Completed(Ok(Vec::new())), dir, &[]);
        assert!(empty.starts_with("review complete: no findings\nreport: "));
    }
}

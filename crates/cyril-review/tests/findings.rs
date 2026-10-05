//! `read_findings`: what `/review` reads when a run completes. A missing and a
//! corrupt `findings.json` are distinct errors, never "0 findings".

use cyril_review::{FindingsError, ReviewRun, read_findings};
use std::error::Error;
use std::fs;
use std::path::Path;

type TestResult = Result<(), Box<dyn Error>>;

fn run_with(findings: Option<&[u8]>) -> Result<(tempfile::TempDir, ReviewRun), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let run = ReviewRun::new(tree.path(), tree.path().join("run"))?;
    fs::create_dir_all(run.dir())?;
    if let Some(bytes) = findings {
        fs::write(run.dir().join("findings.json"), bytes)?;
    }
    Ok((tree, run))
}

#[test]
fn missing_and_corrupt_findings_are_distinct_errors() -> TestResult {
    let (_tree, run) = run_with(None)?;
    assert!(matches!(
        read_findings(&run),
        Err(FindingsError::Missing { .. })
    ));

    let (_tree, run) = run_with(Some(b"[{\"id\": \"C01\""))?;
    assert!(matches!(
        read_findings(&run),
        Err(FindingsError::Corrupt { .. })
    ));

    let (_tree, run) = run_with(Some(b"[]"))?;
    assert_eq!(read_findings(&run)?, []);
    Ok(())
}

#[test]
fn a_commented_pipeline_run_parses_into_typed_findings() -> TestResult {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/cases/pipeline/expected/run/findings.json");
    let (_tree, run) = run_with(Some(&fs::read(golden)?))?;
    let findings = read_findings(&run)?;
    assert_eq!(findings.len(), 11);
    let first = &findings[0];
    assert_eq!(first.id, "C03");
    assert_eq!(first.verdict, cyril_review::Verdict::Confirmed);
    assert_eq!(first.angles, ["b-removed-behavior", "a-line-scan"]);
    assert!(
        first
            .comment
            .as_deref()
            .is_some_and(|comment| comment.starts_with("**issue (blocking,test):**"))
    );
    let duplicate = findings
        .iter()
        .find(|finding| finding.id == "C01")
        .ok_or("C01 missing")?;
    assert_eq!(duplicate.duplicate_of.as_deref(), Some("C03"));
    assert_eq!(duplicate.comment, None);
    Ok(())
}

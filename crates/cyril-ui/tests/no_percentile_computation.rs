//! C8 — `cyril-ui` renders latency statistics; it never computes them.
//!
//! `UsageSummary` arrives with p90/max already computed in `cyril-core`. A
//! percentile derived here would silently diverge from the persisted one and
//! would sit outside core's oracle tests. This is a structural check, not
//! "the reviewer will notice".
//!
//! The scanner deliberately lives in `tests/` rather than `src/`: its own
//! needle strings would otherwise match themselves, which is exactly what
//! happened on the first attempt.

use std::path::{Path, PathBuf};

fn read_source(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn scan(dir: &Path, offenders: &mut Vec<String>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|error| panic!("failed to read entry in {}: {error}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            scan(&path, offenders);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let body = read_source(&path);
        let shown = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .display()
            .to_string();
        if body.contains("CUME_DIST") {
            offenders.push(format!(
                "{shown}: contains a SQL percentile window function"
            ));
        }
        // A sort paired with the 0.9 quantile constant is percentile
        // computation regardless of what it is named.
        let sorts = body.contains(".sort") || body.contains("sort_unstable");
        if sorts && body.contains("0.9") {
            offenders.push(format!("{shown}: sorts and indexes at the 0.9 quantile"));
        }
    }
}

#[test]
fn ui_does_not_compute_percentiles() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    scan(&src, &mut offenders);
    assert!(
        offenders.is_empty(),
        "cyril-ui must not compute percentiles; offenders: {offenders:?}"
    );
}

#[test]
fn ui_declares_no_statistics_dependency() {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = read_source(&manifest_path);
    for banned in ["rusqlite", "statrs", "quantiles"] {
        assert!(
            !manifest.contains(banned),
            "cyril-ui must not depend on {banned}: statistics belong in cyril-core"
        );
    }
}

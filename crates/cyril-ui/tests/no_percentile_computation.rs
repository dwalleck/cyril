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
//!
//! Both checks are deliberately narrow (cyril-9kyk review). The first version
//! flagged any file containing `.sort` *anywhere* together with `0.9`
//! *anywhere* — file granularity across three `src` files that already sort,
//! so the first unrelated `0.95` opacity or `10.9` duration in any of them
//! would have failed this suite with a false accusation about code that has
//! nothing to do with percentiles. It also matched `CUME_DIST` case-sensitively
//! while `cume_dist(` — the spelling SQLite's own documentation uses and the
//! parser accepts — walked straight past, and its `|| body.contains(
//! "sort_unstable")` arm was unreachable because every such call is written
//! `.sort_unstable` and so already contained `.sort`.

use std::path::{Path, PathBuf};

/// SQL percentile window functions, matched case-insensitively.
const SQL_PERCENTILE_FUNCTIONS: [&str; 5] = [
    "cume_dist(",
    "percent_rank(",
    "percentile_cont(",
    "percentile_disc(",
    "ntile(",
];

/// How many lines apart a sort and a `0.9` quantile constant may sit and still
/// be read as one percentile computation. Statement granularity, approximated
/// by proximity — a sort at the top of a file and a ratio at the bottom are
/// unrelated, and saying otherwise is what made the first version a landmine.
const LOCALITY_LINES: usize = 6;

fn read_source(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

/// A sorting call. `.sort_unstable` and `.sort_by` both contain `.sort(`'s
/// prefix but not its paren, so each spelling is listed once and none of the
/// arms is subsumed by another.
fn sorts(line: &str) -> bool {
    line.contains(".sort(") || line.contains(".sort_by") || line.contains(".sort_unstable")
}

/// `0.9` as a standalone literal — not the `0.95` of an opacity or a ratio,
/// and not the tail of a `10.9`. Both neighbours must be non-digits.
fn quantile_constant(line: &str) -> bool {
    line.match_indices("0.9").any(|(index, needle)| {
        let before = line[..index].chars().next_back();
        let after = line[index + needle.len()..].chars().next();
        !before.is_some_and(|character| character.is_ascii_digit())
            && !after.is_some_and(|character| character.is_ascii_digit())
    })
}

/// Every reason `body` counts as computing a percentile. Split out from the
/// directory walk so the matcher can be exercised on synthetic input.
fn offenders_in(shown: &str, body: &str) -> Vec<String> {
    let mut offenders = Vec::new();
    let lowered = body.to_ascii_lowercase();
    for needle in SQL_PERCENTILE_FUNCTIONS {
        if lowered.contains(needle) {
            offenders.push(format!(
                "{shown}: calls the SQL percentile window function {needle}"
            ));
        }
    }
    let lines: Vec<&str> = body.lines().collect();
    let sort_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| sorts(line))
        .map(|(index, _)| index)
        .collect();
    for (index, line) in lines.iter().enumerate() {
        if !quantile_constant(line) {
            continue;
        }
        if let Some(sort_line) = sort_lines
            .iter()
            .find(|candidate| candidate.abs_diff(index) <= LOCALITY_LINES)
        {
            offenders.push(format!(
                "{shown}:{}: indexes at the 0.9 quantile beside the sort on line {}",
                index + 1,
                sort_line + 1
            ));
        }
    }
    offenders
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
        let shown = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .display()
            .to_string();
        offenders.extend(offenders_in(&shown, &read_source(&path)));
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

/// Positive control. An assertion that nothing matched proves nothing unless
/// the matcher can be shown to match, and the lookalike cases below are the
/// exact false positives the file-granular version would have raised.
#[test]
fn scanner_catches_percentiles_and_spares_lookalikes() {
    let caught = offenders_in(
        "fake.rs",
        "fn p90(mut values: Vec<u64>) -> u64 {\n    \
         values.sort_unstable();\n    \
         let position = (0.9 * values.len() as f64).ceil() as usize;\n    \
         values[position - 1]\n}",
    );
    assert_eq!(
        caught.len(),
        1,
        "hand-rolled percentile must be caught: {caught:?}"
    );

    let lowercase_sql = offenders_in("fake.rs", "let sql = \"SELECT cume_dist() OVER (x)\";");
    assert_eq!(
        lowercase_sql.len(),
        1,
        "the lowercase spelling SQLite documents must be caught too"
    );

    // Unrelated code that the previous file-granular check would have failed.
    let lookalike = "fn render(rows: &mut Vec<Row>) {\n    \
         rows.sort_by(|a, b| a.name.cmp(&b.name));\n}\n\n\
         const DIM: f64 = 0.95;\n\
         const HOLD: Duration = Duration::from_secs_f64(10.9);\n";
    assert!(
        offenders_in("fake.rs", lookalike).is_empty(),
        "a sort and an unrelated 0.95 / 10.9 elsewhere in the file are not a percentile"
    );

    assert!(quantile_constant("let cut = (0.9 * len as f64).ceil();"));
    assert!(!quantile_constant("let opacity = 0.95;"));
    assert!(!quantile_constant("Duration::from_secs_f64(10.9)"));
    assert!(sorts("values.sort_unstable();"));
    assert!(sorts("rows.sort_by(|a, b| a.cmp(b));"));
    assert!(sorts("items.sort();"));
    assert!(!sorts("let sorted = already_sorted;"));
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

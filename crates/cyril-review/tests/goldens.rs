//! Golden runs: the native crtool against the Python crtool's captured
//! outputs (tests/goldens/capture.py), under docs/crtool-contract.md's
//! fidelity rule — identical text, value-equal JSON, same exit codes.

use cyril_review::{ReviewRun, run_check};
use serde_json::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Values in the manifest that differ by design: the run's time and the
/// native version stamp (the Python manifest has none).
const VOLATILE_MANIFEST_KEYS: [&str; 2] = ["gathered_at", "crtool_version"];

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/cases")
}

#[test]
fn range_case_matches_python() -> TestResult {
    check_case("range")
}

#[test]
fn worktree_case_matches_python() -> TestResult {
    check_case("worktree")
}

#[test]
fn auto_target_case_matches_python() -> TestResult {
    check_case("auto")
}

#[test]
fn empty_diff_case_matches_python() -> TestResult {
    check_case("empty")
}

#[test]
fn pipeline_case_matches_python() -> TestResult {
    check_case("pipeline")
}

#[test]
fn degraded_pipeline_case_matches_python() -> TestResult {
    check_case("degraded")
}

/// Output-shaping settings a user or repository may carry. The pinned diff
/// and grep options must make every one of them irrelevant.
const HOSTILE_CONFIG: [(&str, &str); 12] = [
    ("diff.noprefix", "true"),
    ("diff.mnemonicPrefix", "true"),
    ("diff.algorithm", "patience"),
    ("diff.submodule", "log"),
    ("diff.relative", "true"),
    ("diff.context", "1"),
    ("diff.indentHeuristic", "false"),
    ("diff.renames", "copies"),
    ("diff.external", "false"),
    ("color.ui", "always"),
    ("grep.column", "true"),
    ("grep.fullName", "false"),
];

#[test]
fn outputs_ignore_hostile_git_config() -> TestResult {
    for name in ["range", "worktree", "auto"] {
        check_case_with(name, &HOSTILE_CONFIG)?;
    }
    Ok(())
}

#[test]
fn unreadable_normalization_keeps_neighbouring_text() {
    // A parser message is normalized; crtool's own wording is not.
    assert_eq!(
        normalize_unreadable_text(
            "a: x unreadable (Expecting value: line 1 column 2 (char 1)); used; b: y unreadable (not a JSON object); used"
        ),
        "a: x unreadable (<error>); used; b: y unreadable (not a JSON object); used"
    );
    assert_eq!(
        normalize_unreadable_text("unreadable: Expecting value: line 1 column 1 (char 0)"),
        "unreadable: <error>"
    );
    assert_eq!(
        normalize_unreadable_text(
            "x unreadable: bad at line 1 column 3\nthe next line is still compared"
        ),
        "x unreadable: <error>\nthe next line is still compared"
    );
    assert_eq!(
        normalize_unreadable_text(
            "decisions.json unreadable (top level is not an object); no dedup"
        ),
        "decisions.json unreadable (top level is not an object); no dedup"
    );
    assert_eq!(normalize_unreadable_text("readable"), "readable");
}

#[test]
fn every_case_directory_has_a_test() -> TestResult {
    let mut names: Vec<String> = fs::read_dir(cases_dir())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    assert_eq!(
        names,
        ["auto", "degraded", "empty", "pipeline", "range", "worktree"]
    );
    Ok(())
}

fn check_case(name: &str) -> TestResult {
    check_case_with(name, &[])
}

fn check_case_with(name: &str, config: &[(&str, &str)]) -> TestResult {
    let case_dir = cases_dir().join(name);
    let case: Value = serde_json::from_slice(&fs::read(case_dir.join("case.json"))?)?;
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    build_repo(tree.path(), &repo, &case)?;
    for (key, value) in config {
        let status = Command::new("git")
            .args(["config", key, value])
            .current_dir(&repo)
            .status()?;
        assert!(status.success(), "git config {key} {value}");
    }
    let run = ReviewRun::new(&repo, tree.path().join("run"))?;

    let expected_steps: Vec<Value> =
        serde_json::from_slice(&fs::read(case_dir.join("expected/steps.json"))?)?;
    let steps = case["steps"].as_array().ok_or("case has no steps")?;
    assert_eq!(steps.len(), expected_steps.len(), "{name}: step count");
    for (step, expected) in steps.iter().zip(&expected_steps) {
        let (exit, stdout) = run_step(&run, step)?;
        assert_eq!(
            Some(i64::from(exit)),
            expected["exit"].as_i64(),
            "{name}: exit of {step}"
        );
        if exit == 0 {
            let want = expected["stdout"].as_str().ok_or("expected stdout")?;
            assert_eq!(
                normalize_unreadable_text(&normalize_elapsed(&run_placeholder(&run, &stdout))),
                normalize_unreadable_text(&normalize_elapsed(want)),
                "{name}: stdout of {step}"
            );
        }
    }

    let actual = files_under(run.dir())?;
    let expected = files_under(&case_dir.join("expected/run"))?;
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "{name}: run directory files"
    );
    for (relative, want) in &expected {
        let got = &match String::from_utf8(actual[relative].clone()) {
            Ok(text) => run_placeholder(&run, &text).into_bytes(),
            Err(_) => actual[relative].clone(),
        };
        let parsed = (
            serde_json::from_slice::<Value>(got),
            serde_json::from_slice::<Value>(want),
        );
        if let (true, (Ok(mut got), Ok(mut want))) = (relative.ends_with(".json"), parsed) {
            normalize_unreadable(&mut got);
            normalize_unreadable(&mut want);
            if relative == "manifest.json" {
                for value in [&mut got, &mut want] {
                    if let Some(object) = value.as_object_mut() {
                        for key in VOLATILE_MANIFEST_KEYS {
                            object.remove(key);
                        }
                    }
                }
            }
            assert_eq!(got, want, "{name}: {relative}");
        } else if relative == "facts/diagnostics.txt" {
            assert_eq!(
                normalize_elapsed(&String::from_utf8_lossy(got)),
                normalize_elapsed(&String::from_utf8_lossy(want)),
                "{name}: {relative}"
            );
        } else if let (Ok(got), Ok(want)) = (std::str::from_utf8(got), std::str::from_utf8(want)) {
            assert_eq!(
                normalize_unreadable_text(got),
                normalize_unreadable_text(want),
                "{name}: {relative}"
            );
        } else {
            assert!(
                got == want,
                "{name}: {relative} differs\n--- native\n{}\n--- python\n{}",
                String::from_utf8_lossy(got),
                String::from_utf8_lossy(want)
            );
        }
    }
    Ok(())
}

/// One step through the library, as `cyril crtool` (or `/review`, for the
/// check command) would run it: exit code and stdout.
fn run_step(run: &ReviewRun, step: &Value) -> TestResult<(i32, String)> {
    let parts = step.as_array().ok_or("step is not a list")?;
    let word = |index: usize| -> TestResult<&str> {
        parts
            .get(index)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("step {step}: no word {index}").into())
    };
    let result = match word(0)? {
        "write" => {
            write_run_file(run, word(1)?, parts.get(2).ok_or("write without a value")?)?;
            return Ok((0, String::new()));
        }
        "gather" => cyril_review::gather(run, word(1)?, word(2)?),
        "facts" => cyril_review::facts(run),
        // ["merge", "--expect", angles] and ["shard"] / ["shard", "--shards", n],
        // spelled as the recipe's command lines.
        "merge" => cyril_review::merge(run, word(2)?),
        "shard" => {
            let shards = if parts.len() > 1 {
                word(2)?.parse()?
            } else {
                4
            };
            cyril_review::shard(run, shards)
        }
        "ballots" => cyril_review::ballots(run),
        "collate" => cyril_review::collate(run),
        "finalize" => cyril_review::finalize(run),
        // ["comments"] or ["comments", "--no-trailer"]
        "comments" => cyril_review::comments(run, parts.len() == 1),
        "diagnostics" => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime
                .block_on(run_check(
                    run,
                    word(1)?,
                    Duration::from_secs(120),
                    std::future::pending(),
                ))
                .map(|result| format!("{}\n", result.summary()))
        }
        other => return Err(format!("unknown step {other:?}").into()),
    };
    Ok(match result {
        Ok(stdout) => (0, stdout),
        Err(error) => (error.exit_code(), String::new()),
    })
}

/// An agent's output: a JSON value, or a string written verbatim.
fn write_run_file(run: &ReviewRun, relative: &str, value: &Value) -> TestResult {
    let path = run.dir().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other)?,
    };
    fs::write(path, text)?;
    Ok(())
}

/// The run directory's absolute path, as queue files spell it.
fn run_placeholder(run: &ReviewRun, text: &str) -> String {
    text.replace(&run.dir().to_string_lossy().replace('\\', "/"), "<RUN>")
}

/// Parse-error wording is the JSON parser's own (Python's `json` there,
/// serde_json here): keep that something was unreadable, not the words.
/// Handles `unreadable: <error>` (to the end) and `unreadable (<error>)`
/// (to the balancing parenthesis), everywhere in the text.
fn normalize_unreadable_text(text: &str) -> String {
    const MARK: &str = "unreadable";
    // Only a parser's message, which names a position; crtool's own wording
    // ("not a JSON object", "top level is not an object") is compared exactly.
    let parser_message = |message: &str| message.contains("line ") && message.contains(" column ");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        let (head, tail) = rest.split_at(at + MARK.len());
        out.push_str(head);
        rest = tail;
        if let Some(after) = tail.strip_prefix(": ") {
            // The error runs to the end of its line, never further.
            let end = after.find('\n').unwrap_or(after.len());
            if parser_message(&after[..end]) {
                out.push_str(": <error>");
                rest = &after[end..];
            }
        } else if let Some(inner) = tail.strip_prefix(" (") {
            let mut depth = 1;
            let close = inner.char_indices().find_map(|(index, character)| {
                match character {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(index)
            });
            if let Some(close) = close.filter(|close| parser_message(&inner[..*close])) {
                out.push_str(" (<error>)");
                rest = &inner[close + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn normalize_unreadable(value: &mut Value) {
    match value {
        Value::String(text) => *text = normalize_unreadable_text(text),
        Value::Array(items) => items.iter_mut().for_each(normalize_unreadable),
        Value::Object(fields) => fields.values_mut().for_each(normalize_unreadable),
        _ => {}
    }
}

/// Elapsed seconds are the one value no two runs share.
fn normalize_elapsed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(" in ") {
        let (head, tail) = rest.split_at(start + 4);
        out.push_str(head);
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && tail[digits..].starts_with('s') {
            out.push('N');
            rest = &tail[digits..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn files_under(root: &Path) -> TestResult<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        if !directory.exists() {
            continue;
        }
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, fs::read(&path)?);
            }
        }
    }
    Ok(files)
}

/// Must stay in step with build() in tests/goldens/capture.py.
fn build_repo(home: &Path, repo: &Path, case: &Value) -> TestResult {
    let config = home.join("gitconfig");
    fs::write(&config, "")?;
    fs::create_dir_all(repo)?;
    let git = |args: &[&str], date: Option<&str>| -> TestResult {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", &config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Golden")
            .env("GIT_AUTHOR_EMAIL", "golden@example.invalid")
            .env("GIT_COMMITTER_NAME", "Golden")
            .env("GIT_COMMITTER_EMAIL", "golden@example.invalid");
        if let Some(date) = date {
            command
                .env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_DATE", date);
        }
        let output = command.output()?;
        if !output.status.success() {
            return Err(
                format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into(),
            );
        }
        Ok(())
    };
    git(&["init", "-q", "-b", "main"], None)?;
    git(&["config", "core.autocrlf", "false"], None)?;
    for (index, commit) in case["commits"]
        .as_array()
        .ok_or("case has no commits")?
        .iter()
        .enumerate()
    {
        if let Some(branch) = commit["branch"].as_str() {
            git(&["checkout", "-q", "-b", branch], None)?;
        }
        apply(repo, commit)?;
        git(&["add", "-A"], None)?;
        let date = format!("2026-01-{:02}T00:00:00Z", index + 1);
        let message = commit["message"]
            .as_str()
            .map_or_else(|| format!("commit {}", index + 1), str::to_owned);
        git(
            &["commit", "-q", "--no-verify", "-m", &message],
            Some(&date),
        )?;
    }
    if let Some(worktree) = case.get("worktree") {
        apply(repo, worktree)?;
    }
    Ok(())
}

fn apply(repo: &Path, change: &Value) -> TestResult {
    let write = |path: &str, bytes: &[u8]| -> TestResult {
        let target = repo.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, bytes)?;
        Ok(())
    };
    for (path, text) in change["files"].as_object().into_iter().flatten() {
        write(
            path,
            text.as_str().ok_or("file text is not a string")?.as_bytes(),
        )?;
    }
    for (path, data) in change["binary"].as_object().into_iter().flatten() {
        let bytes: Vec<u8> = data
            .as_array()
            .ok_or("binary data is not a list")?
            .iter()
            .map(|byte| {
                byte.as_u64()
                    .and_then(|byte| u8::try_from(byte).ok())
                    .ok_or("bad byte")
            })
            .collect::<Result<_, _>>()?;
        write(path, &bytes)?;
    }
    for path in change["delete"].as_array().into_iter().flatten() {
        fs::remove_file(repo.join(path.as_str().ok_or("delete path is not a string")?))?;
    }
    Ok(())
}

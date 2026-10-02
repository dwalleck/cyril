//! The review policy against the vectors it shares with the Python driver's
//! self-test (`experiments/code-review-workflow/selftest_crtool.py`): both
//! implementations must decide every case the same way.

use cyril_core::review::consent::PermissionConsent;
use cyril_core::review::policy::{PolicyScope, decide};
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

fn forward(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn rust_policy_decides_every_shared_vector_like_the_python_driver() -> Result<(), Box<dyn Error>> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review_policy_vectors.json");
    let vectors: Value = serde_json::from_slice(&fs::read(fixture)?)?;
    let tree = tempfile::tempdir()?;
    fs::create_dir_all(tree.path().join("policy-ws"))?;
    let workspace = tree.path().join("policy-ws").canonicalize()?;
    let run_dir = workspace.join(".code-review").join("r");
    fs::create_dir_all(&run_dir)?;
    fs::create_dir_all(workspace.join("src"))?;
    let crtool = vectors["crtool"].as_str().ok_or("fixture has no crtool")?;
    let fill = |text: &str| {
        text.replace("<WS>", &forward(&workspace))
            .replace("<RUN>", &forward(&run_dir))
            .replace("<CRTOOL>", crtool)
    };
    let scope = PolicyScope::new(workspace.clone(), run_dir.clone(), crtool.to_owned());
    let cases = vectors["cases"].as_array().ok_or("fixture has no cases")?;
    assert!(cases.len() >= 20, "the shared vectors went missing");
    let mut mismatches = Vec::new();
    for case in cases {
        let request = &case["request"];
        let field = |key: &str| request[key].as_str().map(fill);
        let raw_commands: Vec<String> = ["raw_command", "raw_cmd"]
            .into_iter()
            .filter_map(field)
            .collect();
        let consent = PermissionConsent::new(
            field("capability").as_deref(),
            field("resource"),
            field("workspace_root").map(PathBuf::from),
            field("tool_id"),
            field("command"),
            raw_commands,
        );
        let decision = decide(&consent, &scope);
        if Some(decision.allowed()) != case["allow"].as_bool() {
            mismatches.push(format!("{}: {}", case["name"], decision.reason()));
        }
    }
    assert!(
        mismatches.is_empty(),
        "policy disagrees with the shared vectors:\n{}",
        mismatches.join("\n")
    );
    Ok(())
}

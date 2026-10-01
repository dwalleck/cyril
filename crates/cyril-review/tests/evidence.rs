use cyril_review::{ReviewClock, ReviewError, ReviewRun, facts, gather};
use serde_json::{Value, json};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

struct FixedClock {
    gathered: String,
}

impl ReviewClock for FixedClock {
    fn gathered_at_utc(&self) -> cyril_review::Result<String> {
        Ok(self.gathered.clone())
    }
}

fn git_output<A>(repo: &Path, args: &[A]) -> Result<Output, Box<dyn Error>>
where
    A: AsRef<OsStr> + std::fmt::Debug,
{
    let global_config = repo.join(".empty-global-gitconfig");
    if !global_config.exists() {
        fs::write(&global_config, b"")?;
    }
    Ok(Command::new("git")
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Evidence")
        .env("GIT_AUTHOR_EMAIL", "evidence@example.invalid")
        .env("GIT_COMMITTER_NAME", "Evidence")
        .env("GIT_COMMITTER_EMAIL", "evidence@example.invalid")
        .args(args)
        .current_dir(repo)
        .output()?)
}

fn git<A>(repo: &Path, args: &[A]) -> Result<Output, Box<dyn Error>>
where
    A: AsRef<OsStr> + std::fmt::Debug,
{
    let output = git_output(repo, args)?;
    if !output.status.success() {
        return Err(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn init_repo(repo: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(repo.join("src"))?;
    git(repo, &["init", "-q", "-b", "work"])?;
    Ok(())
}

fn commit(repo: &Path, message: &str) -> Result<(), Box<dyn Error>> {
    git(repo, &["add", "src/lib.rs"])?;
    git(
        repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", message],
    )?;
    Ok(())
}

fn prepared_repo(
    tree: &tempfile::TempDir,
    baseline: &str,
    changed: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    fs::write(repo.join("src/lib.rs"), baseline)?;
    commit(&repo, "baseline")?;
    fs::write(repo.join("src/lib.rs"), changed)?;
    Ok(repo)
}

fn gathered_run(repo: &Path, directory: &Path) -> Result<ReviewRun, Box<dyn Error>> {
    let run = ReviewRun::new(repo.to_path_buf(), directory.to_path_buf())?;
    let clock = FixedClock {
        gathered: "2026-09-30T12:34:56+00:00".to_owned(),
    };
    gather(&run, "HEAD", "src", &clock)?;
    Ok(run)
}

type ArtifactSnapshot = Vec<(PathBuf, Vec<u8>)>;

fn artifact_snapshot(run: &ReviewRun) -> Result<ArtifactSnapshot, Box<dyn Error>> {
    let paths = [
        "manifest.json",
        "diff.patch",
        "changed-files.txt",
        "patches/001.patch",
        "facts/symbols.json",
        "facts/usages-1.txt",
    ];
    paths
        .iter()
        .map(|relative| {
            let path = run.directory().join(relative);
            let bytes = fs::read(&path)?;
            Ok((path, bytes))
        })
        .collect()
}

fn assert_snapshot(expected: &[(PathBuf, Vec<u8>)]) -> Result<(), Box<dyn Error>> {
    for (path, bytes) in expected {
        assert_eq!(bytes, &fs::read(path)?);
    }
    Ok(())
}

fn mutate_manifest(
    manifest: &mut Value,
    field: &str,
    replacement: Option<Value>,
) -> Result<(), Box<dyn Error>> {
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("manifest is not an object"))?;
    if let Some(file_field) = field.strip_prefix("file.") {
        let files = object
            .get_mut("files")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| std::io::Error::other("manifest files is not an array"))?;
        let file = files
            .first_mut()
            .and_then(Value::as_object_mut)
            .ok_or_else(|| std::io::Error::other("manifest has no file record"))?;
        if let Some(value) = replacement {
            drop(file.insert(file_field.to_owned(), value));
        } else {
            drop(file.remove(file_field));
        }
    } else if let Some(value) = replacement {
        drop(object.insert(field.to_owned(), value));
    } else {
        drop(object.remove(field));
    }
    Ok(())
}

fn assert_manifest_refusal(error: ReviewError) {
    assert!(matches!(
        error,
        ReviewError::InvalidManifest { .. }
            | ReviewError::MissingStamp
            | ReviewError::StampMismatch { .. }
    ));
}

#[cfg(unix)]
fn git_diff_for_path(repo: &Path, path: &OsStr) -> Result<Vec<u8>, Box<dyn Error>> {
    let args = [
        OsStr::new("diff"),
        OsStr::new("--no-renames"),
        OsStr::new("HEAD"),
        OsStr::new("--"),
        path,
    ];
    Ok(git(repo, &args)?.stdout)
}

#[test]
fn gather_and_facts_use_real_git_bytes_and_preserve_identity() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    fs::write(repo.join("src/lib.rs"), "// baseline marker\n")?;
    fs::write(repo.join("NOTES.md"), "baseline\n")?;
    git(&repo, &["add", "src/lib.rs", "NOTES.md"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;

    fs::write(
        repo.join("src/lib.rs"),
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    fs::write(repo.join("NOTES.md"), "updated notes\n")?;

    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    let clock = FixedClock {
        gathered: "2026-09-30T12:34:56+00:00".to_owned(),
    };
    gather(&run, "HEAD", "src", &clock)?;
    let expected_full_diff = git(&repo, &["diff", "--no-renames", "HEAD", "--", "src"])?.stdout;
    let expected_file_diff =
        git(&repo, &["diff", "--no-renames", "HEAD", "--", "src/lib.rs"])?.stdout;
    assert_eq!(expected_full_diff, expected_file_diff);
    let expected_head = String::from_utf8(git(&repo, &["rev-parse", "HEAD"])?.stdout)?;

    let manifest_path = run.directory().join("manifest.json");
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    assert_eq!(manifest["requested_target"], json!("HEAD"));
    assert_eq!(manifest["target"], json!("HEAD"));
    assert_eq!(manifest["scope"], json!(["src"]));
    assert_eq!(manifest["head"].as_str(), Some(expected_head.trim()));

    assert_eq!(manifest["worktree_matches_diff_head"], json!(true));
    assert_eq!(manifest["gathered_at"], json!("2026-09-30T12:34:56+00:00"));
    assert_eq!(manifest["crtool_version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(manifest["total_files"], json!(1));
    assert_eq!(
        manifest["total_patch_bytes"],
        json!(expected_full_diff.len())
    );
    assert_eq!(manifest["warnings"], json!([]));
    assert_eq!(
        manifest["change_docs"],
        json!([{"path": "NOTES.md", "bytes": 14}])
    );
    assert_eq!(
        fs::read_to_string(repo.join("NOTES.md"))?,
        "updated notes\n"
    );

    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("manifest files is not an array"))?;
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], json!("src/lib.rs"));
    assert_eq!(files[0]["status"], json!("M"));
    assert_eq!(files[0]["binary"], json!(false));
    assert_eq!(files[0]["patch_bytes"], json!(expected_file_diff.len()));
    assert!(!run.directory().join("patches/002.patch").exists());

    assert_eq!(
        fs::read(run.directory().join("diff.patch"))?,
        expected_full_diff
    );
    assert_eq!(
        fs::read(run.directory().join("patches/001.patch"))?,
        expected_file_diff
    );

    let symbols: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    let symbols = symbols
        .as_array()
        .ok_or_else(|| std::io::Error::other("symbols is not an array"))?;
    let helper = symbols
        .iter()
        .find(|symbol| symbol["name"] == "helper")
        .ok_or_else(|| std::io::Error::other("helper symbol is missing"))?;
    assert_eq!(helper["kind"], json!("fn"));
    assert_eq!(helper["file"], json!("src/lib.rs"));
    assert_eq!(helper["line"], json!(2));
    assert_eq!(helper["status"], json!("added"));
    assert_eq!(helper["usage_count"], json!(1));
    assert_eq!(
        helper["usages"],
        json!([{
            "file": "src/lib.rs",
            "line": 3,
            "text": "pub fn caller() { helper(); }"
        }])
    );
    let caller = symbols
        .iter()
        .find(|symbol| symbol["name"] == "caller")
        .ok_or_else(|| std::io::Error::other("caller symbol is missing"))?;
    assert_eq!(caller["kind"], json!("fn"));
    assert_eq!(caller["file"], json!("src/lib.rs"));
    assert_eq!(caller["line"], json!(3));
    assert_eq!(caller["status"], json!("added"));
    assert_eq!(caller["usage_count"], json!(0));
    assert_eq!(caller["usages"], json!([]));

    let first_patch = fs::read(run.directory().join("diff.patch"))?;
    let first_file_patch = fs::read(run.directory().join("patches/001.patch"))?;
    let first_symbols = fs::read(run.directory().join("facts/symbols.json"))?;
    fs::write(
        repo.join("src/lib.rs"),
        "// changed after the gathered snapshot\npub fn later() {}\n",
    )?;
    let changed_diff = git(&repo, &["diff", "--no-renames", "HEAD", "--", "src"])?.stdout;
    assert_ne!(first_patch, changed_diff);
    gather(&run, "HEAD", "src", &clock)?;
    assert_eq!(first_patch, fs::read(run.directory().join("diff.patch"))?);
    assert_eq!(
        first_file_patch,
        fs::read(run.directory().join("patches/001.patch"))?
    );
    assert_eq!(
        first_symbols,
        fs::read(run.directory().join("facts/symbols.json"))?
    );
    assert_eq!(manifest_bytes, fs::read(&manifest_path)?);

    let identity_error = gather(&run, "HEAD~1", "src", &clock).expect_err("identity must be fixed");
    assert!(matches!(identity_error, ReviewError::ExistingRun { .. }));
    assert_eq!(first_patch, fs::read(run.directory().join("diff.patch"))?);
    assert_eq!(manifest_bytes, fs::read(&manifest_path)?);

    git(&repo, &["branch", "main", "HEAD"])?;
    git(&repo, &["branch", "master", "HEAD"])?;
    git(&repo, &["add", "src/lib.rs", "NOTES.md"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "change"],
    )?;
    let auto_run = ReviewRun::new(repo.clone(), tree.path().join("auto-run"))?;
    let auto = gather(&auto_run, "auto", "", &clock)?;
    let auto_stdout = String::from_utf8(auto.stdout().to_vec())?;
    assert!(auto_stdout.contains("target=main...HEAD"));
    let auto_manifest: Value =
        serde_json::from_slice(&fs::read(auto_run.directory().join("manifest.json"))?)?;
    assert_eq!(auto_manifest["target"], json!("main...HEAD"));
    Ok(())
}

#[test]
fn gather_public_operation_preserves_valid_unicode_filename_identity() -> Result<(), Box<dyn Error>>
{
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    fs::create_dir_all(repo.join("docs"))?;

    let definition_path = repo.join("src/λ-helper.rs");
    let caller_path = repo.join("src/λ-caller.rs");
    let document_path = repo.join("docs/λ-notes.md");
    fs::write(repo.join("src/base.rs"), "// baseline\n")?;
    fs::write(&document_path, "baseline\n")?;
    git(&repo, &["add", "--all"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;

    fs::write(&definition_path, "pub fn unicode_shared() {}\n")?;
    fs::write(&caller_path, "unicode_shared();\n")?;
    fs::write(&document_path, "updated Unicode document\n")?;
    git(&repo, &["add", "--all"])?;

    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    let clock = FixedClock {
        gathered: "2026-09-30T12:34:56+00:00".to_owned(),
    };
    gather(&run, "HEAD", "src", &clock)?;

    let symbols: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    let symbols = symbols
        .as_array()
        .ok_or_else(|| std::io::Error::other("symbols is not an array"))?;
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0]["name"], json!("unicode_shared"));
    assert_eq!(symbols[0]["file"], json!("src/λ-helper.rs"));
    assert_eq!(symbols[0]["line"], json!(1));
    assert_eq!(symbols[0]["usage_count"], json!(1));
    assert_eq!(
        symbols[0]["usages"],
        json!([{
            "file": "src/λ-caller.rs",
            "line": 1,
            "text": "unicode_shared();"
        }])
    );

    let manifest: Value =
        serde_json::from_slice(&fs::read(run.directory().join("manifest.json"))?)?;
    assert_eq!(
        manifest["change_docs"],
        json!([{"path": "docs/λ-notes.md", "bytes": 25}])
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn gather_public_operation_preserves_raw_filename_identity() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    fs::create_dir_all(repo.join("docs"))?;

    let invalid_name = OsString::from_vec(b"src/invalid-\xff.rs".to_vec());
    let replacement_name = OsString::from("src/invalid-\u{fffd}.rs");
    let usage_name = OsString::from("src/use:\nfile.rs");
    let invalid_doc_name = OsString::from_vec(b"docs/changed-\xff.txt".to_vec());
    let replacement_doc_name = OsString::from("docs/changed-\u{fffd}.txt");
    let invalid_path = repo.join(&invalid_name);
    let replacement_path = repo.join(&replacement_name);
    let usage_path = repo.join(&usage_name);
    let invalid_doc_path = repo.join(&invalid_doc_name);
    let replacement_doc_path = repo.join(&replacement_doc_name);

    fs::write(&invalid_path, "pub fn shared() {}\n")?;
    fs::write(&invalid_doc_path, "invalid baseline\n")?;
    fs::write(&replacement_doc_path, "replacement baseline\n")?;
    git(&repo, &["add", "--all"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;

    fs::write(&invalid_path, "pub fn shared() { invalid_body(); }\n")?;
    fs::write(
        &replacement_path,
        "pub fn shared() { replacement_body(); }\n",
    )?;
    fs::write(&usage_path, "shared();\n")?;
    fs::write(&invalid_doc_path, "invalid changed document\n")?;
    fs::write(
        &replacement_doc_path,
        "replacement changed document with more bytes\n",
    )?;
    git(&repo, &["add", "--all"])?;

    let expected_invalid_patch = git_diff_for_path(&repo, invalid_name.as_os_str())?;
    let expected_replacement_patch = git_diff_for_path(&repo, replacement_name.as_os_str())?;
    let expected_usage_patch = git_diff_for_path(&repo, usage_name.as_os_str())?;

    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    let clock = FixedClock {
        gathered: "2026-09-30T12:34:56+00:00".to_owned(),
    };
    gather(&run, "HEAD", "src", &clock)?;

    let display_name = "src/invalid-\u{fffd}.rs";
    let usage_display = "src/use:\nfile.rs";
    let manifest: Value =
        serde_json::from_slice(&fs::read(run.directory().join("manifest.json"))?)?;
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("manifest files is not an array"))?;
    assert_eq!(files.len(), 3);

    let mut saw_invalid = false;
    let mut saw_replacement = false;
    let mut saw_usage = false;
    let mut ordered_own_bodies = Vec::new();
    for file in files {
        let path = file["path"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("file path is not a string"))?;
        let status = file["status"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("file status is not a string"))?;
        let patch_name = file["patch"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("file patch is not a string"))?;
        let patch = fs::read(run.directory().join(patch_name))?;
        assert_eq!(file["patch_bytes"], json!(patch.len()));
        if path != display_name && path != usage_display {
            return Err("manifest selected an unexpected raw filename".into());
        }
        if patch.as_slice() == expected_invalid_patch.as_slice() {
            assert_eq!(path, display_name);
            assert_eq!(status, "M");
            saw_invalid = true;
            let index = file["index"]
                .as_u64()
                .ok_or_else(|| std::io::Error::other("file index is not numeric"))?;
            ordered_own_bodies.push((index, "pub fn shared() { invalid_body(); }"));
        } else if patch.as_slice() == expected_replacement_patch.as_slice() {
            assert_eq!(path, display_name);
            assert_eq!(status, "A");
            saw_replacement = true;
            let index = file["index"]
                .as_u64()
                .ok_or_else(|| std::io::Error::other("file index is not numeric"))?;
            ordered_own_bodies.push((index, "pub fn shared() { replacement_body(); }"));
        } else if patch.as_slice() == expected_usage_patch.as_slice() {
            assert_eq!(path, usage_display);
            assert_eq!(status, "A");
            saw_usage = true;
        } else {
            return Err("per-file patch selected an unexpected raw filename".into());
        }
    }
    assert!(saw_invalid);
    assert!(saw_replacement);
    assert!(saw_usage);

    let changed_files = fs::read_to_string(run.directory().join("changed-files.txt"))?;
    assert!(
        changed_files
            .lines()
            .eq([display_name, display_name, usage_display]
                .into_iter()
                .flat_map(str::lines))
    );

    let change_docs = manifest["change_docs"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("change docs is not an array"))?;
    assert_eq!(change_docs.len(), 2);
    let invalid_doc_bytes = fs::metadata(&invalid_doc_path)?.len();
    let replacement_doc_bytes = fs::metadata(&replacement_doc_path)?.len();
    assert!(change_docs.iter().any(|document| {
        document["path"] == json!("docs/changed-\u{fffd}.txt")
            && document["bytes"] == json!(invalid_doc_bytes)
    }));
    assert!(change_docs.iter().any(|document| {
        document["path"] == json!("docs/changed-\u{fffd}.txt")
            && document["bytes"] == json!(replacement_doc_bytes)
    }));

    let symbols: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    let symbols = symbols
        .as_array()
        .ok_or_else(|| std::io::Error::other("symbols is not an array"))?;
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["name"], json!("shared"));
    assert_eq!(symbols[1]["name"], json!("shared"));
    ordered_own_bodies.sort_by_key(|(index, _)| *index);
    for (symbol, (_, own_body)) in symbols.iter().zip(ordered_own_bodies) {
        let usages = symbol["usages"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("symbol usages is not an array"))?;
        assert_eq!(symbol["file"], json!(display_name));
        assert_eq!(symbol["status"], json!("added"));
        assert_eq!(symbol["usage_count"], json!(2));
        assert_eq!(usages.len(), 2);
        let other_body = if own_body == "pub fn shared() { invalid_body(); }" {
            "pub fn shared() { replacement_body(); }"
        } else {
            "pub fn shared() { invalid_body(); }"
        };
        assert!(!usages.iter().any(|usage| usage["text"] == json!(own_body)));
        assert!(
            usages
                .iter()
                .any(|usage| usage["text"] == json!(other_body))
        );
        assert!(usages.iter().any(|usage| {
            usage["file"] == json!(usage_display)
                && usage["line"] == json!(1)
                && usage["text"] == json!("shared();")
        }));
    }

    fs::remove_file(run.directory().join("facts/symbols.json"))?;
    facts(&run)?;
    let rebuilt: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    assert_eq!(rebuilt.as_array(), Some(symbols));

    Ok(())
}

#[test]
fn run_paths_are_lexically_absolute_without_existing_directories() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let unique = tree
        .path()
        .file_name()
        .ok_or_else(|| std::io::Error::other("tempdir has no final component"))?;
    let relative_root = PathBuf::from(".cyril-review-evidence").join(unique);
    let relative_workspace = relative_root.join("workspace");
    let relative_directory = relative_root
        .join("runs")
        .join("one")
        .join("..")
        .join("two");
    let current_directory = std::env::current_dir()?;
    let expected_workspace = current_directory.join(&relative_workspace);
    let expected_directory = current_directory
        .join(&relative_root)
        .join("runs")
        .join("two");

    let run = ReviewRun::new(relative_workspace, relative_directory)?;
    assert_eq!(run.workspace(), expected_workspace.as_path());
    assert_eq!(run.directory(), expected_directory.as_path());
    assert!(run.workspace().is_absolute());
    assert!(run.directory().is_absolute());
    assert!(!run.workspace().exists());
    assert!(!run.directory().exists());
    Ok(())
}

#[test]
fn facts_refuses_missing_or_stale_stamps_before_touching_facts() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let run = ReviewRun::new(tree.path().join("workspace"), tree.path().join("run"))?;
    let facts_directory = run.directory().join("facts");
    fs::create_dir_all(&facts_directory)?;
    let symbols = facts_directory.join("symbols.json");
    let first_page = facts_directory.join("usages-1.txt");
    fs::write(&symbols, b"symbols sentinel")?;
    fs::write(&first_page, b"page sentinel")?;
    let manifest = run.directory().join("manifest.json");

    let stale_manifest = br#"{"crtool_version":"stale"}"#;
    fs::write(&manifest, stale_manifest)?;
    let error = facts(&run).expect_err("stale stamp must refuse");
    assert!(matches!(error, ReviewError::StampMismatch { .. }));
    assert_eq!(fs::read(&symbols)?, b"symbols sentinel");
    assert_eq!(fs::read(&first_page)?, b"page sentinel");
    assert_eq!(fs::read(&manifest)?, stale_manifest);

    let missing_manifest = b"{}";
    fs::write(&manifest, missing_manifest)?;
    let error = facts(&run).expect_err("missing stamp must refuse");
    assert!(matches!(error, ReviewError::MissingStamp));
    assert_eq!(fs::read(&symbols)?, b"symbols sentinel");
    assert_eq!(fs::read(&first_page)?, b"page sentinel");
    assert_eq!(fs::read(&manifest)?, missing_manifest);
    Ok(())
}

#[test]
fn malformed_manifests_refuse_both_operations_before_replacing_artifacts()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let fields = [
        "requested_target",
        "target",
        "scope",
        "head",
        "worktree_matches_diff_head",
        "gathered_at",
        "crtool_version",
        "total_files",
        "total_patch_bytes",
        "warnings",
        "files",
        "file.index",
        "file.path",
        "file.status",
        "file.insertions",
        "file.deletions",
        "file.binary",
        "file.patch",
        "file.patch_bytes",
    ];

    let mut cases = Vec::with_capacity(fields.len() * 2 + 3);
    for &field in &fields {
        let replacement = if field == "file.insertions" || field == "file.deletions" {
            json!("wrong")
        } else {
            Value::Null
        };
        cases.push((field, None));
        cases.push((field, Some(replacement)));
    }
    cases.push(("file._raw_path_bytes", Some(json!("wrong"))));
    cases.push(("file._raw_path_bytes", Some(json!([256]))));
    cases.push((
        "file._raw_path_bytes",
        Some(json!(b"src/not-the-file.rs".to_vec())),
    ));

    for (case_index, (field, replacement)) in cases.into_iter().enumerate() {
        let run = gathered_run(&repo, &tree.path().join(format!("run-{case_index}")))?;
        let manifest_path = run.directory().join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
        mutate_manifest(&mut manifest, field, replacement)?;
        fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
        let expected = artifact_snapshot(&run)?;

        let clock = FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        };
        let error = gather(&run, "HEAD", "src", &clock).expect_err(field);
        if field == "file._raw_path_bytes" {
            assert!(matches!(error, ReviewError::InvalidManifest { .. }));
        } else {
            assert_manifest_refusal(error);
        }
        assert_snapshot(&expected)?;

        let error = facts(&run).expect_err(field);
        if field == "file._raw_path_bytes" {
            assert!(matches!(error, ReviewError::InvalidManifest { .. }));
        } else {
            assert_manifest_refusal(error);
        }
        assert_snapshot(&expected)?;
    }
    Ok(())
}

#[test]
fn facts_preserves_unknown_later_step_metadata_when_rewriting_manifest()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    let manifest_path = run.directory().join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let later_step = json!({
        "status": "pending",
        "attempt": 2,
        "records": ["kept", "as-is"]
    });
    manifest["later_step"] = later_step.clone();
    manifest["files"][0]["later_step"] = later_step.clone();
    manifest["facts"]["later_step"] = later_step.clone();
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;

    facts(&run)?;

    let rewritten: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    assert_eq!(rewritten["later_step"], later_step);
    assert_eq!(rewritten["files"][0]["later_step"], later_step);
    assert_eq!(rewritten["facts"]["later_step"], later_step);
    Ok(())
}

#[test]
fn facts_refuses_fatal_git_grep_without_replacing_prior_evidence() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    let expected = artifact_snapshot(&run)?;
    git(&repo, &["config", "grep.threads", "-1"])?;

    let error = facts(&run).expect_err("fatal git grep status must refuse");
    match error {
        ReviewError::GitFailure { args, message } => {
            assert!(args.contains("grep"));
            assert!(message.contains("grep.threads"));
        }
        other => panic!("expected a GitFailure from git grep, got {other:?}"),
    }
    assert_snapshot(&expected)?;
    Ok(())
}

#[test]
fn facts_accepts_real_git_grep_no_match_status_one() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn lonely() {}\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    fs::write(repo.join("src/lib.rs"), "// baseline marker\n")?;
    let grep = git_output(
        &repo,
        &["grep", "-n", "-w", "-F", "-I", "--", "lonely", "--", "*.rs"],
    )?;
    assert_eq!(grep.status.code(), Some(1));

    facts(&run)?;

    let symbols: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    let lonely = symbols
        .as_array()
        .and_then(|symbols| symbols.iter().find(|symbol| symbol["name"] == "lonely"))
        .ok_or_else(|| std::io::Error::other("lonely symbol is missing"))?;
    assert_eq!(lonely["usage_count"], json!(0));
    assert_eq!(lonely["usages"], json!([]));
    Ok(())
}

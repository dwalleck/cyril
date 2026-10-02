use cyril_review::{ReviewClock, ReviewError, ReviewRun, facts, gather};
use serde_json::{Value, json};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::ffi::OsString;
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStringExt;

struct FixedClock {
    gathered: String,
}

impl ReviewClock for FixedClock {
    fn gathered_at_utc(&self) -> cyril_review::Result<String> {
        Ok(self.gathered.clone())
    }

    fn diagnostics_elapsed(&self, _started: Instant) -> Duration {
        // These gather-only fixtures use an explicit zero elapsed sample.
        Duration::ZERO
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

// Standard Git semantic validation: a record's patch must be the change its own
// header path describes, proven against the current workspace by reverse-applying
// it. This validates patch correctness without pinning any output serialization.
// Textual patches only: a default binary patch carries no applicable payload.
//
// `repo` is the repository root because patch headers are root-relative.
fn assert_patch_matches_workspace(repo: &Path, patch_path: &Path) -> Result<(), Box<dyn Error>> {
    let output = git_output(
        repo,
        &[
            OsStr::new("apply"),
            OsStr::new("--reverse"),
            OsStr::new("--check"),
            patch_path.as_os_str(),
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "git apply --reverse --check {} failed: {}",
            patch_path.display(),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

// Durable artifact self-consistency: the manifest's byte counts must describe
// the patch artifacts actually written, whatever produced their bytes.
fn assert_manifest_patch_bytes(run: &ReviewRun) -> Result<(), Box<dyn Error>> {
    let manifest = review_json(run, "manifest.json")?;
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("manifest files is not an array"))?;
    for file in files {
        let patch = file["patch"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("file patch is not a string"))?;
        assert_eq!(
            file["patch_bytes"],
            json!(fs::metadata(run.directory().join(patch))?.len())
        );
    }
    assert_eq!(
        manifest["total_patch_bytes"],
        json!(fs::metadata(run.directory().join("diff.patch"))?.len())
    );
    Ok(())
}

#[test]
fn gather_and_facts_preserve_real_git_evidence_and_identity() -> Result<(), Box<dyn Error>> {
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
    let file_patch = run.directory().join("patches/001.patch");
    assert_eq!(
        files[0]["patch_bytes"],
        json!(fs::metadata(&file_patch)?.len())
    );
    assert_eq!(files[0]["patch"], json!("patches/001.patch"));
    assert!(!run.directory().join("patches/002.patch").exists());
    assert_eq!(
        fs::read_to_string(run.directory().join("changed-files.txt"))?,
        "src/lib.rs\n"
    );

    let full_patch = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(
        &full_patch,
        &[
            "a/src/lib.rs b/src/lib.rs",
            "+pub fn helper() {}",
            "+pub fn caller() { helper(); }",
        ],
    );
    assert_patch_matches_workspace(&repo, &run.directory().join("diff.patch"))?;
    let file_patch_bytes = fs::read(&file_patch)?;
    assert_patch_contains(&file_patch_bytes, &["+pub fn helper() {}"]);
    assert_patch_matches_workspace(&repo, &file_patch)?;

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
    // The stored snapshot must still describe the gathered change, not the
    // later worktree edit; re-gather must reuse it byte-for-byte.
    assert_patch_excludes(&first_patch, &["pub fn later() {}"]);
    assert_patch_contains(&first_patch, &["+pub fn helper() {}"]);
    assert_manifest_patch_bytes(&run)?;
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

// APFS rejects malformed UTF-8 before gather can run. This native Unix
// consumer keeps the NUL-framing, same-symbol and document-lookup obligations
// on admitted Unicode paths; Linux separately proves lossy-label collisions.
#[cfg(unix)]
#[test]
fn gather_public_operation_preserves_unicode_filename_framing() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    fs::create_dir_all(repo.join("docs"))?;

    let definition_name = "src/λ:\nhelper.rs";
    let other_name = "src/\u{fffd}:\nhelper.rs";
    let caller_name = "src/use:\nfile.rs";
    let document_name = "docs/λ:\nnotes.md";
    let other_document_name = "docs/\u{fffd}:\nnotes.md";
    let own_body = "pub fn shared() { unicode_body(); }";
    let other_body = "pub fn shared() { replacement_body(); }";
    let document = "updated Unicode document\n";
    let other_document = "replacement changed document with more bytes\n";

    fs::write(repo.join(definition_name), "pub fn shared() {}\n")?;
    fs::write(repo.join(document_name), "Unicode baseline\n")?;
    fs::write(repo.join(other_document_name), "replacement baseline\n")?;
    git(&repo, &["add", "--all"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;

    fs::write(repo.join(definition_name), format!("{own_body}\n"))?;
    fs::write(repo.join(other_name), format!("{other_body}\n"))?;
    fs::write(repo.join(caller_name), "shared();\n")?;
    fs::write(repo.join(document_name), document)?;
    fs::write(repo.join(other_document_name), other_document)?;
    git(&repo, &["add", "--all"])?;

    let run = gathered_run(&repo, &tree.path().join("run"))?;
    let manifest: Value =
        serde_json::from_slice(&fs::read(run.directory().join("manifest.json"))?)?;
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("manifest files is not an array"))?;
    assert_eq!(files.len(), 3);
    for (name, status, added, excluded) in [
        (
            definition_name,
            "M",
            "+pub fn shared() { unicode_body(); }",
            "replacement_body",
        ),
        (
            other_name,
            "A",
            "+pub fn shared() { replacement_body(); }",
            "unicode_body",
        ),
        (caller_name, "A", "+shared();", "pub fn shared()"),
    ] {
        let mut matching_files = files
            .iter()
            .filter(|file| file["path"].as_str() == Some(name));
        let file = matching_files
            .next()
            .unwrap_or_else(|| panic!("missing native path identity: {name:?}"));
        assert!(matching_files.next().is_none(), "duplicate path: {name:?}");
        assert_eq!(file["status"], json!(status));
        let patch_name = file["patch"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("file patch is not a string"))?;
        let patch_path = run.directory().join(patch_name);
        let patch = fs::read(&patch_path)?;
        assert_eq!(file["patch_bytes"], json!(patch.len()));
        assert_patch_contains(&patch, &[added]);
        assert_patch_excludes(&patch, &[excluded]);
        assert_patch_matches_workspace(&repo, &patch_path)?;
    }

    let change_docs = manifest["change_docs"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("change docs is not an array"))?;
    assert_eq!(change_docs.len(), 2);
    for (name, content) in [
        (document_name, document),
        (other_document_name, other_document),
    ] {
        assert!(change_docs.iter().any(|entry| {
            entry["path"] == json!(name) && entry["bytes"] == json!(content.len())
        }));
    }

    let symbols: Value =
        serde_json::from_slice(&fs::read(run.directory().join("facts/symbols.json"))?)?;
    let symbols = symbols
        .as_array()
        .ok_or_else(|| std::io::Error::other("symbols is not an array"))?;
    assert_eq!(symbols.len(), 2);
    for (name, own_body, other_name, other_body) in [
        (definition_name, own_body, other_name, other_body),
        (other_name, other_body, definition_name, own_body),
    ] {
        let mut matching_symbols = symbols
            .iter()
            .filter(|symbol| symbol["file"].as_str() == Some(name));
        let symbol = matching_symbols
            .next()
            .unwrap_or_else(|| panic!("missing same-name definition: {name:?}"));
        assert!(
            matching_symbols.next().is_none(),
            "duplicate symbol: {name:?}"
        );
        assert_eq!(symbol["name"], json!("shared"));
        assert_eq!(symbol["status"], json!("added"));
        assert_eq!(symbol["line"], json!(1));
        assert_eq!(symbol["usage_count"], json!(2));
        let usages = symbol["usages"]
            .as_array()
            .ok_or_else(|| std::io::Error::other("symbol usages is not an array"))?;
        assert_eq!(usages.len(), 2);
        assert!(!usages.iter().any(|usage| usage["text"] == json!(own_body)));
        assert!(usages.iter().any(|usage| {
            usage["file"] == json!(other_name)
                && usage["line"] == json!(1)
                && usage["text"] == json!(other_body)
        }));
        assert!(usages.iter().any(|usage| {
            usage["file"] == json!(caller_name)
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

// Linux admits both raw FF and valid U+FFFD names as distinct physical files.
// This collision proof must not claim native raw-file lookup on macOS/APFS.
#[cfg(target_os = "linux")]
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
        let patch_path = run.directory().join(patch_name);
        let patch = fs::read(&patch_path)?;
        assert_eq!(file["patch_bytes"], json!(patch.len()));
        if path != display_name && path != usage_display {
            return Err("manifest selected an unexpected raw filename".into());
        }
        // The lossy display labels collide here, so each record is identified by
        // its own changed body; the reverse-apply check then proves the patch is
        // the change to the raw path in its own header.
        let text = patch_text(&patch);
        if text.contains("+pub fn shared() { invalid_body(); }") {
            assert_eq!(path, display_name);
            assert_eq!(status, "M");
            assert_patch_excludes(&patch, &["replacement_body"]);
            saw_invalid = true;
            let index = file["index"]
                .as_u64()
                .ok_or_else(|| std::io::Error::other("file index is not numeric"))?;
            ordered_own_bodies.push((index, "pub fn shared() { invalid_body(); }"));
        } else if text.contains("+pub fn shared() { replacement_body(); }") {
            assert_eq!(path, display_name);
            assert_eq!(status, "A");
            assert_patch_excludes(&patch, &["invalid_body"]);
            saw_replacement = true;
            let index = file["index"]
                .as_u64()
                .ok_or_else(|| std::io::Error::other("file index is not numeric"))?;
            ordered_own_bodies.push((index, "pub fn shared() { replacement_body(); }"));
        } else if text.contains("+shared();") {
            assert_eq!(path, usage_display);
            assert_eq!(status, "A");
            assert_patch_excludes(&patch, &["pub fn shared()"]);
            saw_usage = true;
        } else {
            return Err("per-file patch selected an unexpected raw filename".into());
        }
        assert_patch_matches_workspace(&repo, &patch_path)?;
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

fn seed_prior_facts(run: &ReviewRun) -> Result<(), Box<dyn Error>> {
    fs::write(
        run.directory().join("facts/symbols.json"),
        br#"[{"name":"prior-symbol-not-in-current-source"}]"#,
    )?;
    fs::write(
        run.directory().join("facts/usages-1.txt"),
        b"prior first-page evidence, not regenerated output",
    )?;
    fs::write(
        run.directory().join("facts/usages-99.txt"),
        b"prior stale-page evidence that must not be deleted",
    )?;
    Ok(())
}

fn facts_shape_refusal(rebuild_through_gather: bool) -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    for (index, shape) in [json!(17), json!(["not an object"])]
        .into_iter()
        .enumerate()
    {
        let run = gathered_run(&repo, &tree.path().join(format!("run-{index}")))?;
        seed_prior_facts(&run)?;
        let path = run.directory().join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
        manifest["facts"] = shape;
        fs::write(&path, serde_json::to_vec(&manifest)?)?;
        let mut expected = artifact_snapshot(&run)?;
        let stale_page = run.directory().join("facts/usages-99.txt");
        expected.push((stale_page.clone(), fs::read(&stale_page)?));
        let symbols = run.directory().join("facts/symbols.json");
        if rebuild_through_gather {
            fs::remove_file(&symbols)?;
            expected.retain(|(path, _)| path != &symbols);
        }
        let result = if rebuild_through_gather {
            gather(
                &run,
                "HEAD",
                "src",
                &FixedClock {
                    gathered: "2026-09-30T12:34:56+00:00".to_owned(),
                },
            )
        } else {
            facts(&run)
        };
        assert_snapshot(&expected)?;
        if rebuild_through_gather {
            assert!(!symbols.exists(), "refusal created replacement symbols");
        }
        assert!(matches!(result, Err(ReviewError::InvalidManifest { .. })));
    }
    Ok(())
}

#[test]
fn facts_refuses_nonobject_metadata_before_replacing_symbols_or_deleting_pages()
-> Result<(), Box<dyn Error>> {
    facts_shape_refusal(false)
}

#[test]
fn gather_rebuild_refuses_nonobject_metadata_before_creating_symbols_or_deleting_pages()
-> Result<(), Box<dyn Error>> {
    facts_shape_refusal(true)
}

fn invalid_utf8_manifest_refusal(reuse_through_gather: bool) -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    seed_prior_facts(&run)?;
    let path = run.directory().join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
    manifest["later_step"] = json!("invalid-utf8-marker");
    let mut bytes = serde_json::to_vec(&manifest)?;
    let offset = bytes
        .windows(b"invalid-utf8-marker".len())
        .position(|window| window == b"invalid-utf8-marker")
        .ok_or("invalid UTF-8 fixture marker missing from serialized JSON")?;
    bytes[offset] = 0xff;
    fs::write(&path, &bytes)?;
    let mut expected = artifact_snapshot(&run)?;
    let stale_page = run.directory().join("facts/usages-99.txt");
    expected.push((stale_page.clone(), fs::read(&stale_page)?));
    let result = if reuse_through_gather {
        gather(
            &run,
            "HEAD",
            "src",
            &FixedClock {
                gathered: "2026-09-30T12:34:56+00:00".to_owned(),
            },
        )
    } else {
        facts(&run)
    };
    assert_snapshot(&expected)?;
    assert!(matches!(result, Err(ReviewError::Json { .. })));
    Ok(())
}

#[test]
fn facts_refuses_invalid_utf8_unknown_metadata_without_replacing_artifacts()
-> Result<(), Box<dyn Error>> {
    invalid_utf8_manifest_refusal(false)
}

#[test]
fn gather_reuse_refuses_invalid_utf8_unknown_metadata_without_replacing_artifacts()
-> Result<(), Box<dyn Error>> {
    invalid_utf8_manifest_refusal(true)
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
    let later_step = json!({
        "status": "pending",
        "attempt": 2,
        "records": ["café", "東京", "\u{20000}"]
    });
    for (index, facts_metadata) in [
        None,
        Some(Value::Null),
        Some(json!({"later_step": later_step})),
    ]
    .into_iter()
    .enumerate()
    {
        for rebuild_through_gather in [false, true] {
            let run = gathered_run(
                &repo,
                &tree
                    .path()
                    .join(format!("run-{index}-{rebuild_through_gather}")),
            )?;
            let manifest_path = run.directory().join("manifest.json");
            let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
            manifest["later_step"] = later_step.clone();
            manifest["files"][0]["later_step"] = later_step.clone();
            mutate_manifest(&mut manifest, "facts", facts_metadata.clone())?;
            fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
            if rebuild_through_gather {
                fs::remove_file(run.directory().join("facts/symbols.json"))?;
                gather(
                    &run,
                    "HEAD",
                    "src",
                    &FixedClock {
                        gathered: "2026-09-30T12:34:56+00:00".to_owned(),
                    },
                )?;
            } else {
                facts(&run)?;
            }
            let rewritten: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
            assert_eq!(rewritten["later_step"], later_step);
            assert_eq!(rewritten["files"][0]["later_step"], later_step);
            if facts_metadata.as_ref().is_some_and(Value::is_object) {
                assert_eq!(rewritten["facts"]["later_step"], later_step);
            }
            assert_eq!(rewritten["facts"]["symbols"], json!(2));
            assert_eq!(
                rewritten["facts"]["usages_pages"],
                json!(["facts/usages-1.txt"])
            );
        }
    }
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
fn facts_refuses_truncated_persisted_hunk_without_replacing_prior_evidence()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    // This patch promises one deleted line but ends before supplying it.
    // Decoder framing must not become fabricated source-change content.
    fs::write(
        run.directory().join("patches/001.patch"),
        b"diff --git a/src/lib.rs b/src/lib.rs\nindex 1111111..2222222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +0,0 @@\n",
    )?;
    let expected = artifact_snapshot(&run)?;
    let error = facts(&run).expect_err("truncated source hunk must be refused");
    assert_eq!(error.exit_code(), 2);
    match error {
        ReviewError::GitOperation { operation, .. } => {
            assert!(operation.contains("patches/001.patch"), "{operation}");
        }
        other => panic!("expected stored-patch GitOperation refusal, got {other:?}"),
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

fn review_json(run: &ReviewRun, relative: &str) -> Result<Value, Box<dyn Error>> {
    Ok(serde_json::from_slice(&fs::read(
        run.directory().join(relative),
    )?)?)
}

// Semantic patch checks: fixtures assert the intended consumer-visible change
// (selected raw path, fixture-chosen content, own-definition/caller identities)
// rather than byte-identical Git CLI output, which no backend must reproduce.
fn patch_text(patch: &[u8]) -> String {
    String::from_utf8_lossy(patch).into_owned()
}

fn assert_patch_contains(patch: &[u8], needles: &[&str]) {
    let text = patch_text(patch);
    for needle in needles.iter().copied() {
        assert!(
            text.contains(needle),
            "patch is missing {needle:?}:\n{text}"
        );
    }
}

fn assert_patch_excludes(patch: &[u8], needles: &[&str]) {
    let text = patch_text(patch);
    for needle in needles.iter().copied() {
        assert!(
            !text.contains(needle),
            "patch unexpectedly contains {needle:?}:\n{text}"
        );
    }
}

fn assert_helper_identity(run: &ReviewRun, file: &str) -> Result<(), Box<dyn Error>> {
    let symbols = review_json(run, "facts/symbols.json")?;
    let helper = symbols
        .as_array()
        .and_then(|symbols| symbols.iter().find(|symbol| symbol["name"] == "helper"))
        .ok_or("helper symbol is missing")?;
    assert_eq!(helper["kind"], json!("fn"));
    assert_eq!(helper["file"], json!(file));
    assert_eq!(helper["line"], json!(2));
    assert_eq!(helper["status"], json!("added"));
    assert_eq!(helper["usage_count"], json!(1));
    assert_eq!(
        helper["usages"],
        json!([{
            "file": file,
            "line": 3,
            "text": "pub fn caller() { helper(); }"
        }])
    );
    Ok(())
}

#[test]
fn gather_selects_generated_bracket_paths_literally_without_changing_caller_globs()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    git(&repo, &["config", "core.autocrlf", "false"])?;
    for file in ["src/[id].tsx", "src/i.tsx"] {
        fs::write(repo.join(file), "// baseline marker\n")?;
    }
    git(&repo, &["add", "src"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;
    fs::write(
        repo.join("src/[id].tsx"),
        "// baseline marker\nexport function dynamicRoute() {}\ndynamicRoute();\n",
    )?;
    fs::write(
        repo.join("src/i.tsx"),
        "// baseline marker\nexport function lookalikeRoute() {}\nlookalikeRoute();\n",
    )?;
    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    gather(
        &run,
        "HEAD",
        "src/*.tsx",
        &FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        },
    )?;
    let full = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(
        &full,
        &[
            "+export function dynamicRoute() {}",
            "+dynamicRoute();",
            "+export function lookalikeRoute() {}",
            "+lookalikeRoute();",
        ],
    );
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["scope"], json!(["src/*.tsx"]));
    assert_eq!(manifest["total_files"], json!(2));
    for (index, (file, name, other_file, other_name)) in [
        (
            "src/[id].tsx",
            "dynamicRoute",
            "src/i.tsx",
            "lookalikeRoute",
        ),
        (
            "src/i.tsx",
            "lookalikeRoute",
            "src/[id].tsx",
            "dynamicRoute",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(manifest["files"][index]["path"], json!(file));
        assert_eq!(manifest["files"][index]["status"], json!("M"));
        // The record's own raw path must select exactly its own change; the
        // bracket name is a Git pattern unless the generated selection is used.
        let patch_path = run
            .directory()
            .join(format!("patches/{:03}.patch", index + 1));
        let patch = fs::read(&patch_path)?;
        let own_path = format!("a/{file} b/{file}");
        let own_decl = format!("+export function {name}() {{}}");
        let own_call = format!("+{name}();");
        assert_patch_contains(
            &patch,
            &[own_path.as_str(), own_decl.as_str(), own_call.as_str()],
        );
        let other_path = format!("a/{other_file} b/{other_file}");
        assert_patch_excludes(&patch, &[other_path.as_str(), other_name]);
        assert_patch_matches_workspace(&repo, &patch_path)?;
        assert_eq!(manifest["files"][index]["patch_bytes"], json!(patch.len()));
        let symbols = review_json(&run, "facts/symbols.json")?;
        let symbol = symbols
            .as_array()
            .and_then(|symbols| symbols.iter().find(|symbol| symbol["name"] == name))
            .ok_or("selected route symbol is missing")?;
        assert_eq!(symbol["file"], json!(file));
        assert_eq!(symbol["kind"], json!("function"));
        assert_eq!(symbol["line"], json!(2));
        assert_eq!(symbol["usage_count"], json!(1));
        assert_eq!(
            symbol["usages"],
            json!([{"file": file, "line": 3, "text": format!("{name}();")}])
        );
    }
    Ok(())
}

// The diff is workspace-scoped, but callers outside the workspace subdirectory
// are still callers: the usage search must cover the whole repository.
fn assert_repository_wide_helper_usages(run: &ReviewRun) -> Result<(), Box<dyn Error>> {
    let symbols = review_json(run, "facts/symbols.json")?;
    let helper = symbols
        .as_array()
        .and_then(|symbols| symbols.iter().find(|symbol| symbol["name"] == "helper"))
        .ok_or("helper symbol is missing")?;
    assert_eq!(helper["file"], json!("app/src/lib.rs"));
    assert_eq!(helper["line"], json!(2));
    assert_eq!(helper["usage_count"], json!(2));
    assert_eq!(
        helper["usages"],
        json!([
            {"file": "app/src/lib.rs", "line": 3, "text": "pub fn caller() { helper(); }"},
            {"file": "src/outside.rs", "line": 1, "text": "pub fn outside_source() { helper(); }"}
        ])
    );
    Ok(())
}

fn subdirectory_evidence(repo: &Path, directory: &Path) -> Result<(), Box<dyn Error>> {
    init_repo(repo)?;
    git(repo, &["config", "core.autocrlf", "false"])?;
    fs::create_dir_all(repo.join("app/src"))?;
    fs::write(repo.join("app/src/lib.rs"), "// baseline marker\n")?;
    fs::write(repo.join("src/outside.rs"), "// outside baseline\n")?;
    fs::write(repo.join("NOTES.md"), "baseline notes\n")?;
    git(repo, &["add", "app", "src", "NOTES.md"])?;
    git(
        repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;
    fs::write(
        repo.join("app/src/lib.rs"),
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    fs::write(
        repo.join("src/outside.rs"),
        "pub fn outside_source() { helper(); }\n",
    )?;
    fs::write(repo.join("NOTES.md"), "whole-target outside document\n")?;
    git(repo, &["config", "diff.relative", "true"])?;
    let workspace = repo.join("app");
    // Positive control: diff.relative is genuinely active in this subdirectory,
    // so an unpinned scan reports cwd-relative labels and drops outside paths.
    let relative_control = git(&workspace, &["diff", "HEAD", "--", "."])?.stdout;
    assert_patch_contains(&relative_control, &["a/src/lib.rs b/src/lib.rs"]);
    assert_patch_excludes(&relative_control, &["a/app/src/lib.rs", "outside_source"]);
    let run = ReviewRun::new(workspace, directory.to_path_buf())?;
    gather(
        &run,
        "HEAD",
        ".",
        &FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        },
    )?;
    let full = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(
        &full,
        &[
            "a/app/src/lib.rs b/app/src/lib.rs",
            "+pub fn helper() {}",
            "+pub fn caller() { helper(); }",
        ],
    );
    assert_patch_excludes(&full, &["outside_source", "a/src/lib.rs b/src/lib.rs"]);
    assert_patch_matches_workspace(repo, &run.directory().join("diff.patch"))?;
    let file_patch_path = run.directory().join("patches/001.patch");
    let file_patch = fs::read(&file_patch_path)?;
    assert_patch_contains(
        &file_patch,
        &["a/app/src/lib.rs b/app/src/lib.rs", "+pub fn helper() {}"],
    );
    assert_patch_excludes(&file_patch, &["outside_source"]);
    assert_patch_matches_workspace(repo, &file_patch_path)?;
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["total_files"], json!(1));
    assert_eq!(manifest["scope"], json!(["."]));
    assert_eq!(manifest["files"][0]["path"], json!("app/src/lib.rs"));
    assert_eq!(manifest["files"][0]["status"], json!("M"));
    assert_eq!(manifest["files"][0]["patch_bytes"], json!(file_patch.len()));
    assert_eq!(
        manifest["change_docs"],
        json!([{"path": "NOTES.md", "bytes": 30}])
    );
    assert_repository_wide_helper_usages(&run)?;
    facts(&run)?;
    assert_repository_wide_helper_usages(&run)?;
    assert_eq!(
        review_json(&run, "manifest.json")?["change_docs"],
        json!([{"path": "NOTES.md", "bytes": 30}])
    );
    Ok(())
}

#[test]
fn gather_lists_submodule_pointer_change_despite_diff_submodule_config()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    let mut commits = Vec::new();
    for body in ["// first\n", "// second\n"] {
        fs::write(repo.join("src/lib.rs"), body)?;
        commit(&repo, body)?;
        let id = git(&repo, &["rev-parse", "HEAD"])?.stdout;
        commits.push(String::from_utf8(id)?.trim().to_owned());
    }
    // A gitlink needs no checked-out submodule: point it at the repo's own commits.
    for (message, id) in [("add sub", &commits[0]), ("bump sub", &commits[1])] {
        let cacheinfo = format!("160000,{id},sub");
        git(
            &repo,
            &["update-index", "--add", "--cacheinfo", cacheinfo.as_str()],
        )?;
        git(
            &repo,
            &["commit", "--no-gpg-sign", "--no-verify", "-qm", message],
        )?;
    }
    git(&repo, &["config", "diff.submodule", "log"])?;
    // Positive control: the config really rewrites the record without a header.
    let control = git(&repo, &["diff", "HEAD~1..HEAD"])?.stdout;
    assert_patch_contains(&control, &["Submodule sub "]);
    assert_patch_excludes(&control, &["diff --git"]);

    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    gather(
        &run,
        "HEAD~1..HEAD",
        ".",
        &FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        },
    )?;
    let bumped = format!("+Subproject commit {}", commits[1]);
    let full = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(&full, &["diff --git a/sub b/sub", bumped.as_str()]);
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["total_files"], json!(1));
    assert_eq!(manifest["files"][0]["path"], json!("sub"));
    assert_eq!(manifest["files"][0]["status"], json!("M"));
    Ok(())
}

#[test]
fn gather_and_facts_preserve_subdirectory_scope_with_root_labelled_evidence()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    subdirectory_evidence(&tree.path().join("repo"), &tree.path().join("run"))
}

#[cfg(unix)]
#[test]
fn gather_and_facts_preserve_repository_root_carriage_return() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo\r");
    subdirectory_evidence(&repo, &tree.path().join("run"))?;
    let root = git(&repo, &["rev-parse", "--show-toplevel"])?.stdout;
    assert!(root.ends_with(b"repo\r\n"));
    Ok(())
}

#[test]
fn facts_ignores_active_grep_color_and_column_configuration() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    git(&repo, &["config", "color.grep", "always"])?;
    git(&repo, &["config", "grep.column", "true"])?;
    let colored = git(
        &repo,
        &[
            "grep", "-n", "-z", "-w", "-F", "-I", "-e", "helper", "--", "*.rs",
        ],
    )?
    .stdout;
    assert!(colored.contains(&0x1b), "color.grep control is not active");
    let columns = git(
        &repo,
        &[
            "grep",
            "--no-color",
            "-n",
            "-z",
            "-w",
            "-F",
            "-I",
            "-e",
            "helper",
            "--",
            "*.rs",
        ],
    )?
    .stdout;
    assert!(!columns.contains(&0x1b), "--no-color must strip ANSI");
    assert!(
        columns.windows(3).any(|window| window == b"\x008\x00"),
        "grep.column control is not active"
    );
    facts(&run)?;
    assert_helper_identity(&run, "src/lib.rs")?;
    Ok(())
}

#[test]
fn gather_ignores_active_diff_color_and_portable_external_converter() -> Result<(), Box<dyn Error>>
{
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    git(&repo, &["config", "color.diff", "always"])?;
    let color_control = git(&repo, &["diff", "HEAD", "--", "src"])?.stdout;
    assert!(
        color_control.contains(&0x1b),
        "color.diff control is not active"
    );
    // Git itself is available on every supported host, unlike a Unix script.
    git(&repo, &["config", "diff.external", "git --version"])?;
    let version = git(&repo, &["--version"])?.stdout;
    let external_control = git(&repo, &["diff", "--ext-diff", "HEAD", "--", "src"])?.stdout;
    assert_eq!(
        external_control, version,
        "diff.external control is not active"
    );
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    let full = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(
        &full,
        &[
            "a/src/lib.rs b/src/lib.rs",
            "+pub fn helper() {}",
            "+pub fn caller() { helper(); }",
        ],
    );
    assert_patch_excludes(&full, &["git version"]);
    assert!(!full.contains(&0x1b), "diff.patch retained ANSI color");
    let file_patch = fs::read(run.directory().join("patches/001.patch"))?;
    assert_patch_contains(
        &file_patch,
        &["a/src/lib.rs b/src/lib.rs", "+pub fn helper() {}"],
    );
    assert!(
        !file_patch.contains(&0x1b),
        "file patch retained ANSI color"
    );
    assert_patch_matches_workspace(&repo, &run.directory().join("patches/001.patch"))?;
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["total_files"], json!(1));
    assert_eq!(manifest["files"][0]["status"], json!("M"));
    assert_eq!(manifest["files"][0]["patch_bytes"], json!(file_patch.len()));
    assert_helper_identity(&run, "src/lib.rs")?;
    Ok(())
}

#[test]
fn gather_auto_target_predicate_ignores_active_diff_poisoning() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    git(&repo, &["config", "color.diff", "always"])?;
    git(&repo, &["config", "diff.external", "git --version"])?;
    git(&repo, &["branch", "main", "HEAD"])?;
    git(&repo, &["add", "src/lib.rs"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "change"],
    )?;
    let version = git(&repo, &["--version"])?.stdout;
    let external_control = git(&repo, &["diff", "--ext-diff", "main...HEAD", "--", "src"])?.stdout;
    assert_eq!(
        external_control, version,
        "diff.external control is not active for the resolved range"
    );
    let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
    let output = gather(
        &run,
        "auto",
        "src",
        &FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        },
    )?;
    let stdout = String::from_utf8(output.stdout().to_vec())?;
    assert!(stdout.contains("target=main...HEAD"));
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["target"], json!("main...HEAD"));
    assert_eq!(manifest["total_files"], json!(1));
    assert_eq!(manifest["files"][0]["status"], json!("M"));
    let full = fs::read(run.directory().join("diff.patch"))?;
    assert_patch_contains(
        &full,
        &[
            "a/src/lib.rs b/src/lib.rs",
            "+pub fn helper() {}",
            "+pub fn caller() { helper(); }",
        ],
    );
    assert_patch_excludes(&full, &["git version"]);
    assert!(!full.contains(&0x1b), "diff.patch retained ANSI color");
    let file_patch = fs::read(run.directory().join("patches/001.patch"))?;
    assert_patch_contains(
        &file_patch,
        &["a/src/lib.rs b/src/lib.rs", "+pub fn helper() {}"],
    );
    assert_patch_matches_workspace(&repo, &run.directory().join("patches/001.patch"))?;
    assert_helper_identity(&run, "src/lib.rs")?;
    Ok(())
}

#[test]
fn gather_auto_prefers_configured_upstream_and_falls_back_when_detached()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    let mut source = "// baseline\n".to_owned();
    fs::write(repo.join("src/lib.rs"), &source)?;
    commit(&repo, "baseline")?;
    git(&repo, &["branch", "main"])?;
    source.push_str("pub fn master_base() {}\n");
    fs::write(repo.join("src/lib.rs"), &source)?;
    commit(&repo, "master base")?;
    git(&repo, &["branch", "master"])?;
    source.push_str("pub fn upstream_base() {}\n");
    fs::write(repo.join("src/lib.rs"), &source)?;
    commit(&repo, "upstream base")?;
    git(&repo, &["remote", "add", "origin", "."])?;
    git(&repo, &["update-ref", "refs/remotes/origin/review", "HEAD"])?;
    git(
        &repo,
        &["branch", "--set-upstream-to=origin/review", "work"],
    )?;
    source.push_str("pub fn review_feature() {}\n");
    fs::write(repo.join("src/lib.rs"), &source)?;
    commit(&repo, "review change")?;

    for (name, detached, target, additions) in [
        ("attached", false, "@{upstream}...HEAD", 1),
        ("detached", true, "main...HEAD", 3),
    ] {
        if detached {
            git(&repo, &["checkout", "--detach", "-q", "HEAD"])?;
        }
        let run = ReviewRun::new(repo.clone(), tree.path().join(name))?;
        let output = gather(
            &run,
            "auto",
            "src",
            &FixedClock {
                gathered: "2026-09-30T12:34:56+00:00".to_owned(),
            },
        )?;
        assert!(String::from_utf8_lossy(output.stdout()).contains(target));
        let manifest = review_json(&run, "manifest.json")?;
        assert_eq!(manifest["target"], json!(target));
        assert_eq!(manifest["total_files"], json!(1));
        assert_eq!(manifest["files"][0]["path"], json!("src/lib.rs"));
        assert_eq!(manifest["files"][0]["status"], json!("M"));
        assert_eq!(manifest["files"][0]["insertions"], json!(additions));
        assert_eq!(manifest["files"][0]["deletions"], json!(0));
        let patch = fs::read(run.directory().join("diff.patch"))?;
        assert_patch_contains(&patch, &["+pub fn review_feature() {}"]);
        let earlier_changes = ["+pub fn master_base() {}", "+pub fn upstream_base() {}"];
        if detached {
            assert_patch_contains(&patch, &earlier_changes);
        } else {
            assert_patch_excludes(&patch, &earlier_changes);
        }
        assert_patch_matches_workspace(&repo, &run.directory().join("patches/001.patch"))?;
    }
    Ok(())
}

#[test]
fn gather_preserves_binary_identity_under_active_portable_textconv() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    git(&repo, &["config", "core.autocrlf", "false"])?;
    fs::write(repo.join("src/lib.rs"), b"\0baseline binary\n")?;
    fs::write(
        repo.join(".gitattributes"),
        "src/lib.rs diff=portable-hash\n",
    )?;
    git(&repo, &["add", "src/lib.rs", ".gitattributes"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;
    fs::write(repo.join("src/lib.rs"), b"\0changed binary\n")?;
    git(
        &repo,
        &["config", "diff.portable-hash.textconv", "git hash-object"],
    )?;
    let converted = git(&repo, &["diff", "--textconv", "HEAD", "--", "src/lib.rs"])?.stdout;
    let converted_payload: Vec<_> = converted
        .split(|byte| *byte == b'\n')
        .filter(|line| {
            line.len() == 41
                && matches!(line.first(), Some(b'+') | Some(b'-'))
                && line[1..].iter().all(u8::is_ascii_hexdigit)
        })
        .collect();
    assert_eq!(
        converted_payload.len(),
        2,
        "textconv must expose both blob hashes"
    );
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    // Blob hashes can legitimately occur in binary index metadata. Only the
    // converted +/- hash lines are forbidden as source-change payload.
    for relative in ["diff.patch", "patches/001.patch"] {
        let path = run.directory().join(relative);
        let patch = fs::read(&path)?;
        for converted_line in &converted_payload {
            assert!(
                !patch
                    .split(|byte| *byte == b'\n')
                    .any(|line| line == *converted_line),
                "{relative} carries textconv-converted content"
            );
        }
        let stats = git(
            &repo,
            &[
                OsStr::new("apply"),
                OsStr::new("--numstat"),
                OsStr::new("-z"),
                path.as_os_str(),
            ],
        )?;
        assert_eq!(stats.stdout, b"-\t-\tsrc/lib.rs\0");
    }
    let manifest = review_json(&run, "manifest.json")?;
    assert_eq!(manifest["files"][0]["path"], json!("src/lib.rs"));
    assert_eq!(manifest["files"][0]["status"], json!("M"));
    assert_eq!(manifest["files"][0]["binary"], json!(true));
    assert_eq!(manifest["files"][0]["insertions"], Value::Null);
    assert_eq!(manifest["files"][0]["deletions"], Value::Null);
    assert_eq!(review_json(&run, "facts/symbols.json")?, json!([]));
    Ok(())
}

#[test]
fn gather_refuses_option_shaped_target_before_creating_run_or_outside_output()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(&tree, "// baseline\n", "pub fn helper() {}\n")?;
    let sentinel = tree.path().join("outside-output.patch");
    let target = format!("--output={}", sentinel.display());
    let run = ReviewRun::new(repo, tree.path().join("fresh-run"))?;
    let result = gather(
        &run,
        &target,
        "src",
        &FixedClock {
            gathered: "2026-09-30T12:34:56+00:00".to_owned(),
        },
    );
    assert!(!sentinel.exists(), "target option wrote outside the run");
    assert!(
        !run.directory().exists(),
        "rejected target created run layout"
    );
    let error = result.expect_err("option-shaped target must be refused");
    assert_eq!(error.exit_code(), 2);
    match error {
        ReviewError::InvalidTarget { target: rejected } => assert_eq!(rejected, target),
        other => panic!("expected InvalidTarget, got {other:?}"),
    }
    Ok(())
}

#[test]
fn facts_refuses_persisted_option_shaped_target_without_touching_artifacts()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = prepared_repo(
        &tree,
        "// baseline marker\n",
        "// baseline marker\npub fn helper() {}\npub fn caller() { helper(); }\n",
    )?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    seed_prior_facts(&run)?;
    let sentinel = tree.path().join("outside-facts.patch");
    let target = format!("--output={}", sentinel.display());
    let manifest_path = run.directory().join("manifest.json");
    let mut manifest = review_json(&run, "manifest.json")?;
    manifest["target"] = json!(target);
    fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    let mut expected = artifact_snapshot(&run)?;
    let stale_page = run.directory().join("facts/usages-99.txt");
    expected.push((stale_page.clone(), fs::read(&stale_page)?));
    let result = facts(&run);
    assert!(!sentinel.exists(), "persisted target wrote outside the run");
    assert_snapshot(&expected)?;
    let error = result.expect_err("persisted option-shaped target must be refused");
    assert_eq!(error.exit_code(), 2);
    match error {
        ReviewError::InvalidTarget { target: rejected } => assert_eq!(rejected, target),
        other => panic!("expected InvalidTarget, got {other:?}"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Modified-symbol declaration ownership: the public modified line stays null,
// but only the declaration resolved from the symbol's own Git hunk header (and
// patch provenance) may be dropped from its usages. Every fixture fixes its
// exact source lines and expected retained hit lines/text, and proves the
// persisted facts rebuild agrees on every semantic field and array order.
// ---------------------------------------------------------------------------

// Twin same-name declarations around an unrelated `bridge` declaration:
// shared=1, bridge=10, shared=16, caller=27.
const TWIN_BRIDGE_BASELINE: &str = concat!(
    "pub fn shared() {\n",
    "    let first_alpha = 1;\n",
    "    let first_beta = 2;\n",
    "    let first_gamma = 3;\n",
    "    let first_delta = 4;\n",
    "    let first_epsilon = 5;\n",
    "    first_body(&first_alpha);\n",
    "}\n",
    "\n",
    "pub fn bridge() {\n",
    "    let bridge_alpha = 6;\n",
    "    let bridge_beta = 7;\n",
    "    bridge_work(&bridge_alpha);\n",
    "}\n",
    "\n",
    "pub fn shared() {\n",
    "    let second_alpha = 8;\n",
    "    let second_beta = 9;\n",
    "    let second_gamma = 10;\n",
    "    let second_delta = 11;\n",
    "    let second_epsilon = 12;\n",
    "    let second_zeta = 13;\n",
    "    let second_eta = 14;\n",
    "    second_body(&second_alpha);\n",
    "}\n",
    "\n",
    "pub fn caller() { shared(); }\n",
);

// A test-filtered addition and an accepted added definition share this
// baseline: shared=1, caller=10.
const SINGLE_SHARED_BASELINE: &str = concat!(
    "pub fn shared() {\n",
    "    let alpha = 1;\n",
    "    let beta = 2;\n",
    "    let gamma = 3;\n",
    "    let delta = 4;\n",
    "    let epsilon = 5;\n",
    "    shared_body(&alpha);\n",
    "}\n",
    "\n",
    "pub fn caller() { shared(); }\n",
);

// One hunk whose header names the first declaration contains the second
// same-name declaration's rename: shared=1, shared=10, caller=17.
const MERGED_HUNK_BASELINE: &str = concat!(
    "pub fn shared() {\n",
    "    let alpha = 1;\n",
    "    let beta = 2;\n",
    "    let gamma = 3;\n",
    "    let delta = 4;\n",
    "    let epsilon = 5;\n",
    "    first_body(&alpha);\n",
    "}\n",
    "\n",
    "pub fn shared() {\n",
    "    let second_alpha = 6;\n",
    "    let second_beta = 7;\n",
    "    let second_gamma = 8;\n",
    "    second_body(&second_alpha);\n",
    "}\n",
    "\n",
    "pub fn caller() { shared(); }\n",
);

// Unindented outer declaration plus indented nested same-signature declaration;
// the Git header names only the unindented outer line. shared=1, nested=6,
// caller=16.
const NESTED_DECLARATION_BASELINE: &str = concat!(
    "pub fn shared() {\n",
    "    let alpha = 1;\n",
    "    let beta = 2;\n",
    "    let gamma = 3;\n",
    "    mod nested {\n",
    "        pub fn shared() { nested_body(); }\n",
    "    }\n",
    "    let delta = 4;\n",
    "    let epsilon = 5;\n",
    "    let zeta = 6;\n",
    "    let eta = 7;\n",
    "    let theta = 8;\n",
    "    outer_body(&alpha);\n",
    "}\n",
    "\n",
    "pub fn caller() { shared(); }\n",
);

// Indented same-name declarations in two modules. The custom capture driver
// below emits an unindented header that no current declaration can own:
// shared=2, shared=13, caller=18.
const LOSSY_HEADER_BASELINE: &str = concat!(
    "mod first {\n",
    "    pub fn shared() {\n",
    "        let alpha = 1;\n",
    "        let beta = 2;\n",
    "        let gamma = 3;\n",
    "        let delta = 4;\n",
    "        let epsilon = 5;\n",
    "        first_body(&alpha);\n",
    "    }\n",
    "}\n",
    "\n",
    "mod second {\n",
    "    pub fn shared() {\n",
    "        second_body();\n",
    "    }\n",
    "}\n",
    "\n",
    "pub fn caller() { shared(); }\n",
);

fn baseline_repo(tree: &tempfile::TempDir, baseline: &str) -> Result<PathBuf, Box<dyn Error>> {
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    git(&repo, &["config", "core.autocrlf", "false"])?;
    // Pin the fixture's unindented header owner, independent of builtin Rust
    // driver changes or inherited Git configuration.
    fs::write(repo.join(".gitattributes"), "src/lib.rs diff=ownership\n")?;
    git(&repo, &["config", "diff.ownership.xfuncname", "^pub fn .*"])?;
    git(&repo, &["add", ".gitattributes"])?;
    fs::write(repo.join("src/lib.rs"), baseline)?;
    commit(&repo, "baseline")?;
    Ok(repo)
}

fn twin_bridge_repo(tree: &tempfile::TempDir) -> Result<PathBuf, Box<dyn Error>> {
    baseline_repo(tree, TWIN_BRIDGE_BASELINE)
}

// A modified symbol keeps its public line null, and usage_count always counts
// exactly the retained usage list.
fn assert_symbol_facts(
    run: &ReviewRun,
    file: &str,
    name: &str,
    status: &str,
    line: Value,
    usages: Value,
) -> Result<(), Box<dyn Error>> {
    let symbols = review_json(run, "facts/symbols.json")?;
    let symbol = symbols
        .as_array()
        .and_then(|symbols| {
            symbols
                .iter()
                .find(|symbol| symbol["file"] == file && symbol["name"] == name)
        })
        .ok_or_else(|| format!("symbol {name} in {file} is missing"))?;
    assert_eq!(symbol["line"], line);
    assert_eq!(symbol["status"], json!(status));
    assert_eq!(
        symbol["usage_count"],
        json!(usages.as_array().map_or(0, Vec::len))
    );
    assert_eq!(symbol["usages"], usages);
    Ok(())
}

// Rebuild from persisted input, comparing all fields and array order rather
// than incidental JSON formatting.
fn facts_rebuild_preserves_symbols(run: &ReviewRun) -> Result<(), Box<dyn Error>> {
    let gathered = review_json(run, "facts/symbols.json")?;
    facts(run)?;
    assert_eq!(gathered, review_json(run, "facts/symbols.json")?);
    Ok(())
}

#[test]
fn facts_modified_same_signature_edits_exclude_each_resolved_owner_only()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let first_edit = TWIN_BRIDGE_BASELINE.replace("let first_delta = 4;", "let first_delta = 40;");
    fs::write(repo.join("src/lib.rs"), first_edit)?;
    let run = gathered_run(&repo, &tree.path().join("run-first"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 16, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 27, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    let second_edit = TWIN_BRIDGE_BASELINE.replace("let second_eta = 14;", "let second_eta = 140;");
    fs::write(repo.join("src/lib.rs"), second_edit)?;
    let run = gathered_run(&repo, &tree.path().join("run-second"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 27, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_owner_renamed_or_deleted_across_bridge_keeps_unrelated_survivor()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let renamed = TWIN_BRIDGE_BASELINE
        .replace(
            "pub fn shared() {\n    let second_alpha",
            "pub fn renamed_owner() {\n    let second_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "renamed_owner(&second_alpha);",
        );
    fs::write(repo.join("src/lib.rs"), renamed)?;
    let run = gathered_run(&repo, &tree.path().join("run-rename"))?;
    facts_rebuild_preserves_symbols(&run)?;
    let symbols = review_json(&run, "facts/symbols.json")?;
    let names: Vec<_> = symbols
        .as_array()
        .ok_or("symbols must be an array")?
        .iter()
        .map(|symbol| symbol["name"].clone())
        .collect();
    assert_eq!(
        names,
        vec![json!("bridge"), json!("shared"), json!("renamed_owner")]
    );
    // The renamed declaration is an added definition at its exact new line and
    // excludes only its own line, never the surviving first declaration.
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "renamed_owner",
        "added",
        json!(16),
        json!([{
            "file": "src/lib.rs",
            "line": 24,
            "text": "renamed_owner(&second_alpha);"
        }]),
    )?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 27, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    let deleted = TWIN_BRIDGE_BASELINE
        .replace(
            "pub fn shared() {\n    let second_alpha",
            "    let second_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "second_body(&second_alpha, &second_beta);",
        );
    fs::write(repo.join("src/lib.rs"), deleted)?;
    let run = gathered_run(&repo, &tree.path().join("run-delete"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 26, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_seed_keeps_first_header_owner_over_later_same_name_hunk()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let changed = TWIN_BRIDGE_BASELINE
        .replace("let first_delta = 4;", "let first_delta = 40;")
        .replace(
            "pub fn shared() {\n    let second_alpha",
            "pub fn renamed_owner() {\n    let second_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "renamed_owner(&second_alpha);",
        );
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([{"file": "src/lib.rs", "line": 27, "text": "pub fn caller() { shared(); }"}]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_owner_after_earlier_same_name_rename_or_delete_is_excluded()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let renamed = TWIN_BRIDGE_BASELINE
        .replace(
            "pub fn shared() {\n    let first_alpha",
            "pub fn renamed_first() {\n    let first_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "second_body(&second_alpha, &second_beta);",
        );
    fs::write(repo.join("src/lib.rs"), renamed)?;
    let run = gathered_run(&repo, &tree.path().join("run-rename"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([{"file": "src/lib.rs", "line": 27, "text": "pub fn caller() { shared(); }"}]),
    )?;
    let deleted = TWIN_BRIDGE_BASELINE
        .replace(
            "pub fn shared() {\n    let first_alpha",
            "    let first_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "second_body(&second_alpha, &second_beta);",
        );
    fs::write(repo.join("src/lib.rs"), deleted)?;
    let run = gathered_run(&repo, &tree.path().join("run-delete"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([{"file": "src/lib.rs", "line": 26, "text": "pub fn caller() { shared(); }"}]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_merged_hunk_removal_does_not_retarget_header_owner() -> Result<(), Box<dyn Error>>
{
    let tree = tempfile::tempdir()?;
    let repo = baseline_repo(&tree, MERGED_HUNK_BASELINE)?;
    let changed = MERGED_HUNK_BASELINE
        .replace(
            "pub fn shared() {\n    let second_alpha",
            "pub fn renamed_owner() {\n    let second_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "renamed_owner(&second_alpha);",
        );
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([{"file": "src/lib.rs", "line": 17, "text": "pub fn caller() { shared(); }"}]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_untrimmed_header_keeps_indented_nested_declaration() -> Result<(), Box<dyn Error>>
{
    let tree = tempfile::tempdir()?;
    let repo = baseline_repo(&tree, NESTED_DECLARATION_BASELINE)?;
    let changed = NESTED_DECLARATION_BASELINE.replace("let theta = 8;", "let theta = 80;");
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 6, "text": "pub fn shared() { nested_body(); }"},
            {"file": "src/lib.rs", "line": 16, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_insertion_shift_excludes_shifted_owner() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let changed = TWIN_BRIDGE_BASELINE
        .replace(
            "    bridge_work(&bridge_alpha);",
            "    bridge_work(&bridge_alpha);\n    let bridge_gamma = 8;\n    bridge_extra(&bridge_gamma);",
        )
        .replace("let second_eta = 14;", "let second_eta = 140;");
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 29, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_test_filtered_addition_cannot_impersonate_header_owner()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let changed = TWIN_BRIDGE_BASELINE
        .replace(
            "pub fn shared() {\n    let second_alpha",
            "#[test]\npub fn shared() { test_only_body();\n    let second_alpha",
        )
        .replace(
            "second_body(&second_alpha);",
            "second_body(&second_alpha, &second_beta);",
        );
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    // The replacement is compatible with the old header and lies before its
    // later body hunk. It must remain a usage, not impersonate the removed
    // owner; neither it nor the unrelated first declaration is excludable.
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 17, "text": "pub fn shared() { test_only_body();"},
            {"file": "src/lib.rs", "line": 28, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_added_definition_overwrites_modified_seed() -> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = baseline_repo(&tree, SINGLE_SHARED_BASELINE)?;
    let changed = format!(
        "{}\npub fn shared() {{ added_body(); }}\n",
        SINGLE_SHARED_BASELINE.replace("let delta = 4;", "let delta = 40;"),
    );
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "added",
        json!(12),
        json!([
            {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 10, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_incompatible_lossy_header_keeps_every_declaration() -> Result<(), Box<dyn Error>>
{
    let tree = tempfile::tempdir()?;
    let repo = tree.path().join("repo");
    init_repo(&repo)?;
    git(&repo, &["config", "core.autocrlf", "false"])?;
    fs::write(repo.join(".gitattributes"), "src/lib.rs diff=capture\n")?;
    fs::write(repo.join("src/lib.rs"), LOSSY_HEADER_BASELINE)?;
    git(&repo, &["add", "src/lib.rs", ".gitattributes"])?;
    git(
        &repo,
        &["commit", "--no-gpg-sign", "--no-verify", "-qm", "baseline"],
    )?;
    // The capture drops the indentation, so the header cannot prefix-match any
    // indented declaration and no declaration may be discarded.
    git(
        &repo,
        &[
            "config",
            "diff.capture.xfuncname",
            r"^[ \t]*(pub fn shared\(\) \{)$",
        ],
    )?;
    let changed = LOSSY_HEADER_BASELINE.replace("let epsilon = 5;", "let epsilon = 50;");
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    facts_rebuild_preserves_symbols(&run)?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 2, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 13, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 18, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    Ok(())
}

#[test]
fn facts_modified_first_owner_keeps_added_nonowner_hits_and_later_declaration()
-> Result<(), Box<dyn Error>> {
    let tree = tempfile::tempdir()?;
    let repo = twin_bridge_repo(&tree)?;
    let changed = TWIN_BRIDGE_BASELINE
        .replace("let first_delta = 4;", "let first_delta = 40;")
        .replace(
            "    bridge_work(&bridge_alpha);",
            "    shared();\n    bridge_work(&bridge_alpha);",
        )
        .replace("let second_eta = 14;", "let second_eta = 140;");
    fs::write(repo.join("src/lib.rs"), changed)?;
    let run = gathered_run(&repo, &tree.path().join("run"))?;
    assert_symbol_facts(
        &run,
        "src/lib.rs",
        "shared",
        "modified",
        Value::Null,
        json!([
            {"file": "src/lib.rs", "line": 13, "text": "shared();"},
            {"file": "src/lib.rs", "line": 17, "text": "pub fn shared() {"},
            {"file": "src/lib.rs", "line": 28, "text": "pub fn caller() { shared(); }"},
        ]),
    )?;
    facts_rebuild_preserves_symbols(&run)
}

#[test]
fn facts_persisted_zero_context_insertion_and_deletion_preserve_ownership()
-> Result<(), Box<dyn Error>> {
    for delete_owner in [false, true] {
        let tree = tempfile::tempdir()?;
        let repo = twin_bridge_repo(&tree)?;
        let changed = if delete_owner {
            TWIN_BRIDGE_BASELINE
                .replace(
                    "pub fn shared() {\n    let second_alpha",
                    "    let second_alpha",
                )
                .replace("let second_eta = 14;", "let second_eta = 140;")
        } else {
            TWIN_BRIDGE_BASELINE.replacen(
                "pub fn shared() {\n",
                "pub fn shared() {\n    inserted_body();\n",
                1,
            )
        };
        fs::write(repo.join("src/lib.rs"), changed)?;
        let run = gathered_run(&repo, &tree.path().join("run"))?;
        // Stored patches are public facts inputs. Real Git supplies zero-count
        // hunk coordinates; assertions below concern symbols, never header text.
        let patch = git(
            &repo,
            &[
                "diff",
                "--no-renames",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--unified=0",
                "HEAD",
                "--",
                "src/lib.rs",
            ],
        )?;
        fs::write(run.directory().join("patches/001.patch"), patch.stdout)?;
        facts(&run)?;
        let usages = if delete_owner {
            json!([
                {"file": "src/lib.rs", "line": 1, "text": "pub fn shared() {"},
                {"file": "src/lib.rs", "line": 26, "text": "pub fn caller() { shared(); }"},
            ])
        } else {
            json!([
                {"file": "src/lib.rs", "line": 17, "text": "pub fn shared() {"},
                {"file": "src/lib.rs", "line": 28, "text": "pub fn caller() { shared(); }"},
            ])
        };
        assert_symbol_facts(
            &run,
            "src/lib.rs",
            "shared",
            "modified",
            Value::Null,
            usages,
        )?;
        facts_rebuild_preserves_symbols(&run)?;
    }
    Ok(())
}

#[test]
fn gather_coalesces_file_symlink_typechanges_and_preserves_combined_patches()
-> Result<(), Box<dyn Error>> {
    for to_link in [true, false] {
        let tree = tempfile::tempdir()?;
        let repo = tree.path().join("repo");
        init_repo(&repo)?;
        git(&repo, &["config", "core.symlinks", "false"])?;
        git(&repo, &["config", "core.autocrlf", "false"])?;
        fs::write(repo.join("src/referent.txt"), b"unchanged referent\n")?;
        git(&repo, &["add", "src/referent.txt"])?;
        let stages: [(&str, &[u8]); 2] = if to_link {
            [
                ("100644", b"pub fn before_change() {}\n"),
                ("120000", b"referent.txt"),
            ]
        } else {
            [
                ("120000", b"referent.txt"),
                ("100644", b"pub fn after_change() {}\n"),
            ]
        };
        // Commit real Git symlink entries without requiring filesystem
        // symlink privileges; core.symlinks=false keeps ordinary worktree files.
        for (mode, content) in stages {
            fs::write(repo.join("src/flip.rs"), content)?;
            let oid =
                String::from_utf8(git(&repo, &["hash-object", "-w", "--", "src/flip.rs"])?.stdout)?;
            let entry = format!("{mode},{},src/flip.rs", oid.trim());
            git(&repo, &["update-index", "--add", "--cacheinfo", &entry])?;
            git(
                &repo,
                &["commit", "--no-gpg-sign", "--no-verify", "-qm", mode],
            )?;
        }
        let expected_tree = git(&repo, &["rev-parse", "HEAD^{tree}"])?.stdout;
        let run = ReviewRun::new(repo.clone(), tree.path().join("run"))?;
        gather(
            &run,
            "HEAD~1..HEAD",
            "src/flip.rs",
            &FixedClock {
                gathered: "2026-09-30T12:34:56+00:00".to_owned(),
            },
        )?;
        let manifest = review_json(&run, "manifest.json")?;
        let files = manifest["files"]
            .as_array()
            .ok_or("files must be an array")?;
        assert_eq!(manifest["total_files"], json!(1));
        assert_eq!(
            files.len(),
            1,
            "typechange must not leak separate D/A records"
        );
        assert_eq!(files[0]["path"], json!("src/flip.rs"));
        assert_eq!(files[0]["status"], json!("T"));
        assert_eq!(files[0]["insertions"], json!(1));
        assert_eq!(files[0]["deletions"], json!(1));
        assert_eq!(files[0]["binary"], json!(false));
        assert_manifest_patch_bytes(&run)?;
        let applied = tree.path().join("applied");
        git(
            tree.path(),
            &[
                OsStr::new("clone"),
                OsStr::new("-q"),
                OsStr::new("--shared"),
                OsStr::new("--no-checkout"),
                repo.as_os_str(),
                applied.as_os_str(),
            ],
        )?;
        for relative in ["diff.patch", "patches/001.patch"] {
            git(&applied, &["read-tree", "HEAD~1"])?;
            git(
                &applied,
                &[
                    OsStr::new("apply"),
                    OsStr::new("--cached"),
                    OsStr::new("--binary"),
                    run.directory().join(relative).as_os_str(),
                ],
            )?;
            assert_eq!(
                git(&applied, &["write-tree"])?.stdout,
                expected_tree,
                "{relative} lost typechange path, mode or source content (to_link={to_link})",
            );
        }
    }
    Ok(())
}

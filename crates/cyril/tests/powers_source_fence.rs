//! cyril-v19o C7: no production code can call a powers method that does not
//! exist.
//!
//! The catalog is push-only by decision, not preference. Measured on kiro-cli
//! 2.21.2 against a live KAS session (IAM Identity Center):
//!
//! * `_kiro/powers/items_changed` **arrives unprompted** ~18 ms after
//!   `session/new`, carrying the installed power set — that is the whole
//!   catalog surface cyril consumes.
//! * `_kiro/powers/list` is **never advertised** by the agent in
//!   `extensionMethods`. It does dispatch when called (the PR's own probe
//!   exercised it 1 ms after `session/new` and got three powers back), but
//!   nothing on the wire says so: an unadvertised method cannot be
//!   feature-detected, so an affordance on it would be a control whose
//!   availability the client can never establish.
//! * `_kiro/powers/refresh` is **declared but unimplemented**: calling it
//!   returns `-32603 Unknown ext method`.
//!
//! So neither pull method may be wired to a control, and the requester
//! re-confirmed push-only after the `list` row was challenged. This fence makes
//! that impossible rather than merely absent: the *only* powers method any
//! production source may name is the push.
//!
//! Scope is `crates/*/src/**/*.rs` — production code plus the `#[cfg(test)]`
//! modules inside it. Prose is stripped first, so documentation may discuss the
//! unimplemented methods (this file's own header does; so does the design
//! record). What is fenced is code that could run.

use std::path::{Path, PathBuf};

/// The one powers method cyril may name: the push it receives.
const PUSH_METHOD: &str = "items_changed";

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <root>/crates/cyril
    let candidate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match candidate.canonicalize() {
        Ok(root) => root,
        Err(e) => panic!("repo root {} must resolve: {e}", candidate.display()),
    }
}

/// Read a repo file with line endings normalized (the cyril-xi4a hazard: a CRLF
/// checkout must produce the same verdict as an LF one).
fn read_normalized(path: &Path) -> String {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Drop `//` line comments so the scan judges CODE, not prose.
///
/// A `//` preceded by `:` is kept: that is a URL (`https://…`), and cutting
/// there would silently truncate real code to its right — a false negative, the
/// direction that makes a fence quietly stop fencing.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| {
            let bytes = line.as_bytes();
            let cut = line.match_indices("//").find_map(|(i, _)| {
                if i > 0 && bytes[i - 1] == b':' {
                    None
                } else {
                    Some(i)
                }
            });
            match cut {
                Some(i) => &line[..i],
                None => line,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `.rs` file under `crates/*/src`, sorted for a deterministic verdict.
fn production_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let crates_dir = repo_root().join("crates");
    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", crates_dir.display()));
    for krate in entries.filter_map(Result::ok) {
        let src = krate.path().join("src");
        if !src.is_dir() {
            continue;
        }
        collect_rs(&src, &mut out);
    }
    out.sort();
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", dir.display()));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Which `kiro/powers/<segment>` wire methods does this source name that are
/// not the push? Returns the offending `powers/…` spellings, deduplicated and
/// in first-seen order.
///
/// The `kiro/` namespace is part of the pattern deliberately, and the reason is
/// a real false positive caught by this fence's own first run: the fixture
/// lives at `tests/fixtures/kas/powers/items-changed-2.21.2.json`, so a
/// directory path contains `powers/items` followed by `-`. A path is not a
/// method. Wire methods are always namespaced — `_kiro/powers/refresh`, which
/// contains `kiro/powers/refresh` — so anchoring on the namespace keeps the
/// census pointed at calls.
fn unimplemented_powers_methods(src: &str) -> Vec<String> {
    const NAMESPACE: &str = "kiro/powers/";
    let code = strip_line_comments(src);
    let mut found: Vec<String> = Vec::new();
    for (start, _) in code.match_indices(NAMESPACE) {
        let rest = &code[start + NAMESPACE.len()..];
        let end = rest
            .find(|c: char| !is_ident_byte(c as u8))
            .unwrap_or(rest.len());
        let segment = &rest[..end];
        if segment == PUSH_METHOD || segment.is_empty() {
            continue;
        }
        let spelling = format!("powers/{segment}");
        if !found.contains(&spelling) {
            found.push(spelling);
        }
    }
    found
}

// ── C7: the census ───────────────────────────────────────────────────────────

/// The one production file that must name the push: the converter that owns
/// the method string.
///
/// Anchoring the non-vacuity control here is the point. A control that only
/// asked "is the string somewhere in the tree?" stayed green with the
/// converter deleted — a doc comment in `types/power.rs` and two test doubles
/// spell the method too (review finding 4).
///
/// It is a path, not a string: comparing the census's findings as strings is
/// separator-dependent, and `display()` renders `\` on Windows while a literal
/// spells `/` — which turned the Windows leg red on a walk that had found the
/// converter (round-3 review finding 1). `Path` equality compares components,
/// so it holds under either separator.
fn converter_path() -> &'static Path {
    Path::new("crates/cyril-core/src/protocol/convert/kas/powers.rs")
}

/// Everything before a file's `#[cfg(test)]` module: the code that ships.
/// The control must not be satisfiable by a test that names the method either.
fn production_prefix(src: &str) -> &str {
    src.split("#[cfg(test)]").next().unwrap_or(src)
}

/// The claim: no production source names a powers method other than the push.
#[test]
fn no_production_source_names_an_unusable_powers_method() {
    let sources = production_sources();
    // Non-vacuity: the walk must actually reach the tree, and the push method
    // must be found — a scanner that reads nothing reports no violations.
    assert!(
        sources.len() >= 100,
        "expected the whole crates/*/src tree, scanned {} files — if the layout \
         moved, this fence needs updating, not deleting",
        sources.len()
    );

    let mut violators: Vec<String> = Vec::new();
    let mut push_seen_in: Vec<PathBuf> = Vec::new();
    for path in &sources {
        let src = read_normalized(path);
        let rel = path.strip_prefix(repo_root()).unwrap_or(path).to_path_buf();
        // Same stripping as the violator scan, and shipping code only: prose
        // and test doubles must not be able to satisfy a control that exists
        // to prove this census is looking at real code.
        if production_prefix(&strip_line_comments(&src))
            .contains(&format!("kiro/powers/{PUSH_METHOD}"))
        {
            push_seen_in.push(rel.clone());
        }
        for spelling in unimplemented_powers_methods(&src) {
            violators.push(format!("{}: {spelling}", rel.display()));
        }
    }

    assert!(
        push_seen_in.iter().any(|seen| seen == converter_path()),
        "the converter must name the push method in production code \
         ({}) — its absence means this census is scanning the wrong \
         tree, or the one consumer it protects is gone. Seen in: {push_seen_in:?}",
        converter_path().display()
    );
    assert!(
        violators.is_empty(),
        "these production sources name a powers method cyril cannot use \
         (`list` is unadvertised, so its availability cannot be feature-detected; \
         `refresh` answers -32603 Unknown ext method) — an affordance on either \
         is a control the client can never establish: {violators:?}. \
         Scanned {} files across crates/*/src.",
        sources.len()
    );
}

// ── Non-vacuity: the census can actually go red ──────────────────────────────

/// The scanner detects the exact violation it exists to catch, and does not
/// fire on the correct implementation or on prose about the wrong one.
#[test]
fn powers_census_detects_the_methods_it_exists_to_catch() {
    let refresh = r#"let response = client.ext_method("_kiro/powers/refresh", json!({}));"#;
    assert_eq!(
        unimplemented_powers_methods(refresh),
        ["powers/refresh"],
        "calling the unimplemented refresh must be caught"
    );

    let list = r#"bridge.send(BridgeCommand::ExtMethod { method: "kiro/powers/list" });"#;
    assert_eq!(
        unimplemented_powers_methods(list),
        ["powers/list"],
        "calling the unadvertised list must be caught"
    );

    // The push is the one method that may be named — in either spelling.
    assert!(
        unimplemented_powers_methods("const METHOD: &str = \"kiro/powers/items_changed\";")
            .is_empty(),
        "the push method must not be flagged, or the fence would forbid the \
         feature it protects"
    );
    assert!(
        unimplemented_powers_methods("UntypedMessage::new(\"_kiro/powers/items_changed\", p)")
            .is_empty(),
        "the raw underscore spelling of the push must not be flagged either"
    );

    // Prose about the unusable methods is documentation, not a call. Without
    // comment stripping this very file could not explain what it fences.
    let commented = "// `_kiro/powers/refresh` is declared but unimplemented.\nfn f() {}\n";
    assert!(
        unimplemented_powers_methods(commented).is_empty(),
        "a comment naming the unimplemented method must not count as calling it"
    );
    // …and stripping must not blind the scanner to real code on the same line
    // class of input.
    let commented_violation = "// explanation\nlet m = \"kiro/powers/refresh\";\n";
    assert_eq!(
        unimplemented_powers_methods(commented_violation),
        ["powers/refresh"],
        "comment stripping must not swallow an actual call"
    );

    // A filesystem path is not a method: the fixture directory is
    // `kas/powers/items-changed-2.21.2.json`, which contains `powers/items`.
    // This exact string reddened the first run of the census above.
    let fixture_path =
        r#"include_str!("../../tests/fixtures/kas/powers/items-changed-2.21.2.json")"#;
    assert!(
        unimplemented_powers_methods(fixture_path).is_empty(),
        "a path segment must not be read as a method name"
    );

    // Near-miss names are not silently accepted as the push.
    assert_eq!(
        unimplemented_powers_methods("const M: &str = \"kiro/powers/items_changed_v2\";"),
        ["powers/items_changed_v2"],
        "only the measured push method may pass; a lookalike is still a method \
         that no agent implements"
    );
}

/// CRLF hazard fence: a CRLF checkout reaches the same verdict as an LF one,
/// and the read path strips CRLF before anything looks at the text.
///
/// The function under test is fed the RAW CRLF string. Re-normalizing the
/// argument first (`f(&crlf.replace("\r\n", "\n"))`) makes this `f(lf) == f(lf)`
/// by construction — the assertion that shipped here until round 3, which no
/// implementation of the scanner could fail (round-3 review finding 3).
#[test]
fn powers_census_is_line_ending_agnostic() {
    let lf = "let m = \"kiro/powers/refresh\";\nlet p = \"kiro/powers/items_changed\";\n";
    let crlf = lf.replace('\n', "\r\n");

    // Non-vacuity first: the CRLF input must still trip the census. Without
    // this, "same verdict" would be satisfied by a scanner that finds nothing.
    assert_eq!(
        unimplemented_powers_methods(&crlf),
        ["powers/refresh"],
        "the raw CRLF text must still be judged as code"
    );
    assert_eq!(
        unimplemented_powers_methods(&crlf),
        unimplemented_powers_methods(lf),
        "a CRLF checkout must produce the same verdict as an LF one — the \
         cyril-xi4a failure mode"
    );

    // Mixed endings, which is what a partial checkout or an editor round-trip
    // leaves behind: the LF-terminated line must not swallow the CRLF one.
    assert_eq!(
        unimplemented_powers_methods("let a = \"kiro/powers/list\";\r\nlet b = 1;\n"),
        ["powers/list"]
    );

    // The read path the census owns normalizes the file, so nothing downstream
    // can be handed a `\r` at all.
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("temp dir: {e}"));
    let file = dir.path().join("crlf.rs");
    std::fs::write(&file, &crlf).unwrap_or_else(|e| panic!("write a CRLF source: {e}"));
    let read = read_normalized(&file);
    assert!(
        !read.contains('\r'),
        "read_normalized must strip CRLF: {read:?}"
    );
    assert_eq!(unimplemented_powers_methods(&read), ["powers/refresh"]);
}

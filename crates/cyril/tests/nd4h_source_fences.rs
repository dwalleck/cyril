//! cyril-nd4h structural fences (claims C3, C6, C8, C9).
//!
//! These claims are about the *shape* of the source, not about runtime values,
//! so their permanent form is a scan of the source text. Each fence below is
//! paired with a proof that it can actually fail — a scanner that cannot go red
//! is decoration.
//!
//! CRLF HAZARD: cyril-xi4a was a P1 where exactly this class of source-scanning
//! test reddened Windows CI on a checkout with CRLF line endings. Every read
//! here goes through `read_normalized`, and `scanner_verdict_is_line_ending_agnostic`
//! fences the normalization itself.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is <root>/crates/cyril
    let candidate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match candidate.canonicalize() {
        Ok(root) => root,
        Err(e) => panic!("repo root {} must resolve: {e}", candidate.display()),
    }
}

/// Read a repo file with line endings normalized (cyril-xi4a).
fn read_normalized(rel: &str) -> String {
    let path = repo_root().join(rel);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Drop `//` line comments so the scan judges CODE, not prose.
///
/// Without this the C3 fence flags its own subject matter: the comment above
/// the `EnableMouseCapture` branch explains that mouse capture follows
/// `ui.mouse_capture`, and naming the field in an explanation is not a second
/// read of it. Scanning raw text would force the code to be undocumented in
/// order to pass, which is a fence shaping the source for the scanner's
/// convenience rather than the other way round.
fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whole-word search: `mouse_capture` must NOT match inside `mouse_captured`.
///
/// This distinction is the entire point. The original cyril-nd4h investigation
/// found that a naive substring grep for `mouse_capture` reports 17 hits in
/// this codebase — every one of them `mouse_captured` / `set_mouse_captured` /
/// `toggle_mouse_capture`, none of them the config field. A substring-based
/// fence here would flag the correct implementation as a violation.
fn contains_word(haystack: &str, word: &str) -> bool {
    haystack.match_indices(word).any(|(start, _)| {
        let bytes = haystack.as_bytes();
        let end = start + word.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
        before_ok && after_ok
    })
}

// ── C3: exactly one read of the configured mouse mode ────────────────────────

/// The claim: `main.rs` derives the terminal's startup mouse mode from `App`'s
/// state and never reads the config field itself. Two independent reads is how
/// the flag and the terminal drift apart (inverted first Ctrl+M).
#[test]
fn main_does_not_read_mouse_capture_directly() {
    let main_rs = strip_line_comments(&read_normalized("crates/cyril/src/main.rs"));
    assert!(
        !contains_word(&main_rs, "mouse_capture"),
        "main.rs must not read ui.mouse_capture itself — it derives the startup \
         mode from app.mouse_captured(). A second independent read is the \
         desync that makes the first Ctrl+M press appear dead."
    );
    assert!(
        main_rs.contains("app.mouse_captured()"),
        "main.rs must still gate EnableMouseCapture on App's state"
    );
}

/// Non-vacuity: the scanner detects the violation it exists to catch.
#[test]
fn c3_scanner_detects_a_second_config_read() {
    let violating = "if config.ui.mouse_capture {\n    execute!(EnableMouseCapture)\n}\n";
    assert!(
        contains_word(violating, "mouse_capture"),
        "the C3 scanner must flag a direct config read"
    );

    // ...and does NOT fire on the correct implementation, whose `mouse_captured`
    // merely CONTAINS the field name as a substring.
    let correct = "if app.mouse_captured() {\n    execute!(EnableMouseCapture)\n}\n";
    assert!(
        !contains_word(correct, "mouse_capture"),
        "whole-word matching must not flag `mouse_captured` — a substring \
         scanner would reject the correct implementation"
    );

    // A comment that merely NAMES the field is not a read of it.
    let commented = "// mouse capture follows ui.mouse_capture\nif app.mouse_captured() {}\n";
    assert!(
        !contains_word(&strip_line_comments(commented), "mouse_capture"),
        "documenting the field must not count as reading it, or the fence \
         would force the code to go undocumented to pass"
    );
    // ...but stripping comments must not blind the fence to real code.
    let commented_violation = "// explanation\nif config.ui.mouse_capture {}\n";
    assert!(
        contains_word(&strip_line_comments(commented_violation), "mouse_capture"),
        "comment stripping must not swallow an actual second read"
    );
}

/// CRLF hazard fence (cyril-xi4a): identical verdict under both line endings.
#[test]
fn scanner_verdict_is_line_ending_agnostic() {
    let lf = "if config.ui.mouse_capture {\n    execute!(x)\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    assert_eq!(
        contains_word(lf, "mouse_capture"),
        contains_word(&crlf.replace("\r\n", "\n"), "mouse_capture"),
        "a CRLF checkout must produce the same verdict as an LF one — this is \
         the cyril-xi4a failure mode that turned Windows CI red"
    );
}

// ── C6: the UiConfig destructure stays exhaustive ────────────────────────────

/// The claim: adding a `UiConfig` field cannot compile until a consumption site
/// handles it. That guarantee is worth exactly as much as the absence of `..`.
#[test]
fn app_new_destructures_ui_config_exhaustively() {
    let app_rs = read_normalized("crates/cyril/src/app.rs");
    let Some(start) = app_rs.find("let &config::UiConfig {") else {
        panic!(
            "App::new must destructure UiConfig by name — if this moved, the C6 \
             guarantee moved with it and this fence needs updating, not deleting"
        );
    };
    let rest = &app_rs[start..];
    let Some(end) = rest.find("} = ui;") else {
        panic!("the UiConfig destructure must bind from `ui`");
    };
    let block = &rest[..end];

    assert!(
        !block.contains(".."),
        "the UiConfig destructure must stay exhaustive — a `..` silently \
         re-opens the door to config fields that nothing consumes, which is the \
         entire defect cyril-nd4h fixed:\n{block}"
    );
    assert!(
        block.contains("max_messages") && block.contains("mouse_capture"),
        "both surviving fields must be bound at the consumption site"
    );
}

// ── C8: removing the dead knob did not disturb the real caches ───────────────

/// The claim: the highlight and markdown caches still hold 256 entries. The bug
/// class: an implementation that "helpfully" wires the removed field's
/// documented default (20) into the caches on its way out, silently cutting
/// capacity by 12.8x.
#[test]
fn highlight_and_markdown_caches_still_hold_256() {
    for rel in [
        "crates/cyril-ui/src/highlight.rs",
        "crates/cyril-ui/src/widgets/markdown.rs",
    ] {
        let src = read_normalized(rel);
        assert!(
            src.contains("HashCache::new(256)"),
            "{rel} must still construct its cache at 256 — the removed \
             highlight_cache_size documented 20, and quietly adopting that \
             number would shrink the live cache 12.8x"
        );
    }
}

// ── C9: documentation describes what the code actually does ──────────────────

/// The claim: no doc surface still advertises a config key that does not exist.
/// The bug class: editing the prose file but forgetting a summary table — this
/// repo has THREE surfaces listing these keys, and the original ticket named
/// only two of them.
#[test]
fn docs_do_not_advertise_removed_config_keys() {
    for rel in [
        "AGENTS.md",
        ".agents/summary/codebase_info.md",
        ".agents/summary/data_models.md",
    ] {
        let doc = read_normalized(rel);
        for removed in ["highlight_cache_size", "stream_buffer_timeout_ms"] {
            assert!(
                !contains_word(&doc, removed),
                "{rel} still documents `{removed}`, which no longer exists — a \
                 documented-but-absent key is the same defect as a \
                 documented-but-ignored one"
            );
        }
    }
}

/// `HashCache` is insertion-order with oldest-half bulk eviction, not an LRU.
/// The summary table called it an LRU, which is a third documentation defect
/// independent of the values being wrong.
#[test]
fn docs_do_not_call_the_hash_cache_an_lru() {
    let doc = read_normalized(".agents/summary/codebase_info.md");
    assert!(
        !doc.contains("LRU"),
        "HashCache evicts the oldest HALF on overflow (cache.rs) — calling it an \
         LRU misdescribes both its eviction policy and its retention curve"
    );
}

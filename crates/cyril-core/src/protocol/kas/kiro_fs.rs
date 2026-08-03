//! The `_kiro/fs/*` dialect (cyril-kf2g): Kiro's superset of the bare-ACP fs
//! host callbacks in [`super::host_io`].
//!
//! **The dialect is client-selected.** `resolveCapabilities()` reads the family
//! from `clientCapabilities.fs._meta.kiro.{readFile,writeFile,stat,readDirectory,
//! delete}` — nested under `fs`, *not* under top-level `_meta.kiro`, which is why
//! an earlier probe advertising `_meta.kiro.kiroFsReadFile` (the *resolved*
//! capability name, not the wire name) moved nothing. Advertising a flag swaps
//! that one operation's adapter: `kiroFsReadFile` → `KiroEnhancedReadAdapter`,
//! and so on, each falling back to the bare-ACP adapter and then to KAS's
//! in-process `NodeFileSystem`.
//!
//! **Why take the dialect.** `_kiro/fs/read_file` carries pagination (`line`,
//! `limit` — live-observed `{line: 0, limit: 2001}`) that ACP's
//! `fs/read_text_file` has no field for, and `stat`/`read_directory`/`delete`
//! have no bare-ACP equivalent at all: without them those operations never
//! reach cyril, they run inside the agent process where nothing can audit them.
//!
//! **Semantics are ported from the reference, not invented.** For every method
//! here KAS ships an in-process implementation (`NodeFileSystem`, and
//! `spliceRange` in `file-operations-utils.ts`) that serves the *same port*
//! when the client declines the capability. Those are the contract: a client
//! that answers differently makes the agent behave differently depending on a
//! capability flag it cannot see the consequences of. All shapes below were
//! carved from `@kiro/agent/dist/server/acp-server.js` (KAS 0.27.8 /
//! kiro-cli 2.16.0) and cross-checked against the live capture
//! `experiments/conductor-spike/kas-pushed-2.16.0.jsonl`.
//!
//! **Deviations from the reference** — the complete list, kept here so a reader
//! can trust "faithful port" everywhere else: (1) [`respond_read_directory`]
//! sorts entries, where the reference returns raw readdir order; (2)
//! [`respond_delete`] stats with `symlink_metadata` where the reference uses
//! `fs.stat`, which differs only for a **dangling** symlink (the reference
//! throws `ENOENT`; cyril unlinks the link and succeeds — the more useful
//! answer, since the link is real even when its target is not).
//!
//! Two carved traps in particular:
//!
//! - **`line` is 0-based here**, and slicing joins with `\n` (dropping the
//!   trailing-newline structure) — *unlike* ACP's `fs/read_text_file`, whose
//!   `line` is 1-based and whose slice preserves newlines
//!   ([`super::host_io::slice_lines`]). The only live-observed value is
//!   `line: 0`, where both readings agree — so a wrong reading would ship
//!   silently and misread every paginated follow-up. See [`slice_lines_0based`].
//! - **Advertising `writeFile` also re-routes *range* writes.**
//!   `createAdaptersFromCapabilities` picks `KiroRangeWrite` (which sends
//!   `_meta.kiro.range`) over `LocalSpliceRangeWrite` (which splices agent-side
//!   and sends whole-file content). A responder that ignored the range would
//!   turn every partial edit into a full-file overwrite. See [`splice_range`].
//!
//! **Audit, not gate.** Every mutation here logs at `info!` with its session and
//! path, because these are exactly the side effects ADR-0003 says cyril exists
//! to observe. It is a *record*, not a policy check: the central write/exec gate
//! seam is still deferred to its first consumer (cyril-g9vt).
//!
//! **Permission posture (live-verified 2026-08-01, `kas-fs-write-2.16.0.jsonl`,
//! `probe-kas-fs-write-permission-2.16.0.py`).** KAS raises
//! `session/request_permission` at the **tool-approval** layer, before the
//! host callback — not per callback. Measured on 2.16.0: 2/2 writes and 1/1
//! delete were each preceded by a permission frame in the same turn
//! (`"Replace in File"`, `"Write File"`, `"Delete File"`). Two consequences:
//!
//! - Taking this dialect does **not** change the permission posture. The same
//!   approval precedes `fs/write_text_file` and `_kiro/fs/write_file` alike,
//!   so advertising `writeFile` moves no write off a gated path.
//! - An earlier revision of this comment asserted that KAS raises no
//!   permission for `_kiro/fs/delete`. That was read from carved source and is
//!   **wrong on the wire** — a `"Delete File"` approval does precede it.
//!
//! Advertising `delete` remains a deliberate grant, but of a path the user is
//! prompted for, not a silent one. What is genuinely ungated is the *scope*:
//! [`to_native_checked`] requires only an absolute path, so an approved delete
//! is unconfined and recurses. The approval is the gate; cyril adds none.

use agent_client_protocol as acp;
use serde::Deserialize;

use super::host_io::{io_err, to_native_checked, write_atomic};
use super::json_ext_response;

/// The acp-stripped method names (the acp library strips the leading `_`, per
/// the `SHELL_TYPE_METHOD` precedent). Paired with the `*_WIRE` names below —
/// [`stripped_names_match_their_wire_names`] pins the relationship, so a rename
/// cannot move one without the other.
pub(crate) const READ_FILE_METHOD: &str = "kiro/fs/read_file";
pub(crate) const WRITE_FILE_METHOD: &str = "kiro/fs/write_file";
pub(crate) const STAT_METHOD: &str = "kiro/fs/stat";
pub(crate) const READ_DIRECTORY_METHOD: &str = "kiro/fs/read_directory";
pub(crate) const DELETE_METHOD: &str = "kiro/fs/delete";

/// The same methods as they appear **on the wire**, with the leading `_`.
///
/// Errors and audit lines use these rather than hand-written literals: the
/// underscore-stripped form above is an artifact of the acp library, and a
/// reader grepping for the method Kiro documents needs to find something.
pub(crate) const READ_FILE_WIRE: &str = "_kiro/fs/read_file";
pub(crate) const WRITE_FILE_WIRE: &str = "_kiro/fs/write_file";
pub(crate) const STAT_WIRE: &str = "_kiro/fs/stat";
pub(crate) const READ_DIRECTORY_WIRE: &str = "_kiro/fs/read_directory";
pub(crate) const DELETE_WIRE: &str = "_kiro/fs/delete";

/// Which operation an [`FsOp`] row is. Exists so [`dispatch`] can match
/// *exhaustively*: adding a row to [`FS_OPS`] without giving it a responder is
/// then a compile error rather than a silent protocol-default null.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FsOpKind {
    ReadFile,
    WriteFile,
    Stat,
    ReadDirectory,
    Delete,
}

/// One operation of the dialect: which operation it is, the capability flag
/// that selects it, the acp-stripped name the library dispatches on, and the
/// wire name.
pub(crate) struct FsOp {
    pub(crate) kind: FsOpKind,
    /// Key inside `fs._meta.kiro` that selects this operation.
    pub(crate) flag: &'static str,
    /// acp-stripped method name (what `handle_ext_request` matches).
    pub(crate) method: &'static str,
    /// Wire method name, for errors and audit lines.
    pub(crate) wire: &'static str,
}

/// Route one `_kiro/fs/*` request to its responder.
///
/// The `match` is exhaustive over [`FsOpKind`], which is the point: the
/// advertisement, the dispatch, and the table are now one source instead of
/// three that a comment asked you to keep in step.
pub(crate) async fn dispatch(
    op: &FsOp,
    params: &serde_json::Value,
) -> acp::Result<acp::ExtResponse> {
    // Named with the WIRE spelling: this is the single point every dialect call
    // passes through, and `_kiro/fs/...` is what a reader correlating a log
    // against a capture greps for — the acp-stripped form appears nowhere on
    // the wire.
    tracing::debug!(method = op.wire, "KAS fs dialect dispatch");
    match op.kind {
        FsOpKind::ReadFile => respond_read_file(params).await,
        FsOpKind::WriteFile => respond_write_file(params).await,
        FsOpKind::Stat => respond_stat(params).await,
        FsOpKind::ReadDirectory => respond_read_directory(params).await,
        FsOpKind::Delete => respond_delete(params).await,
    }
}

/// Find the operation a method name selects, if any.
pub(crate) fn op_for_method(method: &str) -> Option<&'static FsOp> {
    FS_OPS.iter().find(|op| op.method == method)
}

/// The table row for an op kind — a TOTAL match (no unwrap/expect/panic/
/// fallback). The index mapping mirrors `FS_OPS`'s declaration order, fenced
/// by [`tests::op_for_kind_indices_match_fs_ops`].
pub(crate) fn op_for_kind(kind: FsOpKind) -> &'static FsOp {
    match kind {
        FsOpKind::ReadFile => &FS_OPS[0],
        FsOpKind::WriteFile => &FS_OPS[1],
        FsOpKind::Stat => &FS_OPS[2],
        FsOpKind::ReadDirectory => &FS_OPS[3],
        FsOpKind::Delete => &FS_OPS[4],
    }
}

/// The five operations in one place.
///
/// Three sites have to move together — the advertisement
/// ([`capabilities_meta`]), the dispatch cascade in `client.rs`, and the error
/// strings. Deriving the advertisement from this table and fencing the dispatch
/// against it turns a convention into a check: an advertised flag with no arm
/// answers the protocol-default null, which the agent cannot distinguish from a
/// successful empty result.
pub(crate) const FS_OPS: &[FsOp] = &[
    FsOp {
        kind: FsOpKind::ReadFile,
        flag: "readFile",
        method: READ_FILE_METHOD,
        wire: READ_FILE_WIRE,
    },
    FsOp {
        kind: FsOpKind::WriteFile,
        flag: "writeFile",
        method: WRITE_FILE_METHOD,
        wire: WRITE_FILE_WIRE,
    },
    FsOp {
        kind: FsOpKind::Stat,
        flag: "stat",
        method: STAT_METHOD,
        wire: STAT_WIRE,
    },
    FsOp {
        kind: FsOpKind::ReadDirectory,
        flag: "readDirectory",
        method: READ_DIRECTORY_METHOD,
        wire: READ_DIRECTORY_WIRE,
    },
    FsOp {
        kind: FsOpKind::Delete,
        flag: "delete",
        method: DELETE_METHOD,
        wire: DELETE_WIRE,
    },
];

/// The `fs._meta.kiro` object that selects this dialect at `initialize`.
///
/// Nested under `fs`, **not** under top-level `_meta.kiro` — that placement is
/// the whole gate (`resolveCapabilities()` reads `clientCapabilities.fs._meta
/// .kiro`), and it is where an earlier probe went wrong. The keys are the
/// *wire* names, which differ from the resolved capability names KAS uses
/// internally (`readFile` here, `kiroFsReadFile` there).
///
/// It lives beside the responders deliberately: advertising a flag with no
/// responder behind it makes KAS route that operation into a `-32601`, so the
/// two must move together.
pub(crate) fn capabilities_meta() -> acp::Meta {
    let mut meta = acp::Meta::new();
    // Derived from FS_OPS, not hand-listed: the advertisement and the dispatch
    // cascade must agree, and a second literal list is how they drift apart.
    let flags: serde_json::Map<String, serde_json::Value> = FS_OPS
        .iter()
        .map(|op| (op.flag.to_string(), serde_json::Value::Bool(true)))
        .collect();
    meta.insert("kiro".to_string(), serde_json::Value::Object(flags));
    meta
}

/// Maximum file size `_kiro/fs/read_file` will return, mirroring
/// `NodeFileSystem.MAX_READ_SIZE`. Refusing at the same threshold as the
/// in-process reference keeps the capability behavior-neutral — a client that
/// happily returned 500 MB would put it straight into the model's context.
const MAX_READ_SIZE: u64 = 10 * 1024 * 1024;

/// `{sessionId, path}` — the shape shared by `stat`, `read_directory`, and
/// `delete`. `session_id` is carried (not skipped) so every audit line can name
/// the session that caused the side effect.
#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PathParams {
    pub(crate) session_id: String,
    pub(crate) path: std::path::PathBuf,
    /// Documented by the covenant as `{path, recursive?}`, but **never sent** by
    /// the 2.16.0 `KiroDeleteAdapter`, which posts `{sessionId, path}` only. Read
    /// anyway so a future agent that starts sending it is honored rather than
    /// silently ignored; `None` means "not requested", never "false".
    recursive: Option<bool>,
}

/// `_kiro/fs/read_file` params. `line`/`limit` are omitted by the adapter when
/// null, so `Option` here means genuinely absent — not "0".
#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadFileParams {
    pub(crate) session_id: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) line: Option<usize>,
    pub(crate) limit: Option<usize>,
}

/// `_kiro/fs/write_file` params. The optional range rides in
/// `_meta.kiro.range`, which is why `meta` is modeled rather than ignored.
#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WriteFileParams {
    pub(crate) session_id: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) content: String,
    #[serde(rename = "_meta")]
    pub(crate) meta: Option<WriteMeta>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub(crate) struct WriteMeta {
    pub(crate) kiro: Option<WriteKiroMeta>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub(crate) struct WriteKiroMeta {
    pub(crate) range: Option<Range>,
}

/// An LSP-style range: **0-based** lines and **UTF-16** character offsets.
/// Every level is optional because the reference splice reads it through
/// optional chaining (`range.start?.line ?? 0`), so `{}` is a legal range
/// meaning "the whole file".
#[derive(Debug, Default, Deserialize, serde::Serialize)]
pub(crate) struct Range {
    pub(crate) start: Option<Position>,
    pub(crate) end: Option<Position>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
pub(crate) struct Position {
    pub(crate) line: Option<usize>,
    pub(crate) character: Option<usize>,
}

/// Parse ext params into `T`, mapping a shape mismatch to `-32602` (invalid
/// params) rather than letting it read as a filesystem failure. The method name
/// leads the message so a dialect drift is diagnosable from the wire alone.
fn parse_params<T: for<'de> Deserialize<'de>>(
    method: &str,
    params: &serde_json::Value,
) -> acp::Result<T> {
    serde_json::from_value(params.clone()).map_err(|e| {
        tracing::warn!(method, error = %e, "malformed _kiro/fs params");
        acp::Error::new(-32602, format!("{method}: invalid params: {e}"))
    })
}

/// Answer `_kiro/fs/read_file` → `{content}`.
///
/// Reads the file at the (translated) path and applies [`slice_lines_0based`].
/// A missing, unreadable, or non-UTF-8 file returns `Err` — never `Ok("")`,
/// which would masquerade as a successful read of an empty file. A file over
/// [`MAX_READ_SIZE`] is refused with the reference implementation's own wording,
/// so the model sees the same message whichever side served the read.
pub(crate) async fn respond_read_file(params: &serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let p: ReadFileParams = parse_params(READ_FILE_METHOD, params)?;
    let path = to_native_checked(&p.path)?;
    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|e| io_err(READ_FILE_WIRE, &path, e))?
        .len();
    if size > MAX_READ_SIZE {
        // Reference wording (NodeFileSystem.readTextFile), one decimal place.
        let mb = size as f64 / 1024.0 / 1024.0;
        return Err(acp::Error::new(
            -32603,
            format!(
                "File is too large to read ({mb:.1}MB). Maximum supported size is {}MB.",
                MAX_READ_SIZE / 1024 / 1024
            ),
        ));
    }
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| io_err(READ_FILE_WIRE, &path, e))?;
    tracing::debug!(
        session = %p.session_id, path = %path.display(), line = ?p.line, limit = ?p.limit,
        "KAS _kiro/fs/read_file"
    );
    json_ext_response(&serde_json::json!({
        "content": slice_lines_0based(&text, p.line, p.limit),
    }))
}

/// Select lines from `text`, replicating `NodeFileSystem.readTextFile` exactly:
/// both bounds absent returns the text untouched; otherwise `line` is a
/// **0-based** start index, the end is `line + limit` (or the line count when
/// `limit` is absent), and the selection is rejoined with `\n`.
///
/// That rejoin is not an oversight to "fix": `content.split('\n')` on a
/// newline-terminated file yields a trailing empty element, so a selection that
/// stops short of the end comes back **without** a trailing newline — and a
/// selection that runs to the end keeps one. Diverging would change the bytes
/// the model sees relative to the in-process path.
///
/// O(L) over the file's lines, single pass.
fn slice_lines_0based(text: &str, line: Option<usize>, limit: Option<usize>) -> String {
    if line.is_none() && limit.is_none() {
        return text.to_string();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let start = line.unwrap_or(0).min(lines.len());
    let end = match limit {
        // `start + limit` can overflow only for absurd inputs; saturate rather
        // than panic in debug / wrap in release.
        Some(n) => start.saturating_add(n).min(lines.len()),
        None => lines.len(),
    };
    if start >= end {
        return String::new();
    }
    lines[start..end].join("\n")
}

/// Answer `_kiro/fs/write_file` → `{}`.
///
/// Without a range this is a whole-file atomic write, identical to
/// `fs/write_text_file`. With one, it is a read-modify-write: the existing
/// content is spliced by [`splice_range`] and the result written atomically —
/// strictly safer than the agent-side `LocalSpliceRangeWrite`, which does the
/// same read/splice but finishes with a plain write.
///
/// A missing target splices against empty content (matching
/// `LocalSpliceRangeWrite`, which swallows only path-not-found); any other read
/// failure aborts without touching the file.
pub(crate) async fn respond_write_file(
    params: &serde_json::Value,
) -> acp::Result<acp::ExtResponse> {
    let p: WriteFileParams = parse_params(WRITE_FILE_METHOD, params)?;
    let path = to_native_checked(&p.path)?;
    let range = p.meta.and_then(|m| m.kiro).and_then(|k| k.range);

    let content = match &range {
        None => p.content,
        Some(range) => {
            let existing = match tokio::fs::read_to_string(&path).await {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(io_err(WRITE_FILE_WIRE, &path, e)),
            };
            splice_range(&existing, range, &p.content)
        }
    };

    tracing::info!(
        session = %p.session_id, path = %path.display(), ranged = range.is_some(),
        bytes = content.len(), "KAS _kiro/fs/write_file"
    );
    let target = path.clone();
    tokio::task::spawn_blocking(move || write_atomic(&target, &content))
        .await
        .map_err(|e| {
            tracing::warn!(path = %path.display(), error = %e, "KAS _kiro/fs/write_file task failed");
            acp::Error::new(
                -32603,
                format!("{WRITE_FILE_WIRE} {}: task failed: {e}", path.display()),
            )
        })?
        .map_err(|e| io_err(WRITE_FILE_WIRE, &path, e))?;
    json_ext_response(&serde_json::json!({}))
}

/// Replace the text `range` selects in `content` with `new_text`.
///
/// A faithful port of `spliceRange` (`src/platform/file-operations-utils.ts`),
/// including the parts that look like bugs — an out-of-range `start.line`
/// appends past the end, an absent `end` means "to the last line", and `{}`
/// replaces the whole file. The agent computes these ranges against the string
/// it got from a previous read, so cyril's job is to agree with the reference,
/// not to second-guess it. Expected outputs in the tests were produced by
/// running the carved JS itself, not by reading it.
///
/// Character offsets are **UTF-16 code units** (the agent measures them in a JS
/// string), which is why this cannot index by Rust `char`s — see
/// [`utf16_offset_to_byte`].
fn splice_range(content: &str, range: &Range, new_text: &str) -> String {
    let lines: Vec<&str> = content.split('\n').collect();
    // `split` always yields at least one element, so `len - 1` cannot underflow.
    let last = lines.len() - 1;

    let start_line = range
        .start
        .as_ref()
        .and_then(|p| p.line)
        .unwrap_or(0)
        .min(lines.len());
    let start_char = range.start.as_ref().and_then(|p| p.character).unwrap_or(0);
    let end_line = range
        .end
        .as_ref()
        .and_then(|p| p.line)
        .unwrap_or(last)
        .min(lines.len());
    // `lines[endLine] ? lines[endLine].length : 0` — an out-of-range index and
    // an empty line both give 0, which `utf16_len` of "" already is.
    let end_char = range
        .end
        .as_ref()
        .and_then(|p| p.character)
        .unwrap_or_else(|| lines.get(end_line).map_or(0, |l| utf16_len(l)));

    let mut out = lines[..start_line].join("\n");
    if start_line > 0 {
        out.push('\n');
    }
    if let Some(l) = lines.get(start_line) {
        out.push_str(&l[..utf16_offset_to_byte(l, start_char)]);
    }
    out.push_str(new_text);
    if let Some(l) = lines.get(end_line) {
        out.push_str(&l[utf16_offset_to_byte(l, end_char)..]);
    }
    if end_line < last {
        out.push('\n');
        out.push_str(&lines[end_line + 1..].join("\n"));
    }
    out
}

/// Length of `s` in UTF-16 code units — what a JS `String.length` reports.
fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Byte offset in `s` of UTF-16 code-unit index `target`, clamped to `s.len()`.
///
/// JS slices by UTF-16 code unit, so an astral character (one Rust `char`, two
/// units) shifts every offset after it. An offset landing *inside* a surrogate
/// pair would split it in JS, producing a lone surrogate — not representable in
/// a Rust `str`, so it rounds up to the pair's end and warns. Rounding
/// consistently (rather than up for one bound and down for the other) preserves
/// the property that an empty range splices nothing.
fn utf16_offset_to_byte(s: &str, target: usize) -> usize {
    let mut units = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        if units >= target {
            if units > target {
                tracing::warn!(
                    target,
                    landed_at = units,
                    "range offset fell inside a surrogate pair; rounded to the character boundary"
                );
            }
            return byte_idx;
        }
        units += ch.len_utf16();
    }
    s.len()
}

/// Answer `_kiro/fs/stat` → `{type, size}`.
///
/// Both keys are required: KAS's `isFSStatCapabilityResponse` guard checks for
/// `type` *and* `size` and throws "Invalid stat response" without them, even
/// though only `type` is consumed. Follows symlinks (`fs.stat` semantics), so a
/// dangling link errors rather than reporting a type — which is what makes
/// `exists()` answer `false` for it.
pub(crate) async fn respond_stat(params: &serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let p: PathParams = parse_params(STAT_METHOD, params)?;
    let path = to_native_checked(&p.path)?;
    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| io_err(STAT_WIRE, &path, e))?;
    // The reference's third variant, "symlink", is unreachable here for the same
    // reason it is unreachable there: the stat followed the link.
    let kind = if meta.is_dir() { "directory" } else { "file" };
    tracing::debug!(session = %p.session_id, path = %path.display(), kind, "KAS _kiro/fs/stat");
    json_ext_response(&serde_json::json!({ "type": kind, "size": meta.len() }))
}

/// Answer `_kiro/fs/read_directory` → `{entries: [{name, type}]}`.
///
/// Entry types use the reference's *un*-followed classification (`readdir`
/// `withFileTypes`): a symlink reports `"symlink"` whatever it points at, so
/// directory listings must not resolve links even though [`respond_stat`] does.
///
/// A missing directory returns `{entries: []}`, not an error — the reference
/// maps `ENOENT` to an empty listing and callers depend on it. That is the one
/// place here where empty does not mean "nothing there", so it is logged.
///
/// **DEVIATION from the reference — entries are sorted.** `NodeFileSystem`
/// returns raw `readdir` order, which is filesystem-dependent; sorting makes
/// captures and transcripts reproducible, and the agent imposes no ordering
/// (`KiroReadDirectoryAdapter` passes entries through unmodified). This is the
/// only intentional divergence in this module — everything else is a faithful
/// port — so it is named here, in the module header's deviation list, and in
/// the covenant note rather than only at the call site. Drop the sort if strict
/// parity ever matters more than reproducibility.
pub(crate) async fn respond_read_directory(
    params: &serde_json::Value,
) -> acp::Result<acp::ExtResponse> {
    let p: PathParams = parse_params(READ_DIRECTORY_METHOD, params)?;
    let path = to_native_checked(&p.path)?;
    let mut dir = match tokio::fs::read_dir(&path).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                session = %p.session_id, path = %path.display(),
                "KAS _kiro/fs/read_directory on a missing path; empty listing (reference parity)"
            );
            return json_ext_response(&serde_json::json!({ "entries": [] }));
        }
        Err(e) => return Err(io_err(READ_DIRECTORY_WIRE, &path, e)),
    };

    let mut entries: Vec<(String, &'static str)> = Vec::new();
    while let Some(entry) = dir
        .next_entry()
        .await
        .map_err(|e| io_err(READ_DIRECTORY_WIRE, &path, e))?
    {
        let kind = match entry.file_type().await {
            // Order matters and matches the reference: a symlink to a directory
            // is `symlink`, not `directory` (`file_type` does not follow).
            Ok(t) if t.is_dir() => "directory",
            Ok(t) if t.is_symlink() => "symlink",
            Ok(_) => "file",
            Err(e) => {
                // A racing unlink between readdir and the type query, or an
                // EACCES on the parent. Skipping would silently shrink the
                // listing, and the reference's FileType is 3-valued
                // (directory|symlink|file) with no "unknown" — so parity forces
                // one of the three, and "file" is the least-privileged guess.
                //
                // `warn!`, not `debug!`: this is an error becoming a plausible
                // default, which CLAUDE.md ("Errors are not default values")
                // says must carry at least a warning. `host_io` makes the same
                // call for the same reason.
                tracing::warn!(
                    path = %entry.path().display(), error = %e,
                    "dir entry type unavailable; reporting as `file`"
                );
                "file"
            }
        };
        entries.push((entry.file_name().to_string_lossy().into_owned(), kind));
    }
    // Readdir order is filesystem-dependent; sorting makes captures and
    // transcripts reproducible. The agent imposes no ordering.
    entries.sort();
    tracing::debug!(
        session = %p.session_id, path = %path.display(), count = entries.len(),
        "KAS _kiro/fs/read_directory"
    );
    let entries: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|(name, kind)| serde_json::json!({ "name": name, "type": kind }))
        .collect();
    json_ext_response(&serde_json::json!({ "entries": entries }))
}

/// Answer `_kiro/fs/delete` → `{}`.
///
/// Directories are removed **recursively**, matching `NodeFileSystem.delete`
/// (`fs.rm(resolved, {recursive: true})`). That is a deliberate parity choice
/// rather than a comfortable one: a client that quietly refused non-empty
/// directories would make the agent's delete succeed or fail depending on a
/// capability flag, which is a worse failure mode than the one the advertisement
/// already accepts. An explicit `recursive: false` — which 2.16.0 never sends —
/// is honored and refuses a non-empty directory.
///
/// Logged at `info!`: this is the most destructive callback cyril answers. A
/// `"Delete File"` `session/request_permission` DOES precede it (live-verified
/// 2026-08-01, `kas-fs-write-2.16.0.jsonl`) — but that approval names one path
/// and cyril bounds nothing, so the `info!` line remains the only record of
/// what was actually removed. See the module header on the permission posture.
pub(crate) async fn respond_delete(params: &serde_json::Value) -> acp::Result<acp::ExtResponse> {
    let p: PathParams = parse_params(DELETE_METHOD, params)?;
    let path = to_native_checked(&p.path)?;
    // symlink_metadata, not metadata: deleting a symlink must unlink the link
    // itself, never recurse into the directory it points at. DEVIATION: the
    // reference stats (follows), so a DANGLING link throws ENOENT there and is
    // unlinked here. Deliberate — the link exists, and refusing to remove it
    // because its target vanished is the less useful of the two answers. See
    // the module header's deviation list.
    let meta = tokio::fs::symlink_metadata(&path)
        .await
        .map_err(|e| io_err(DELETE_WIRE, &path, e))?;
    let recursive = p.recursive.unwrap_or(true);
    tracing::info!(
        session = %p.session_id, path = %path.display(),
        dir = meta.is_dir(), recursive, "KAS _kiro/fs/delete"
    );
    let result = if meta.is_dir() {
        if recursive {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_dir(&path).await
        }
    } else {
        tokio::fs::remove_file(&path).await
    };
    result.map_err(|e| io_err(DELETE_WIRE, &path, e))?;
    json_ext_response(&serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    // cyril-g9vt: op_for_kind's indexed match mirrors FS_OPS's declaration
    // order — fence the two against drift (reordering FS_OPS without updating
    // the match would silently return the wrong op).
    #[test]
    fn op_for_kind_indices_match_fs_ops() {
        for op in FS_OPS {
            assert_eq!(
                op_for_kind(op.kind).method,
                op.method,
                "op_for_kind({:?}) must return the FS_OPS row for {}",
                op.kind,
                op.wire
            );
        }
    }

    /// The response's JSON body — asserting on the payload the agent parses,
    /// not on cyril's in-memory value, so a serialization mistake is caught.
    fn body(resp: acp::ExtResponse) -> serde_json::Value {
        serde_json::from_str(resp.0.get()).expect("ext response body is JSON")
    }

    fn range(v: serde_json::Value) -> Range {
        serde_json::from_value(v).unwrap()
    }

    // ---- slice_lines_0based -------------------------------------------------

    // Oracle: every expected value below was produced by RUNNING the carved
    // `NodeFileSystem.readTextFile` slicing under node, not by reading it.
    //
    // The load-bearing case is `line=1, limit=1` -> "l2": the ACP dialect's
    // 1-based `slice_lines` would answer "l1\n" for the same numbers. The only
    // value ever seen live is `line: 0`, where both readings agree — so
    // reusing the ACP helper here would have passed every live capture and
    // been wrong for every paginated follow-up.
    #[test]
    fn slice_0based_matches_reference_oracle() {
        const C: &str = "l1\nl2\nl3\nl4\nl5\n";
        assert_eq!(
            slice_lines_0based(C, None, None),
            C,
            "both absent: untouched"
        );
        assert_eq!(
            slice_lines_0based(C, Some(0), Some(2001)),
            C,
            "the live-observed pair reads the whole file"
        );
        assert_eq!(slice_lines_0based(C, Some(0), None), C);
        assert_eq!(
            slice_lines_0based(C, None, Some(2)),
            "l1\nl2",
            "a short selection loses the trailing newline (join, not split_inclusive)"
        );
        assert_eq!(
            slice_lines_0based(C, Some(1), Some(1)),
            "l2",
            "0-BASED: 1-based would give l1"
        );
        assert_eq!(slice_lines_0based(C, Some(2), Some(2)), "l3\nl4");
        assert_eq!(
            slice_lines_0based(C, Some(99), Some(2)),
            "",
            "start past EOF yields empty, not the tail"
        );
        assert_eq!(slice_lines_0based("a\nb", Some(0), Some(1)), "a");
    }

    #[test]
    fn slice_0based_disagrees_with_the_acp_dialect() {
        // Non-vacuity fence for the claim above: if someone "unifies" the two
        // helpers, this fails. They are different dialects, not duplication.
        let text = "l1\nl2\nl3\n";
        assert_ne!(
            slice_lines_0based(text, Some(1), Some(1)),
            super::super::host_io::slice_lines(text.to_string(), Some(1), Some(1)),
            "the 0-based and 1-based readings must not coincide at line=1"
        );
    }

    // cyril-kf2g review fence: the wire name and the acp-stripped name are two
    // spellings of one method, and nothing but this test ties them together —
    // a rename that moved one would leave errors naming a method that is no
    // longer dispatched.
    #[test]
    fn stripped_names_match_their_wire_names() {
        for op in FS_OPS {
            assert_eq!(
                op.wire,
                format!("_{}", op.method),
                "{} must be the wire spelling of {}",
                op.wire,
                op.method
            );
        }
    }

    // The advertisement is DERIVED from FS_OPS, so this pins the derivation
    // itself: every advertised key is a flag in the table and vice versa. The
    // dispatch half of the same invariant is fenced in `client.rs`
    // (`every_advertised_fs_flag_is_dispatched`).
    #[test]
    fn advertised_flags_are_exactly_the_table() {
        let meta = capabilities_meta();
        let kiro = serde_json::to_value(&meta).unwrap();
        let obj = kiro["kiro"].as_object().expect("fs._meta.kiro object");
        let mut advertised: Vec<&str> = obj.keys().map(String::as_str).collect();
        advertised.sort_unstable();
        let mut expected: Vec<&str> = FS_OPS.iter().map(|o| o.flag).collect();
        expected.sort_unstable();
        assert_eq!(advertised, expected);
        assert!(
            obj.values().all(|v| v == &serde_json::Value::Bool(true)),
            "flags are advertised as `true`, never as an object"
        );
    }

    // ---- splice_range -------------------------------------------------------

    /// The first `_meta.kiro.range` ever captured on the wire
    /// (`kas-fs-write-2.16.0.jsonl`, 2026-08-01): a real partial edit of line 3
    /// of a 5-line file. Until this capture every range case was carved-source
    /// only. Confirms 0-based lines and that `character` indexes within a line.
    #[test]
    fn splice_matches_the_live_captured_range() {
        let content = "alpha\nbravo\ncharlie\ndelta\necho\n";
        let r = range(serde_json::json!({
            "start": {"line": 2, "character": 0},
            "end":   {"line": 2, "character": 7}
        }));
        assert_eq!(
            splice_range(content, &r, "CHARLIE-EDITED"),
            "alpha\nbravo\nCHARLIE-EDITED\ndelta\necho\n",
            "must reproduce the file the live turn actually produced"
        );
    }

    // Oracle: expected values produced by running the carved `spliceRange`
    // under node (KAS 0.27.8). Each case is one row of that run.
    #[test]
    fn splice_matches_reference_oracle() {
        let cases: &[(&str, &str, serde_json::Value, &str, &str)] = &[
            (
                "replace_middle_line",
                "l1\nl2\nl3\n",
                json!({"start":{"line":1,"character":0},"end":{"line":1,"character":2}}),
                "XX",
                "l1\nXX\nl3\n",
            ),
            (
                "insert_at_point",
                "l1\nl2\nl3\n",
                json!({"start":{"line":1,"character":1},"end":{"line":1,"character":1}}),
                "INS",
                "l1\nlINS2\nl3\n",
            ),
            (
                "span_two_lines",
                "aaa\nbbb\nccc\n",
                json!({"start":{"line":0,"character":1},"end":{"line":1,"character":2}}),
                "-",
                "a-b\nccc\n",
            ),
            (
                "no_start_defaults_top",
                "aaa\nbbb\n",
                json!({"end":{"line":0,"character":3}}),
                "Z",
                "Z\nbbb\n",
            ),
            (
                "no_end_defaults_lastline",
                "aaa\nbbb\n",
                json!({"start":{"line":0,"character":0}}),
                "Z",
                "Z",
            ),
            ("empty_range_object", "aaa\nbbb\n", json!({}), "Z", "Z"),
            (
                "start_line_past_eof",
                "aaa\n",
                json!({"start":{"line":9,"character":0},"end":{"line":9,"character":0}}),
                "Z",
                "aaa\n\nZ",
            ),
            (
                "end_char_past_eol",
                "aaa\nbbb\n",
                json!({"start":{"line":0,"character":1},"end":{"line":0,"character":99}}),
                "Z",
                "aZ\nbbb\n",
            ),
            (
                "whole_file_no_trailing_nl",
                "aaa\nbbb",
                json!({"start":{"line":0,"character":0},"end":{"line":1,"character":3}}),
                "Z",
                "Z",
            ),
            (
                "empty_existing_file",
                "",
                json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}}),
                "NEW",
                "NEW",
            ),
            (
                "multiline_newtext",
                "aaa\nbbb\n",
                json!({"start":{"line":0,"character":0},"end":{"line":0,"character":3}}),
                "x\ny",
                "x\ny\nbbb\n",
            ),
            (
                // UTF-16, not chars: 😀 spans units 1-2, so character 3 is 'b'.
                // A Rust-char implementation would cut at 'c' and produce
                // "a😀bZ" — this row is the fence for that bug.
                "utf16_astral_before_cut",
                "a\u{1F600}bc\nzzz\n",
                json!({"start":{"line":0,"character":3},"end":{"line":0,"character":4}}),
                "Z",
                "a\u{1F600}Zc\nzzz\n",
            ),
            (
                "utf16_multibyte_bmp",
                "héllo\nzzz\n",
                json!({"start":{"line":0,"character":2},"end":{"line":0,"character":3}}),
                "Z",
                "héZlo\nzzz\n",
            ),
        ];
        for (name, content, r, new_text, expected) in cases {
            assert_eq!(
                splice_range(content, &range(r.clone()), new_text),
                *expected,
                "spliceRange parity case {name}"
            );
        }
    }

    #[test]
    fn utf16_offsets_are_code_units_not_chars() {
        // Direct fence on the helper: for an astral char the two indexings
        // diverge, and the wrong one is silently plausible on ASCII input.
        let s = "a\u{1F600}bc";
        assert_eq!(utf16_len(s), 5, "1 + 2 + 1 + 1 code units");
        assert_eq!(s.chars().count(), 4, "but only 4 chars");
        assert_eq!(
            utf16_offset_to_byte(s, 3),
            5,
            "unit 3 is the byte before 'b'"
        );
        assert_eq!(utf16_offset_to_byte(s, 99), s.len(), "clamped to the end");
        // Mid-surrogate rounds to the pair's end rather than splitting it.
        assert_eq!(utf16_offset_to_byte(s, 2), 5);
    }

    // ---- responders ---------------------------------------------------------

    #[tokio::test]
    async fn read_file_paginates_and_errors_on_missing() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("notes.txt");
        std::fs::write(&f, "l1\nl2\nl3\nl4\nl5\n").unwrap();
        let resp = respond_read_file(&json!({
            "sessionId": "s", "path": f, "line": 1, "limit": 1
        }))
        .await
        .unwrap();
        assert_eq!(body(resp)["content"], "l2");

        // Missing must be an error, never Ok("") — the empty-read masquerade.
        let missing = dir.path().join("nope.txt");
        assert!(
            respond_read_file(&json!({"sessionId": "s", "path": missing}))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn read_file_rejects_relative_paths() {
        // Shared contract with host_io: a relative path would resolve against
        // the bridge process cwd and read the WRONG file.
        let err = respond_read_file(&json!({"sessionId": "s", "path": "rel.txt"}))
            .await
            .expect_err("relative path must be rejected");
        assert!(
            format!("{err:?}").contains("must be absolute"),
            "expected absolute-path rejection, got {err:?}"
        );
    }

    #[tokio::test]
    async fn write_file_whole_and_ranged() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a/b/c.txt"); // parents do not exist
        respond_write_file(&json!({"sessionId": "s", "path": f, "content": "l1\nl2\nl3\n"}))
            .await
            .unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "l1\nl2\nl3\n");

        // Ranged write must SPLICE, not overwrite. This is the data-loss fence:
        // an implementation that ignores `_meta.kiro.range` leaves just "XX".
        respond_write_file(&json!({
            "sessionId": "s", "path": f, "content": "XX",
            "_meta": {"kiro": {"range": {
                "start": {"line": 1, "character": 0},
                "end": {"line": 1, "character": 2}
            }}}
        }))
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "l1\nXX\nl3\n",
            "ranged write must splice; a whole-file write would leave only XX"
        );
    }

    #[tokio::test]
    async fn ranged_write_to_missing_file_splices_against_empty() {
        // LocalSpliceRangeWrite swallows path-not-found and splices against "".
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("fresh.txt");
        respond_write_file(&json!({
            "sessionId": "s", "path": f, "content": "NEW",
            "_meta": {"kiro": {"range": {
                "start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}
            }}}
        }))
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "NEW");
    }

    #[tokio::test]
    async fn stat_reports_type_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        std::fs::write(&f, "12345").unwrap();

        let file = body(
            respond_stat(&json!({"sessionId": "s", "path": f}))
                .await
                .unwrap(),
        );
        assert_eq!(file["type"], "file");
        assert_eq!(file["size"], 5);
        // `size` is required by KAS's response guard even though only `type` is
        // consumed — dropping it yields "Invalid stat response".
        assert!(file.get("size").is_some(), "size key is load-bearing");

        let d = body(
            respond_stat(&json!({"sessionId": "s", "path": dir.path()}))
                .await
                .unwrap(),
        );
        assert_eq!(d["type"], "directory");

        assert!(
            respond_stat(&json!({"sessionId": "s", "path": dir.path().join("nope")}))
                .await
                .is_err(),
            "a missing path must error so exists() answers false"
        );
    }

    #[tokio::test]
    async fn read_directory_types_sorts_and_tolerates_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("z.txt"), "").unwrap();
        std::fs::create_dir(dir.path().join("adir")).unwrap();

        let got = body(
            respond_read_directory(&json!({"sessionId": "s", "path": dir.path()}))
                .await
                .unwrap(),
        );
        let entries = got["entries"].as_array().unwrap().clone();
        assert_eq!(
            entries,
            vec![
                json!({"name": "adir", "type": "directory"}),
                json!({"name": "z.txt", "type": "file"}),
            ],
            "entries must be name-sorted with reference type names"
        );

        // ENOENT -> empty listing, per the reference, NOT an error.
        let missing = body(
            respond_read_directory(&json!({"sessionId": "s", "path": dir.path().join("nope")}))
                .await
                .unwrap(),
        );
        assert_eq!(missing["entries"], json!([]));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_directory_reports_symlinks_unfollowed() {
        // The reference lists with `withFileTypes` (lstat), so a symlink to a
        // directory is "symlink" — even though respond_stat, which follows,
        // would call the same target "directory". Fails if the listing
        // resolves links.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link")).unwrap();

        let got = body(
            respond_read_directory(&json!({"sessionId": "s", "path": dir.path()}))
                .await
                .unwrap(),
        );
        let entries = got["entries"].as_array().unwrap();
        let link = entries
            .iter()
            .find(|e| e["name"] == "link")
            .expect("link entry");
        assert_eq!(link["type"], "symlink");
    }

    #[tokio::test]
    async fn delete_removes_files_and_recurses_into_directories() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        std::fs::write(&f, "x").unwrap();
        respond_delete(&json!({"sessionId": "s", "path": f}))
            .await
            .unwrap();
        assert!(!f.exists());

        // The 2.16.0 adapter sends no `recursive`, and the reference removes
        // directories recursively — a non-empty directory must go.
        let sub = dir.path().join("tree/inner");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("deep.txt"), "x").unwrap();
        respond_delete(&json!({"sessionId": "s", "path": dir.path().join("tree")}))
            .await
            .unwrap();
        assert!(!dir.path().join("tree").exists());
    }

    #[tokio::test]
    async fn delete_honors_explicit_non_recursive() {
        // `recursive: false` is never sent today but is in the covenant; when
        // it is sent, a non-empty directory must survive.
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path().join("tree");
        std::fs::create_dir(&tree).unwrap();
        std::fs::write(tree.join("f.txt"), "x").unwrap();
        assert!(
            respond_delete(&json!({"sessionId": "s", "path": tree, "recursive": false}))
                .await
                .is_err()
        );
        assert!(tree.join("f.txt").exists(), "nothing may be removed");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn delete_unlinks_the_symlink_not_its_target() {
        // symlink_metadata, not metadata: following would recurse into the
        // pointed-at directory and delete a tree nobody asked about.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep.txt"), "x").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        respond_delete(&json!({"sessionId": "s", "path": link}))
            .await
            .unwrap();
        assert!(!link.exists(), "the link is gone");
        assert!(
            target.join("keep.txt").exists(),
            "the link's target must be untouched"
        );
    }

    #[tokio::test]
    async fn malformed_params_are_invalid_params_not_io_errors() {
        // A dialect drift must read as -32602 on the wire, not as a phantom
        // filesystem failure.
        let err = respond_stat(&json!({"sessionId": "s"}))
            .await
            .expect_err("missing path must be rejected");
        assert!(
            format!("{err:?}").contains("invalid params"),
            "expected an invalid-params error, got {err:?}"
        );
    }
}

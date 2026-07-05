//! KAS-5a host-I/O responders (cyril-7bdu): cyril answers the `fs/*` server→client
//! requests KAS sends when cyril advertises `fs` capabilities (KasEngine, Slice 1).
//!
//! KAS delegates file I/O to the host; these resolvers make cyril the executor —
//! the audit/gate/transform point (ADR-0003). Wire shapes verified @ 2.10.0
//! (`.cyril-7bdu/host_callbacks_2.10.0.json`): bare ACP `fs/read_text_file` /
//! `fs/write_text_file`, every call carries `sessionId`, paths absolute.
//!
//! **Non-blocking invariant (ADR-0004 / claim C4) — satisfied architecturally.**
//! The `KiroClient` fs overrides call these directly, and the acp connection
//! spawns *each* inbound request as its own `spawn_local` task (`rpc.rs:272`,
//! wired at `bridge.rs`), so requests never serialize. These resolvers' only
//! obligation is to *yield*: reads use async `tokio::fs` (which offloads the
//! blocking work to tokio's blocking threadpool), and the atomic write runs its
//! whole sync sequence inside ONE `tokio::task::spawn_blocking` hop — the
//! sanctioned form for sync `std::fs` in this module (cyril-0v42). Either way a
//! stuck file op cannot pin the single-threaded bridge runtime. **Never** call
//! synchronous `std::fs` / `std::process` directly on the bridge runtime: that
//! would pin the bridge thread and starve the loop. (The central loop-mediation
//! *gate* seam is deferred to its first consumer — cyril-g9vt.)

use agent_client_protocol as acp;

/// Answer `fs/read_text_file`: read the file at the (translated) path and return
/// its content, honoring the request's 1-based `line` start and `limit` line count.
///
/// A missing, unreadable, or non-UTF-8 file returns `Err` — never `Ok("")`
/// (a silent empty would masquerade as a successful read of an empty file). The
/// caller surfaces the error to KAS as a failed host callback.
pub(crate) async fn read_text_file(
    req: &acp::ReadTextFileRequest,
) -> acp::Result<acp::ReadTextFileResponse> {
    let path = to_native_checked(&req.path)?;
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| io_err("read_text_file", &path, e))?;
    Ok(acp::ReadTextFileResponse::new(slice_lines(
        text, req.line, req.limit,
    )))
}

/// Answer `fs/write_text_file`: atomically write `content` to the (translated)
/// path via [`write_atomic`] (temp + fsync + rename — never truncate-in-place),
/// creating any missing parent directories (`mkdir -p`). An empty `content`
/// writes an empty file — not a no-op. A failed mkdir, refused target
/// (directory / read-only / dangling symlink), or failed write returns `Err`.
pub(crate) async fn write_text_file(
    req: &acp::WriteTextFileRequest,
) -> acp::Result<acp::WriteTextFileResponse> {
    let path = to_native_checked(&req.path)?;
    let target = path.clone();
    let content = req.content.clone();
    tokio::task::spawn_blocking(move || write_atomic(&target, &content))
        .await
        .map_err(|e| {
            tracing::debug!(path = %path.display(), error = %e, "KAS fs write task failed");
            acp::Error::new(
                -32603,
                format!("write_text_file {}: task failed: {e}", path.display()),
            )
        })?
        .map_err(|e| io_err("write_text_file", &path, e))?;
    Ok(acp::WriteTextFileResponse::new())
}

/// Write `content` to `path` atomically: temp file in the target's own
/// directory → write → fsync → clone target permissions → rename over the
/// canonical target. An interrupted write can never leave a partial file —
/// the target is byte-exactly old or byte-exactly new (cyril-0v42; probe
/// evidence in `.cyril-0v42/`, where a SIGKILL'd `tokio::fs::write` left a
/// 0-byte target).
///
/// Behavior gates (design decisions D1-D3, all probe-validated):
/// - a directory, dangling-symlink, or read-only target is refused with a
///   distinct error and nothing is mutated (rename would otherwise silently
///   bypass a 0444 file's protection — probe S17);
/// - a symlink target is written THROUGH via canonicalize (link preserved,
///   destination replaced — matching today's behavior, probe S14);
/// - a missing target gets the same `0o666 & !umask` mode a direct create
///   yields (the mode passes through `open(2)`, where the process umask
///   applies natively — probe S15) and missing parents are created;
/// - an existing target keeps its permission bits (probe S13; naive
///   temp+persist clobbers 0755 → 0600, probe S3).
///
/// The temp lives in the canonical target's parent, NEVER the default temp
/// directory: `$TMPDIR` is environment-controlled and can sit on a different
/// filesystem, where rename fails EXDEV (probe S9/S18). Sync `std::fs` is
/// correct here — the caller runs this inside `spawn_blocking` (see module
/// doc). On SIGKILL a hidden `.tmpXXXXXX` may be orphaned in the target's
/// directory (`Drop` cleanup cannot run); every non-kill failure path cleans
/// it via `Drop`.
///
/// Deliberately distinct from `crate::kiro_agent_config::write_atomic`
/// (default-build, fixed temp name, no fsync/mode handling): that helper is
/// a single-writer convenience for cyril's own config file, while this one
/// guards arbitrary USER files, so it pays for durability (fsync),
/// concurrency-safe random temp names, and permission fidelity — different
/// tiers, not duplication.
fn write_atomic(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind, Write as _};
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        // Missing target (or dangling link): canonicalize the parent instead,
        // creating it mkdir-p style first — the existing resolver contract.
        Err(_) => {
            let parent = path.parent().ok_or_else(|| {
                Error::new(ErrorKind::InvalidInput, "target has no parent directory")
            })?;
            std::fs::create_dir_all(parent)?;
            let name = path
                .file_name()
                .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "target has no file name"))?;
            std::fs::canonicalize(parent)?.join(name)
        }
    };
    let existing = match std::fs::metadata(&canonical) {
        Ok(m) if m.is_dir() => {
            return Err(Error::new(ErrorKind::InvalidInput, "target is a directory"));
        }
        Ok(m) if m.permissions().readonly() => {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "target is read-only",
            ));
        }
        Ok(m) => Some(m.permissions()),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            // create_new/O_EXCL semantics: a symlink whose destination is
            // missing still EXISTS as a link — refuse rather than replace the
            // user's link with a regular file (D2; rename would destroy it).
            if std::fs::symlink_metadata(&canonical).is_ok() {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "target is a dangling symlink",
                ));
            }
            None
        }
        Err(e) => return Err(e),
    };
    let dir = canonical.parent().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "canonical target has no parent directory",
        )
    })?;
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    if existing.is_none() {
        use std::os::unix::fs::PermissionsExt as _;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let mut tmp = builder.tempfile_in(dir).map_err(|e| {
        Error::new(
            e.kind(),
            format!("create temp file in {}: {e}", dir.display()),
        )
    })?;
    tmp.write_all(content.as_bytes())?;
    tmp.as_file().sync_all()?;
    if let Some(perms) = existing {
        tmp.as_file().set_permissions(perms)?;
    }
    tmp.persist(&canonical).map_err(|e| {
        Error::new(
            e.error.kind(),
            format!("persist temp over {}: {}", canonical.display(), e.error),
        )
    })?;
    Ok(())
}

/// Confirm the agent-provided `path` is absolute, then translate it to the native
/// filesystem path. ACP guarantees an absolute `path`; a relative one would
/// otherwise resolve against the bridge's process cwd and silently read/write the
/// WRONG file — load-bearing (CLAUDE.md), so a runtime check, not a
/// `debug_assert!`. The `-32602` (invalid params) code + "must be absolute"
/// message distinguish this from a missing-file `-32603`.
///
/// `pub(crate)`: KAS-5b's `terminal_io::create` reuses this for `terminal/create`'s
/// `cwd` — same contract (absolute-or-reject, then translate), so it is not duplicated.
///
/// Absoluteness is judged on the *agent* (POSIX) path with `has_root()`, NOT
/// `is_absolute()`: KAS runs under Unix/WSL and sends `/`-rooted paths, but on
/// Windows `Path::is_absolute()` is `false` for any path without a drive prefix
/// (`/mnt/c/...`, `/home/...`), so an `is_absolute()` check — before *or* after
/// translation — would reject every KAS callback on Windows. `has_root()` is
/// `true` for a `/`-rooted path on both platforms and `false` for a relative one.
/// Translation then maps `/mnt/<drive>` to `<DRIVE>:\` on Windows (no-op on
/// Linux); a non-translatable-but-absolute path (e.g. a WSL-internal `/home/...`
/// reached from a Windows host) is left to fail as a normal `-32603` NotFound
/// rather than be misreported as non-absolute.
pub(crate) fn to_native_checked(path: &std::path::Path) -> acp::Result<std::path::PathBuf> {
    if !path.has_root() {
        tracing::warn!(path = %path.display(), "KAS host-io path is not absolute; rejecting");
        return Err(acp::Error::new(
            -32602,
            format!("path must be absolute: {}", path.display()),
        ));
    }
    Ok(crate::platform::path::to_native(path))
}

/// Build a `-32603` host-callback error for a failed fs op, logging the io error
/// (incl. `NotFound` vs `PermissionDenied`) so wire/FS drift is diagnosable —
/// surface, don't swallow (CLAUDE.md). `op` names the operation and leads both the
/// structured log and the wire message.
fn io_err(op: &str, path: &std::path::Path, e: std::io::Error) -> acp::Error {
    tracing::debug!(op = %op, path = %path.display(), error = %e, "KAS fs host-io failed");
    acp::Error::new(-32603, format!("{op} {}: {e}", path.display()))
}

/// Select `[line, line+limit)` (1-based `line`) from `text`, preserving each
/// line's trailing newline. `None`/`None` returns the whole text unchanged.
///
/// Takes `text` by value so the whole-file (`None`/`None`) path moves the buffer
/// out with no copy; only a real slice allocates.
///
/// O(L) over the file's lines (single pass); L ≲ 10^5 for a large source file,
/// well under the 10^6 loop budget.
fn slice_lines(text: String, line: Option<u32>, limit: Option<u32>) -> String {
    if line.is_none() && limit.is_none() {
        return text;
    }
    // `split_inclusive` keeps the `\n` on each piece, so a selected slice round-trips
    // byte-exact (unlike `.lines()`, which strips newlines).
    let start = line.unwrap_or(1).saturating_sub(1) as usize;
    let rest = text.split_inclusive('\n').skip(start);
    match limit {
        Some(m) => rest.take(m as usize).collect(),
        None => rest.collect(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn read_req(
        path: &std::path::Path,
        line: Option<u32>,
        limit: Option<u32>,
    ) -> acp::ReadTextFileRequest {
        acp::ReadTextFileRequest::new(acp::SessionId::new("s"), path)
            .line(line)
            .limit(limit)
    }

    #[test]
    fn slice_lines_whole_file_when_no_line_limit() {
        let t = "l1\nl2\nl3\n";
        assert_eq!(slice_lines(t.to_string(), None, None), t);
    }

    #[test]
    fn slice_lines_honors_line_and_limit() {
        // Stress fixture (a): 5 distinct lines; line=2,limit=1 must yield exactly
        // "l2\n" — fails if the resolver ignores line/limit (returns whole file).
        let t = "l1\nl2\nl3\nl4\nl5\n";
        assert_eq!(slice_lines(t.to_string(), Some(2), Some(1)), "l2\n");
        // line only: from line 4 to end.
        assert_eq!(slice_lines(t.to_string(), Some(4), None), "l4\nl5\n");
        // limit only: first 2 lines.
        assert_eq!(slice_lines(t.to_string(), None, Some(2)), "l1\nl2\n");
    }

    #[test]
    fn slice_lines_line_zero_is_whole_file() {
        // `line: 0` is the ONLY line value KAS is observed to send on the wire
        // (.cyril-7bdu/fixtures/fs__read_text_file.json). `saturating_sub` floors
        // 0 -> start 0, so it must return the whole file from the top — identical
        // to `line: 1` and to `None`. Locks in the real wire value as a regression
        // guard (a 0-based reinterpretation of `line` would break this).
        let t = "l1\nl2\nl3\n";
        assert_eq!(slice_lines(t.to_string(), Some(0), None), t);
        assert_eq!(
            slice_lines(t.to_string(), Some(0), None),
            slice_lines(t.to_string(), Some(1), None)
        );
    }

    #[tokio::test]
    async fn read_returns_content_and_honors_line_limit() {
        // Claim C6. Round-trip through the real resolver: write a 5-line file,
        // read line=2 limit=1 -> "l2\n". Oracle: the expected slice computed here,
        // independent of the resolver's read path.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("notes.txt");
        std::fs::write(&f, "l1\nl2\nl3\nl4\nl5\n").unwrap();
        let resp = read_text_file(&read_req(&f, Some(2), Some(1)))
            .await
            .unwrap();
        assert_eq!(resp.content, "l2\n");
        let whole = read_text_file(&read_req(&f, None, None)).await.unwrap();
        assert_eq!(whole.content, "l1\nl2\nl3\nl4\nl5\n");
    }

    #[tokio::test]
    async fn read_missing_path_errors_not_empty() {
        // Claim C7 / stress fixture (b): a nonexistent path must return Err, never
        // Ok("") — fails under `read_to_string(..).unwrap_or_default()`.
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.txt");
        let result = read_text_file(&read_req(&missing, None, None)).await;
        assert!(result.is_err(), "missing path must error, got {result:?}");
    }

    #[tokio::test]
    async fn write_creates_parents_and_exact_content() {
        // Claim C8 / stress fixture: write EMPTY content into a path whose parent
        // dir does NOT exist -> the dir is created and an empty file written. Fails
        // under a missing `create_dir_all` (write errors) or an `if !is_empty`
        // guard (empty content no-ops, file absent). Oracle: read back with std::fs.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a/b/c.txt"); // a/b does not exist yet
        let req = acp::WriteTextFileRequest::new(acp::SessionId::new("s"), &target, "");
        write_text_file(&req).await.unwrap();
        assert!(target.exists(), "write must create parent dirs + the file");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "");
        // Non-empty Unicode round-trips byte-exact.
        let req2 =
            acp::WriteTextFileRequest::new(acp::SessionId::new("s"), &target, "héllo\n世界\n");
        write_text_file(&req2).await.unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "héllo\n世界\n");
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_preserves_existing_mode() {
        // C2 fence (cyril-0v42, probe S13): 0755 stays 0755 — a naive
        // NamedTempFile+persist clobbers it to 0600. The 0600 case guards the
        // WIDENING direction S13 alone cannot catch: a bug that applies the
        // fresh-file 0666&umask branch to existing targets would leak a
        // secret's bits up to 0644.
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        for mode in [0o755_u32, 0o600] {
            let f = dir.path().join(format!("m{mode:o}.txt"));
            std::fs::write(&f, "OLD").unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(mode)).unwrap();
            write_atomic(&f, "NEW").unwrap();
            assert_eq!(std::fs::read_to_string(&f).unwrap(), "NEW");
            assert_eq!(
                std::fs::metadata(&f).unwrap().permissions().mode() & 0o7777,
                mode,
                "mode {mode:o} must survive the atomic write"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_fresh_matches_plain_create_mode() {
        // C3 fence (cyril-0v42, probe S15): a fresh target (missing parents
        // included) must get the same umask-derived mode a direct create
        // yields. The control file is made by std::fs::File::create — the
        // very mechanism today's write path uses — so the expected mode is
        // computed independently of the code under test. Fails if the temp
        // file's restrictive default (0600) leaks through to the target.
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control.txt");
        drop(std::fs::File::create(&control).unwrap());
        let fresh = dir.path().join("a/b/fresh.txt");
        write_atomic(&fresh, "NEW").unwrap();
        assert_eq!(
            std::fs::metadata(&fresh).unwrap().permissions().mode() & 0o7777,
            std::fs::metadata(&control).unwrap().permissions().mode() & 0o7777,
            "fresh atomic write must match plain-create umask mode"
        );
    }

    #[test]
    fn write_atomic_empty_and_unicode_byte_exact() {
        // C5 fence (cyril-0v42, probe S8): empty content REPLACES existing
        // content with an empty file — fails under an `if !content.is_empty()`
        // no-op guard. Unicode must round-trip byte-exact.
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("c.txt");
        std::fs::write(&f, "OLD").unwrap();
        write_atomic(&f, "").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"");
        write_atomic(&f, "héllo\n世界\n").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "héllo\n世界\n");
    }

    #[tokio::test]
    async fn relative_path_rejected_with_absolute_error() {
        // Claim C10: a non-absolute path is rejected with the DISTINCT "must be
        // absolute" error — never silently read/written against the bridge process
        // cwd. The distinct error (vs a -32603 missing-file error) is what makes
        // this non-vacuous: a no-guard impl would instead try to read/write
        // "rel.txt" relative to the process cwd, yielding a different error (or, if
        // such a file existed, Ok) — both fail these assertions.
        let rel = std::path::Path::new("kas5a_relative_xyz.txt");
        let rerr = read_text_file(&read_req(rel, None, None))
            .await
            .expect_err("relative read must be rejected");
        assert!(
            format!("{rerr:?}").contains("must be absolute"),
            "expected absolute-path rejection, got {rerr:?}"
        );
        let wreq = acp::WriteTextFileRequest::new(acp::SessionId::new("s"), rel, "x");
        let werr = write_text_file(&wreq)
            .await
            .expect_err("relative write must be rejected");
        assert!(
            format!("{werr:?}").contains("must be absolute"),
            "expected absolute-path rejection, got {werr:?}"
        );
    }
}

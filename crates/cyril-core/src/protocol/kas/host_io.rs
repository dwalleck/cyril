//! KAS-5a host-I/O responders (cyril-7bdu): cyril answers the `fs/*` server→client
//! requests KAS sends when cyril advertises `fs` capabilities (KasEngine, Slice 1).
//!
//! KAS delegates file I/O to the host; these resolvers make cyril the executor —
//! the audit/gate/transform point (ADR-0003). Wire shapes verified @ 2.10.0
//! (`.cyril-7bdu/host_callbacks_2.10.0.json`): bare ACP `fs/read_text_file` /
//! `fs/write_text_file`, every call carries `sessionId`, paths absolute.
//!
//! These run **off the bridge loop** (spawned per request, Slice 5) and use async
//! `tokio::fs` — a synchronous `std::fs` call here would pin the single-threaded
//! bridge runtime and starve the loop (ADR-0004 non-blocking invariant).

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
    require_absolute(&req.path)?;
    let path = crate::platform::path::to_native(&req.path);
    let text = tokio::fs::read_to_string(&path).await.map_err(|e| {
        // Surface, don't swallow (CLAUDE.md): the io error (incl. NotFound vs
        // PermissionDenied) rides the message so wire/FS drift is diagnosable.
        tracing::debug!(path = %path.display(), error = %e, "KAS fs/read_text_file failed");
        acp::Error::new(-32603, format!("read_text_file {}: {e}", path.display()))
    })?;
    Ok(acp::ReadTextFileResponse::new(slice_lines(
        &text, req.line, req.limit,
    )))
}

/// Answer `fs/write_text_file`: write `content` to the (translated) path,
/// creating any missing parent directories (`mkdir -p`). An empty `content`
/// writes an empty file — not a no-op. A failed mkdir or write returns `Err`.
pub(crate) async fn write_text_file(
    req: &acp::WriteTextFileRequest,
) -> acp::Result<acp::WriteTextFileResponse> {
    require_absolute(&req.path)?;
    let path = crate::platform::path::to_native(&req.path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            tracing::debug!(path = %path.display(), error = %e, "KAS fs/write_text_file mkdir failed");
            acp::Error::new(-32603, format!("create parent dir for {}: {e}", path.display()))
        })?;
    }
    tokio::fs::write(&path, &req.content).await.map_err(|e| {
        tracing::debug!(path = %path.display(), error = %e, "KAS fs/write_text_file failed");
        acp::Error::new(-32603, format!("write_text_file {}: {e}", path.display()))
    })?;
    Ok(acp::WriteTextFileResponse::new())
}

/// Reject a non-absolute host-io path with a distinct error. ACP guarantees an
/// absolute `path`; a relative one would otherwise resolve against the bridge's
/// process cwd and silently read/write the WRONG file — load-bearing (CLAUDE.md),
/// so a runtime check, not a `debug_assert!`. The `-32602` (invalid params) code +
/// "must be absolute" message distinguish this from a missing-file `-32603`.
fn require_absolute(path: &std::path::Path) -> acp::Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        tracing::warn!(path = %path.display(), "KAS host-io path is not absolute; rejecting");
        Err(acp::Error::new(
            -32602,
            format!("path must be absolute: {}", path.display()),
        ))
    }
}

/// Select `[line, line+limit)` (1-based `line`) from `text`, preserving each
/// line's trailing newline. `None`/`None` returns the whole text unchanged.
///
/// O(L) over the file's lines (single pass); L ≲ 10^5 for a large source file,
/// well under the 10^6 loop budget.
fn slice_lines(text: &str, line: Option<u32>, limit: Option<u32>) -> String {
    if line.is_none() && limit.is_none() {
        return text.to_string();
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
        assert_eq!(slice_lines(t, None, None), t);
    }

    #[test]
    fn slice_lines_honors_line_and_limit() {
        // Stress fixture (a): 5 distinct lines; line=2,limit=1 must yield exactly
        // "l2\n" — fails if the resolver ignores line/limit (returns whole file).
        let t = "l1\nl2\nl3\nl4\nl5\n";
        assert_eq!(slice_lines(t, Some(2), Some(1)), "l2\n");
        // line only: from line 4 to end.
        assert_eq!(slice_lines(t, Some(4), None), "l4\nl5\n");
        // limit only: first 2 lines.
        assert_eq!(slice_lines(t, None, Some(2)), "l1\nl2\n");
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

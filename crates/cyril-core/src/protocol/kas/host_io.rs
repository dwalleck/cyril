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
}

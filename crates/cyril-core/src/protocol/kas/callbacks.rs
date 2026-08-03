//! Typed host callbacks (cyril-g9vt, ADR-0004 amendment): the exhaustive
//! internal form every handled host-callback request and control notification
//! takes before crossing the bridge's host-callback mediation seam. No raw
//! JSON and no opaque closures cross — `KiroClient` parses HERE, the mediator
//! handles lifecycle, and the dispatch side resolves against the
//! Engine-selected adapter set (ADR-0001).
//!
//! Variant census: the enum grows one family per cutover slice (variants are
//! STAGED with their constructing slice — lib-target dead_code enforces it;
//! the full 15-variant shape is at commit b36c3a8 and re-lands family by
//! family). [`WIRE_METHODS`] stays the full 19-method census
//! (`.cyril-dn91/findings.md`) throughout, fenced by
//! [`tests::census_matches_the_dn91_count`].

use agent_client_protocol as acp;

use crate::protocol::host_mediator::{CallbackMeta, CancelKey, finish};
use crate::protocol::kas::kiro_fs;
use crate::types::SessionId;

/// The typed reply channel for one request-kind callback: the acp-side
/// request task awaits this and converts the typed outcome to the exact ACP
/// response/error.
pub(crate) type Reply<T> = tokio::sync::oneshot::Sender<acp::Result<T>>;

/// One handled host callback, parsed and typed. Request variants carry their
/// typed reply channel; control variants carry none. Families beyond Auth
/// land with their cutover slices (Host I/O, then Hooks).
#[derive(Debug)]
pub(crate) enum HostCallback {
    /// `_kiro/auth/getAccessToken` (KAS-1).
    GetAccessToken { reply: Reply<acp::ExtResponse> },
    /// Bare-ACP `fs/read_text_file` (KAS-5a) — already typed by the acp layer.
    ReadTextFile {
        req: acp::ReadTextFileRequest,
        reply: Reply<acp::ReadTextFileResponse>,
    },
    /// Bare-ACP `fs/write_text_file` (KAS-5a).
    WriteTextFile {
        req: acp::WriteTextFileRequest,
        reply: Reply<acp::WriteTextFileResponse>,
    },
    /// One `_kiro/fs/*` dialect op (cyril-kf2g), params parsed per op.
    KiroFs {
        args: KiroFsArgs,
        reply: Reply<acp::ExtResponse>,
    },
    /// `terminal/create` (KAS-5b).
    CreateTerminal {
        req: acp::CreateTerminalRequest,
        reply: Reply<acp::CreateTerminalResponse>,
    },
    /// `terminal/wait_for_exit`.
    WaitForTerminalExit {
        req: acp::WaitForTerminalExitRequest,
        reply: Reply<acp::WaitForTerminalExitResponse>,
    },
    /// `terminal/output`.
    TerminalOutput {
        req: acp::TerminalOutputRequest,
        reply: Reply<acp::TerminalOutputResponse>,
    },
    /// `terminal/release`.
    ReleaseTerminal {
        req: acp::ReleaseTerminalRequest,
        reply: Reply<acp::ReleaseTerminalResponse>,
    },
    /// `terminal/kill`.
    KillTerminal {
        req: acp::KillTerminalRequest,
        reply: Reply<acp::KillTerminalResponse>,
    },
    /// `_kiro/terminal/shell_type`.
    ShellType {
        session_id: Option<String>,
        reply: Reply<acp::ExtResponse>,
    },
}

/// `_kiro/fs/*` per-op typed params, reusing the structs `kiro_fs` already
/// deserializes with (single owner of the field knowledge). Dispatch
/// re-serializes them for the untouched responder signatures — a validated
/// struct's round-trip cannot fail, so the double-parse is shape-safe.
#[derive(Debug)]
pub(crate) enum KiroFsArgs {
    ReadFile(kiro_fs::ReadFileParams),
    WriteFile(kiro_fs::WriteFileParams),
    Stat(kiro_fs::PathParams),
    ReadDirectory(kiro_fs::PathParams),
    Delete(kiro_fs::PathParams),
}

impl KiroFsArgs {
    /// Parse one dialect op's params into its typed form. `Err` carries the
    /// serde diagnostic — the caller maps it to invalid-params (never the old
    /// `parse_ext_params` Null fallback: for a HANDLED family, unparseable
    /// params are a wire error, not an empty request).
    pub(crate) fn parse(
        op: &'static kiro_fs::FsOp,
        params: &serde_json::Value,
    ) -> Result<Self, String> {
        let fail = |e: serde_json::Error| format!("parse {} params: {e}", op.wire);
        Ok(match op.kind {
            kiro_fs::FsOpKind::ReadFile => {
                Self::ReadFile(serde_json::from_value(params.clone()).map_err(fail)?)
            }
            kiro_fs::FsOpKind::WriteFile => {
                Self::WriteFile(serde_json::from_value(params.clone()).map_err(fail)?)
            }
            kiro_fs::FsOpKind::Stat => {
                Self::Stat(serde_json::from_value(params.clone()).map_err(fail)?)
            }
            kiro_fs::FsOpKind::ReadDirectory => {
                Self::ReadDirectory(serde_json::from_value(params.clone()).map_err(fail)?)
            }
            kiro_fs::FsOpKind::Delete => {
                Self::Delete(serde_json::from_value(params.clone()).map_err(fail)?)
            }
        })
    }

    fn op(&self) -> &'static kiro_fs::FsOp {
        let kind = match self {
            Self::ReadFile(_) => kiro_fs::FsOpKind::ReadFile,
            Self::WriteFile(_) => kiro_fs::FsOpKind::WriteFile,
            Self::Stat(_) => kiro_fs::FsOpKind::Stat,
            Self::ReadDirectory(_) => kiro_fs::FsOpKind::ReadDirectory,
            Self::Delete(_) => kiro_fs::FsOpKind::Delete,
        };
        kiro_fs::op_for_kind(kind)
    }

    /// Re-serialize for the untouched `kiro_fs::dispatch(op, &Value)`
    /// signature (see the type doc).
    fn to_params(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            Self::ReadFile(p) => serde_json::to_value(p),
            Self::WriteFile(p) => serde_json::to_value(p),
            Self::Stat(p) | Self::ReadDirectory(p) | Self::Delete(p) => serde_json::to_value(p),
        }
    }
}

impl HostCallback {
    /// Resolve this callback's reply channel with `err` — the terminal path
    /// for aborted, shut-down, or dispatch-refused work. Consuming `self`
    /// guarantees a request-kind callback cannot outlive its resolution. A
    /// dead receiver (the acp-side task already gone) is logged, never a
    /// panic (design C7).
    /// Test-staged: its production consumers (the dispatch refusal/abort
    /// paths of later family slices) re-land it.
    #[cfg(test)]
    pub(crate) fn resolve_err(self, err: acp::Error) {
        fn send<T>(kind: &'static str, reply: Reply<T>, err: acp::Error) {
            if reply.send(Err(err)).is_err() {
                tracing::debug!(kind, "callback error reply dropped (responder gone)");
            }
        }
        let kind = self.kind();
        match self {
            Self::GetAccessToken { reply } => send(kind, reply, err),
            Self::ReadTextFile { reply, .. } => send(kind, reply, err),
            Self::WriteTextFile { reply, .. } => send(kind, reply, err),
            Self::KiroFs { reply, .. } => send(kind, reply, err),
            Self::CreateTerminal { reply, .. } => send(kind, reply, err),
            Self::WaitForTerminalExit { reply, .. } => send(kind, reply, err),
            Self::TerminalOutput { reply, .. } => send(kind, reply, err),
            Self::ReleaseTerminal { reply, .. } => send(kind, reply, err),
            Self::KillTerminal { reply, .. } => send(kind, reply, err),
            Self::ShellType { reply, .. } => send(kind, reply, err),
        }
    }
}

impl CallbackMeta for HostCallback {
    fn cancels(&self) -> Option<CancelKey> {
        None
    }

    fn cancel_key(&self) -> Option<CancelKey> {
        None
    }

    fn scope(&self) -> Option<SessionId> {
        let sid = |s: &acp::SessionId| SessionId::new(s.to_string());
        match self {
            Self::GetAccessToken { .. } | Self::KiroFs { .. } => None,
            Self::ReadTextFile { req, .. } => Some(sid(&req.session_id)),
            Self::WriteTextFile { req, .. } => Some(sid(&req.session_id)),
            Self::CreateTerminal { req, .. } => Some(sid(&req.session_id)),
            Self::WaitForTerminalExit { req, .. } => Some(sid(&req.session_id)),
            Self::TerminalOutput { req, .. } => Some(sid(&req.session_id)),
            Self::ReleaseTerminal { req, .. } => Some(sid(&req.session_id)),
            Self::KillTerminal { req, .. } => Some(sid(&req.session_id)),
            Self::ShellType { session_id, .. } => {
                session_id.as_ref().map(|s| SessionId::new(s.clone()))
            }
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::GetAccessToken { .. } => "auth/getAccessToken",
            Self::ReadTextFile { .. } => "fs/read_text_file",
            Self::WriteTextFile { .. } => "fs/write_text_file",
            Self::KiroFs { args, .. } => args.op().wire,
            Self::CreateTerminal { .. } => "terminal/create",
            Self::WaitForTerminalExit { .. } => "terminal/wait_for_exit",
            Self::TerminalOutput { .. } => "terminal/output",
            Self::ReleaseTerminal { .. } => "terminal/release",
            Self::KillTerminal { .. } => "terminal/kill",
            Self::ShellType { .. } => "_kiro/terminal/shell_type",
        }
    }
}

/// Loop-side context the dispatch resolves against. Grows one field per
/// adapter family as its cutover slice lands; the mediator never sees it
/// (design C12).
pub(crate) struct DispatchCtx {
    /// The bridge's internal notification sender — [`finish`]'s ordering
    /// channel for user-visible callback failures.
    pub(crate) notify_tx: tokio::sync::mpsc::Sender<crate::types::RoutedNotification>,
    /// The terminal registry (KAS-5b), loop-side since cyril-g9vt slice 5 —
    /// constructed in `run_bridge` and owned by the dispatch context, deleting
    /// the cyril-3lh8 escape (the `Rc` formerly grabbed out of `KiroClient`).
    /// Still the sole owner of process lifecycle.
    pub(crate) terminals: std::rc::Rc<crate::protocol::kas::terminal_io::TerminalRegistry>,
}

/// Resolve one accepted callback against the adapter-side responders. The
/// match is exhaustive over the CURRENT variant set — a family cannot reach
/// the channel before its cutover slice constructs it.
pub(crate) async fn dispatch(cb: HostCallback, ctx: &DispatchCtx) {
    match cb {
        HostCallback::CreateTerminal { req, reply } => {
            let result = ctx.terminals.create(&req);
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("terminal/create reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::WaitForTerminalExit { req, reply } => {
            let result = ctx.terminals.wait(&req).await;
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("terminal/wait_for_exit reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::TerminalOutput { req, reply } => {
            let result = ctx.terminals.output(&req);
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("terminal/output reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::ReleaseTerminal { req, reply } => {
            let result = ctx.terminals.release(&req).await;
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("terminal/release reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::KillTerminal { req, reply } => {
            let result = ctx.terminals.kill(&req).await;
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("terminal/kill reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::ShellType { reply, .. } => {
            let result = ctx.terminals.respond_shell_type();
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("shell_type reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::ReadTextFile { req, reply } => {
            let result = crate::protocol::kas::host_io::read_text_file(&req).await;
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("fs/read_text_file reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::WriteTextFile { req, reply } => {
            let result = crate::protocol::kas::host_io::write_text_file(&req).await;
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("fs/write_text_file reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::KiroFs { args, reply } => {
            let op = args.op();
            let result = match args.to_params() {
                Ok(params) => crate::protocol::kas::kiro_fs::dispatch(op, &params).await,
                Err(e) => Err(acp::Error::new(
                    -32603,
                    format!("re-serialize {} params: {e}", op.wire),
                )),
            };
            finish(&ctx.notify_tx, None, move || {
                if reply.send(result).is_err() {
                    tracing::debug!(wire = op.wire, "kiro_fs reply dropped (responder gone)");
                }
            })
            .await;
        }
        HostCallback::GetAccessToken { reply } => {
            let result = crate::protocol::kas::auth::respond_get_access_token().await;
            let notify = result.as_ref().err().and_then(auth_failure_notification);
            finish(&ctx.notify_tx, notify, move || {
                if reply.send(result).is_err() {
                    tracing::debug!("getAccessToken reply dropped (responder gone)");
                }
            })
            .await;
        }
    }
}

/// The user-visible surface of a failed auth callback (cyril-l7tw C11, moved
/// here from `KiroClient::notify_if_auth_failure` by the mediation cutover):
/// the responder's message with the login hint appended exactly once.
/// `MethodNotFound` is exempt — a refusal is not an auth failure (dn91 C13);
/// with parse-time refusals it cannot reach dispatch, but the guard keeps the
/// exemption local should a responder ever emit that code itself.
fn auth_failure_notification(e: &acp::Error) -> Option<crate::types::Notification> {
    if e.code == acp::ErrorCode::MethodNotFound {
        return None;
    }
    let mut message = e.message.clone();
    let hint = crate::protocol::kas::auth::LOGIN_HINT;
    if !message.contains(hint) {
        message.push_str(&format!(" — {hint} and retry"));
    }
    Some(crate::types::Notification::BridgeError {
        operation: "auth".into(),
        message,
    })
}

/// The full census of wire methods the callback seam will cover — 19,
/// matching the dn91 probe census. Static from slice 2 onward; the enum
/// catches up to it family by family, and slice 8's end-to-end census drives
/// every entry through the client entry points.
#[cfg(test)]
pub(crate) const WIRE_METHODS: &[&str] = &[
    "auth/getAccessToken",
    "fs/read_text_file",
    "fs/write_text_file",
    "_kiro/fs/read_file",
    "_kiro/fs/write_file",
    "_kiro/fs/stat",
    "_kiro/fs/read_directory",
    "_kiro/fs/delete",
    "terminal/create",
    "terminal/wait_for_exit",
    "terminal/output",
    "terminal/release",
    "terminal/kill",
    "_kiro/terminal/shell_type",
    "_kiro/hooks/list",
    "_kiro/hooks/executeHook",
    "_kiro/hooks/sessionStart",
    "_kiro/hooks/cancel",
    "_kiro/hooks/didChange",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // The census: 19 wire methods, exactly the dn91 probe count (the
    // independent oracle for the seam's completeness target).
    #[test]
    fn census_matches_the_dn91_count() {
        assert_eq!(WIRE_METHODS.len(), 19);
        let mut sorted: Vec<_> = WIRE_METHODS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 19, "census entries are distinct");
    }

    // The reply channel rides the envelope to the resolution task and back —
    // the typed round-trip the dispatch depends on.
    #[tokio::test]
    async fn envelope_reply_channel_round_trips() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cb = HostCallback::GetAccessToken { reply: tx };
        assert_eq!(cb.kind(), "auth/getAccessToken");
        assert!(WIRE_METHODS.contains(&cb.kind()));
        assert_eq!(cb.scope(), None);
        match cb {
            HostCallback::GetAccessToken { reply } => {
                let body =
                    serde_json::value::RawValue::from_string("{\"ok\":true}".into()).unwrap();
                reply
                    .send(Ok(acp::ExtResponse::new(body.into())))
                    .expect("receiver alive");
            }
            other => panic!("wrong variant: {other:?}"),
        }
        let got = rx.await.expect("reply delivered").expect("ok reply");
        assert_eq!(got.0.get(), "{\"ok\":true}");
    }

    // resolve_err is the terminal path for abort/shutdown/refusal: the typed
    // error reaches the awaiting receiver; a dead receiver doesn't panic.
    #[tokio::test]
    async fn resolve_err_reaches_receiver_and_survives_drop() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        HostCallback::GetAccessToken { reply: tx }.resolve_err(acp::Error::new(-32603, "shutdown"));
        let err = rx.await.expect("delivered").expect_err("error reply");
        assert_eq!(err.message, "shutdown");

        let (tx, rx) = tokio::sync::oneshot::channel::<acp::Result<acp::ExtResponse>>();
        drop(rx);
        HostCallback::GetAccessToken { reply: tx }.resolve_err(acp::Error::new(-32603, "shutdown"));
    }

    // cyril-l7tw C11, migrated from KiroClient::notify_if_auth_failure by the
    // mediation cutover (slice 3): a failed auth callback surfaces as
    // BridgeError("auth") with the responder message + the login hint.
    #[test]
    fn auth_failure_notification_carries_message_and_hint() {
        let n = auth_failure_notification(&acp::Error::new(-32603, "sqlite store locked"))
            .expect("failure surfaces");
        match n {
            crate::types::Notification::BridgeError { operation, message } => {
                assert_eq!(operation, "auth");
                assert!(message.contains("sqlite store locked"), "got: {message}");
                assert!(
                    message.contains("kiro-cli login"),
                    "hint present: {message}"
                );
            }
            other => panic!("expected BridgeError, got {other:?}"),
        }
    }

    // l7tw C11 stress: the responder's own messages already carry the hint —
    // it must not double.
    #[test]
    fn auth_failure_hint_not_doubled() {
        let n = auth_failure_notification(&acp::Error::new(
            -32603,
            "kiro token expired; run `kiro-cli login`",
        ))
        .expect("failure surfaces");
        match n {
            crate::types::Notification::BridgeError { message, .. } => {
                assert_eq!(message.matches("kiro-cli login").count(), 1, "{message}");
            }
            other => panic!("expected BridgeError, got {other:?}"),
        }
    }

    // dn91 C13 continuity: a method-not-found never surfaces as an auth
    // failure (refusals are parse-time and cannot reach dispatch; the guard
    // keeps the exemption if a responder ever emits the code).
    #[test]
    fn auth_refusal_code_is_exempt() {
        assert!(auth_failure_notification(&acp::Error::method_not_found()).is_none());
    }
}

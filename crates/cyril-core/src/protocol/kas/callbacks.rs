//! Typed host callbacks (cyril-g9vt, ADR-0004 amendment): the exhaustive
//! internal form every handled host-callback request and control notification
//! takes before crossing the bridge's host-callback mediation seam. No raw
//! JSON and no opaque closures cross — `KiroClient` parses HERE, the mediator
//! handles lifecycle, and the dispatch side resolves against the
//! Engine-selected adapter set (ADR-0001).
//!
//! The exhaustive callback enum and [`WIRE_METHODS`] census grow together
//! when a supported host concern is installed. Capability knowledge stays in
//! parsing/dispatch, never in the generic host lifecycle mediator.

use agent_client_protocol::schema::v1 as acp;

use crate::protocol::host_mediator::{CallbackMeta, CancelKey, finish};
use crate::protocol::kas::hooks::HooksListArgs;
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
    /// `_kiro/auth/getAccessToken` (KAS-1); `force_refresh` is the request's
    /// `forceRefresh: true` (cyril-zbu7 — relayed to kiro-cli as
    /// `--force-refresh`).
    GetAccessToken {
        force_refresh: bool,
        reply: Reply<acp::ExtResponse>,
    },
    /// `_kiro/openExternalUrl {url}` — global, with no session/server wire fields.
    OpenExternalUrl {
        args: OpenExternalUrlArgs,
        reply: Reply<acp::ExtResponse>,
    },
    McpElicitation {
        args: super::mcp::ElicitationArgs,
        reply: Reply<acp::ExtResponse>,
    },
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
    /// `_kiro/hooks/list {trigger, toolId?, toolTags?}`. A missing trigger is
    /// preserved as `None` — the responder replies empty (not an error).
    HooksList {
        args: HooksListArgs,
        reply: Reply<acp::ExtResponse>,
    },
    /// `_kiro/hooks/executeHook`. Cancellation stays with the existing
    /// `HookOps` mechanism (the CANCEL control signals it), so this is NOT
    /// mediator-cancellable.
    HooksExecute {
        args: HooksExecuteArgs,
        reply: Reply<acp::ExtResponse>,
    },
    /// `_kiro/hooks/sessionStart`.
    HooksSessionStart { reply: Reply<acp::ExtResponse> },
    /// CONTROL `_kiro/hooks/cancel {operationId}` — signals the HookOps
    /// mechanism; no reply.
    HooksCancel { operation_id: String },
    /// CONTROL `_kiro/hooks/didChange` — under `kas` hook generation carries
    /// the agent's full new registry (parsed, cyril-gk17); under `host` the
    /// payload has no hooks array (`None`). No reply; emits HooksChanged.
    HooksDidChange {
        hooks: Option<Vec<crate::types::HookInfo>>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostFamily {
    Auth,
    ExternalUrl,
    McpElicitation,
    HostIo,
    HooksInbound,
    HooksAny,
}

impl HostCallback {
    pub(crate) const fn family(&self) -> HostFamily {
        match self {
            Self::GetAccessToken { .. } => HostFamily::Auth,
            Self::OpenExternalUrl { .. } => HostFamily::ExternalUrl,
            Self::McpElicitation { .. } => HostFamily::McpElicitation,
            Self::ReadTextFile { .. }
            | Self::WriteTextFile { .. }
            | Self::KiroFs { .. }
            | Self::CreateTerminal { .. }
            | Self::WaitForTerminalExit { .. }
            | Self::TerminalOutput { .. }
            | Self::ReleaseTerminal { .. }
            | Self::KillTerminal { .. }
            | Self::ShellType { .. } => HostFamily::HostIo,
            Self::HooksList { .. }
            | Self::HooksExecute { .. }
            | Self::HooksSessionStart { .. }
            | Self::HooksCancel { .. } => HostFamily::HooksInbound,
            Self::HooksDidChange { .. } => HostFamily::HooksAny,
        }
    }

    pub(crate) fn refuse(self) {
        let error = || agent_client_protocol::Error::method_not_found();
        macro_rules! reject {
            ($reply:expr) => {
                if $reply.send(Err(error())).is_err() {
                    tracing::debug!("host callback refusal receiver dropped");
                }
            };
        }
        match self {
            Self::GetAccessToken { reply, .. }
            | Self::OpenExternalUrl { reply, .. }
            | Self::McpElicitation { reply, .. }
            | Self::KiroFs { reply, .. }
            | Self::ShellType { reply, .. }
            | Self::HooksList { reply, .. }
            | Self::HooksExecute { reply, .. }
            | Self::HooksSessionStart { reply } => reject!(reply),
            Self::ReadTextFile { reply, .. } => reject!(reply),
            Self::WriteTextFile { reply, .. } => reject!(reply),
            Self::CreateTerminal { reply, .. } => reject!(reply),
            Self::WaitForTerminalExit { reply, .. } => reject!(reply),
            Self::TerminalOutput { reply, .. } => reject!(reply),
            Self::ReleaseTerminal { reply, .. } => reject!(reply),
            Self::KillTerminal { reply, .. } => reject!(reply),
            Self::HooksCancel { .. } | Self::HooksDidChange { .. } => {
                tracing::debug!("host control notification refused by engine adapter gate");
            }
        }
    }
}

/// Required URL shape at the wire seam. Validation must not rewrite the URL
/// or include its potentially sensitive query/fragment in error diagnostics.
#[derive(Debug)]
pub(crate) struct OpenExternalUrlArgs {
    url: String,
}

impl OpenExternalUrlArgs {
    pub(crate) fn parse(params: &serde_json::Value) -> Result<Self, &'static str> {
        let url = params
            .get("url")
            .and_then(serde_json::Value::as_str)
            .filter(|url| !url.trim().is_empty())
            .ok_or("openExternalUrl requires a nonempty string `url`")?;
        Ok(Self {
            url: url.to_owned(),
        })
    }
}

/// `executeHook` typed params. Semantic defaults (missing command → exit 127
/// reply, missing userPrompt → empty with a warn) stay with the responder;
/// re-serialized in dispatch for `respond_execute`'s raw-params signature.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HooksExecuteArgs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_id: Option<String>,
}

impl HooksExecuteArgs {
    /// Parse executeHook params at the seam. `Err` on malformed input (a
    /// handled family — not the old Null fallback).
    pub(crate) fn parse(params: &serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(params.clone()).map_err(|e| format!("parse executeHook params: {e}"))
    }
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
            Self::GetAccessToken { reply, .. } => send(kind, reply, err),
            Self::OpenExternalUrl { reply, .. } => send(kind, reply, err),
            Self::McpElicitation { reply, .. } => send(kind, reply, err),
            Self::ReadTextFile { reply, .. } => send(kind, reply, err),
            Self::WriteTextFile { reply, .. } => send(kind, reply, err),
            Self::KiroFs { reply, .. } => send(kind, reply, err),
            Self::CreateTerminal { reply, .. } => send(kind, reply, err),
            Self::WaitForTerminalExit { reply, .. } => send(kind, reply, err),
            Self::TerminalOutput { reply, .. } => send(kind, reply, err),
            Self::ReleaseTerminal { reply, .. } => send(kind, reply, err),
            Self::KillTerminal { reply, .. } => send(kind, reply, err),
            Self::ShellType { reply, .. } => send(kind, reply, err),
            Self::HooksList { reply, .. } => send(kind, reply, err),
            Self::HooksExecute { reply, .. } => send(kind, reply, err),
            Self::HooksSessionStart { reply } => send(kind, reply, err),
            Self::HooksCancel { .. } | Self::HooksDidChange { .. } => {}
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
            Self::McpElicitation { args, .. } => Some(args.notice.session_id.clone()),
            Self::GetAccessToken { .. } | Self::OpenExternalUrl { .. } | Self::KiroFs { .. } => {
                None
            }
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
            Self::HooksExecute { args, .. } => {
                args.session_id.as_ref().map(|s| SessionId::new(s.clone()))
            }
            Self::HooksList { .. }
            | Self::HooksSessionStart { .. }
            | Self::HooksCancel { .. }
            | Self::HooksDidChange { .. } => None,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::GetAccessToken { .. } => "auth/getAccessToken",
            Self::OpenExternalUrl { .. } => "_kiro/openExternalUrl",
            Self::McpElicitation { .. } => "_kiro/mcp/elicitation",
            Self::ReadTextFile { .. } => "fs/read_text_file",
            Self::WriteTextFile { .. } => "fs/write_text_file",
            Self::KiroFs { args, .. } => args.op().wire,
            Self::CreateTerminal { .. } => "terminal/create",
            Self::WaitForTerminalExit { .. } => "terminal/wait_for_exit",
            Self::TerminalOutput { .. } => "terminal/output",
            Self::ReleaseTerminal { .. } => "terminal/release",
            Self::KillTerminal { .. } => "terminal/kill",
            Self::ShellType { .. } => "_kiro/terminal/shell_type",
            Self::HooksList { .. } => "_kiro/hooks/list",
            Self::HooksExecute { .. } => "_kiro/hooks/executeHook",
            Self::HooksSessionStart { .. } => "_kiro/hooks/sessionStart",
            Self::HooksCancel { .. } => "_kiro/hooks/cancel",
            Self::HooksDidChange { .. } => "_kiro/hooks/didChange",
        }
    }
}

/// Loop-side context the dispatch resolves against. Grows one field per
/// adapter family as its cutover slice lands; the mediator never sees it
/// (design C12).
pub(crate) struct DispatchCtx {
    /// The serial domain mediator ingress — [`finish`]'s ordering channel for
    /// user-visible callback failures.
    pub(crate) notify_tx: crate::protocol::domain_mediator::DomainChannels,
    /// The terminal registry (KAS-5b), loop-side since cyril-g9vt slice 5 —
    /// constructed in `run_bridge` and owned by the dispatch context, deleting
    /// the cyril-3lh8 escape (the `Rc` formerly grabbed out of `KiroClient`).
    /// Still the sole owner of process lifecycle.
    pub(crate) terminals: std::rc::Rc<crate::protocol::kas::terminal_io::TerminalRegistry>,
    /// The hooks registry (KAS-7), loop-side since cyril-g9vt slice 6 —
    /// `Some` iff the bound engine's Hooks direction is Inbound. Presence is
    /// the inbound-serving gate the client checks before sending (dn91 C10);
    /// a callback that reaches dispatch has already passed it.
    pub(crate) hooks: Option<std::rc::Rc<crate::protocol::kas::hooks::HookRegistry>>,
    /// In-flight hook ops for CANCEL (cyril-jiyn) — the mechanism the CANCEL
    /// control signals; executeHook stays non-mediator-cancellable.
    pub(crate) hook_ops: crate::protocol::kas::hooks::HookOps,
    /// Session workspace, the cwd for hook command execution.
    pub(crate) cwd: std::path::PathBuf,
    /// The kiro-cli the `getAccessToken` responder asks (cyril-zbu7) — `Some`
    /// on every KAS spawn, `None` for engines that never serve auth.
    pub(crate) kas_token: Option<std::rc::Rc<crate::protocol::kas::auth::KasTokenSource>>,
}

/// Enqueue `notify` (if any) BEFORE sending the typed reply — the ADR-0004
/// failure-ordering `finish()` owns the order; this is the shared tail every
/// request-kind arm ends with. A dead responder (the acp task already gone) is
/// logged under the callback's `kind`, never a panic (design C7).
async fn resolve<T>(
    ctx: &DispatchCtx,
    kind: &'static str,
    notify: Option<crate::types::Notification>,
    reply: Reply<T>,
    result: acp::Result<T>,
) {
    finish(&ctx.notify_tx, notify, move || {
        if reply.send(result).is_err() {
            tracing::debug!(kind, "callback reply dropped (responder gone)");
        }
    })
    .await;
}

/// Resolve one accepted callback against the adapter-side responders. The
/// match is exhaustive over the CURRENT variant set — a family cannot reach
/// the channel before its cutover slice constructs it. Request-kind arms end
/// in [`resolve`]; the two control arms (cancel, didChange) have no reply.
pub(crate) async fn dispatch(cb: HostCallback, ctx: &DispatchCtx) {
    let kind = cb.kind();
    match cb {
        HostCallback::McpElicitation { args, reply } => {
            let result = super::mcp::decline(args, &ctx.notify_tx).await;
            resolve(ctx, kind, None, reply, result).await;
        }
        HostCallback::OpenExternalUrl { args, reply } => {
            // Acknowledgement means only bounded-path delivery acceptance.
            // A closed domain channel must never look like successful handling.
            let result = ctx
                .notify_tx
                .enqueue(crate::protocol::domain_mediator::DomainWork::Routed(
                    crate::types::Notification::ExternalUrlRequested { url: args.url }.into(),
                ))
                .await
                .and_then(|()| super::json_ext_response(&serde_json::json!({"success": true})));
            resolve(ctx, kind, None, reply, result).await;
        }
        HostCallback::CreateTerminal { req, reply } => {
            let r = ctx.terminals.create(&req);
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::WaitForTerminalExit { req, reply } => {
            let r = ctx.terminals.wait(&req).await;
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::TerminalOutput { req, reply } => {
            let r = ctx.terminals.output(&req);
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::ReleaseTerminal { req, reply } => {
            let r = ctx.terminals.release(&req).await;
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::KillTerminal { req, reply } => {
            let r = ctx.terminals.kill(&req).await;
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::ShellType { reply, .. } => {
            let r = ctx.terminals.respond_shell_type();
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::HooksList { args, reply } => {
            // Presence checked at the client before sending; a None here would
            // be a caller bug, so refuse rather than silently serve nothing.
            let r = match &ctx.hooks {
                Some(h) => h.respond_list(&args),
                None => Err(acp::Error::method_not_found()),
            };
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::HooksExecute { args, reply } => {
            let r = match serde_json::to_value(&args) {
                Ok(params) => {
                    crate::protocol::kas::hooks::respond_execute(&params, &ctx.cwd, &ctx.hook_ops)
                        .await
                }
                Err(e) => Err(acp::Error::new(
                    -32603,
                    format!("re-serialize executeHook params: {e}"),
                )),
            };
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::HooksSessionStart { reply } => {
            let r = match &ctx.hooks {
                Some(h) => crate::protocol::kas::hooks::respond_session_start(h, &ctx.cwd).await,
                None => Err(acp::Error::method_not_found()),
            };
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::ReadTextFile { req, reply } => {
            let r = crate::protocol::kas::host_io::read_text_file(&req).await;
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::WriteTextFile { req, reply } => {
            let r = crate::protocol::kas::host_io::write_text_file(&req).await;
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::KiroFs { args, reply } => {
            let op = args.op();
            let r = match args.to_params() {
                Ok(params) => crate::protocol::kas::kiro_fs::dispatch(op, &params).await,
                Err(e) => Err(acp::Error::new(
                    -32603,
                    format!("re-serialize {} params: {e}", op.wire),
                )),
            };
            resolve(ctx, kind, None, reply, r).await;
        }
        HostCallback::GetAccessToken {
            force_refresh,
            reply,
        } => {
            let outcome = crate::protocol::kas::auth::respond_get_access_token(
                ctx.kas_token.as_deref(),
                force_refresh,
            )
            .await;
            let (notify, r) = match outcome {
                Ok((response, warning)) => (
                    warning.map(|message| crate::types::Notification::SystemNotify {
                        level: crate::types::event::SystemNotifyLevel::Warning,
                        message,
                    }),
                    Ok(response),
                ),
                Err((acp_error, reason)) => {
                    (Some(auth_failure_notification(&reason)), Err(acp_error))
                }
            };
            resolve(ctx, kind, notify, reply, r).await;
        }
        // Control notifications — no reply channel.
        HostCallback::HooksCancel { operation_id } => {
            ctx.hook_ops.cancel(&operation_id);
        }
        HostCallback::HooksDidChange { hooks } => match hooks {
            Some(hooks) => {
                tracing::debug!(count = hooks.len(), "KAS hook registry changed");
                if ctx
                    .notify_tx
                    .enqueue(crate::protocol::domain_mediator::DomainWork::Routed(
                        crate::types::Notification::HooksChanged { hooks }.into(),
                    ))
                    .await
                    .is_err()
                {
                    tracing::debug!("HooksChanged send failed (bridge closing)");
                }
            }
            None => tracing::info!(
                "KAS hooks changed on disk; host-registry reload deferred (cyril-2adk)"
            ),
        },
    }
}

/// The user-visible surface of a failed auth callback (cyril-l7tw C11, moved
/// here from `KiroClient::notify_if_auth_failure` by the mediation cutover):
/// the typed reason's own actionable message (cyril-zbu7 — each
/// [`KasTokenError`](crate::protocol::kas::auth::KasTokenError) variant
/// carries its remediation, so a missing kiro-cli is not told to log in). A
/// refusal (`MethodNotFound`, dn91 C13) never reaches here: refusals are
/// parse-time, and the responder's error type cannot express one.
fn auth_failure_notification(
    reason: &crate::protocol::kas::auth::KasTokenError,
) -> crate::types::Notification {
    crate::types::Notification::BridgeError {
        operation: "auth".into(),
        message: reason.to_string(),
    }
}

/// The full census of wire methods covered by the callback seam: the original
/// 19-method dn91 census plus the supported external-URL callback.
#[cfg(test)]
pub(crate) const WIRE_METHODS: &[&str] = &[
    "auth/getAccessToken",
    "_kiro/openExternalUrl",
    "_kiro/mcp/elicitation",
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

    // The original dn91 census plus the real external-URL callback.
    #[test]
    fn census_includes_the_external_url_callback() {
        assert_eq!(WIRE_METHODS.len(), 21);
        let mut sorted: Vec<_> = WIRE_METHODS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 21, "census entries are distinct");
    }

    // The reply channel rides the envelope to the resolution task and back —
    // the typed round-trip the dispatch depends on.
    #[tokio::test]
    async fn envelope_reply_channel_round_trips() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cb = HostCallback::GetAccessToken {
            force_refresh: false,
            reply: tx,
        };
        assert_eq!(cb.kind(), "auth/getAccessToken");
        assert!(WIRE_METHODS.contains(&cb.kind()));
        assert_eq!(cb.scope(), None);
        match cb {
            HostCallback::GetAccessToken { reply, .. } => {
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
        HostCallback::GetAccessToken {
            force_refresh: false,
            reply: tx,
        }
        .resolve_err(acp::Error::new(-32603, "shutdown"));
        let err = rx.await.expect("delivered").expect_err("error reply");
        assert_eq!(err.message, "shutdown");

        let (tx, rx) = tokio::sync::oneshot::channel::<acp::Result<acp::ExtResponse>>();
        drop(rx);
        HostCallback::GetAccessToken {
            force_refresh: false,
            reply: tx,
        }
        .resolve_err(acp::Error::new(-32603, "shutdown"));
    }

    /// The `BridgeError("auth")` message a failed auth callback surfaces.
    fn surfaced(reason: &crate::protocol::kas::auth::KasTokenError) -> String {
        match auth_failure_notification(reason) {
            crate::types::Notification::BridgeError { operation, message } => {
                assert_eq!(operation, "auth");
                message
            }
            other => panic!("expected BridgeError, got {other:?}"),
        }
    }

    // cyril-l7tw C11 + cyril-zbu7 C2: a failed auth callback surfaces as
    // BridgeError("auth") carrying the typed reason's OWN remediation — the
    // login hint exactly once for kiro-cli's error envelope, and never for a
    // failure re-login cannot fix (kiro-cli absent / wedged).
    #[test]
    fn auth_failure_notification_carries_the_typed_remediation() {
        use crate::protocol::kas::auth::KasTokenError;
        let spawn = surfaced(&KasTokenError::Spawn {
            command: "kiro-cli chat _ get-kas-token".into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
            remedy: "install kiro-cli on PATH",
        });
        assert!(
            spawn.contains("install kiro-cli on PATH, then retry"),
            "{spawn}"
        );
        assert!(!spawn.contains("kiro-cli login"), "misdirects: {spawn}");
        let timeout = surfaced(&KasTokenError::Timeout {
            command: "kiro-cli chat _ get-kas-token".into(),
            secs: 30,
        });
        assert!(timeout.contains("did not answer within 30 s"), "{timeout}");
        assert!(
            timeout.contains("refresh lock timed out"),
            "names the peer-lock cause: {timeout}"
        );
        assert!(!timeout.contains("kiro-cli login"), "misdirects: {timeout}");
        let logged_out = surfaced(&KasTokenError::ErrorEnvelope {
            message: "You are not logged in. Please log in with `kiro-cli login`.".into(),
            hint: String::new(),
        });
        assert_eq!(
            logged_out.matches("kiro-cli login").count(),
            1,
            "{logged_out}"
        );
    }
}

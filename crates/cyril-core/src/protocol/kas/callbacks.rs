//! Typed host callbacks (cyril-g9vt, ADR-0004 amendment): the exhaustive
//! internal form every handled host-callback request and control notification
//! takes before crossing the bridge's host-callback mediation seam. No raw
//! JSON and no opaque closures cross — `KiroClient` parses HERE, the mediator
//! handles lifecycle, and the dispatch side resolves against the
//! Engine-selected adapter set (ADR-0001).
//!
//! Variant census: the enum's variants expand to the 19 handled wire methods
//! (`.cyril-dn91/findings.md`): auth ×1, bare-ACP fs ×2, `_kiro/fs/*` ×5 (one
//! variant × [`KiroFsArgs`]), terminal ×5 + shell_type, hooks requests ×3,
//! hooks controls ×2 — fenced by [`tests::census_matches_the_dn91_count`].

use agent_client_protocol as acp;

use crate::protocol::host_mediator::{CallbackMeta, CancelKey};
use crate::protocol::kas::kiro_fs;
use crate::types::{HookInfo, SessionId};

/// The typed reply channel for one request-kind callback: the acp-side
/// request task awaits this and converts the typed outcome to the exact ACP
/// response/error.
pub(crate) type Reply<T> = tokio::sync::oneshot::Sender<acp::Result<T>>;

/// The `hooks/executeHook` cancel-key kind — one owner for the string both
/// the execute registration and the cancel control derive their key from, so
/// they can never drift apart.
const EXECUTE_KIND: &str = "hooks/executeHook";

/// One handled host callback, parsed and typed. Request variants carry their
/// typed reply channel; control variants carry none.
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
    /// `_kiro/hooks/list {trigger, toolId?}`. A missing trigger is preserved
    /// as `None` — the responder's reply-empty behavior (not an error) is the
    /// dispatch side's to keep.
    HooksList {
        trigger: Option<String>,
        tool_id: Option<String>,
        reply: Reply<acp::ExtResponse>,
    },
    /// `_kiro/hooks/executeHook` (cancellable by `operation_id`).
    HooksExecute {
        args: HooksExecuteArgs,
        reply: Reply<acp::ExtResponse>,
    },
    /// `_kiro/hooks/sessionStart`.
    HooksSessionStart { reply: Reply<acp::ExtResponse> },
    /// CONTROL `_kiro/hooks/cancel {operationId}` — aborts an accepted
    /// execute; consumed by the mediator, never dispatched.
    HooksCancel { operation_id: String },
    /// CONTROL `_kiro/hooks/didChange` — under `kas` hook generation carries
    /// the agent's full new registry (parsed, cyril-gk17); under `host` the
    /// payload has no hooks array (`None`).
    HooksDidChange { hooks: Option<Vec<HookInfo>> },
}

/// `_kiro/fs/*` per-op typed params, reusing the structs `kiro_fs` already
/// deserializes with (single owner of the field knowledge).
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

    fn wire(&self) -> &'static str {
        match self {
            Self::ReadFile(_) => "_kiro/fs/read_file",
            Self::WriteFile(_) => "_kiro/fs/write_file",
            Self::Stat(_) => "_kiro/fs/stat",
            Self::ReadDirectory(_) => "_kiro/fs/read_directory",
            Self::Delete(_) => "_kiro/fs/delete",
        }
    }
}

/// `executeHook` typed params. Semantic defaults (missing command → exit 127
/// reply, missing userPrompt → empty with a warn) stay with the dispatch side
/// to preserve today's behavior and log lines — parsing only records absence.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HooksExecuteArgs {
    pub(crate) command: Option<String>,
    pub(crate) user_prompt: Option<String>,
    /// Wire seconds (the 2.13.0 bundle carve — NOT millis).
    pub(crate) timeout: Option<u64>,
    pub(crate) operation_id: Option<String>,
    pub(crate) session_id: Option<String>,
}

impl HostCallback {
    /// Resolve this callback's reply channel with `err` — the terminal path
    /// for aborted, shut-down, or dispatch-refused work. Consuming `self`
    /// guarantees a request-kind callback cannot outlive its resolution;
    /// control variants carry no responder and drop silently. A dead receiver
    /// (the acp-side task already gone) is logged, never a panic (design C7).
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
            Self::HooksList { reply, .. } => send(kind, reply, err),
            Self::HooksExecute { reply, .. } => send(kind, reply, err),
            Self::HooksSessionStart { reply } => send(kind, reply, err),
            Self::HooksCancel { .. } | Self::HooksDidChange { .. } => {}
        }
    }
}

impl CallbackMeta for HostCallback {
    fn cancels(&self) -> Option<CancelKey> {
        match self {
            Self::HooksCancel { operation_id } => {
                Some(CancelKey::new(EXECUTE_KIND, operation_id.clone()))
            }
            _ => None,
        }
    }

    fn cancel_key(&self) -> Option<CancelKey> {
        match self {
            Self::HooksExecute { args, .. } => args
                .operation_id
                .as_ref()
                .map(|id| CancelKey::new(EXECUTE_KIND, id.clone())),
            _ => None,
        }
    }

    fn scope(&self) -> Option<SessionId> {
        let sid = |s: &acp::SessionId| SessionId::new(s.to_string());
        match self {
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
            Self::GetAccessToken { .. }
            | Self::KiroFs { .. }
            | Self::HooksList { .. }
            | Self::HooksSessionStart { .. }
            | Self::HooksCancel { .. }
            | Self::HooksDidChange { .. } => None,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::GetAccessToken { .. } => "auth/getAccessToken",
            Self::ReadTextFile { .. } => "fs/read_text_file",
            Self::WriteTextFile { .. } => "fs/write_text_file",
            Self::KiroFs { args, .. } => args.wire(),
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

/// The full census of wire methods the callback seam covers — 19, matching
/// the dn91 probe census. The census fence walks this table; slice 8's
/// end-to-end census drives each through the client entry points.
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

    fn reply<T>() -> Reply<T> {
        tokio::sync::oneshot::channel().0
    }

    // The reply channel rides the envelope to the resolution task and back —
    // the typed round-trip the slice-3 dispatch depends on.
    #[tokio::test]
    async fn envelope_reply_channel_round_trips() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let cb = HostCallback::GetAccessToken { reply: tx };
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

    // The list/didChange payload fields ride the envelope intact — their
    // dispatch-side consumers (slice 6) receive exactly what was parsed.
    #[test]
    fn hooks_payload_fields_round_trip() {
        let cb = HostCallback::HooksList {
            trigger: Some("promptSubmit".into()),
            tool_id: Some("fs_write".into()),
            reply: reply(),
        };
        match cb {
            HostCallback::HooksList {
                trigger, tool_id, ..
            } => {
                assert_eq!(trigger.as_deref(), Some("promptSubmit"));
                assert_eq!(tool_id.as_deref(), Some("fs_write"));
            }
            other => panic!("wrong variant: {other:?}"),
        }
        match (HostCallback::HooksDidChange { hooks: None }) {
            HostCallback::HooksDidChange { hooks } => assert!(hooks.is_none()),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    // resolve_err is the terminal path for abort/shutdown/refusal: the typed
    // error reaches the awaiting receiver; a dead receiver doesn't panic.
    #[tokio::test]
    async fn resolve_err_reaches_receiver_and_survives_drop() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        HostCallback::HooksSessionStart { reply: tx }
            .resolve_err(acp::Error::new(-32603, "shutdown"));
        let err = rx.await.expect("delivered").expect_err("error reply");
        assert_eq!(err.message, "shutdown");

        let (tx, rx) = tokio::sync::oneshot::channel::<acp::Result<acp::ExtResponse>>();
        drop(rx);
        HostCallback::GetAccessToken { reply: tx }.resolve_err(acp::Error::new(-32603, "shutdown")); // must not panic
    }

    // Every variant constructs, self-identifies inside the census, and
    // resolves through the terminal path — the unit precursor of slice 8's
    // end-to-end census walk. A variant whose kind() string drifts out of
    // WIRE_METHODS fails here by name.
    #[test]
    fn every_variant_constructs_and_self_identifies() {
        let sid = || acp::SessionId::new("s");
        let tid = || acp::TerminalId::new("t");
        let stat_op = kiro_fs::op_for_method("kiro/fs/stat").unwrap();
        let exec_args = |v: serde_json::Value| serde_json::from_value(v).unwrap();
        let all: Vec<HostCallback> = vec![
            HostCallback::GetAccessToken { reply: reply() },
            HostCallback::ReadTextFile {
                req: acp::ReadTextFileRequest::new(sid(), "/t"),
                reply: reply(),
            },
            HostCallback::WriteTextFile {
                req: acp::WriteTextFileRequest::new(sid(), "/t", "x"),
                reply: reply(),
            },
            HostCallback::KiroFs {
                args: KiroFsArgs::parse(
                    stat_op,
                    &serde_json::json!({"sessionId": "s", "path": "/t"}),
                )
                .unwrap(),
                reply: reply(),
            },
            HostCallback::CreateTerminal {
                req: acp::CreateTerminalRequest::new(sid(), "true"),
                reply: reply(),
            },
            HostCallback::WaitForTerminalExit {
                req: acp::WaitForTerminalExitRequest::new(sid(), tid()),
                reply: reply(),
            },
            HostCallback::TerminalOutput {
                req: acp::TerminalOutputRequest::new(sid(), tid()),
                reply: reply(),
            },
            HostCallback::ReleaseTerminal {
                req: acp::ReleaseTerminalRequest::new(sid(), tid()),
                reply: reply(),
            },
            HostCallback::KillTerminal {
                req: acp::KillTerminalRequest::new(sid(), tid()),
                reply: reply(),
            },
            HostCallback::ShellType {
                session_id: Some("s".into()),
                reply: reply(),
            },
            HostCallback::HooksList {
                trigger: Some("promptSubmit".into()),
                tool_id: None,
                reply: reply(),
            },
            HostCallback::HooksExecute {
                args: exec_args(serde_json::json!({"command": "true", "operationId": "o"})),
                reply: reply(),
            },
            HostCallback::HooksSessionStart { reply: reply() },
            HostCallback::HooksCancel {
                operation_id: "o".into(),
            },
            HostCallback::HooksDidChange { hooks: None },
        ];
        assert_eq!(all.len(), 15, "15 variants expand to the 19 wire methods");
        for cb in all {
            assert!(
                WIRE_METHODS.contains(&cb.kind()),
                "{} missing from the census",
                cb.kind()
            );
            cb.resolve_err(acp::Error::new(-32603, "census"));
        }
    }

    // The census: 19 wire methods, exactly the dn91 probe count (the
    // independent oracle for this enum's completeness).
    #[test]
    fn census_matches_the_dn91_count() {
        assert_eq!(WIRE_METHODS.len(), 19);
        let mut sorted: Vec<_> = WIRE_METHODS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 19, "census entries are distinct");
    }

    // Malformed params for a HANDLED family are a typed parse error — never
    // the old Null-tolerant fallback (the slice-2 stress bug class).
    #[test]
    fn malformed_kiro_fs_params_error_typed() {
        let op = kiro_fs::op_for_method("kiro/fs/stat").expect("stat op");
        let err = KiroFsArgs::parse(op, &serde_json::json!({"path": 42}))
            .expect_err("non-string path must fail parse");
        assert!(
            err.contains("_kiro/fs/stat"),
            "diagnostic names the op: {err}"
        );

        let ok = KiroFsArgs::parse(op, &serde_json::json!({"sessionId": "s", "path": "/tmp/x"}));
        assert!(ok.is_ok(), "well-formed stat parses");
    }

    // Field preservation: read_file line/limit survive the typed round-trip
    // (the dropped-optional-field bug class), and absence stays None.
    #[test]
    fn read_file_optionals_preserved() {
        let op = kiro_fs::op_for_method("kiro/fs/read_file").expect("read op");
        let args = KiroFsArgs::parse(
            op,
            &serde_json::json!({"sessionId": "s", "path": "/tmp/x", "line": 7, "limit": 50}),
        )
        .unwrap();
        match args {
            KiroFsArgs::ReadFile(p) => {
                assert_eq!(p.line, Some(7));
                assert_eq!(p.limit, Some(50));
            }
            other => panic!("wrong op: {other:?}"),
        }
        let bare =
            KiroFsArgs::parse(op, &serde_json::json!({"sessionId": "s", "path": "/t"})).unwrap();
        match bare {
            KiroFsArgs::ReadFile(p) => {
                assert_eq!(p.line, None, "absent stays None, never 0");
                assert_eq!(p.limit, None);
            }
            other => panic!("wrong op: {other:?}"),
        }

        // The other ops' payloads ride intact too (slice-4 dispatch consumes
        // them): write carries content; the path-shaped ops carry the path.
        let wr = kiro_fs::op_for_method("kiro/fs/write_file").unwrap();
        match KiroFsArgs::parse(
            wr,
            &serde_json::json!({"sessionId": "s", "path": "/t", "content": "body"}),
        )
        .unwrap()
        {
            KiroFsArgs::WriteFile(p) => assert_eq!(p.content, "body"),
            other => panic!("wrong op: {other:?}"),
        }
        for (method, want) in [
            ("kiro/fs/stat", "/a"),
            ("kiro/fs/read_directory", "/b"),
            ("kiro/fs/delete", "/c"),
        ] {
            let op = kiro_fs::op_for_method(method).unwrap();
            let parsed =
                KiroFsArgs::parse(op, &serde_json::json!({"sessionId": "s", "path": want}))
                    .unwrap();
            match parsed {
                KiroFsArgs::Stat(p) | KiroFsArgs::ReadDirectory(p) | KiroFsArgs::Delete(p) => {
                    assert_eq!(p.path, std::path::PathBuf::from(want));
                }
                other => panic!("wrong op for {method}: {other:?}"),
            }
        }
    }

    // Meta contract: execute registers under the operation id, cancel targets
    // the SAME key (single-owner kind string), and an id-less execute is
    // simply not cancellable.
    #[test]
    fn execute_and_cancel_share_the_key() {
        let exec = HostCallback::HooksExecute {
            args: serde_json::from_value(serde_json::json!({
                "command": "true", "operationId": "op-9", "sessionId": "s"
            }))
            .unwrap(),
            reply: reply(),
        };
        let cancel = HostCallback::HooksCancel {
            operation_id: "op-9".into(),
        };
        assert_eq!(exec.cancel_key(), cancel.cancels());
        assert!(exec.cancels().is_none(), "execute is work, not a control");
        assert!(
            cancel.cancel_key().is_none(),
            "cancel is not itself cancellable"
        );

        let anon = HostCallback::HooksExecute {
            args: serde_json::from_value(serde_json::json!({"command": "true"})).unwrap(),
            reply: reply(),
        };
        assert!(anon.cancel_key().is_none());
        assert_eq!(anon.scope(), None);

        // The execution payload rides intact for the slice-6 dispatch: wire
        // seconds stay seconds, absence stays None (never a default).
        let full: HooksExecuteArgs = serde_json::from_value(serde_json::json!({
            "command": "sleep 1", "userPrompt": "p", "timeout": 30, "operationId": "o"
        }))
        .unwrap();
        assert_eq!(full.command.as_deref(), Some("sleep 1"));
        assert_eq!(full.user_prompt.as_deref(), Some("p"));
        assert_eq!(full.timeout, Some(30));
        let sparse: HooksExecuteArgs = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(
            sparse.command, None,
            "missing command recorded, not defaulted"
        );
        assert_eq!(sparse.user_prompt, None);
        assert_eq!(sparse.timeout, None);
    }

    // Scope extraction: acp-typed requests carry their session; the kinds
    // table names every variant distinctly (census localization).
    #[test]
    fn scope_and_kind_extraction() {
        let cb = HostCallback::ReadTextFile {
            req: acp::ReadTextFileRequest::new(acp::SessionId::new("sess-1"), "/tmp/f"),
            reply: reply(),
        };
        assert_eq!(cb.scope(), Some(SessionId::new("sess-1")));
        assert_eq!(cb.kind(), "fs/read_text_file");

        let op = kiro_fs::op_for_method("kiro/fs/delete").unwrap();
        let cb = HostCallback::KiroFs {
            args: KiroFsArgs::parse(op, &serde_json::json!({"sessionId": "s", "path": "/t"}))
                .unwrap(),
            reply: reply(),
        };
        assert_eq!(cb.kind(), "_kiro/fs/delete");
        assert!(WIRE_METHODS.contains(&cb.kind()));
    }
}

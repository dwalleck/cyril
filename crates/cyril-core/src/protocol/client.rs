use std::cell::RefCell;
use std::collections::HashMap;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::protocol::convert;
use crate::types::*;

/// The central ACP Client implementation for the bridge thread.
///
/// Lives in the `!Send` bridge thread and uses `RefCell<HashMap>` for
/// caching tool call `raw_input`. Permission requests arrive without
/// `raw_input`, so the client looks it up from this cache.
pub(crate) struct KiroClient {
    notification_tx: mpsc::Sender<RoutedNotification>,
    permission_tx: mpsc::Sender<PermissionRequest>,
    tool_call_inputs: RefCell<HashMap<String, serde_json::Value>>,
    /// The bound engine (ADR-0001): all wire→internal conversion dispatches
    /// through it, so v2 and KAS share this client unchanged.
    engine: std::rc::Rc<dyn crate::protocol::engine::Engine>,
    /// KAS-5b (cyril-ufie): live `terminal/*` host-callback registry. KAS-only —
    /// v2 advertises no `terminal` capability, so the overrides never fire there.
    /// `Rc` so the bridge loop shares the SAME registry (same `LocalSet` thread)
    /// and its CancelRequest arm can reap a cancelled session's live terminals
    /// (cyril-3lh8); the registry stays the sole owner of process lifecycle.
    #[cfg(feature = "kas")]
    terminals: std::rc::Rc<crate::protocol::kas::terminal_io::TerminalRegistry>,
}

impl KiroClient {
    pub fn new(
        notification_tx: mpsc::Sender<RoutedNotification>,
        permission_tx: mpsc::Sender<PermissionRequest>,
        engine: std::rc::Rc<dyn crate::protocol::engine::Engine>,
    ) -> Self {
        Self {
            notification_tx,
            permission_tx,
            tool_call_inputs: RefCell::new(HashMap::new()),
            engine,
            #[cfg(feature = "kas")]
            terminals: std::rc::Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new()),
        }
    }

    /// cyril-3lh8: hand the bridge loop a shared handle to the terminal
    /// registry, grabbed BEFORE the ACP connection takes ownership of the
    /// client. The loop only triggers `reap_session` from its CancelRequest
    /// arm — the registry remains the sole owner of process lifecycle.
    #[cfg(feature = "kas")]
    pub(crate) fn terminals(
        &self,
    ) -> std::rc::Rc<crate::protocol::kas::terminal_io::TerminalRegistry> {
        std::rc::Rc::clone(&self.terminals)
    }
}

#[async_trait(?Send)]
impl acp::Client for KiroClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let tool_call =
            convert::to_tool_call_from_permission(&args, &self.tool_call_inputs.borrow());
        let options = convert::to_permission_options(&args);
        let message = convert::extract_permission_message(&args);
        let trust_options = convert::extract_trust_options(&args);

        let (responder_tx, responder_rx) = tokio::sync::oneshot::channel();

        let request = PermissionRequest {
            tool_call,
            message,
            options,
            trust_options,
            responder: responder_tx,
        };

        self.permission_tx
            .send(request)
            .await
            .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;

        let response = responder_rx
            .await
            .map_err(|_| acp::Error::new(-32603, "permission response dropped"))?;

        Ok(convert::from_permission_response(response, &args))
    }

    async fn session_notification(&self, args: acp::SessionNotification) -> acp::Result<()> {
        // Log tool call details for debugging content/locations/diff availability
        match &args.update {
            acp::SessionUpdate::ToolCall(tc) => {
                tracing::info!(
                    id = %tc.tool_call_id,
                    title = %tc.title,
                    kind = ?tc.kind,
                    status = ?tc.status,
                    content_count = tc.content.len(),
                    locations_count = tc.locations.len(),
                    has_raw_input = tc.raw_input.is_some(),
                    "ToolCall notification"
                );
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                tracing::info!(
                    id = %update.tool_call_id,
                    title = ?update.fields.title,
                    kind = ?update.fields.kind,
                    status = ?update.fields.status,
                    has_raw_input = update.fields.raw_input.is_some(),
                    "ToolCallUpdate notification"
                );
            }
            _ => {}
        }

        convert::cache_tool_call_input(&args, &self.tool_call_inputs);

        let notification = {
            let inputs = self.tool_call_inputs.borrow();
            self.engine.convert_session_update(&args, &inputs)
        };
        if let Some(notification) = notification {
            // Every session notification carries the session_id from the
            // envelope. The App routes based on whether this matches the main
            // session or a known subagent.
            let session_id = SessionId::new(args.session_id.to_string());
            let routed = RoutedNotification::scoped(session_id, notification);
            self.notification_tx
                .send(routed)
                .await
                .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
        }

        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        let params: serde_json::Value = match serde_json::from_str(args.params.get()) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    error = %e,
                    method = %args.method,
                    "failed to parse ext_notification params"
                );
                serde_json::Value::Null
            }
        };

        match self
            .engine
            .convert_ext_notification(args.method.as_ref(), &params)
        {
            Ok(Some(notification)) => {
                // ToolCallChunk carries an inline session_id from the outer
                // kiro.dev/session/update envelope, MetadataUpdated from the
                // params-level sessionId on kiro.dev/metadata (cyril-fh06).
                // Promote both to channel-level RoutedNotification routing so
                // the App can divert subagent-session frames away from the
                // main pipeline.
                let routed = match &notification {
                    Notification::ToolCallChunk {
                        session_id: Some(sid),
                        ..
                    }
                    | Notification::MetadataUpdated {
                        session_id: Some(sid),
                        ..
                    } => RoutedNotification::scoped(sid.clone(), notification),
                    _ => RoutedNotification::global(notification),
                };
                self.notification_tx
                    .send(routed)
                    .await
                    .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
            }
            // Known-but-not-forwarded (multi-session), unknown, or
            // malformed-but-suppressed (e.g. oauth_request missing URL).
            // Individual handlers log warnings for the malformed cases.
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    method = %args.method,
                    "malformed extension notification"
                );
            }
        }
        Ok(())
    }

    /// Handle incoming server→client ext REQUESTS. KAS-1/dcc6 answers
    /// `_kiro/auth/getAccessToken` (both KAS spawn modes run
    /// `--auth=acp-callback`) from kiro-cli's sqlite credential store; every
    /// other ext request gets the protocol default. v2 never sends this, and
    /// the cfg-split keeps the credential code out of a default build
    /// (ADR-0002). cyril-l7tw C11: an auth-callback failure ALSO surfaces to
    /// the App as a BridgeError — the JSON-RPC error alone travels to KAS,
    /// which fails the turn while the user sees nothing actionable.
    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        let method = args.method.to_string();
        let result = Self::handle_ext_request(args).await;
        self.notify_if_auth_failure(&method, &result).await;
        result
    }

    /// KAS-5a (cyril-7bdu): answer `fs/read_text_file` by reading the file via the
    /// async host-io resolver. Only present under `kas` — v2 advertises no fs caps
    /// (KasEngine, Slice 1), so a v2 agent never calls this. Resolution runs in the
    /// acp connection's per-request `spawn_local` task (`rpc.rs:272`), off the
    /// bridge loop and non-blocking via async `tokio::fs` (ADR-0004 invariant). The
    /// loop-mediation gate seam is deferred to its first consumer (cyril-g9vt).
    #[cfg(feature = "kas")]
    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        crate::protocol::kas::host_io::read_text_file(&args).await
    }

    /// KAS-5a (cyril-7bdu): answer `fs/write_text_file` via the async host-io
    /// resolver (`mkdir -p` + write). KAS-only, same non-blocking rationale as
    /// `read_text_file` above; KAS sends a separate `session/request_permission`
    /// for the write, handled by the existing approval path.
    #[cfg(feature = "kas")]
    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        crate::protocol::kas::host_io::write_text_file(&args).await
    }

    /// KAS-5b (cyril-ufie): answer `terminal/create` by spawning the command in the
    /// terminal registry. Returns the id immediately (non-blocking). KAS-only.
    #[cfg(feature = "kas")]
    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        self.terminals.create(&args)
    }

    /// KAS-5b: answer `terminal/wait_for_exit` by awaiting the command via
    /// `tokio::process` (never `std::process` — single-threaded bridge). Reply is
    /// flat `{exitCode, signal}` (the prove-it finding).
    #[cfg(feature = "kas")]
    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        self.terminals.wait(&args).await
    }

    /// KAS-5b: answer `terminal/output` with a non-blocking snapshot of the
    /// terminal's combined stdout+stderr and exit status.
    #[cfg(feature = "kas")]
    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        self.terminals.output(&args)
    }

    /// KAS-5b: answer `terminal/release` — kill + reap the child and free the id.
    #[cfg(feature = "kas")]
    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        self.terminals.release(&args).await
    }

    /// KAS-5b: answer `terminal/kill` — terminate the child but keep the id valid.
    #[cfg(feature = "kas")]
    async fn kill_terminal(
        &self,
        args: acp::KillTerminalRequest,
    ) -> acp::Result<acp::KillTerminalResponse> {
        self.terminals.kill(&args).await
    }
}

impl KiroClient {
    /// cyril-l7tw C11: when the `getAccessToken` responder fails, tell the App
    /// (BridgeError, operation "auth") in addition to the JSON-RPC error that
    /// travels back to KAS. The responder's messages already carry the
    /// actionable `kiro-cli login` hint (kas/auth.rs); the hint is appended
    /// only when absent so it is never doubled. Failures of OTHER ext methods
    /// stay out of scope — their unhandled-default path is not an error.
    #[cfg(feature = "kas")]
    async fn notify_if_auth_failure(&self, method: &str, result: &acp::Result<acp::ExtResponse>) {
        if method != crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD {
            return;
        }
        let Err(e) = result else { return };
        let mut message = e.message.clone();
        // LOGIN_HINT is the single owner of the remediation wording — the
        // responder's own diagnostics embed it, so this check can't drift.
        let hint = crate::protocol::kas::auth::LOGIN_HINT;
        if !message.contains(hint) {
            message.push_str(&format!(" — {hint} and retry"));
        }
        let note = Notification::BridgeError {
            operation: "auth".into(),
            message,
        };
        if self.notification_tx.send(note.into()).await.is_err() {
            tracing::debug!("auth BridgeError send failed (bridge closing)");
        }
    }

    /// Default build: no KAS, no auth callback, nothing to surface.
    #[cfg(not(feature = "kas"))]
    async fn notify_if_auth_failure(&self, _method: &str, _result: &acp::Result<acp::ExtResponse>) {
    }

    // `#[cfg]` blocks (not a `cfg!(...)` runtime branch) are required: the `kas`
    // module — and thus `kas::auth::respond_get_access_token` — does not exist in
    // a default build, so a single body referencing it would fail to compile.
    /// Route an ext request (`_kiro/*`): KAS-1 `getAccessToken`, KAS-5b
    /// `terminal/shell_type`.
    #[cfg(feature = "kas")]
    async fn handle_ext_request(args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        if args.method.as_ref() == crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD {
            return crate::protocol::kas::auth::respond_get_access_token().await;
        }
        if args.method.as_ref() == crate::protocol::kas::terminal_io::SHELL_TYPE_METHOD {
            return crate::protocol::kas::terminal_io::respond_shell_type();
        }
        // The bare-ACP fs/terminal lifecycle host callbacks are TYPED acp::Client
        // methods (the overrides above), not ext requests: fs/read_text_file (KAS-5a,
        // cyril-7bdu) and terminal/{create,output,wait_for_exit,release,kill} (KAS-5b,
        // cyril-ufie). This arm answers only the `_kiro/*`-prefixed ext requests.
        unhandled_ext_response(args.method.as_ref())
    }

    /// Default build: no KAS ext requests are handled.
    #[cfg(not(feature = "kas"))]
    async fn handle_ext_request(args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        unhandled_ext_response(args.method.as_ref())
    }
}

/// The ACP protocol default for an unhandled ext request: a `null` result.
fn default_ext_response() -> acp::Result<acp::ExtResponse> {
    Ok(acp::ExtResponse::new(
        serde_json::value::RawValue::NULL.to_owned().into(),
    ))
}

/// Log an unhandled `_kiro/*` ext request, then answer with the protocol
/// default ([`default_ext_response`]). The breadcrumb is load-bearing: if KAS
/// renames a method (or the acp library's leading-underscore stripping
/// changes), the caller gets a success-shaped null and fails opaquely on its
/// side — this log line is the only cyril-side evidence (dcc6 review F15).
fn unhandled_ext_response(method: &str) -> acp::Result<acp::ExtResponse> {
    tracing::debug!(
        method,
        "unhandled ext request answered with protocol-default null"
    );
    default_ext_response()
}

#[cfg(all(test, feature = "kas"))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use agent_client_protocol::Client as _;

    // cyril-l7tw C11 fence: a failed getAccessToken callback emits
    // BridgeError("auth", <responder message + login hint>) on the internal
    // channel — deterministic (constructed Err, no sqlite store involved;
    // injectable store wiring is cyril-5db7). Catches the pre-l7tw behavior:
    // error swallowed into the JSON-RPC reply alone.
    #[tokio::test]
    async fn auth_callback_err_emits_bridge_error() {
        let (ntx, mut nrx) = mpsc::channel(4);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        );
        let err: acp::Result<acp::ExtResponse> =
            Err(acp::Error::new(-32603, "sqlite store locked"));
        client
            .notify_if_auth_failure(crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD, &err)
            .await;
        let routed = nrx.try_recv().expect("BridgeError emitted");
        match routed.notification {
            Notification::BridgeError { operation, message } => {
                assert_eq!(operation, "auth");
                assert!(message.contains("sqlite store locked"), "got: {message}");
                assert!(
                    message.contains("kiro-cli login"),
                    "actionable hint present, got: {message}"
                );
            }
            other => panic!("expected BridgeError, got {other:?}"),
        }
    }

    // l7tw C11 stress: the responder's own messages already carry the login
    // hint (kas/auth.rs) — it must not be doubled.
    #[tokio::test]
    async fn auth_hint_not_doubled() {
        let (ntx, mut nrx) = mpsc::channel(4);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        );
        let err: acp::Result<acp::ExtResponse> = Err(acp::Error::new(
            -32603,
            "kiro token expired; run `kiro-cli login`",
        ));
        client
            .notify_if_auth_failure(crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD, &err)
            .await;
        let routed = nrx.try_recv().expect("BridgeError emitted");
        match routed.notification {
            Notification::BridgeError { message, .. } => {
                assert_eq!(
                    message.matches("kiro-cli login").count(),
                    1,
                    "hint appears exactly once, got: {message}"
                );
            }
            other => panic!("expected BridgeError, got {other:?}"),
        }
    }

    // l7tw C11 scope check + C12-kas: a NON-auth ext failure emits nothing,
    // and a SUCCESSFUL auth callback emits nothing.
    #[tokio::test]
    async fn non_auth_ext_err_emits_nothing() {
        let (ntx, mut nrx) = mpsc::channel(4);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        );
        let err: acp::Result<acp::ExtResponse> = Err(acp::Error::new(-32603, "boom"));
        client
            .notify_if_auth_failure("kiro/some/other_method", &err)
            .await;
        client
            .notify_if_auth_failure(
                crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
                &default_ext_response(),
            )
            .await;
        assert!(
            nrx.try_recv().is_err(),
            "neither non-auth failures nor auth successes emit BridgeError"
        );
    }

    #[tokio::test]
    async fn read_text_file_override_returns_content() {
        // KAS-5a / claim C2 fence: a KAS `fs/read_text_file` reaches KiroClient's
        // typed override (NOT the acp default `method_not_found`) and returns the
        // file's content end-to-end. Fails if the override is missing/miswired.
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        );
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.txt");
        std::fs::write(&f, "hello").unwrap();
        let resp = client
            .read_text_file(acp::ReadTextFileRequest::new(acp::SessionId::new("s"), &f))
            .await
            .expect("override resolves, not method_not_found");
        assert_eq!(resp.content, "hello");
    }

    #[tokio::test]
    async fn write_text_file_override_writes_file() {
        // KAS-5a / claim C2 fence (write): KAS `fs/write_text_file` reaches the
        // typed override and writes to disk (not method_not_found).
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        );
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("out.txt");
        client
            .write_text_file(acp::WriteTextFileRequest::new(
                acp::SessionId::new("s"),
                &f,
                "written",
            ))
            .await
            .expect("write override resolves");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "written");
    }

    fn kas_client() -> KiroClient {
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
        )
    }

    #[tokio::test]
    async fn create_terminal_override_reaches_registry() {
        // KAS-5b fixture M: a KAS `terminal/create` reaches KiroClient's typed
        // override (NOT the acp default `method_not_found`) and returns an id.
        // Fails if the override is missing/miswired.
        let client = kas_client();
        let resp = client
            .create_terminal(acp::CreateTerminalRequest::new(
                acp::SessionId::new("s"),
                "true",
            ))
            .await
            .expect("create_terminal override resolves, not method_not_found");
        assert_eq!(resp.terminal_id.to_string(), "term-1");
    }

    #[tokio::test]
    async fn shell_type_ext_request_routes() {
        // KAS-5b fixture N: `_kiro/terminal/shell_type` (acp-stripped to
        // `kiro/terminal/shell_type`) routes through ext_method to the responder,
        // returning {shellType}. Fails if the arm matches the un-stripped name -> the
        // default null response.
        let client = kas_client();
        let params: std::sync::Arc<serde_json::value::RawValue> =
            serde_json::value::RawValue::from_string("{\"sessionId\":\"s\"}".to_string())
                .unwrap()
                .into();
        let resp = client
            .ext_method(acp::ExtRequest::new("kiro/terminal/shell_type", params))
            .await
            .expect("shell_type routes");
        assert!(
            resp.0.get().contains("shellType"),
            "ext reply must carry shellType, got {}",
            resp.0.get()
        );
    }
}

#[cfg(test)]
mod metadata_routing_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use agent_client_protocol::Client as _;

    fn ext_frame(method: &str, params: serde_json::Value) -> acp::ExtNotification {
        let raw: std::sync::Arc<serde_json::value::RawValue> =
            serde_json::value::RawValue::from_string(params.to_string())
                .expect("valid JSON")
                .into();
        acp::ExtNotification::new(method, raw)
    }

    fn v2_client(
        ntx: mpsc::Sender<RoutedNotification>,
        ptx: mpsc::Sender<PermissionRequest>,
    ) -> KiroClient {
        KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::V2Engine),
        )
    }

    /// Drain every routed notification currently buffered on the channel.
    fn drain(nrx: &mut mpsc::Receiver<RoutedNotification>) -> Vec<RoutedNotification> {
        let mut out = Vec::new();
        while let Ok(routed) = nrx.try_recv() {
            out.push(routed);
        }
        out
    }

    /// Replay routed notifications into a main `SessionController` following the
    /// App's routing contract (`app.rs::handle_notification`): the main state
    /// machines receive a notification only when the routed `session_id` is
    /// `None` or equals the main session's id.
    fn replay_to_main(
        routed_frames: &[RoutedNotification],
        main_id: &SessionId,
    ) -> crate::session::SessionController {
        let mut session = crate::session::SessionController::new();
        session.apply_notification(&Notification::SessionCreated {
            session_id: main_id.clone(),
            current_mode: None,
            current_model: None,
            available_modes: Vec::new(),
            available_models: Vec::new(),
        });
        for routed in routed_frames {
            let to_main = routed.session_id.as_ref().is_none_or(|sid| sid == main_id);
            if to_main {
                session.apply_notification(&routed.notification);
            }
        }
        session
    }

    // cyril-fh06 fence: during a multi-subagent v2 turn, every session emits its
    // OWN `_kiro.dev/metadata` frame with a params-level `sessionId` (committed
    // capture: experiments/conductor-spike/trace-2.4.1-multi-subagent.jsonl has
    // metadata frames for 5 distinct sessionIds). Kiro's own TUI drops frames
    // whose sessionId differs from the current session; cyril must route them
    // scoped so main-toolbar context/credits/duration/effort come only from
    // main-session frames.
    #[tokio::test]
    async fn subagent_metadata_does_not_stamp_main_session() {
        let (ntx, mut nrx) = mpsc::channel(8);
        let (ptx, _prx) = mpsc::channel(1);
        let client = v2_client(ntx, ptx);

        // Frame shapes mirror the committed 2.4.1 capture (meteringUsage entries
        // carry `unit`/`unitPlural`, which cyril deliberately ignores).
        let main_frame = ext_frame(
            "kiro.dev/metadata",
            serde_json::json!({
                "sessionId": "main-sess",
                "contextUsagePercentage": 42.0,
                "meteringUsage": [
                    {"value": 0.25, "unit": "credit", "unitPlural": "credits"}
                ],
                "turnDurationMs": 5000,
                "effort": "high",
            }),
        );
        let sub_frame = ext_frame(
            "kiro.dev/metadata",
            serde_json::json!({
                "sessionId": "sub-sess",
                "contextUsagePercentage": 77.7,
                "meteringUsage": [
                    {"value": 9.9, "unit": "credit", "unitPlural": "credits"}
                ],
                "turnDurationMs": 157_152,
                "effort": "low",
            }),
        );
        client.ext_notification(main_frame).await.unwrap();
        client.ext_notification(sub_frame).await.unwrap();

        let routed_frames = drain(&mut nrx);
        assert_eq!(routed_frames.len(), 2, "both frames must be forwarded");

        // Channel-level scoping: the subagent frame must arrive scoped to its
        // own session (mirroring the ToolCallChunk promotion), so App routing
        // can divert it away from the main pipeline.
        assert_eq!(
            routed_frames[1].session_id,
            Some(SessionId::new("sub-sess")),
            "subagent metadata frame must be scoped to its sessionId, not global"
        );

        // Replay through the App routing contract: main values must come only
        // from the main-session frame.
        let main_id = SessionId::new("main-sess");
        let mut session = replay_to_main(&routed_frames, &main_id);
        let usage = session
            .context_usage()
            .expect("main-session metadata frame applied")
            .percentage();
        assert!(
            (usage - 42.0).abs() < f64::EPSILON,
            "main context usage must come only from the main frame, got {usage}"
        );

        // Metering: buffered per-turn, surfaced on TurnCompleted.
        session.apply_notification(&Notification::TurnCompleted {
            stop_reason: crate::types::StopReason::EndTurn,
        });
        let metering = session
            .last_turn()
            .and_then(|t| t.metering())
            .expect("main frame carried metering");
        assert!(
            (metering.credits() - 0.25).abs() < f64::EPSILON,
            "main turn credits must come only from the main frame, got {}",
            metering.credits()
        );
    }

    // cyril-fh06 acceptance: a metadata frame WITHOUT a sessionId stays global
    // and still applies to the main session (byte-identical single-session
    // behavior).
    #[tokio::test]
    async fn metadata_without_session_id_still_applies_to_main() {
        let (ntx, mut nrx) = mpsc::channel(8);
        let (ptx, _prx) = mpsc::channel(1);
        let client = v2_client(ntx, ptx);

        let frame = ext_frame(
            "kiro.dev/metadata",
            serde_json::json!({ "contextUsagePercentage": 13.5 }),
        );
        client.ext_notification(frame).await.unwrap();

        let routed_frames = drain(&mut nrx);
        assert_eq!(routed_frames.len(), 1);
        assert_eq!(
            routed_frames[0].session_id, None,
            "sessionId-less metadata must stay global"
        );

        let main_id = SessionId::new("main-sess");
        let session = replay_to_main(&routed_frames, &main_id);
        let usage = session
            .context_usage()
            .expect("global metadata frame applies to main")
            .percentage();
        assert!(
            (usage - 13.5).abs() < f64::EPSILON,
            "global metadata frame must apply to main, got {usage}"
        );
    }
}

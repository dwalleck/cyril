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
    #[cfg(feature = "kas")]
    terminals: crate::protocol::kas::terminal_io::TerminalRegistry,
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
            terminals: crate::protocol::kas::terminal_io::TerminalRegistry::new(),
        }
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
                // kiro.dev/session/update envelope. Promote it to the
                // channel-level RoutedNotification routing.
                let routed = match &notification {
                    Notification::ToolCallChunk {
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

    /// Handle incoming server→client ext REQUESTS. KAS-1 answers
    /// `_kiro/auth/getAccessToken` (wrapper mode, `--auth=acp-callback`) from
    /// kiro-cli's own token file; every other ext request gets the protocol
    /// default. The v2 free path never sends this, and the cfg-split keeps the
    /// credential code out of a default build (ADR-0002).
    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        Self::handle_ext_request(args).await
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
}

impl KiroClient {
    // `#[cfg]` blocks (not a `cfg!(...)` runtime branch) are required: the `kas`
    // module — and thus `kas::auth::respond_get_access_token` — does not exist in
    // a default build, so a single body referencing it would fail to compile.
    /// Route an ext request: KAS-1 handles only `getAccessToken`.
    #[cfg(feature = "kas")]
    async fn handle_ext_request(args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        if args.method.as_ref() == crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD {
            return crate::protocol::kas::auth::respond_get_access_token().await;
        }
        // fs/terminal host callbacks are TYPED acp::Client methods, not ext
        // requests: fs/read_text_file is the `read_text_file` override above
        // (KAS-5a, cyril-7bdu); terminal/* is KAS-5b (cyril-ufie). This arm only
        // answers `_kiro/*` ext requests.
        default_ext_response()
    }

    /// Default build: no KAS ext requests are handled.
    #[cfg(not(feature = "kas"))]
    async fn handle_ext_request(_args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        default_ext_response()
    }
}

/// The ACP protocol default for an unhandled ext request: a `null` result.
fn default_ext_response() -> acp::Result<acp::ExtResponse> {
    Ok(acp::ExtResponse::new(
        serde_json::value::RawValue::NULL.to_owned().into(),
    ))
}

#[cfg(all(test, feature = "kas"))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use agent_client_protocol::Client as _;

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
            std::rc::Rc::new(crate::protocol::engine::KasEngine),
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
            std::rc::Rc::new(crate::protocol::engine::KasEngine),
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
}

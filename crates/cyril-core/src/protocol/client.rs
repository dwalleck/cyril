use std::cell::RefCell;
use std::collections::HashMap;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::protocol::convert;
use crate::types::*;

#[cfg(feature = "kas")]
pub(crate) type ResolvedHostShell = Option<crate::protocol::kas::host_shell::HostShell>;
#[cfg(not(feature = "kas"))]
pub(crate) struct ResolvedHostShell;

/// The host-callback mediation channel item (cyril-g9vt). In a default build
/// the item is uninhabited, so the channel exists but no traffic can — and
/// this alias lives HERE (not in `host_mediator`) so the mediator module
/// never names a kas type (design C12).
#[cfg(feature = "kas")]
pub(crate) type HostCallbackItem = crate::protocol::kas::callbacks::HostCallback;
#[cfg(not(feature = "kas"))]
pub(crate) type HostCallbackItem = crate::protocol::host_mediator::NeverCallback;

/// A dangling mediation sender for tests whose paths never mediate
/// (refusal/parse tests); handled-path tests use [`spawn_test_mediation`].
#[cfg(test)]
pub(crate) fn test_host_tx() -> mpsc::Sender<HostCallbackItem> {
    mpsc::channel(4).0
}

/// Inline mediation for client unit tests (cyril-g9vt): a background consumer
/// drains the host channel through the REAL accept/dispatch pair, resolving
/// serially — concurrency is the harness seam tests' subject, not the unit
/// tests'. Notifications the dispatch emits are dropped (tests that assert
/// them run at the harness level).
#[cfg(all(test, feature = "kas"))]
pub(crate) fn spawn_test_mediation() -> mpsc::Sender<HostCallbackItem> {
    let (tx, mut rx) = mpsc::channel::<HostCallbackItem>(16);
    let (ntx, nrx) = mpsc::channel(16);
    std::mem::forget(nrx); // keep finish()'s sends deliverable
    tokio::spawn(async move {
        let mut mediator = crate::protocol::host_mediator::HostMediator::new();
        let ctx = crate::protocol::kas::callbacks::DispatchCtx { notify_tx: ntx };
        while let Some(cb) = rx.recv().await {
            match mediator.accept(cb) {
                crate::protocol::host_mediator::Accept::Spawn(job) => {
                    crate::protocol::kas::callbacks::dispatch(job.callback, &ctx).await;
                    mediator.complete(job.id);
                }
                crate::protocol::host_mediator::Accept::Consumed => {}
            }
        }
    });
    tx
}

#[cfg(test)]
pub(crate) fn test_host_shell(engine: crate::types::AgentEngine) -> ResolvedHostShell {
    #[cfg(feature = "kas")]
    {
        (engine == crate::types::AgentEngine::Kas)
            .then(crate::protocol::kas::host_shell::HostShell::test_posix)
    }
    #[cfg(not(feature = "kas"))]
    {
        let _ = engine;
        ResolvedHostShell
    }
}

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
    /// KAS-7 (cyril-jiyn): the hook registry serving `_kiro/hooks/*`, loaded
    /// once at construction from the workspace + global `.kiro/hooks`.
    /// `Some` iff the bound engine's Hooks adapter direction is `Inbound`
    /// (cyril-dn91 C10) — Outbound runs hooks agent-side and v2/Off have no
    /// hooks capability, and neither gets an empty-registry stand-in (the
    /// sentinel the no-sentinel rule forbids). Registry presence IS the
    /// inbound-serving gate consulted by the dispatch arms.
    #[cfg(feature = "kas")]
    hooks: Option<std::rc::Rc<crate::protocol::kas::hooks::HookRegistry>>,
    /// Session workspace, the cwd for hook command execution (cyril-jiyn).
    #[cfg(feature = "kas")]
    cwd: std::path::PathBuf,
    /// In-flight hook operations, so `_kiro/hooks/cancel` can abort one
    /// (cyril-jiyn).
    #[cfg(feature = "kas")]
    hook_ops: crate::protocol::kas::hooks::HookOps,
    /// Ingress to the bridge's host-callback mediation seam (cyril-g9vt):
    /// parsed, typed callbacks cross here in wire order and this client
    /// awaits each callback's typed reply. Bounded — a full channel makes the
    /// acp request task await capacity (lossless backpressure). kas-only:
    /// a default build constructs no callbacks (the item is uninhabited).
    #[cfg(feature = "kas")]
    host_tx: mpsc::Sender<HostCallbackItem>,
}

impl KiroClient {
    pub fn new(
        notification_tx: mpsc::Sender<RoutedNotification>,
        permission_tx: mpsc::Sender<PermissionRequest>,
        engine: std::rc::Rc<dyn crate::protocol::engine::Engine>,
        host_shell: ResolvedHostShell,
        host_tx: mpsc::Sender<HostCallbackItem>,
        cwd: &std::path::Path,
    ) -> Self {
        #[cfg(not(feature = "kas"))]
        let _ = cwd; // hooks registry (the only cwd consumer) is kas-only
        #[cfg(not(feature = "kas"))]
        let _ = host_shell;
        #[cfg(not(feature = "kas"))]
        let _ = host_tx; // no callback can exist to send (uninhabited item)
        #[cfg(feature = "kas")]
        let hooks = match engine.adapters().hooks {
            crate::protocol::engine::HooksAdapter::Inbound => Some(std::rc::Rc::new(
                crate::protocol::kas::hooks::HookRegistry::load(
                    cwd,
                    crate::kiro_agent_config::home_dir()
                        .map(|h| h.join(".kiro"))
                        .as_deref(),
                ),
            )),
            _ => None,
        };
        Self {
            notification_tx,
            permission_tx,
            tool_call_inputs: RefCell::new(HashMap::new()),
            engine,
            #[cfg(feature = "kas")]
            terminals: std::rc::Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new(
                host_shell.map(std::rc::Rc::new),
            )),
            #[cfg(feature = "kas")]
            hooks,
            #[cfg(feature = "kas")]
            cwd: cwd.to_path_buf(),
            #[cfg(feature = "kas")]
            hook_ops: crate::protocol::kas::hooks::HookOps::default(),
            #[cfg(feature = "kas")]
            host_tx,
        }
    }

    /// Send one typed callback across the mediation seam. A closed channel
    /// means the bridge loop is gone — surfaced as a JSON-RPC error, never a
    /// silent drop.
    #[cfg(feature = "kas")]
    async fn send_host(&self, cb: HostCallbackItem) -> acp::Result<()> {
        self.host_tx.send(cb).await.map_err(|_| {
            acp::Error::new(
                -32603,
                "host-callback mediation unavailable (bridge closing)",
            )
        })
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

    /// Gate a Host I/O family callback on the bound engine's adapter
    /// (cyril-dn91): `Err(method_not_found)` with the refusal breadcrumb when
    /// absent. One helper for all ten call sites, so a future host-io method
    /// cannot forget the gate's shape.
    #[cfg(feature = "kas")]
    fn require_host_io(&self, method: &str) -> acp::Result<()> {
        if self.engine.adapters().host_io.is_none() {
            return refuse_unadapted(method);
        }
        Ok(())
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

        // KAS-7 hooks host notifications (cyril-jiyn), handled cyril-side rather
        // than converted to a UI notification: `cancel` aborts an in-flight
        // hook by operationId; `didChange` announces on-disk hook edits (no
        // hot-reload in v1 — cyril-2adk — so it is logged, not acted on).
        #[cfg(feature = "kas")]
        {
            if args.method.as_ref() == crate::protocol::kas::hooks::CANCEL_METHOD {
                // Cancel aborts an in-flight INBOUND hook op — ops only exist
                // when cyril serves hooks (cyril-dn91 C12 prose; the gate
                // mirrors the executeHook arm's).
                if self.hooks.is_none() {
                    tracing::debug!("hooks/cancel dropped: no inbound hooks adapter");
                    return Ok(());
                }
                if let Some(op_id) = params.get("operationId").and_then(|o| o.as_str()) {
                    self.hook_ops.cancel(op_id);
                }
                return Ok(());
            }
            if args.method.as_ref() == crate::protocol::kas::hooks::DID_CHANGE_METHOD {
                // didChange is meaningful in EITHER hooks direction (Inbound:
                // on-disk edit note, cyril-2adk; Outbound: KAS pushes its full
                // new registry, cyril-gk17) — but an engine with no hooks
                // capability gets no HooksChanged surface (cyril-dn91 C12).
                if matches!(
                    self.engine.adapters().hooks,
                    crate::protocol::engine::HooksAdapter::None
                ) {
                    tracing::debug!("hooks/didChange dropped: engine has no hooks capability");
                    return Ok(());
                }
                // Under `kas_hooks = "kas"` the notification carries KAS's FULL
                // new registry, so cyril can refresh what it shows without
                // asking (cyril-gk17). Under `"host"` cyril owns the registry
                // and the payload has no `hooks` array — reloading cyril's own
                // on-disk registry is a different job, still cyril-2adk.
                match crate::protocol::kas::hooks::parse_wire_hooks(&params) {
                    Some(hooks) => {
                        tracing::debug!(count = hooks.len(), "KAS hook registry changed");
                        if self
                            .notification_tx
                            .send(Notification::HooksChanged { hooks }.into())
                            .await
                            .is_err()
                        {
                            tracing::debug!("HooksChanged send failed (bridge closing)");
                        }
                    }
                    None => {
                        tracing::info!(
                            "KAS hooks changed on disk; host-registry reload deferred (cyril-2adk)"
                        );
                    }
                }
                return Ok(());
            }
        }

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

    /// Handle incoming server→client ext REQUESTS. Handled families are
    /// parsed to typed callbacks and cross the host-callback mediation seam
    /// (cyril-g9vt); un-adaptered families are refused at parse time
    /// (cyril-dn91); unrecognized ext requests get the protocol default. The
    /// cfg-split keeps KAS code out of a default build (ADR-0002). The l7tw
    /// C11 auth-failure notification now rides the dispatch outcome, ordered
    /// by the mediator (`host_mediator::finish`).
    async fn ext_method(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        self.handle_ext_request(args).await
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
        self.require_host_io("fs/read_text_file")?;
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.send_host(
            crate::protocol::kas::callbacks::HostCallback::ReadTextFile { req: args, reply },
        )
        .await?;
        await_reply(rx).await
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
        // Refusal precedes the responder — no filesystem side effect may
        // occur for an un-adaptered engine (cyril-dn91 C2).
        self.require_host_io("fs/write_text_file")?;
        let (reply, rx) = tokio::sync::oneshot::channel();
        self.send_host(
            crate::protocol::kas::callbacks::HostCallback::WriteTextFile { req: args, reply },
        )
        .await?;
        await_reply(rx).await
    }

    /// KAS-5b (cyril-ufie): answer `terminal/create` by spawning the command in the
    /// terminal registry. Returns the id immediately (non-blocking). KAS-only.
    #[cfg(feature = "kas")]
    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        self.require_host_io("terminal/create")?;
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
        self.require_host_io("terminal/wait_for_exit")?;
        self.terminals.wait(&args).await
    }

    /// KAS-5b: answer `terminal/output` with a non-blocking snapshot of the
    /// terminal's combined stdout+stderr and exit status.
    #[cfg(feature = "kas")]
    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        self.require_host_io("terminal/output")?;
        self.terminals.output(&args)
    }

    /// KAS-5b: answer `terminal/release` — kill + reap the child and free the id.
    #[cfg(feature = "kas")]
    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        self.require_host_io("terminal/release")?;
        self.terminals.release(&args).await
    }

    /// KAS-5b: answer `terminal/kill` — terminate the child but keep the id valid.
    #[cfg(feature = "kas")]
    async fn kill_terminal(
        &self,
        args: acp::KillTerminalRequest,
    ) -> acp::Result<acp::KillTerminalResponse> {
        self.require_host_io("terminal/kill")?;
        self.terminals.kill(&args).await
    }
}

impl KiroClient {
    // `#[cfg]` blocks (not a `cfg!(...)` runtime branch) are required: the `kas`
    // module — and thus the typed callback enum — does not exist in a default
    // build, so a single body referencing it would fail to compile.
    /// Route an ext request (`_kiro/*`): KAS-1 `getAccessToken` (mediated),
    /// KAS-5b `terminal/shell_type`.
    #[cfg(feature = "kas")]
    async fn handle_ext_request(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        if args.method.as_ref() == crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD {
            if self.engine.adapters().auth.is_none() {
                return refuse_unadapted(args.method.as_ref());
            }
            let (reply, rx) = tokio::sync::oneshot::channel();
            self.send_host(crate::protocol::kas::callbacks::HostCallback::GetAccessToken { reply })
                .await?;
            return await_reply(rx).await;
        }
        if args.method.as_ref() == crate::protocol::kas::terminal_io::SHELL_TYPE_METHOD {
            self.require_host_io(args.method.as_ref())?;
            return self.terminals.respond_shell_type();
        }
        // Inbound hooks serving (list/executeHook/sessionStart) requires the
        // Inbound registry — its presence IS the direction gate (cyril-dn91
        // C5/C10). Outbound advertises hooks but serves nothing inbound;
        // executeHook especially must not run wire-supplied commands without
        // an Inbound adapter.
        if args.method.as_ref() == crate::protocol::kas::hooks::LIST_METHOD {
            let Some(hooks) = &self.hooks else {
                return refuse_unadapted(args.method.as_ref());
            };
            let params = parse_ext_params(&args);
            return hooks.respond_list(&params);
        }
        if args.method.as_ref() == crate::protocol::kas::hooks::EXECUTE_METHOD {
            if self.hooks.is_none() {
                return refuse_unadapted(args.method.as_ref());
            }
            let params = parse_ext_params(&args);
            return crate::protocol::kas::hooks::respond_execute(
                &params,
                &self.cwd,
                &self.hook_ops,
            )
            .await;
        }
        if args.method.as_ref() == crate::protocol::kas::hooks::SESSION_START_METHOD {
            let Some(hooks) = &self.hooks else {
                return refuse_unadapted(args.method.as_ref());
            };
            return crate::protocol::kas::hooks::respond_session_start(hooks, &self.cwd).await;
        }
        // cyril-kf2g: the `_kiro/fs/*` superset dialect, selected by the
        // `fs._meta.kiro` capabilities this engine advertises. Both the
        // advertisement and this dispatch derive from `kiro_fs::FS_OPS`, and
        // `kiro_fs::dispatch` matches exhaustively over the op kind — so an
        // operation cannot be advertised without a responder. Left unpaired it
        // would answer the protocol-default null, which the agent reads as a
        // successful empty result. Fenced by
        // `every_advertised_fs_flag_is_dispatched`.
        {
            use crate::protocol::kas::{callbacks, kiro_fs};
            if let Some(op) = kiro_fs::op_for_method(args.method.as_ref()) {
                self.require_host_io(args.method.as_ref())?;
                // Typed parse at the seam (cyril-g9vt): malformed params on a
                // HANDLED family are a wire error, not the old Null fallback.
                let parsed = callbacks::KiroFsArgs::parse(op, &parse_ext_params(&args))
                    .map_err(|e| acp::Error::new(-32602, e))?;
                let (reply, rx) = tokio::sync::oneshot::channel();
                self.send_host(callbacks::HostCallback::KiroFs {
                    args: parsed,
                    reply,
                })
                .await?;
                return await_reply(rx).await;
            }
        }
        // The bare-ACP fs/terminal lifecycle host callbacks are TYPED acp::Client
        // methods (the overrides above), not ext requests: fs/read_text_file (KAS-5a,
        // cyril-7bdu) and terminal/{create,output,wait_for_exit,release,kill} (KAS-5b,
        // cyril-ufie). This arm answers only the `_kiro/*`-prefixed ext requests.
        unhandled_ext_response(args.method.as_ref())
    }

    /// Default build: no KAS ext requests are handled.
    #[cfg(not(feature = "kas"))]
    async fn handle_ext_request(&self, args: acp::ExtRequest) -> acp::Result<acp::ExtResponse> {
        unhandled_ext_response(args.method.as_ref())
    }
}

/// Parse an ext request's params to JSON, logging (not swallowing) a parse
/// failure. `RawValue` is pre-validated JSON so this is practically
/// unreachable, but a `Null` fallback with no breadcrumb is the one spot that
/// would diverge from the module's log-before-fallback posture.
#[cfg(feature = "kas")]
fn parse_ext_params(args: &acp::ExtRequest) -> serde_json::Value {
    match serde_json::from_str(args.params.get()) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(method = %args.method, error = %e, "ext request params not JSON; using null");
            serde_json::Value::Null
        }
    }
}

/// Refuse a host callback whose family the bound engine installs no adapter
/// for (cyril-dn91, ADR-0001 amendment): JSON-RPC method-not-found — never the
/// protocol-default null, which the agent reads as success-with-empty-result.
/// Generic so both ext requests and typed `acp::Client` overrides share it.
#[cfg(feature = "kas")]
fn refuse_unadapted<T>(method: &str) -> acp::Result<T> {
    tracing::debug!(
        method,
        "host callback refused: bound engine installs no adapter for this family"
    );
    Err(acp::Error::method_not_found())
}

/// Await a mediated callback's typed reply. A dropped responder means the
/// resolution was aborted (cancel, shutdown, or a resolution-task fault) —
/// surfaced as a JSON-RPC error, never a hang or a silent default.
#[cfg(feature = "kas")]
async fn await_reply<T>(rx: tokio::sync::oneshot::Receiver<acp::Result<T>>) -> acp::Result<T> {
    match rx.await {
        Ok(result) => result,
        Err(_) => {
            tracing::debug!("host callback aborted before resolution");
            Err(acp::Error::new(-32603, "host callback aborted"))
        }
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
            test_host_shell(crate::types::AgentEngine::Kas),
            spawn_test_mediation(),
            std::path::Path::new("/tmp"),
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
            test_host_shell(crate::types::AgentEngine::Kas),
            spawn_test_mediation(),
            std::path::Path::new("/tmp"),
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
        kas_client_with_shell(test_host_shell(crate::types::AgentEngine::Kas))
    }

    /// A V2-bound client in a kas-feature build — the cyril-dn91 defect
    /// configuration. Returns the notification receiver so refusal side
    /// effects (or their required absence) are observable.
    fn v2_bound_client() -> (KiroClient, mpsc::Receiver<RoutedNotification>) {
        let (ntx, nrx) = mpsc::channel(4);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::V2Engine),
            test_host_shell(crate::types::AgentEngine::V2),
            test_host_tx(),
            std::path::Path::new("/tmp"),
        );
        (client, nrx)
    }

    // cyril-dn91 C1 fence: a V2-bound client (kas build) REFUSES
    // getAccessToken with JSON-RPC method-not-found — not the protocol-default
    // null (agent reads that as success), not a responder/store error. Fails
    // against the pre-dn91 ungated arm, which ran the store read under V2.
    #[tokio::test]
    async fn v2_refuses_auth_callback() {
        let (client, _nrx) = v2_bound_client();
        let params = serde_json::value::RawValue::from_string("{}".to_string())
            .unwrap()
            .into();
        let err = client
            .ext_method(acp::ExtRequest::new(
                crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
                params,
            ))
            .await
            .expect_err("V2 installs no auth adapter — must refuse, not answer");
        assert_eq!(
            err.code,
            acp::ErrorCode::MethodNotFound,
            "refusal must be method-not-found, got: {err:?}"
        );
    }

    // cyril-dn91 C13 fence: the refusal travels the same ext_method path as
    // real auth failures, but must NOT emit BridgeError("auth") — a refusal is
    // not actionable. Fails under the plausible buggy impl: gate added inside
    // handle_ext_request with notify_if_auth_failure left untouched.
    #[tokio::test]
    async fn auth_refusal_emits_no_bridge_error() {
        let (client, mut nrx) = v2_bound_client();
        let params = serde_json::value::RawValue::from_string("{}".to_string())
            .unwrap()
            .into();
        let _ = client
            .ext_method(acp::ExtRequest::new(
                crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
                params,
            ))
            .await;
        assert!(
            nrx.try_recv().is_err(),
            "a refusal must not surface as a BridgeError"
        );
    }

    fn kas_client_with_shell(shell: ResolvedHostShell) -> KiroClient {
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
            shell,
            test_host_tx(),
            std::path::Path::new("/tmp"),
        )
    }

    // cyril-jiyn slice 3v: the `_kiro/hooks/list` ext request routes through
    // handle_ext_request to the client's registry and replies with the
    // matching hooks. A Host-mode KasEngine client loads a real registry from
    // a tempdir. Fails if the method arm is missing (falls to the null
    // default) or the registry isn't wired to the dispatch.
    #[tokio::test]
    async fn hooks_list_ext_request_routes_to_registry() {
        use agent_client_protocol::Client as _;

        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".kiro/hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(
            hooks_dir.join("h.json"),
            r#"{"version":"v1","hooks":[
                {"name":"greet","trigger":"UserPromptSubmit",
                 "action":{"type":"command","command":"echo hi"}}
            ]}"#,
        )
        .unwrap();

        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine {
                hooks_mode: crate::types::kas_hooks::KasHooksMode::Host,
            }),
            test_host_shell(crate::types::AgentEngine::Kas),
            test_host_tx(),
            dir.path(),
        );

        let params = serde_json::value::RawValue::from_string(
            serde_json::json!({"trigger": "promptSubmit"}).to_string(),
        )
        .unwrap();
        let resp = client
            .ext_method(acp::ExtRequest::new(
                crate::protocol::kas::hooks::LIST_METHOD,
                params.into(),
            ))
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        let hooks = body["hooks"].as_array().expect("hooks array");
        assert_eq!(hooks.len(), 1, "the promptSubmit hook is served");
        assert_eq!(hooks[0]["id"], "h:greet");
    }

    // cyril-kf2g review fence: the OTHER half of the advertise/dispatch pairing.
    // `kiro_fs` fences that the advertisement derives from FS_OPS; this fences
    // that every entry in that table actually reaches an arm. Before this,
    // three of the five advertised flags were dispatched by nothing but a
    // comment asking future editors to keep them in sync.
    //
    // An undispatched method answers the protocol-default NULL body, which is
    // indistinguishable from a successful empty result on the wire — so the
    // assertion is specifically "not null", not "no error".
    #[tokio::test]
    async fn every_advertised_fs_flag_is_dispatched() {
        use crate::protocol::kas::kiro_fs;
        use agent_client_protocol::Client as _;

        let dir = tempfile::tempdir().unwrap();
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
            test_host_shell(crate::types::AgentEngine::Kas),
            spawn_test_mediation(),
            dir.path(),
        );

        for op in kiro_fs::FS_OPS {
            // A fresh target per op: `delete` consumes what it is given and
            // `write_file` must not clobber what another op still needs. The
            // one directory-shaped op needs a directory, or it fails on the
            // target rather than on the wiring.
            let target = if op.flag == "readDirectory" {
                let d = dir.path().join("listing");
                std::fs::create_dir_all(&d).unwrap();
                d
            } else {
                let f = dir.path().join(format!("{}.txt", op.flag));
                std::fs::write(&f, "seed\n").unwrap();
                f
            };
            let params = serde_json::json!({
                "sessionId": "s", "path": target, "content": "seed\n"
            });
            let raw = serde_json::value::RawValue::from_string(params.to_string()).unwrap();

            // An `Err` still proves dispatch — only a responder can produce one.
            // The undispatched signature is specifically `Ok(null)`: the
            // protocol-default body, which the agent reads as a successful empty
            // result. So that, and only that, is the failure.
            match client
                .ext_method(acp::ExtRequest::new(op.method, raw.into()))
                .await
            {
                Err(_) => {}
                Ok(resp) => {
                    let body: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
                    assert!(
                        !body.is_null(),
                        "{} answered the protocol-default null — it is advertised \
                         via FS_OPS but reaches no arm in handle_ext_request",
                        op.wire
                    );
                }
            }
        }
    }

    // cyril-kf2g: the `_kiro/fs/*` dialect routes through handle_ext_request to
    // the kiro_fs responders. The unit tests in `kiro_fs` cover semantics; this
    // one covers the WIRING, which they cannot — a responder that is written,
    // tested, and never dispatched answers the protocol-default null, and the
    // agent reads that as a successful empty result. One method per direction:
    // a read-only one (stat) and the destructive one (delete).
    #[tokio::test]
    async fn kiro_fs_ext_requests_route_to_responders() {
        use agent_client_protocol::Client as _;

        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("probe.txt");
        std::fs::write(&f, "12345").unwrap();

        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
            test_host_shell(crate::types::AgentEngine::Kas),
            spawn_test_mediation(),
            dir.path(),
        );
        let call = async |method: &'static str, params: serde_json::Value| {
            let raw = serde_json::value::RawValue::from_string(params.to_string()).unwrap();
            let resp = client
                .ext_method(acp::ExtRequest::new(method, raw.into()))
                .await
                .unwrap_or_else(|e| panic!("{method} must be dispatched, got {e:?}"));
            serde_json::from_str::<serde_json::Value>(resp.0.get()).unwrap()
        };

        let stat = call(
            crate::protocol::kas::kiro_fs::STAT_METHOD,
            serde_json::json!({"sessionId": "s", "path": f}),
        )
        .await;
        assert_eq!(stat["type"], "file", "stat must reach the responder");
        assert_eq!(stat["size"], 5);
        assert!(
            !stat.is_null(),
            "a null body is the undispatched signature, not a result"
        );

        let deleted = call(
            crate::protocol::kas::kiro_fs::DELETE_METHOD,
            serde_json::json!({"sessionId": "s", "path": f}),
        )
        .await;
        assert!(deleted.is_object(), "delete replies with an object");
        assert!(
            !f.exists(),
            "delete must actually reach the filesystem — the side effect IS the wiring proof"
        );
    }

    // cyril-dn91 C2 fence: typed fs callbacks refuse under V2 with
    // method-not-found — and the write refusal precedes any filesystem side
    // effect (the gate-after-side-effect bug this fixture is designed to
    // fail under).
    #[tokio::test]
    async fn v2_refuses_typed_fs() {
        let (client, _nrx) = v2_bound_client();
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.txt");
        std::fs::write(&f, "seed").unwrap();

        let err = client
            .read_text_file(acp::ReadTextFileRequest::new(acp::SessionId::new("s"), &f))
            .await
            .expect_err("V2 installs no host_io adapter — read must refuse");
        assert_eq!(err.code, acp::ErrorCode::MethodNotFound);

        let target = dir.path().join("must-not-exist.txt");
        let err = client
            .write_text_file(acp::WriteTextFileRequest::new(
                acp::SessionId::new("s"),
                &target,
                "boom",
            ))
            .await
            .expect_err("write must refuse");
        assert_eq!(err.code, acp::ErrorCode::MethodNotFound);
        assert!(
            !target.exists(),
            "refusal must precede the write side effect"
        );
    }

    // cyril-dn91 C3 fence: every `_kiro/fs/*` op refuses under V2, walked over
    // FS_OPS — the same table the advertisement derives from, so a gate
    // missing on any one arm fails its named op, and a sixth op added later is
    // fenced automatically. The delete target must survive its refusal.
    #[tokio::test]
    async fn v2_refuses_kiro_fs_all_ops() {
        use crate::protocol::kas::kiro_fs;

        let (client, _nrx) = v2_bound_client();
        let dir = tempfile::tempdir().unwrap();
        for op in kiro_fs::FS_OPS {
            let f = dir.path().join(format!("{}.txt", op.flag));
            std::fs::write(&f, "seed").unwrap();
            let params = serde_json::json!({"sessionId": "s", "path": f, "content": "seed"});
            let raw = serde_json::value::RawValue::from_string(params.to_string()).unwrap();
            match client
                .ext_method(acp::ExtRequest::new(op.method, raw.into()))
                .await
            {
                Err(e) => assert_eq!(
                    e.code,
                    acp::ErrorCode::MethodNotFound,
                    "{} must refuse with method-not-found",
                    op.wire
                ),
                Ok(resp) => panic!("{} must refuse, got body {}", op.wire, resp.0.get()),
            }
            assert!(
                f.exists(),
                "{}: refusal must precede any side effect",
                op.flag
            );
        }
    }

    // cyril-dn91 C4 fence: the whole terminal family refuses under V2 with
    // method-not-found — NOT the registry's "no resolved host shell" responder
    // error (the pre-dn91 indirect gate), and not only on the ext arm: each
    // typed override is gated individually, so an ext-arm-only gate fails
    // here.
    #[tokio::test]
    async fn v2_refuses_terminal_family() {
        let (client, _nrx) = v2_bound_client();
        let sid = || acp::SessionId::new("s");
        let tid = acp::TerminalId::new("term-x");

        let assert_refused = |err: acp::Error, what: &str| {
            assert_eq!(err.code, acp::ErrorCode::MethodNotFound, "{what}: {err:?}");
            assert_ne!(
                err.message, "KAS terminal callback has no resolved host shell",
                "{what} must refuse at the adapter gate, not the shell registry"
            );
        };
        assert_refused(
            client
                .create_terminal(acp::CreateTerminalRequest::new(sid(), "true"))
                .await
                .expect_err("create refused"),
            "terminal/create",
        );
        assert_refused(
            client
                .wait_for_terminal_exit(acp::WaitForTerminalExitRequest::new(sid(), tid.clone()))
                .await
                .expect_err("wait refused"),
            "terminal/wait_for_exit",
        );
        assert_refused(
            client
                .terminal_output(acp::TerminalOutputRequest::new(sid(), tid.clone()))
                .await
                .expect_err("output refused"),
            "terminal/output",
        );
        assert_refused(
            client
                .release_terminal(acp::ReleaseTerminalRequest::new(sid(), tid.clone()))
                .await
                .expect_err("release refused"),
            "terminal/release",
        );
        assert_refused(
            client
                .kill_terminal(acp::KillTerminalRequest::new(sid(), tid))
                .await
                .expect_err("kill refused"),
            "terminal/kill",
        );

        let params = serde_json::value::RawValue::from_string("{\"sessionId\":\"s\"}".to_string())
            .unwrap()
            .into();
        assert_refused(
            client
                .ext_method(acp::ExtRequest::new("kiro/terminal/shell_type", params))
                .await
                .expect_err("shell_type refused"),
            "shell_type",
        );
    }

    // cyril-dn91 C10 fence: the hook registry exists iff the Hooks direction
    // is Inbound — no empty-registry stand-in for Outbound/Off (the sentinel
    // ADR-0001's amendment forbids). Fails under the keep-always-constructing
    // impl.
    #[test]
    fn registry_present_iff_inbound() {
        use crate::types::kas_hooks::KasHooksMode;

        let client_for = |mode| {
            let (ntx, _nrx) = mpsc::channel(1);
            let (ptx, _prx) = mpsc::channel(1);
            KiroClient::new(
                ntx,
                ptx,
                std::rc::Rc::new(crate::protocol::engine::KasEngine { hooks_mode: mode }),
                test_host_shell(crate::types::AgentEngine::Kas),
                test_host_tx(),
                std::path::Path::new("/tmp"),
            )
        };
        assert!(
            client_for(KasHooksMode::Host).hooks.is_some(),
            "Inbound loads a registry"
        );
        assert!(
            client_for(KasHooksMode::Kas).hooks.is_none(),
            "Outbound gets NO registry"
        );
        assert!(
            client_for(KasHooksMode::Off).hooks.is_none(),
            "Off gets NO registry"
        );
        let (v2, _nrx) = v2_bound_client();
        assert!(v2.hooks.is_none(), "V2 gets NO registry");
    }

    // cyril-dn91 C12 fence: didChange carrying a full registry payload must
    // NOT surface HooksChanged when the bound engine has no hooks capability.
    // Fails against the pre-dn91 ungated arm (which forwarded it under V2);
    // the Outbound cell proves the gate is direction-shaped, not
    // registry-presence-shaped (Outbound has no registry but DOES get
    // HooksChanged, cyril-gk17).
    #[tokio::test]
    async fn did_change_gated_by_hooks_direction() {
        let payload = serde_json::json!({"hooks": [
            {"id": "k:h", "name": "h", "trigger": "UserPromptSubmit", "enabled": true}
        ]});

        let (v2, mut v2_rx) = v2_bound_client();
        let raw = serde_json::value::RawValue::from_string(payload.to_string()).unwrap();
        v2.ext_notification(acp::ExtNotification::new(
            crate::protocol::kas::hooks::DID_CHANGE_METHOD,
            raw.into(),
        ))
        .await
        .unwrap();
        assert!(
            v2_rx.try_recv().is_err(),
            "no-hooks engine must not surface HooksChanged"
        );

        let (ntx, mut kas_rx) = mpsc::channel(4);
        let (ptx, _prx) = mpsc::channel(1);
        let outbound = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine {
                hooks_mode: crate::types::kas_hooks::KasHooksMode::Kas,
            }),
            test_host_shell(crate::types::AgentEngine::Kas),
            test_host_tx(),
            std::path::Path::new("/tmp"),
        );
        let raw = serde_json::value::RawValue::from_string(payload.to_string()).unwrap();
        outbound
            .ext_notification(acp::ExtNotification::new(
                crate::protocol::kas::hooks::DID_CHANGE_METHOD,
                raw.into(),
            ))
            .await
            .unwrap();
        match kas_rx.try_recv() {
            Ok(routed) => assert!(
                matches!(routed.notification, Notification::HooksChanged { .. }),
                "Outbound still surfaces HooksChanged"
            ),
            Err(_) => panic!("Outbound didChange must still surface HooksChanged (cyril-gk17)"),
        }
    }

    // cyril-jiyn claim 12 fence: the _kiro/hooks/didChange notification is
    // consumed without error (and without reaching the converter's
    // unknown-variant path). Fails if the arm is missing.
    #[tokio::test]
    async fn hooks_did_change_consumed() {
        let client = kas_client();
        let params = serde_json::value::RawValue::from_string("{}".to_string()).unwrap();
        let res = client
            .ext_notification(acp::ExtNotification::new(
                crate::protocol::kas::hooks::DID_CHANGE_METHOD,
                params.into(),
            ))
            .await;
        assert!(res.is_ok(), "didChange is consumed cleanly");
    }

    // cyril-jiyn claim 13 fence: a slow executeHook does not serialize the
    // client — an independent shell_type ext request completes long before the
    // slow hook finishes. A synchronous (blocking) executor would make the
    // shell_type reply wait for the whole hook. Uses join! on one task; the
    // async executor + per-request handling let both progress.
    #[tokio::test]
    async fn slow_hook_does_not_block_loop() {
        use agent_client_protocol::Client as _;

        let dir = tempfile::tempdir().unwrap();
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine {
                hooks_mode: crate::types::kas_hooks::KasHooksMode::Host,
            }),
            test_host_shell(crate::types::AgentEngine::Kas),
            test_host_tx(),
            dir.path(),
        );

        let exec_params = serde_json::value::RawValue::from_string(
            serde_json::json!({
                "hookId": "h", "hookName": "slow", "command": "sleep 3",
                "sessionId": "s", "userPrompt": ""
            })
            .to_string(),
        )
        .unwrap();
        let slow = client.ext_method(acp::ExtRequest::new(
            crate::protocol::kas::hooks::EXECUTE_METHOD,
            exec_params.into(),
        ));

        let shell_start = std::time::Instant::now();
        // Capture when shell_type RESOLVES inside the block — measuring after
        // join! would always include the 3s (join! awaits both). A blocking
        // executor would starve this future until the hook finished.
        let (_slow_res, (shell_res, shell_elapsed)) = tokio::join!(slow, async {
            let params =
                serde_json::value::RawValue::from_string("{\"sessionId\":\"s\"}".to_string())
                    .unwrap();
            let r = client
                .ext_method(acp::ExtRequest::new(
                    crate::protocol::kas::terminal_io::SHELL_TYPE_METHOD,
                    params.into(),
                ))
                .await;
            (r, shell_start.elapsed())
        });
        assert!(shell_res.is_ok());
        assert!(
            shell_elapsed < std::time::Duration::from_secs(2),
            "shell_type resolved while the 3s hook was still running (not serialized): {shell_elapsed:?}"
        );
    }

    #[tokio::test]
    async fn create_terminal_override_reaches_registry() {
        // KAS-5b fixture M: a KAS `terminal/create` reaches KiroClient's typed
        // override (NOT the acp default `method_not_found`) and returns an id.
        // Fails if the override is missing/miswired. `create` spawns the shell
        // for real, so it needs the per-platform runnable fixture — the
        // `/bin/sh` routing fixture does not exist on Windows.
        let client = kas_client_with_shell(Some(
            crate::protocol::kas::host_shell::HostShell::test_runnable_on_host(),
        ));
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
        assert_eq!(resp.0.get(), r#"{"shellType":"posix"}"#);
    }

    #[tokio::test]
    async fn shell_type_refuses_a_missing_kas_snapshot() {
        let (ntx, _nrx) = mpsc::channel(1);
        let (ptx, _prx) = mpsc::channel(1);
        let client = KiroClient::new(
            ntx,
            ptx,
            std::rc::Rc::new(crate::protocol::engine::KasEngine::default()),
            None,
            test_host_tx(),
            std::path::Path::new("/tmp"),
        );
        let params = serde_json::value::RawValue::from_string("{}".to_string())
            .unwrap()
            .into();
        let err = client
            .ext_method(acp::ExtRequest::new("kiro/terminal/shell_type", params))
            .await
            .expect_err("missing KAS shell snapshot must be refused");
        assert_eq!(
            err.message,
            "KAS terminal callback has no resolved host shell"
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
            test_host_shell(crate::types::AgentEngine::V2),
            test_host_tx(),
            std::path::Path::new("/tmp"),
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
            (metering.credits().unwrap() - 0.25).abs() < f64::EPSILON,
            "main turn credits must come only from the main frame, got {:?}",
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

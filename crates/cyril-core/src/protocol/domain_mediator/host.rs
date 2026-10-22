use std::cell::RefCell;
use std::rc::Rc;

use agent_client_protocol::{Responder, UntypedMessage};
use tokio::sync::mpsc;

use super::HostWork;
use crate::protocol::engine::Engine;
use crate::protocol::host_mediator::HostMediator;

pub(super) fn run(
    mut rx: mpsc::Receiver<HostWork>,
    engine: Rc<dyn Engine>,
    mediator: Rc<RefCell<HostMediator>>,
    #[cfg(feature = "kas")] ctx: Rc<crate::protocol::kas::callbacks::DispatchCtx>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_local(async move {
        while let Some(work) = rx.recv().await {
            match work {
                HostWork::ExtensionRequest { request, responder } => {
                    dispatch_extension(
                        request,
                        responder,
                        &engine,
                        &mediator,
                        #[cfg(feature = "kas")]
                        &ctx,
                    );
                }
                #[cfg(test)]
                HostWork::Probe { .. } => {}
                #[cfg(feature = "kas")]
                HostWork::Callback(callback) => {
                    if supports(engine.adapters(), callback.family()) {
                        accept(callback, &mediator, &ctx);
                    } else {
                        callback.refuse();
                    }
                }
            }
        }
    })
}

fn dispatch_extension(
    request: UntypedMessage,
    responder: Responder<serde_json::Value>,
    _engine: &Rc<dyn Engine>,
    _mediator: &Rc<RefCell<HostMediator>>,
    #[cfg(feature = "kas")] ctx: &Rc<crate::protocol::kas::callbacks::DispatchCtx>,
) {
    let method = super::canonical_extension_method(request.method()).to_owned();
    #[cfg(not(feature = "kas"))]
    {
        if matches!(
            method.as_str(),
            crate::protocol::engine::OPEN_EXTERNAL_URL_METHOD
                | crate::protocol::engine::MCP_ELICITATION_METHOD
        ) {
            respond_extension_error(
                responder,
                &method,
                agent_client_protocol::Error::method_not_found(),
            );
            return;
        }
        tracing::debug!(%method, "unhandled extension request answered with protocol-default null");
        if let Err(error) = responder.respond(serde_json::Value::Null) {
            tracing::debug!(%error, %method, "extension response receiver dropped");
        }
    }

    #[cfg(feature = "kas")]
    {
        use crate::protocol::kas::{callbacks, hooks, kiro_fs, terminal_io};

        let params = request.params;
        let adapters = _engine.adapters();
        use crate::protocol::kas::callbacks::HostFamily;
        // Every branch gates through the SAME fenced `supports()` predicate
        // the HostWork::Callback path uses (cyril-dn91): a gate change in one
        // place cannot silently diverge the two dispatch paths.
        let callback_and_reply = if method == crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD {
            if !supports(adapters, HostFamily::Auth) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::GetAccessToken {
                    force_refresh: crate::protocol::kas::auth::force_refresh_requested(&params),
                    reply,
                },
                response_rx,
            )
        } else if method == crate::protocol::engine::OPEN_EXTERNAL_URL_METHOD {
            if !supports(adapters, HostFamily::ExternalUrl) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let args = match callbacks::OpenExternalUrlArgs::parse(&params) {
                Ok(args) => args,
                Err(error) => {
                    respond_extension_error(
                        responder,
                        &method,
                        agent_client_protocol::Error::invalid_params().data(error),
                    );
                    return;
                }
            };
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::OpenExternalUrl { args, reply },
                response_rx,
            )
        } else if method == crate::protocol::engine::MCP_ELICITATION_METHOD {
            if !supports(adapters, HostFamily::McpElicitation) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let args = match crate::protocol::kas::mcp::ElicitationArgs::parse(&params) {
                Ok(args) => args,
                Err(error) => {
                    respond_extension_error(
                        responder,
                        &method,
                        agent_client_protocol::Error::invalid_params().data(error),
                    );
                    return;
                }
            };
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::McpElicitation { args, reply },
                response_rx,
            )
        } else if method == terminal_io::SHELL_TYPE_METHOD {
            if !supports(adapters, HostFamily::HostIo) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let session_id = params
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::ShellType { session_id, reply },
                response_rx,
            )
        } else if method == hooks::LIST_METHOD {
            if !supports(adapters, HostFamily::HooksInbound) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::HooksList {
                    args: hooks::HooksListArgs::parse(&params),
                    reply,
                },
                response_rx,
            )
        } else if method == hooks::EXECUTE_METHOD {
            if !supports(adapters, HostFamily::HooksInbound) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let args = match callbacks::HooksExecuteArgs::parse(&params) {
                Ok(args) => args,
                Err(error) => {
                    respond_extension_error(
                        responder,
                        &method,
                        agent_client_protocol::Error::invalid_params().data(error),
                    );
                    return;
                }
            };
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::HooksExecute { args, reply },
                response_rx,
            )
        } else if method == hooks::SESSION_START_METHOD {
            if !supports(adapters, HostFamily::HooksInbound) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (
                callbacks::HostCallback::HooksSessionStart { reply },
                response_rx,
            )
        } else if let Some(operation) = kiro_fs::op_for_method(&method) {
            if !supports(adapters, HostFamily::HostIo) {
                respond_extension_error(
                    responder,
                    &method,
                    agent_client_protocol::Error::method_not_found(),
                );
                return;
            }
            let args = match callbacks::KiroFsArgs::parse(operation, &params) {
                Ok(args) => args,
                Err(error) => {
                    respond_extension_error(
                        responder,
                        &method,
                        agent_client_protocol::Error::invalid_params().data(error),
                    );
                    return;
                }
            };
            let (reply, response_rx) = tokio::sync::oneshot::channel();
            (callbacks::HostCallback::KiroFs { args, reply }, response_rx)
        } else {
            tracing::debug!(%method, "unhandled extension request answered with protocol-default null");
            if let Err(error) = responder.respond(serde_json::Value::Null) {
                tracing::debug!(%error, %method, "extension response receiver dropped");
            }
            return;
        };

        let (callback, response_rx) = callback_and_reply;
        accept(callback, _mediator, ctx);
        tokio::task::spawn_local(async move {
            let response = match response_rx.await {
                Ok(Ok(response)) => serde_json::from_str(response.0.get()).map_err(|error| {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                }),
                Ok(Err(error)) => Err(error),
                Err(error) => Err(agent_client_protocol::Error::internal_error()
                    .data(format!("host callback response dropped: {error}"))),
            };
            if let Err(error) = responder.respond_with_result(response) {
                tracing::debug!(%error, %method, "extension response receiver dropped");
            }
        });
    }
}

fn respond_extension_error(
    responder: Responder<serde_json::Value>,
    method: &str,
    response: agent_client_protocol::Error,
) {
    if let Err(error) = responder.respond_with_error(response) {
        tracing::debug!(%error, %method, "extension error response receiver dropped");
    }
}

#[cfg(feature = "kas")]
pub(crate) fn supports(
    adapters: crate::protocol::engine::Adapters,
    family: crate::protocol::kas::callbacks::HostFamily,
) -> bool {
    use crate::protocol::engine::HooksAdapter;
    use crate::protocol::kas::callbacks::HostFamily;

    match family {
        HostFamily::Auth => adapters.auth.is_some(),
        HostFamily::HostIo => adapters.host_io.is_some(),
        HostFamily::ExternalUrl => adapters.external_url.is_some(),
        HostFamily::McpElicitation => adapters.mcp_elicitation.is_some(),
        HostFamily::HooksInbound => adapters.hooks == HooksAdapter::Inbound,
        HostFamily::HooksAny => adapters.hooks != HooksAdapter::None,
    }
}

#[cfg(feature = "kas")]
fn accept(
    callback: crate::protocol::kas::callbacks::HostCallback,
    mediator: &Rc<RefCell<HostMediator>>,
    ctx: &Rc<crate::protocol::kas::callbacks::DispatchCtx>,
) {
    match mediator.borrow_mut().accept(callback) {
        crate::protocol::host_mediator::Accept::Spawn(job) => {
            spawn_job(job, mediator, ctx);
        }
        crate::protocol::host_mediator::Accept::Consumed => {}
    }
}

#[cfg(feature = "kas")]
fn spawn_job(
    job: crate::protocol::host_mediator::Job<crate::protocol::kas::callbacks::HostCallback>,
    mediator: &Rc<RefCell<HostMediator>>,
    ctx: &Rc<crate::protocol::kas::callbacks::DispatchCtx>,
) {
    let mediator = Rc::clone(mediator);
    let ctx = Rc::clone(ctx);
    let crate::protocol::host_mediator::Job {
        callback,
        id,
        cancelled,
    } = job;
    tokio::task::spawn_local(async move {
        match cancelled {
            Some(cancel) => {
                tokio::select! {
                    biased;
                    _ = cancel => {
                        tracing::debug!("host callback aborted by cancel");
                    }
                    () = crate::protocol::kas::callbacks::dispatch(callback, &ctx) => {}
                }
            }
            None => crate::protocol::kas::callbacks::dispatch(callback, &ctx).await,
        }
        mediator.borrow_mut().complete(id);
    });
}

#[cfg(all(test, feature = "kas"))]
mod tests {
    #![allow(clippy::unwrap_used)]

    use agent_client_protocol::schema::v1 as acp;
    use agent_client_protocol::{Agent, Client, ConnectionTo};

    use super::*;
    use crate::protocol::domain_mediator::DomainChannels;
    use crate::protocol::engine::KasEngine;
    use crate::protocol::kas::callbacks::DispatchCtx;
    use crate::protocol::kas::hooks::{HookOps, HookRegistry};
    use crate::protocol::kas::terminal_io::TerminalRegistry;
    use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
    use crate::protocol::source_observer::IngressTracker;

    /// Drive wire `_kiro/auth/getAccessToken` requests (one per `params`)
    /// from a fake agent through the real host-callback boundary — extension
    /// dispatch, `forceRefresh` parsing, `DispatchCtx.kas_token`, the
    /// responder and its notifications — and return each reply body plus
    /// every `Routed` notification the dispatch enqueued, in order.
    async fn auth_round_trip(
        source: crate::protocol::kas::auth::KasTokenSource,
        params: Vec<serde_json::Value>,
    ) -> (Vec<serde_json::Value>, Vec<crate::types::Notification>) {
        let (channels, mut domain_rx, host_rx) =
            DomainChannels::new(IngressTracker::new()).unwrap();
        let ws = tempfile::tempdir().unwrap();
        let ctx = Rc::new(DispatchCtx {
            notify_tx: channels.clone(),
            terminals: Rc::new(TerminalRegistry::new(None)),
            hooks: None,
            hook_ops: HookOps::default(),
            cwd: ws.path().to_path_buf(),
            kas_token: Some(Rc::new(source)),
        });
        let engine: Rc<dyn Engine> = Rc::new(KasEngine::default());
        let host = run(
            host_rx,
            engine,
            Rc::new(RefCell::new(HostMediator::new())),
            ctx,
        );
        let replies = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = replies.clone();
        let agent = Agent
            .builder()
            .name("auth-callback-test-agent")
            .on_receive_request(
                async move |request: acp::InitializeRequest,
                            responder,
                            connection: ConnectionTo<Client>| {
                    let callback_connection = connection.clone();
                    let sink = sink.clone();
                    let params = params.clone();
                    connection.spawn(async move {
                        for p in params {
                            let reply = callback_connection
                                .send_request(UntypedMessage::new("_kiro/auth/getAccessToken", p)?)
                                .block_task()
                                .await?;
                            sink.lock().unwrap().push(reply);
                        }
                        responder.respond(acp::InitializeResponse::new(request.protocol_version))
                    })?;
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            );
        let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
            .await
            .unwrap();
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            runtime
                .connection()
                .send_request(acp::InitializeRequest::new(
                    agent_client_protocol::schema::ProtocolVersion::V1,
                ))
                .block_task(),
        )
        .await
        .unwrap_or_else(|_| panic!("auth callbacks did not complete in time"))
        .unwrap_or_else(|e| panic!("auth callbacks failed: {e}"));
        runtime.shutdown().await;
        host.abort();
        assert!(host.await.unwrap_err().is_cancelled());
        let mut notifications = Vec::new();
        while let Ok(work) = domain_rx.try_recv() {
            if let crate::protocol::domain_mediator::DomainWork::Routed(routed) = work {
                notifications.push(routed.notification);
            }
        }
        let replies = replies.lock().unwrap().clone();
        (replies, notifications)
    }

    // cyril-zbu7 C3/C4 at the host-callback boundary: a wire `forceRefresh:
    // true` reaches kiro-cli as `--force-refresh`, `{}` does not, and the
    // bound source's data is relayed — a dropped `forceRefresh` parse or a
    // dropped `kas_token` (Unbound) fails here.
    #[tokio::test(flavor = "current_thread")]
    async fn get_access_token_round_trips_through_the_bound_kiro_cli() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let dir = tempfile::tempdir().unwrap();
                let envelope = r#"{"kind":"getKasToken","data":{"accessToken":"AT-wire","expiresAt":"2099-01-01T00:00:00Z","profileArn":"arn:p"}}"#;
                let fake = crate::protocol::kas::auth::tests::fake_kiro_cli(
                    dir.path(),
                    &format!(
                        "printf '%s\\n' \"$*\" >> \"${{0%/*}}/journal.txt\"\nprintf '%s\\n' '{envelope}'"
                    ),
                    &format!("echo %* >> \"%~dp0journal.txt\"\necho {envelope}"),
                );
                let source = crate::protocol::kas::auth::KasTokenSource::new(
                    fake,
                    crate::types::SpawnEnvironment::Inherit,
                );
                let (replies, notifications) = auth_round_trip(
                    source,
                    vec![serde_json::json!({"forceRefresh": true}), serde_json::json!({})],
                )
                .await;
                assert_eq!(replies.len(), 2);
                for reply in &replies {
                    assert_eq!(reply["accessToken"], "AT-wire");
                    assert_eq!(reply["profileArn"], "arn:p");
                }
                let journal = std::fs::read_to_string(dir.path().join("journal.txt")).unwrap();
                let calls: Vec<&str> = journal.lines().map(str::trim).collect();
                assert_eq!(
                    calls,
                    ["chat _ get-kas-token --force-refresh", "chat _ get-kas-token"]
                );
                assert!(notifications.is_empty(), "{notifications:?}");
            })
            .await;
    }

    // cyril-zbu7 C10 at the host-callback boundary: the fallback's warning
    // reaches the UI as ONE SystemNotify warning (not per request), and the
    // stored token is relayed.
    #[tokio::test(flavor = "current_thread")]
    async fn fallback_warning_reaches_the_ui_once() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let dir = tempfile::tempdir().unwrap();
                let store = crate::protocol::kas::auth::tests::store_with_token(
                    dir.path(),
                    "2099-01-01T00:00:00Z",
                );
                let source = crate::protocol::kas::auth::KasTokenSource::new(
                    crate::protocol::kas::auth::tests::kiro_cli_without_helper(dir.path()),
                    crate::types::SpawnEnvironment::Inherit,
                )
                .with_store(store);
                let (replies, notifications) =
                    auth_round_trip(source, vec![serde_json::json!({}), serde_json::json!({})])
                        .await;
                assert_eq!(replies.len(), 2);
                assert_eq!(replies[0]["accessToken"], "AT-stored");
                assert_eq!(notifications.len(), 1, "{notifications:?}");
                match &notifications[0] {
                    crate::types::Notification::SystemNotify { level, message } => {
                        assert_eq!(*level, crate::types::event::SystemNotifyLevel::Warning);
                        assert!(message.contains("get-kas-token"), "{message}");
                    }
                    other => panic!("expected a SystemNotify warning, got {other:?}"),
                }
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn hook_tags_cross_host_callback_boundary_and_malformed_tags_degrade() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let ws = tempfile::tempdir().unwrap();
                let dir = ws.path().join(".kiro/hooks");
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(
                    dir.join("tags.json"),
                    r#"{"version":"v1","hooks":[
                        {"name":"always","trigger":"PreToolUse",
                         "action":{"type":"command","command":"echo always"}},
                        {"name":"shell","trigger":"PreToolUse","matcher":"shell",
                         "action":{"type":"command","command":"echo shell"}}
                    ]}"#,
                )
                .unwrap();
                let (channels, _domain_rx, host_rx) =
                    DomainChannels::new(IngressTracker::new()).unwrap();
                let ctx = Rc::new(DispatchCtx {
                    notify_tx: channels.clone(),
                    terminals: Rc::new(TerminalRegistry::new(None)),
                    hooks: Some(Rc::new(HookRegistry::load(ws.path(), None))),
                    hook_ops: HookOps::default(),
                    cwd: ws.path().to_path_buf(),
                    kas_token: None,
                });
                let engine: Rc<dyn Engine> = Rc::new(KasEngine {
                    hooks_mode: crate::types::kas_hooks::KasHooksMode::Host,
                    ..KasEngine::default()
                });
                let host = run(
                    host_rx,
                    engine,
                    Rc::new(RefCell::new(HostMediator::new())),
                    ctx,
                );
                let agent = Agent
                    .builder()
                    .name("hook-tag-callback-test-agent")
                    .on_receive_request(
                        async move |request: acp::InitializeRequest,
                                    responder,
                                    connection: ConnectionTo<Client>| {
                            let callback_connection = connection.clone();
                            connection.spawn(async move {
                                // Malformed tags degrade to ID matching (string members
                                // kept) rather than refusing: KAS skips every hook for a
                                // tool whose list call fails.
                                let cases = [
                                    (
                                        serde_json::json!({
                                            "trigger": "preToolUse",
                                            "toolId": "execute_bash",
                                            "toolTags": ["shell", "@builtin"]
                                        }),
                                        vec!["tags:always", "tags:shell"],
                                    ),
                                    (
                                        serde_json::json!({
                                            "trigger": "preToolUse",
                                            "toolId": "execute_bash"
                                        }),
                                        vec!["tags:always"],
                                    ),
                                    (
                                        serde_json::json!({
                                            "trigger": "preToolUse",
                                            "toolTags": ["shell", "@builtin"]
                                        }),
                                        vec!["tags:always"],
                                    ),
                                    (
                                        serde_json::json!({
                                            "trigger": "preToolUse",
                                            "toolId": "execute_bash",
                                            "toolTags": "shell"
                                        }),
                                        vec!["tags:always"],
                                    ),
                                    (
                                        serde_json::json!({
                                            "trigger": "preToolUse",
                                            "toolId": "execute_bash",
                                            "toolTags": ["shell", 42]
                                        }),
                                        vec!["tags:always", "tags:shell"],
                                    ),
                                ];
                                for (params, expected) in cases {
                                    let response = callback_connection
                                        .send_request(UntypedMessage::new(
                                            "_kiro/hooks/list",
                                            params,
                                        )?)
                                        .block_task()
                                        .await?;
                                    let ids: Vec<&str> = response["hooks"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .map(|hook| hook["id"].as_str().unwrap())
                                        .collect();
                                    assert_eq!(ids, expected);
                                }
                                responder
                                    .respond(acp::InitializeResponse::new(request.protocol_version))
                            })?;
                            Ok(())
                        },
                        agent_client_protocol::on_receive_request!(),
                    );
                let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default())
                    .await
                    .unwrap();
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    runtime
                        .connection()
                        .send_request(acp::InitializeRequest::new(
                            agent_client_protocol::schema::ProtocolVersion::V1,
                        ))
                        .block_task(),
                )
                .await
                .expect("hook callback requests complete")
                .expect("hook callback boundary assertions pass");
                runtime.shutdown().await;
                host.abort();
                assert!(host.await.unwrap_err().is_cancelled());
            })
            .await;
    }
}

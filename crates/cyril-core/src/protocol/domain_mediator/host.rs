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
                callbacks::HostCallback::GetAccessToken { reply },
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

#[cfg(feature = "kas")]
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

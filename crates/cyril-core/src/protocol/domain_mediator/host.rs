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
                    trigger: params
                        .get("trigger")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
                    tool_id: params
                        .get("toolId")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned),
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

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Client, ConnectTo, ConnectionTo, UntypedMessage};
use tokio::sync::oneshot;

use crate::protocol::domain_mediator::{DomainChannels, DomainWork, HostWork};

#[cfg(feature = "kas")]
pub(crate) type ResolvedHostShell = Option<crate::protocol::kas::host_shell::HostShell>;
#[cfg(not(feature = "kas"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedHostShell;

/// Build the stable SDK2 client. One untyped notification handler decodes
/// each frame exactly once and retains unknown `session/update` variants
/// through the untyped fence; every handler only enqueues bounded domain work.
#[cfg(feature = "kas")]
async fn forward_host_response<T>(
    responder: agent_client_protocol::Responder<T>,
    response_rx: tokio::sync::oneshot::Receiver<agent_client_protocol::Result<T>>,
) -> agent_client_protocol::Result<()>
where
    T: agent_client_protocol::JsonRpcResponse,
{
    match response_rx.await {
        Ok(Ok(response)) => responder.respond(response),
        Ok(Err(error)) => responder.respond_with_error(error),
        Err(_) => responder.respond_with_error(
            agent_client_protocol::Error::internal_error()
                .data("host callback response unavailable"),
        ),
    }
}

/// The stable v1 `session/update` discriminator tags, consulted only AFTER a
/// typed decode fails: an unknown tag is retained through the untyped fence,
/// a known tag with an undecodable payload is a malformed frame worth a warn.
/// Routing never depends on this list — a frame that decodes is handled typed
/// regardless — so an SDK bump adding a variant flows through automatically;
/// `known_session_update_tags_match_the_schema` keeps the list itself honest.
pub(crate) const KNOWN_SESSION_UPDATE_TAGS: &[&str] = &[
    "user_message_chunk",
    "agent_message_chunk",
    "agent_thought_chunk",
    "tool_call",
    "tool_call_update",
    "plan",
    "available_commands_update",
    "current_mode_update",
    "config_option_update",
    "session_info_update",
    "usage_update",
];

pub(crate) fn is_unknown_session_update(message: &UntypedMessage) -> bool {
    if message.method() != "session/update"
        || message
            .params
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        return false;
    }
    message
        .params
        .get("update")
        .and_then(|update| update.get("sessionUpdate"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|tag| !KNOWN_SESSION_UPDATE_TAGS.contains(&tag))
}

pub(crate) async fn run_client(
    agent: impl ConnectTo<Client> + 'static,
    channels: DomainChannels,
    connection_tx: oneshot::Sender<ConnectionTo<agent_client_protocol::Agent>>,
    shutdown_rx: oneshot::Receiver<()>,
) -> agent_client_protocol::Result<()> {
    let notification_channels = channels.clone();
    let permission_channels = channels.clone();
    #[cfg(feature = "kas")]
    let read_channels = channels.clone();
    #[cfg(feature = "kas")]
    let write_channels = channels.clone();
    #[cfg(feature = "kas")]
    let create_terminal_channels = channels.clone();
    #[cfg(feature = "kas")]
    let wait_terminal_channels = channels.clone();
    #[cfg(feature = "kas")]
    let output_terminal_channels = channels.clone();
    #[cfg(feature = "kas")]
    let release_terminal_channels = channels.clone();
    #[cfg(feature = "kas")]
    let kill_terminal_channels = channels.clone();
    let builder = Client
        .builder()
        .name("cyril")
        .on_receive_notification(
            // One untyped handler decodes each frame exactly once: `session/update`
            // is borrow-decoded from `params` in place (no per-frame
            // `to_untyped_message` rebuild between chained handlers), unknown
            // tags are retained through the untyped fence, and a known tag
            // whose payload fails the typed decode is dropped with a warn —
            // JSON-RPC notifications have no reply target, so a log is the
            // only possible diagnostic for a malformed update.
            async move |message: UntypedMessage, _cx| {
                if notification_channels.is_transport_closed(&message) {
                    return notification_channels
                        .enqueue(DomainWork::TransportClosed)
                        .await;
                }
                if message.method() == "session/update" {
                    return match <acp::SessionNotification as serde::Deserialize>::deserialize(
                        &message.params,
                    ) {
                        Ok(notification) => {
                            let _ingress = notification_channels.enter_ingress();
                            notification_channels
                                .enqueue(DomainWork::Session(notification))
                                .await
                        }
                        Err(_) if is_unknown_session_update(&message) => {
                            let _ingress = notification_channels.enter_ingress();
                            notification_channels
                                .enqueue(DomainWork::UnknownSessionUpdate(message))
                                .await
                        }
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                "malformed standard session/update dropped"
                            );
                            Ok(())
                        }
                    };
                }
                if !message.method().starts_with('_') {
                    tracing::debug!(
                        method = message.method(),
                        "unknown standard notification ignored"
                    );
                    return Ok(());
                }
                let _ingress = notification_channels.enter_ingress();
                notification_channels
                    .enqueue(DomainWork::ExtensionNotification(message))
                    .await
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: acp::RequestPermissionRequest, responder, _cx| {
                permission_channels
                    .enqueue(DomainWork::Permission { request, responder })
                    .await
            },
            agent_client_protocol::on_receive_request!(),
        );

    #[cfg(feature = "kas")]
    let builder = builder
        .on_receive_request(
            async move |request: acp::ReadTextFileRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                read_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::ReadTextFile {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::WriteTextFileRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                write_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::WriteTextFile {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::CreateTerminalRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                create_terminal_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::CreateTerminal {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::WaitForTerminalExitRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                wait_terminal_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::WaitForTerminalExit {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::TerminalOutputRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                output_terminal_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::TerminalOutput {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::ReleaseTerminalRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                release_terminal_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::ReleaseTerminal {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request: acp::KillTerminalRequest, responder, cx| {
                let (reply, response_rx) = tokio::sync::oneshot::channel();
                kill_terminal_channels
                    .enqueue_host(HostWork::Callback(
                        crate::protocol::kas::callbacks::HostCallback::KillTerminal {
                            req: request,
                            reply,
                        },
                    ))
                    .await?;
                cx.spawn(forward_host_response(responder, response_rx))?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        );
    let ext_request_channels = channels;
    let builder = builder.on_receive_request(
        // A standard-namespace request reaching this catch-all has no typed
        // handler on THIS build (e.g. fs/terminal without `kas`): answer the
        // truthful -32601 so agents can capability-fall-back, never a false
        // "malformed" -32602 — the request was never parsed at all.
        async move |request: UntypedMessage, responder, _cx| {
            if !request.method().starts_with('_') {
                return Err(agent_client_protocol::Error::method_not_found());
            }
            ext_request_channels
                .enqueue_host(HostWork::ExtensionRequest { request, responder })
                .await
        },
        agent_client_protocol::on_receive_request!(),
    );
    builder
        .connect_with(agent, async move |connection| {
            if connection_tx.send(connection).is_err() {
                return Err(agent_client_protocol::Error::internal_error()
                    .data("SDK runtime handoff receiver dropped"));
            }
            shutdown_rx.await.map_err(|_| {
                agent_client_protocol::Error::internal_error()
                    .data("SDK runtime shutdown channel closed")
            })
        })
        .await
}

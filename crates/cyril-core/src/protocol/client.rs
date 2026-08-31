use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Client, ConnectTo, ConnectionTo, Handled, UntypedMessage};
use tokio::sync::oneshot;

use crate::protocol::domain_mediator::{DomainChannels, DomainWork, HostWork};

#[cfg(feature = "kas")]
pub(crate) type ResolvedHostShell = Option<crate::protocol::kas::host_shell::HostShell>;
#[cfg(not(feature = "kas"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedHostShell;

/// Build the stable SDK2 client. The untyped notification handler is first so
/// future `session/update` variants are retained rather than rejected by the
/// strict v1 decoder; every typed handler only enqueues bounded domain work.
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

pub(crate) fn malformed_session_update(message: &UntypedMessage) -> agent_client_protocol::Error {
    let detail =
        match <acp::SessionNotification as serde::Deserialize>::deserialize(&message.params) {
            Ok(_) => "typed session/update unexpectedly reached the untyped fallback".to_owned(),
            Err(error) => error.to_string(),
        };
    agent_client_protocol::Error::invalid_params()
        .data(format!("malformed standard session/update: {detail}"))
}
fn is_known_client_request(method: &str) -> bool {
    let methods = acp::CLIENT_METHOD_NAMES;
    [
        methods.session_request_permission,
        methods.fs_write_text_file,
        methods.fs_read_text_file,
        methods.terminal_create,
        methods.terminal_output,
        methods.terminal_release,
        methods.terminal_wait_for_exit,
        methods.terminal_kill,
    ]
    .contains(&method)
}

pub(crate) async fn run_client(
    agent: impl ConnectTo<Client> + 'static,
    channels: DomainChannels,
    connection_tx: oneshot::Sender<ConnectionTo<agent_client_protocol::Agent>>,
    shutdown_rx: oneshot::Receiver<()>,
) -> agent_client_protocol::Result<()> {
    let unknown_channels = channels.clone();
    let session_channels = channels.clone();
    let ext_notification_channels = channels.clone();
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
            async move |message: UntypedMessage, cx| {
                if unknown_channels.is_transport_closed(&message) {
                    unknown_channels
                        .enqueue(DomainWork::TransportClosed)
                        .await?;
                    return Ok(Handled::Yes);
                }
                if is_unknown_session_update(&message) {
                    let _ingress = unknown_channels.enter_ingress();
                    unknown_channels
                        .enqueue(DomainWork::UnknownSessionUpdate(message))
                        .await?;
                    return Ok(Handled::Yes);
                }
                Ok(Handled::No {
                    message: (message, cx),
                    retry: false,
                })
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |message: acp::SessionNotification, _cx| {
                let _ingress = session_channels.enter_ingress();
                session_channels.enqueue(DomainWork::Session(message)).await
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |message: UntypedMessage, _cx| {
                if message.method() == "session/update" {
                    return Err(malformed_session_update(&message));
                }
                if !message.method().starts_with('_') {
                    tracing::debug!(
                        method = message.method(),
                        "unknown standard notification ignored"
                    );
                    return Ok(());
                }
                let _ingress = ext_notification_channels.enter_ingress();
                ext_notification_channels
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
        async move |request: UntypedMessage, responder, _cx| {
            if !request.method().starts_with('_') {
                if is_known_client_request(request.method()) {
                    return Err(agent_client_protocol::Error::invalid_params()
                        .data(format!("malformed standard request: {}", request.method())));
                }
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

//! MCP operations send synchronously, await off-loop, and return correlated data.

use agent_client_protocol::{Agent, ConnectionTo};
#[cfg(feature = "kas")]
use std::time::Duration;

#[cfg(feature = "kas")]
use super::super::CommandOutcome;
use super::super::DomainMediator;
#[cfg(feature = "kas")]
use super::super::mcp::McpRequestPlan;
use crate::types::{
    McpErrorKind, McpOperationError, McpOutcome, McpRequest, McpUpdate, Notification,
    RoutedNotification,
};

/// KAS's response ceiling is about 60 seconds; allow transport scheduling
/// headroom without changing the generic command or session startup deadlines.
#[cfg(feature = "kas")]
pub(super) const RESET_TIMEOUT: Duration = Duration::from_secs(75);
#[cfg(feature = "kas")]
const READ_TIMEOUT: Duration = Duration::from_secs(65);

#[cfg(feature = "kas")]
pub(super) fn wire(request: &McpRequest) -> (&'static str, serde_json::Value, Duration) {
    use crate::types::McpOperation;
    match &request.operation {
        McpOperation::Reset { start_oauth } => {
            let params = if *start_oauth {
                serde_json::json!({"serverName": request.server_name, "startOAuth": true})
            } else {
                serde_json::json!({"serverName": request.server_name})
            };
            ("kiro/mcp/resetServer", params, RESET_TIMEOUT)
        }
        McpOperation::GetPrompt { name, arguments } => (
            "kiro/mcp/getPrompt",
            serde_json::json!({"serverName": request.server_name, "promptName": name, "arguments": arguments}),
            READ_TIMEOUT,
        ),
        McpOperation::GetResource { uri } => (
            "kiro/mcp/getResource",
            serde_json::json!({"serverName": request.server_name, "uri": uri}),
            READ_TIMEOUT,
        ),
    }
}

impl DomainMediator {
    pub(super) async fn mcp_command(
        &mut self,
        _connection: &ConnectionTo<Agent>,
        request: McpRequest,
    ) -> crate::Result<()> {
        if self.config.engine.kind() != crate::types::AgentEngine::Kas || !cfg!(feature = "kas") {
            let outcome = McpOutcome {
                id: 0,
                request,
                result: Err(McpOperationError {
                    kind: McpErrorKind::Unsupported,
                    message: "MCP management requires the bound KAS engine and KAS build support."
                        .into(),
                }),
            };
            return self.publish_mcp_outcome(outcome).await;
        }
        #[cfg(feature = "kas")]
        {
            let pending = match self.mcp.request(request) {
                McpRequestPlan::Pending(pending) => pending,
                McpRequestPlan::Immediate(outcome) => {
                    return self.publish_mcp_outcome(outcome).await;
                }
            };
            self.notify(RoutedNotification::scoped(
                pending.request.session_id.clone(),
                Notification::McpUpdate(McpUpdate::Pending(pending.clone())),
            ))
            .await?;
            let (method, params, timeout) = wire(&pending.request);
            let channels = self.channels.clone();
            let sent = super::extensions::send_extension(_connection, method, params);
            self.spawn_command(async move {
                let result = match sent {
                    Ok(sent) => match tokio::time::timeout(timeout, sent).await {
                        Ok(Ok(value)) => crate::protocol::convert::kas::mcp::result(&pending.request.operation, value),
                        Ok(Err(error)) => Err(rpc_error(error)),
                        Err(_) => Err(McpOperationError { kind: McpErrorKind::Timeout, message: format!("MCP request timed out after {} seconds. Check the server's current status before retrying.", timeout.as_secs()) }),
                    },
                    Err(error) => Err(McpOperationError { kind: McpErrorKind::Transport, message: format!("Could not send MCP request: {error}") }),
                };
                channels.enqueue_outcome(CommandOutcome::Mcp(McpOutcome { id: pending.id, request: pending.request, result })).await;
            });
        }
        Ok(())
    }

    pub(crate) async fn publish_mcp_outcome(&mut self, outcome: McpOutcome) -> crate::Result<()> {
        self.mcp.state.apply(&McpUpdate::Outcome(outcome.clone()));
        self.notify(RoutedNotification::scoped(
            outcome.request.session_id.clone(),
            Notification::McpUpdate(McpUpdate::Outcome(outcome)),
        ))
        .await
    }
}

#[cfg(feature = "kas")]
fn rpc_error(error: agent_client_protocol::Error) -> McpOperationError {
    let kind = if agent_client_protocol::is_incoming_transport_closed(&error) {
        McpErrorKind::Transport
    } else if error.code == agent_client_protocol::ErrorCode::MethodNotFound {
        McpErrorKind::Unsupported
    } else {
        // KAS sends management refusals as RPC errors, not a typed policy code.
        // Preserve the original actionable explanation; never guess from text.
        McpErrorKind::Rejected
    };
    McpOperationError {
        kind,
        message: format!("MCP request failed: {error}"),
    }
}

#[cfg(all(test, feature = "kas"))]
mod tests {
    use super::*;
    use crate::types::{McpOperation, SessionId};
    use serde_json::json;

    #[test]
    fn exact_management_payloads_omit_synthetic_session_and_preserve_string_arguments() {
        let request = |operation| McpRequest {
            session_id: SessionId::new("pinned-root"),
            server_name: "opaque / server".into(),
            operation,
        };
        let (method, payload, timeout) = wire(&request(McpOperation::Reset { start_oauth: false }));
        assert_eq!(method, "kiro/mcp/resetServer");
        assert_eq!(payload, json!({"serverName":"opaque / server"}));
        assert!(timeout > Duration::from_secs(60));
        assert_eq!(
            wire(&request(McpOperation::Reset { start_oauth: true })).1,
            json!({"serverName":"opaque / server","startOAuth":true})
        );
        let arguments = std::collections::BTreeMap::from([
            ("required".into(), "  \n{not:json}\t ".into()),
            ("empty".into(), String::new()),
        ]);
        assert_eq!(
            wire(&request(McpOperation::GetPrompt {
                name: "prompt name".into(),
                arguments
            }))
            .1,
            json!({"serverName":"opaque / server","promptName":"prompt name","arguments":{"required":"  \n{not:json}\t ","empty":""}})
        );
        assert_eq!(
            wire(&request(McpOperation::GetResource {
                uri: "canary:///opaque?x=%2F".into()
            }))
            .1,
            json!({"serverName":"opaque / server","uri":"canary:///opaque?x=%2F"})
        );
        assert_eq!(
            rpc_error(agent_client_protocol::Error::method_not_found()).kind,
            McpErrorKind::Unsupported
        );
        assert_eq!(
            rpc_error(agent_client_protocol::Error::internal_error().data(json!({
                "reason":agent_client_protocol::INCOMING_TRANSPORT_CLOSED_REASON,
                "method":"_kiro/mcp/getResource"
            })))
            .kind,
            McpErrorKind::Transport
        );
        let refused = rpc_error(
            agent_client_protocol::Error::internal_error().data("MCP governance disabled"),
        );
        assert_eq!(refused.kind, McpErrorKind::Rejected);
        assert!(refused.message.contains("MCP governance disabled"));
    }
}

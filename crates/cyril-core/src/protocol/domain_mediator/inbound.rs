use agent_client_protocol::{UntypedMessage, schema::v1 as acp};

#[cfg(feature = "kas")]
use super::HostWork;
use super::{DomainMediator, canonical_extension_method, now_std};
use crate::protocol::turn_mediator::Disposition;
use crate::types::{Notification, RoutedNotification, SessionId};

pub(super) fn session_id(notification: &acp::SessionNotification) -> SessionId {
    SessionId::new(notification.session_id.to_string())
}

impl DomainMediator {
    pub(super) async fn handle_routed(
        &mut self,
        routed: RoutedNotification,
    ) -> crate::Result<bool> {
        self.handle_routed_with_source_disposition(routed, None)
            .await
    }

    pub(super) async fn handle_routed_with_source_disposition(
        &mut self,
        routed: RoutedNotification,
        terminal_disposition: Option<crate::types::SourceTurnDisposition>,
    ) -> crate::Result<bool> {
        if routed.session_id.is_none()
            || self.turn_mediator.active_turn_session() == routed.session_id.as_ref()
        {
            self.turn_liveness.stamp(now_std());
        }
        let completed_turn = match self.turn_mediator.observe(&routed) {
            Disposition::Absorb { .. }
            | Disposition::DropStale { .. }
            | Disposition::DropUnowned => return Ok(false),
            Disposition::ForwardTurnComplete => true,
            Disposition::Forward => false,
        };
        if let Notification::TurnCompleted { stop_reason } = &routed.notification {
            self.source_observer.finish(
                terminal_disposition
                    .unwrap_or_else(|| crate::protocol::bridge::source_disposition(*stop_reason)),
            );
        }
        if completed_turn {
            self.turn_liveness.end();
        }
        self.notify(routed).await?;
        Ok(completed_turn)
    }

    pub(super) async fn handle_session(
        &mut self,
        args: acp::SessionNotification,
    ) -> crate::Result<bool> {
        match &args.update {
            acp::SessionUpdate::ToolCall(tool_call) => {
                tracing::info!(
                    id = %tool_call.tool_call_id,
                    title = %tool_call.title,
                    kind = ?tool_call.kind,
                    status = ?tool_call.status,
                    content_count = tool_call.content.len(),
                    locations_count = tool_call.locations.len(),
                    has_raw_input = tool_call.raw_input.is_some(),
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

        let session_id = session_id(&args);
        let (kind_present, status_present) = match &args.update {
            acp::SessionUpdate::ToolCall(_) => (true, true),
            acp::SessionUpdate::ToolCallUpdate(update) => {
                (update.fields.kind.is_some(), update.fields.status.is_some())
            }
            _ => (false, false),
        };
        let converted = self.config.engine.convert_session_update(&args);
        if let Some(
            Notification::ToolCallStarted(tool_call) | Notification::ToolCallUpdated(tool_call),
        ) = &converted
        {
            self.tool_call_ledger.merge(
                session_id.clone(),
                tool_call,
                kind_present,
                status_present,
            );
        }
        if let Some(notification) = converted {
            let routed = RoutedNotification::scoped(session_id, notification);
            self.source_observer.observe(&routed);
            self.handle_routed(routed).await
        } else {
            Ok(false)
        }
    }

    pub(super) async fn handle_extension_notification(
        &mut self,
        args: UntypedMessage,
    ) -> crate::Result<bool> {
        let method = canonical_extension_method(args.method());
        let params = &args.params;
        #[cfg(feature = "kas")]
        {
            use crate::protocol::engine::HooksAdapter;
            use crate::protocol::kas::callbacks::HostCallback;
            if method == crate::protocol::kas::hooks::CANCEL_METHOD {
                if self.config.engine.adapters().hooks != HooksAdapter::Inbound {
                    tracing::debug!("hooks/cancel dropped: no inbound hooks adapter");
                    return Ok(false);
                }
                if let Some(operation_id) = params
                    .get("operationId")
                    .and_then(serde_json::Value::as_str)
                {
                    self.channels
                        .enqueue_host(HostWork::Callback(HostCallback::HooksCancel {
                            operation_id: operation_id.to_owned(),
                        }))
                        .await
                        .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))?;
                }
                return Ok(false);
            }
            if method == crate::protocol::kas::hooks::DID_CHANGE_METHOD {
                if self.config.engine.adapters().hooks == HooksAdapter::None {
                    tracing::debug!("hooks/didChange dropped: engine has no hooks capability");
                    return Ok(false);
                }
                self.channels
                    .enqueue_host(HostWork::Callback(HostCallback::HooksDidChange {
                        hooks: crate::protocol::kas::hooks::parse_wire_hooks(params),
                    }))
                    .await
                    .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))?;
                return Ok(false);
            }
        }

        match self.config.engine.convert_ext_notification(method, params) {
            Ok(Some(notification)) => {
                let routed = match &notification {
                    Notification::ToolCallChunk {
                        session_id: Some(id),
                        ..
                    }
                    | Notification::MetadataUpdated {
                        session_id: Some(id),
                        ..
                    } => RoutedNotification::scoped(id.clone(), notification),
                    _ => RoutedNotification::global(notification),
                };
                self.source_observer.observe(&routed);
                self.handle_routed(routed).await
            }
            Ok(None) => Ok(false),
            Err(error) => {
                tracing::warn!(method, %error, "extension conversion failed");
                Ok(false)
            }
        }
    }
}

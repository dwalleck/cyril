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
}

impl KiroClient {
    pub fn new(
        notification_tx: mpsc::Sender<RoutedNotification>,
        permission_tx: mpsc::Sender<PermissionRequest>,
    ) -> Self {
        Self {
            notification_tx,
            permission_tx,
            tool_call_inputs: RefCell::new(HashMap::new()),
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

        let (responder_tx, responder_rx) = tokio::sync::oneshot::channel();

        let request = PermissionRequest {
            tool_call,
            message,
            options,
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
            convert::session_update_to_notification(&args, &inputs)
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

        match convert::to_ext_notification(args.method.as_ref(), &params) {
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
}

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
    notification_tx: mpsc::Sender<Notification>,
    permission_tx: mpsc::Sender<PermissionRequest>,
    tool_call_inputs: RefCell<HashMap<String, serde_json::Value>>,
}

impl KiroClient {
    pub fn new(
        notification_tx: mpsc::Sender<Notification>,
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

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<()> {
        convert::cache_tool_call_input(&args, &self.tool_call_inputs);

        if let Some(notification) =
            convert::session_update_to_notification(&args, &self.tool_call_inputs.borrow())
        {
            self.notification_tx
                .send(notification)
                .await
                .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
        }

        Ok(())
    }

    async fn ext_notification(
        &self,
        args: acp::ExtNotification,
    ) -> acp::Result<()> {
        let params: serde_json::Value = serde_json::from_str(args.params.get())
            .unwrap_or_else(|_| serde_json::Value::Null);

        match convert::to_ext_notification(args.method.as_ref(), &params) {
            Ok(notification) => {
                self.notification_tx
                    .send(notification)
                    .await
                    .map_err(|_| acp::Error::new(-32603, "bridge closed"))?;
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    method = %args.method,
                    "unrecognized extension notification"
                );
            }
        }
        Ok(())
    }
}

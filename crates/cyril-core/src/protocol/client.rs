use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::event::{AppEvent, ExtensionEvent, InteractionRequest, ProtocolEvent};
use crate::kiro_ext::KiroCommandsPayload;

/// Construct an ACP internal error with a message.
fn internal_err(msg: impl Into<String>) -> acp::Error {
    acp::Error::new(-32603, msg)
}

/// The central ACP Client implementation.
/// Handles agent callbacks: permissions, notifications, and extensions.
///
/// Uses `Rc<RefCell<_>>` for interior mutability since everything is `!Send`
/// (required by `#[async_trait(?Send)]` on the ACP `Client` trait).
pub struct KiroClient {
    event_tx: mpsc::UnboundedSender<AppEvent>,
    /// Cache of `raw_input` from ToolCall/ToolCallUpdate notifications, keyed by tool call ID.
    /// Permission requests arrive without `raw_input`, so we look it up here to enrich them.
    tool_call_inputs: RefCell<HashMap<acp::ToolCallId, serde_json::Value>>,
}

impl KiroClient {
    pub fn new(
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Rc<Self> {
        Rc::new(Self {
            event_tx,
            tool_call_inputs: RefCell::new(HashMap::new()),
        })
    }

    /// Send an event to the TUI, logging if the receiver has been dropped.
    fn emit(&self, event: AppEvent) {
        if self.event_tx.send(event).is_err() {
            tracing::error!("Event channel closed — TUI receiver is gone, events are being dropped");
        }
    }
}

#[async_trait(?Send)]
impl acp::Client for KiroClient {
    async fn request_permission(
        &self,
        mut args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        // Permission requests arrive without raw_input — enrich from our cache
        // so the approval UI can display details like URLs and commands.
        if args.tool_call.fields.raw_input.is_none() {
            if let Some(cached) = self.tool_call_inputs.borrow().get(&args.tool_call.tool_call_id) {
                args.tool_call.fields.raw_input = Some(cached.clone());
            }
        }

        let (tx, rx) = oneshot::channel();
        self.emit(AppEvent::Interaction(InteractionRequest::Permission {
            request: args,
            responder: tx,
        }));

        rx.await.map_err(|_| internal_err("Permission request channel closed"))
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<()> {
        match args.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::AgentMessage {
                    session_id: args.session_id,
                    chunk,
                }));
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::AgentThought {
                    session_id: args.session_id,
                    chunk,
                }));
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                if let Some(ref raw_input) = tool_call.raw_input {
                    self.tool_call_inputs
                        .borrow_mut()
                        .insert(tool_call.tool_call_id.clone(), raw_input.clone());
                }
                self.emit(AppEvent::Protocol(ProtocolEvent::ToolCallStarted {
                    session_id: args.session_id,
                    tool_call,
                }));
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                if let Some(ref raw_input) = update.fields.raw_input {
                    self.tool_call_inputs
                        .borrow_mut()
                        .insert(update.tool_call_id.clone(), raw_input.clone());
                }
                self.emit(AppEvent::Protocol(ProtocolEvent::ToolCallUpdated {
                    session_id: args.session_id,
                    update,
                }));
            }
            acp::SessionUpdate::Plan(plan) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::PlanUpdated {
                    session_id: args.session_id,
                    plan,
                }));
            }
            acp::SessionUpdate::AvailableCommandsUpdate(commands) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::CommandsUpdated {
                    session_id: args.session_id,
                    commands,
                }));
            }
            acp::SessionUpdate::CurrentModeUpdate(mode) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::ModeChanged {
                    session_id: args.session_id,
                    mode,
                }));
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                self.emit(AppEvent::Protocol(ProtocolEvent::ConfigOptionsUpdated {
                    session_id: args.session_id,
                    config_options: update.config_options,
                }));
            }
            _ => {
                tracing::debug!("Unhandled session notification variant");
            }
        }
        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        tracing::info!("Received ext_notification: method={}", args.method);
        tracing::info!("ext_notification params: {}", args.params);

        if args.method.as_ref() == "kiro.dev/commands/available" {
            match serde_json::from_str::<KiroCommandsPayload>(args.params.get()) {
                Ok(payload) => {
                    let commands = payload.commands();
                    tracing::info!(
                        "Parsed {} Kiro commands from ext_notification",
                        commands.len()
                    );
                    self.emit(AppEvent::Extension(ExtensionEvent::KiroCommandsAvailable { commands }));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/commands/available: {e}");
                }
            }
        } else if args.method.as_ref() == "kiro.dev/metadata" {
            // Log the full raw payload so we can discover all available fields
            tracing::info!("kiro.dev/metadata raw: {}", args.params.get());

            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct MetadataPayload {
                session_id: String,
                #[serde(default)]
                context_usage_percentage: f64,
            }
            match serde_json::from_str::<MetadataPayload>(args.params.get()) {
                Ok(payload) => {
                    self.emit(AppEvent::Extension(ExtensionEvent::KiroMetadata {
                        session_id: payload.session_id,
                        context_usage_pct: payload.context_usage_percentage,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/metadata: {e}");
                }
            }
        } else {
            tracing::warn!("Unrecognized ext_notification: method={}", args.method);
            self.emit(AppEvent::Extension(ExtensionEvent::Unknown {
                method: args.method.to_string(),
                params: args.params.get().to_string(),
            }));
        }

        Ok(())
    }
}

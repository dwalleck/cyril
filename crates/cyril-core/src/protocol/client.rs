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
        tracing::info!(
            "RequestPermission: tool={} id={} options={:?}",
            args.tool_call.fields.title.as_deref().unwrap_or("?"),
            args.tool_call.tool_call_id,
            args.options.iter().map(|o| o.option_id.to_string()).collect::<Vec<_>>()
        );
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
                if let acp::ContentBlock::Text(ref text) = chunk.content {
                    tracing::info!(
                        "AgentMessageChunk: {} chars",
                        text.text.len()
                    );
                } else {
                    tracing::info!(
                        "AgentMessageChunk: non-text content block"
                    );
                }
                self.emit(AppEvent::Protocol(ProtocolEvent::AgentMessage {
                    session_id: args.session_id,
                    chunk,
                }));
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                if let acp::ContentBlock::Text(ref text) = chunk.content {
                    tracing::info!(
                        "AgentThoughtChunk: {} chars",
                        text.text.len()
                    );
                } else {
                    tracing::info!(
                        "AgentThoughtChunk: non-text content block"
                    );
                }
                self.emit(AppEvent::Protocol(ProtocolEvent::AgentThought {
                    session_id: args.session_id,
                    chunk,
                }));
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                tracing::info!(
                    "ToolCall: id={} title={} kind={:?} status={:?} raw_input={:?}",
                    tool_call.tool_call_id,
                    tool_call.title,
                    tool_call.kind,
                    tool_call.status,
                    tool_call.raw_input
                );
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
                tracing::info!(
                    "ToolCallUpdate: id={} title={:?} kind={:?} status={:?}",
                    update.tool_call_id,
                    update.fields.title,
                    update.fields.kind,
                    update.fields.status,
                );
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
                tracing::info!(
                    "Plan: {} entries (completed={}, in_progress={}, pending={})",
                    plan.entries.len(),
                    plan.entries.iter().filter(|e| e.status == acp::PlanEntryStatus::Completed).count(),
                    plan.entries.iter().filter(|e| e.status == acp::PlanEntryStatus::InProgress).count(),
                    plan.entries.iter().filter(|e| e.status == acp::PlanEntryStatus::Pending).count(),
                );
                self.emit(AppEvent::Protocol(ProtocolEvent::PlanUpdated {
                    session_id: args.session_id,
                    plan,
                }));
            }
            acp::SessionUpdate::AvailableCommandsUpdate(commands) => {
                tracing::info!(
                    "AvailableCommandsUpdate: {} commands",
                    commands.available_commands.len()
                );
                self.emit(AppEvent::Protocol(ProtocolEvent::CommandsUpdated {
                    session_id: args.session_id,
                    commands,
                }));
            }
            acp::SessionUpdate::CurrentModeUpdate(mode) => {
                tracing::info!(
                    "CurrentModeUpdate: mode={}",
                    mode.current_mode_id
                );
                self.emit(AppEvent::Protocol(ProtocolEvent::ModeChanged {
                    session_id: args.session_id,
                    mode,
                }));
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                tracing::info!(
                    "ConfigOptionUpdate: {} options",
                    update.config_options.len()
                );
                self.emit(AppEvent::Protocol(ProtocolEvent::ConfigOptionsUpdated {
                    session_id: args.session_id,
                    config_options: update.config_options,
                }));
            }
            _ => {
                tracing::warn!(
                    session_id = %args.session_id,
                    "Unhandled session update: {}",
                    serde_json::to_string(&args.update).unwrap_or_else(|_| "(serialize error)".into())
                );
            }
        }
        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        tracing::info!("ext_notification: method={} params={}", args.method, args.params);

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
        } else if args.method.as_ref() == "kiro.dev/agent/switched" {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct AgentSwitchedPayload {
                agent_name: String,
                #[serde(default)]
                previous_agent_name: String,
                welcome_message: Option<String>,
            }
            match serde_json::from_str::<AgentSwitchedPayload>(args.params.get()) {
                Ok(payload) => {
                    tracing::info!(
                        "Agent switched: {} -> {}",
                        payload.previous_agent_name,
                        payload.agent_name
                    );
                    self.emit(AppEvent::Extension(ExtensionEvent::AgentSwitched {
                        agent_name: payload.agent_name,
                        previous_agent_name: payload.previous_agent_name,
                        welcome_message: payload.welcome_message,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/agent/switched: {e}");
                }
            }
        } else if args.method.as_ref() == "kiro.dev/session/update" {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct KiroSessionUpdate {
                update: KiroUpdateInner,
            }
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct KiroUpdateInner {
                session_update: String,
                #[serde(default)]
                tool_call_id: String,
                #[serde(default)]
                title: String,
                #[serde(default)]
                kind: String,
            }
            match serde_json::from_str::<KiroSessionUpdate>(args.params.get()) {
                Ok(payload) if payload.update.session_update == "tool_call_chunk" => {
                    self.emit(AppEvent::Extension(ExtensionEvent::ToolCallChunk {
                        tool_call_id: payload.update.tool_call_id,
                        title: payload.update.title,
                        kind: payload.update.kind,
                    }));
                }
                Ok(payload) => {
                    tracing::warn!(
                        "Unhandled kiro.dev/session/update variant: {} (raw: {})",
                        payload.update.session_update,
                        args.params.get()
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/session/update: {e}");
                }
            }
        } else if args.method.as_ref() == "kiro.dev/compaction/status" {
            #[derive(serde::Deserialize)]
            struct StatusPayload {
                #[serde(default)]
                message: String,
            }
            match serde_json::from_str::<StatusPayload>(args.params.get()) {
                Ok(payload) => {
                    self.emit(AppEvent::Extension(ExtensionEvent::CompactionStatus {
                        message: payload.message,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/compaction/status: {e}");
                }
            }
        } else if args.method.as_ref() == "kiro.dev/clear/status" {
            #[derive(serde::Deserialize)]
            struct StatusPayload {
                #[serde(default)]
                message: String,
            }
            match serde_json::from_str::<StatusPayload>(args.params.get()) {
                Ok(payload) => {
                    self.emit(AppEvent::Extension(ExtensionEvent::ClearStatus {
                        message: payload.message,
                    }));
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/clear/status: {e}");
                }
            }
        } else {
            tracing::warn!(
                "Unrecognized ext_notification: method={} params={}",
                args.method,
                args.params.get()
            );
            self.emit(AppEvent::Extension(ExtensionEvent::Unknown {
                method: args.method.to_string(),
                params: args.params.get().to_string(),
            }));
        }

        Ok(())
    }
}

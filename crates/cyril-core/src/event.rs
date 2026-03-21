use agent_client_protocol as acp;
use tokio::sync::oneshot;

use crate::kiro_ext::KiroExtCommand;

/// Protocol-level events from ACP session notifications.
#[derive(Debug)]
pub enum ProtocolEvent {
    AgentMessage {
        session_id: acp::SessionId,
        chunk: acp::ContentChunk,
    },
    AgentThought {
        session_id: acp::SessionId,
        chunk: acp::ContentChunk,
    },
    ToolCallStarted {
        session_id: acp::SessionId,
        tool_call: acp::ToolCall,
    },
    ToolCallUpdated {
        session_id: acp::SessionId,
        update: acp::ToolCallUpdate,
    },
    PlanUpdated {
        session_id: acp::SessionId,
        plan: acp::Plan,
    },
    ModeChanged {
        session_id: acp::SessionId,
        mode: acp::CurrentModeUpdate,
    },
    ConfigOptionsUpdated {
        session_id: acp::SessionId,
        config_options: Vec<acp::SessionConfigOption>,
    },
    CommandsUpdated {
        session_id: acp::SessionId,
        commands: acp::AvailableCommandsUpdate,
    },
}

/// Requests from the agent that need a user response.
#[derive(Debug)]
pub enum InteractionRequest {
    Permission {
        request: acp::RequestPermissionRequest,
        responder: oneshot::Sender<acp::RequestPermissionResponse>,
    },
}

/// Kiro-specific extension events via ext_notification.
#[derive(Debug)]
pub enum ExtensionEvent {
    KiroCommandsAvailable {
        commands: Vec<KiroExtCommand>,
    },
    KiroMetadata {
        session_id: String,
        context_usage_pct: f64,
    },
    /// The agent was switched (e.g. via /agent picker).
    AgentSwitched {
        agent_name: String,
        previous_agent_name: String,
        welcome_message: Option<String>,
    },
    /// Lightweight tool call progress from kiro.dev/session/update.
    ToolCallChunk {
        tool_call_id: String,
        title: String,
        kind: String,
    },
    /// Compaction progress from kiro.dev/compaction/status.
    CompactionStatus {
        message: String,
    },
    /// Clear progress from kiro.dev/clear/status.
    ClearStatus {
        message: String,
    },
    /// An extension notification we don't have a specific handler for.
    Unknown {
        method: String,
        params: String,
    },
}

/// Top-level event sent from KiroClient to the TUI.
#[derive(Debug)]
pub enum AppEvent {
    Protocol(ProtocolEvent),
    Interaction(InteractionRequest),
    Extension(ExtensionEvent),
}

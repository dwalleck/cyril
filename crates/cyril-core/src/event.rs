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
}

/// Internal application events (not from the agent).
#[derive(Debug)]
pub enum InternalEvent {
    HookFeedback { text: String },
}

/// Top-level event sent from KiroClient to the TUI.
#[derive(Debug)]
pub enum AppEvent {
    Protocol(ProtocolEvent),
    Interaction(InteractionRequest),
    Extension(ExtensionEvent),
    Internal(InternalEvent),
}

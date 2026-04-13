use crate::types::command::{CommandInfo, ConfigOption};
use crate::types::message::{AgentMessage, AgentThought};
use crate::types::plan::Plan;
use crate::types::session::{ContextUsage, SessionId, StopReason, TokenCounts, TurnMetering};
use crate::types::tool_call::{ToolCall, ToolCallId};

/// Notifications emitted by the ACP bridge. All variants are Send + Sync + Clone.
/// This is the primary channel for agent state updates crossing from the bridge thread into the App.
#[derive(Debug, Clone)]
pub enum Notification {
    // Agent output
    AgentMessage(AgentMessage),
    AgentThought(AgentThought),

    // Tool lifecycle
    ToolCallStarted(ToolCall),
    ToolCallUpdated(ToolCall),

    // Session state
    PlanUpdated(Plan),
    ModeChanged {
        mode_id: String,
    },
    ConfigOptionsUpdated(Vec<ConfigOption>),
    CommandsUpdated {
        commands: Vec<CommandInfo>,
        prompts: Vec<crate::types::PromptInfo>,
    },

    // Command system
    CommandOptionsReceived {
        command: String,
        options: Vec<crate::types::command::CommandOption>,
    },

    /// Response from executing an agent command via kiro.dev/commands/execute.
    CommandExecuted {
        command: String,
        response: serde_json::Value,
    },

    // Kiro extensions
    MetadataUpdated {
        context_usage: ContextUsage,
        metering: Option<TurnMetering>,
        tokens: Option<TokenCounts>,
    },
    AgentSwitched {
        name: String,
        welcome: Option<String>,
        previous_agent: Option<String>,
        model: Option<String>,
    },
    CompactionStatus {
        message: String,
    },
    ClearStatus {
        message: String,
    },
    RateLimited {
        message: String,
    },
    ToolCallChunk {
        tool_call_id: ToolCallId,
        title: String,
        kind: String,
        /// Session ID from the outer `kiro.dev/session/update` envelope.
        /// Used by the bridge → client pathway to convert into a
        /// `RoutedNotification` at the channel boundary. By the time this
        /// notification reaches state machines, routing has already happened
        /// and this field is effectively just a tag.
        session_id: Option<SessionId>,
    },
    McpServerInitFailure {
        server_name: String,
        error: Option<String>,
    },
    McpOAuthRequest {
        server_name: String,
        url: String,
    },
    McpServerInitialized {
        server_name: String,
    },
    AgentNotFound {
        requested: String,
        fallback: Option<String>,
    },
    AgentConfigError {
        path: String,
        error: String,
    },
    ModelNotFound {
        requested: String,
        fallback: Option<String>,
    },

    // Subagent lifecycle (kiro.dev/subagent/*)
    SubagentListUpdated {
        subagents: Vec<crate::types::SubagentInfo>,
        pending_stages: Vec<crate::types::PendingStage>,
    },
    InboxNotification {
        session_id: SessionId,
        message_count: u32,
        escalation_count: u32,
        senders: Vec<String>,
    },
    /// A subagent was spawned successfully via `session/spawn`.
    SubagentSpawned {
        session_id: SessionId,
        name: String,
    },
    /// A subagent was terminated successfully via `session/terminate`.
    SubagentTerminated {
        session_id: SessionId,
    },
    /// A bridge command failed. Surfaces to the UI as a system message.
    BridgeError {
        operation: String,
        message: String,
    },

    // Lifecycle
    SessionCreated {
        session_id: SessionId,
        current_mode: Option<String>,
        current_model: Option<String>,
    },
    TurnCompleted {
        stop_reason: StopReason,
    },
    BridgeDisconnected {
        reason: String,
    },
}

/// A notification paired with its source session ID for routing.
///
/// `session_id == None` means the notification is **global** — bridge
/// lifecycle events (`BridgeDisconnected`, `SubagentSpawned`, etc.),
/// `SubagentListUpdated`, or anything else that doesn't belong to a specific
/// ACP session. The App routes `None` through its main pipeline because
/// there's nothing to compare against.
///
/// `session_id == Some(id)` means the notification originated from a
/// specific session (every standard ACP `SessionNotification` is wrapped
/// this way with the envelope's `session_id`). The App compares `id` against
/// its known main session: if it matches, dispatches to the main state
/// machines; otherwise routes to `SubagentUiState`.
///
/// This is the primary channel type from the bridge to the App.
#[derive(Debug, Clone)]
pub struct RoutedNotification {
    pub session_id: Option<SessionId>,
    pub notification: Notification,
}

impl RoutedNotification {
    /// Create a routed notification with no session scope (global or main).
    pub fn global(notification: Notification) -> Self {
        Self {
            session_id: None,
            notification,
        }
    }

    /// Create a routed notification scoped to a specific session.
    pub fn scoped(session_id: SessionId, notification: Notification) -> Self {
        Self {
            session_id: Some(session_id),
            notification,
        }
    }
}

impl From<Notification> for RoutedNotification {
    fn from(notification: Notification) -> Self {
        Self::global(notification)
    }
}

/// A request from the agent that needs user approval.
/// NOT Clone — owns a oneshot sender for the response.
#[derive(Debug)]
pub struct PermissionRequest {
    pub tool_call: ToolCall,
    pub message: String,
    pub options: Vec<PermissionOption>,
    pub responder: tokio::sync::oneshot::Sender<PermissionResponse>,
}

/// An option in a permission request dialog.
#[derive(Debug, Clone)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    pub is_destructive: bool,
}

/// The user's response to a permission request.
#[derive(Debug, Clone)]
pub enum PermissionResponse {
    AllowOnce,
    AllowAlways,
    Reject,
    Cancel,
}

/// Commands sent from the App to the ACP bridge.
#[derive(Debug)]
pub enum BridgeCommand {
    SendPrompt {
        session_id: SessionId,
        content_blocks: Vec<String>,
    },
    NewSession {
        cwd: std::path::PathBuf,
    },
    LoadSession {
        session_id: SessionId,
    },
    CancelRequest,
    SetMode {
        mode_id: String,
    },
    ExtMethod {
        method: String,
        params: serde_json::Value,
    },
    QueryCommandOptions {
        command: String,
        session_id: SessionId,
    },
    /// Execute an agent command and emit the response as a notification.
    ExecuteCommand {
        command: String,
        session_id: SessionId,
        args: serde_json::Value,
    },
    // Subagent session control
    SpawnSession {
        task: String,
        name: String,
    },
    TerminateSession {
        session_id: SessionId,
    },
    SendMessage {
        session_id: SessionId,
        content: String,
    },
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_call::{ToolCallId, ToolCallStatus, ToolKind};

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_clone<T: Clone>() {}

    #[test]
    fn notification_is_send_sync_clone() {
        assert_send::<Notification>();
        assert_sync::<Notification>();
        assert_clone::<Notification>();
    }

    #[test]
    fn permission_request_is_send() {
        assert_send::<PermissionRequest>();
    }

    #[test]
    fn bridge_command_is_send() {
        assert_send::<BridgeCommand>();
    }

    #[test]
    fn notification_agent_message() {
        let n = Notification::AgentMessage(AgentMessage {
            text: "hello".into(),
            is_streaming: true,
        });
        assert!(matches!(n, Notification::AgentMessage(_)));
    }

    #[test]
    fn notification_tool_call_started() {
        let tc = ToolCall::new(
            ToolCallId::new("tc_1"),
            "read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let n = Notification::ToolCallStarted(tc);
        assert!(matches!(n, Notification::ToolCallStarted(_)));
    }

    #[test]
    fn notification_metadata_updated() {
        let n = Notification::MetadataUpdated {
            context_usage: ContextUsage::new(75.0),
            metering: None,
            tokens: None,
        };
        if let Notification::MetadataUpdated {
            context_usage,
            metering,
            tokens,
        } = n
        {
            assert!((context_usage.percentage() - 75.0).abs() < f64::EPSILON);
            assert!(metering.is_none());
            assert!(tokens.is_none());
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn notification_turn_completed() {
        let n = Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        };
        assert!(matches!(n, Notification::TurnCompleted { .. }));
    }

    #[test]
    fn notification_bridge_disconnected() {
        let n = Notification::BridgeDisconnected {
            reason: "process exited".into(),
        };
        assert!(matches!(n, Notification::BridgeDisconnected { .. }));
    }

    #[test]
    fn notification_clone() {
        let n = Notification::AgentMessage(AgentMessage {
            text: "test".into(),
            is_streaming: false,
        });
        let n2 = n.clone();
        assert!(matches!(n2, Notification::AgentMessage(_)));
    }

    #[test]
    fn permission_option_fields() {
        let opt = PermissionOption {
            id: "allow_once".into(),
            label: "Allow Once".into(),
            is_destructive: false,
        };
        assert_eq!(opt.id, "allow_once");
        assert!(!opt.is_destructive);
    }

    #[test]
    fn permission_response_variants() {
        let responses = [
            PermissionResponse::AllowOnce,
            PermissionResponse::AllowAlways,
            PermissionResponse::Reject,
            PermissionResponse::Cancel,
        ];
        assert_eq!(responses.len(), 4);
    }

    #[test]
    fn bridge_command_send_prompt() {
        let cmd = BridgeCommand::SendPrompt {
            session_id: SessionId::new("sess_1"),
            content_blocks: vec!["hello".into()],
        };
        assert!(matches!(cmd, BridgeCommand::SendPrompt { .. }));
    }

    #[test]
    fn bridge_command_new_session() {
        let cmd = BridgeCommand::NewSession {
            cwd: std::path::PathBuf::from("/tmp"),
        };
        assert!(matches!(cmd, BridgeCommand::NewSession { .. }));
    }

    #[test]
    fn bridge_command_shutdown() {
        let cmd = BridgeCommand::Shutdown;
        assert!(matches!(cmd, BridgeCommand::Shutdown));
    }

    #[test]
    fn bridge_command_query_command_options() {
        let cmd = BridgeCommand::QueryCommandOptions {
            command: "model".into(),
            session_id: SessionId::new("sess_1"),
        };
        assert!(matches!(cmd, BridgeCommand::QueryCommandOptions { .. }));
    }

    #[test]
    fn notification_command_options_received() {
        let n = Notification::CommandOptionsReceived {
            command: "model".into(),
            options: vec![crate::types::command::CommandOption {
                label: "Claude Sonnet".into(),
                value: "claude-sonnet".into(),
                description: None,
                group: None,
                is_current: true,
            }],
        };
        if let Notification::CommandOptionsReceived { command, options } = n {
            assert_eq!(command, "model");
            assert_eq!(options.len(), 1);
            assert!(options[0].is_current);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn notification_command_options_received_is_clone() {
        let n = Notification::CommandOptionsReceived {
            command: "model".into(),
            options: vec![],
        };
        let n2 = n.clone();
        assert!(matches!(n2, Notification::CommandOptionsReceived { .. }));
    }

    #[test]
    fn notification_command_executed() {
        let n = Notification::CommandExecuted {
            command: "tools".into(),
            response: serde_json::json!({"success": true, "message": "OK"}),
        };
        if let Notification::CommandExecuted { command, response } = n {
            assert_eq!(command, "tools");
            assert_eq!(response["success"], true);
            assert_eq!(response["message"], "OK");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn notification_command_executed_is_clone() {
        let n = Notification::CommandExecuted {
            command: "context".into(),
            response: serde_json::json!({"success": true}),
        };
        let n2 = n.clone();
        assert!(matches!(n2, Notification::CommandExecuted { .. }));
    }

    #[test]
    fn subagent_notification_is_send_sync_clone() {
        assert_send::<crate::types::SubagentInfo>();
        assert_sync::<crate::types::SubagentInfo>();
        assert_clone::<crate::types::SubagentInfo>();
        assert_send::<crate::types::PendingStage>();
        assert_sync::<crate::types::PendingStage>();
        assert_clone::<crate::types::PendingStage>();
    }

    #[test]
    fn routed_notification_global_has_no_session_id() {
        let routed = RoutedNotification::global(Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(routed.session_id.is_none());
        assert!(matches!(
            routed.notification,
            Notification::TurnCompleted { .. }
        ));
    }

    #[test]
    fn routed_notification_scoped_preserves_session_id() {
        let routed = RoutedNotification::scoped(
            SessionId::new("sub-1"),
            Notification::AgentMessage(AgentMessage {
                text: "hello".into(),
                is_streaming: true,
            }),
        );
        assert_eq!(
            routed.session_id.as_ref().map(|s| s.as_str()),
            Some("sub-1")
        );
    }

    #[test]
    fn notification_into_routed_is_global() {
        let routed: RoutedNotification = Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        }
        .into();
        assert!(routed.session_id.is_none());
    }

    #[test]
    fn subagent_list_updated_variant() {
        let n = Notification::SubagentListUpdated {
            subagents: vec![],
            pending_stages: vec![],
        };
        assert!(matches!(n, Notification::SubagentListUpdated { .. }));
    }

    #[test]
    fn inbox_notification_variant() {
        let n = Notification::InboxNotification {
            session_id: SessionId::new("main"),
            message_count: 2,
            escalation_count: 0,
            senders: vec!["subagent".into()],
        };
        if let Notification::InboxNotification {
            message_count,
            escalation_count,
            ..
        } = n
        {
            assert_eq!(message_count, 2);
            assert_eq!(escalation_count, 0);
        }
    }

    #[test]
    fn routed_notification_clone_preserves_session_id() {
        let original = RoutedNotification::scoped(
            SessionId::new("sub-1"),
            Notification::AgentMessage(AgentMessage {
                text: "hello from subagent".into(),
                is_streaming: true,
            }),
        );
        let cloned = original.clone();
        assert_eq!(
            cloned.session_id.as_ref().map(|s| s.as_str()),
            Some("sub-1")
        );
        if let Notification::AgentMessage(msg) = cloned.notification {
            assert_eq!(msg.text, "hello from subagent");
        } else {
            panic!("inner should be AgentMessage");
        }
    }

    #[test]
    fn bridge_command_execute_command() {
        let cmd = BridgeCommand::ExecuteCommand {
            command: "compact".into(),
            session_id: SessionId::new("sess_1"),
            args: serde_json::json!({}),
        };
        assert!(matches!(cmd, BridgeCommand::ExecuteCommand { .. }));
    }
}

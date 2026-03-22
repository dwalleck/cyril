use crate::types::command::{CommandInfo, ConfigOption};
use crate::types::message::{AgentMessage, AgentThought};
use crate::types::plan::Plan;
use crate::types::session::{ContextUsage, SessionId};
use crate::types::tool_call::ToolCall;

/// Notifications emitted by the ACP bridge. All variants are Send + Sync + Clone.
/// This is the only way protocol state crosses into the Send world.
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
    CommandsUpdated(Vec<CommandInfo>),

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
    ContextUsageUpdated(ContextUsage),
    AgentSwitched {
        name: String,
        welcome: Option<String>,
    },
    CompactionStatus {
        message: String,
    },
    ClearStatus {
        message: String,
    },
    ToolCallChunk {
        tool_call_id: String,
        title: String,
        kind: String,
    },

    // Lifecycle
    SessionCreated {
        session_id: SessionId,
        current_mode: Option<String>,
    },
    TurnCompleted,
    BridgeDisconnected {
        reason: String,
    },
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
        session_id: String,
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
            None,
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        let n = Notification::ToolCallStarted(tc);
        assert!(matches!(n, Notification::ToolCallStarted(_)));
    }

    #[test]
    fn notification_context_usage() {
        let n = Notification::ContextUsageUpdated(ContextUsage::new(75.0));
        if let Notification::ContextUsageUpdated(usage) = n {
            assert!((usage.percentage() - 75.0).abs() < f64::EPSILON);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn notification_turn_completed() {
        let n = Notification::TurnCompleted;
        assert!(matches!(n, Notification::TurnCompleted));
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
    fn bridge_command_execute_command() {
        let cmd = BridgeCommand::ExecuteCommand {
            command: "compact".into(),
            session_id: SessionId::new("sess_1"),
            args: serde_json::json!({}),
        };
        assert!(matches!(cmd, BridgeCommand::ExecuteCommand { .. }));
    }
}

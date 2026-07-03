use crate::types::command::{CommandInfo, ConfigOption};
use crate::types::message::{AgentMessage, AgentThought, UserMessage};
use crate::types::plan::Plan;
use crate::types::session::{
    CompactionPhase, ContextBreakdown, ContextUsage, EffortLevel, ModeId, ModelInfo, SessionId,
    SessionMode, StopReason, TokenCounts, TurnMetering,
};
use crate::types::tool_call::{ToolCall, ToolCallId};

/// Notifications emitted by the ACP bridge. All variants are Send + Sync + Clone.
/// This is the primary channel for agent state updates crossing from the bridge thread into the App.
#[derive(Debug, Clone)]
pub enum Notification {
    // Agent output
    AgentMessage(AgentMessage),
    AgentThought(AgentThought),

    // User messages (replayed by the agent during session/load history replay)
    UserMessage(UserMessage),

    // Tool lifecycle
    ToolCallStarted(ToolCall),
    ToolCallUpdated(ToolCall),

    // Session state
    PlanUpdated(Plan),
    ModeChanged {
        mode_id: ModeId,
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

    /// Response from `kiro.dev/settings/list`. Carries Kiro's on-disk
    /// settings snapshot as a flat dotted-key map (`chat.enableThinking`,
    /// `introspect.progressiveMode`, etc.) with optional sub-object
    /// nesting (`chat: {...}` alongside `chat.enableNotifications: true`).
    /// Round-trips `~/.kiro/settings/cli.json` byte for byte.
    SettingsList {
        settings: serde_json::Value,
    },

    // Kiro extensions
    MetadataUpdated {
        context_usage: ContextUsage,
        metering: Option<TurnMetering>,
        tokens: Option<TokenCounts>,
        /// Thinking-effort level reported under thinking models (Kiro 2.5.0+).
        /// `None` when the metadata frame omits it — which happens on
        /// non-thinking models and (observed) mid-turn on context-only frames,
        /// so consumers must treat absence as "no update", not "cleared".
        effort: Option<EffortLevel>,
    },
    /// ACP `usage_update` session notification (unstable_session_usage).
    /// Carries absolute token counts rather than the percentage from
    /// `kiro.dev/metadata`. Both may arrive within a turn; whichever notification
    /// lands last wins (both write `context_usage` in UiState / SessionController).
    UsageUpdated {
        used: u64,
        size: u64,
    },
    /// KAS `session_info_update` → `context_usage` (KAS-2b, cyril-5et2). KAS
    /// pushes the categorized breakdown proactively each turn (v2 sends only the
    /// scalar via `MetadataUpdated`). `usage_percentage` is the flat
    /// `_meta.kiro.usagePercentage`; `breakdown` is `None` on frames that carry
    /// only the scalar — consumers must treat absence as "no update", not
    /// "cleared" (retain-last, same discipline as `MetadataUpdated.effort`).
    ContextBreakdownUpdated {
        usage_percentage: f64,
        breakdown: Option<ContextBreakdown>,
    },
    AgentSwitched {
        name: String,
        welcome: Option<String>,
        previous_agent: Option<String>,
        model: Option<String>,
    },
    /// Kiro-specific `kiro.dev/compaction/status`. `phase` carries the
    /// lifecycle state; `summary` is populated when Kiro provides a
    /// post-compaction summary (typically only with `Completed`).
    CompactionStatus {
        phase: CompactionPhase,
        summary: Option<String>,
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

    // Queue steering (Kiro 2.7.0+, `_kiro.dev/session/update` echoes; ROADMAP K1a).
    // The three echo variants below are echoes of a steer the client requested —
    // emitted for cyril's own steers AND (future, multi-client) for steers another
    // client originated. The converter produces them unconditionally: even when the
    // payload field is absent the variant is still emitted, so the depth counter
    // always transitions (a dropped transition would desync it permanently). K1a
    // routes them global; the session_id from the envelope is intentionally dropped
    // (only ToolCallChunk is promoted to scoped routing today — scoped steering is
    // K1c, cyril-28z2). `SteeringUnsupported` is the exception: it is
    // bridge-synthesized, not a converter-produced wire echo.
    /// A steer was accepted and queued for injection at the next tool boundary.
    /// `message` is the steer text, or `None` when the echo omitted it — the frame
    /// is still counted; only the (K1b) display text degrades. Never `Some("")`.
    SteeringQueued {
        message: Option<String>,
    },
    /// A queued steer was injected into the turn at a tool boundary. `content` is
    /// the injected text, or `None` when the echo omitted it. The variant is emitted
    /// (and depth decremented) regardless of whether `content` parsed — dropping it
    /// on a missing field would permanently inflate the queue counter.
    SteeringConsumed {
        content: Option<String>,
    },
    /// The queued steer was dropped before pickup (via `_session/steer/clear`).
    SteeringCleared,
    /// The agent does not implement `_session/steer` (`-32601`); bridge-synthesized,
    /// not a wire echo. Surfaced once per session as a system message. The
    /// once-per-session dedup lives in the bridge (its `steering_unsupported` set),
    /// NOT in `UiState`, which adds a system message for every one it receives.
    SteeringUnsupported {
        message: String,
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
        current_mode: Option<ModeId>,
        current_model: Option<String>,
        /// Full mode catalog from `NewSessionResponse.modes.availableModes`.
        /// Empty when the agent didn't report any.
        available_modes: Vec<SessionMode>,
        /// Full model catalog from `NewSessionResponse.models.availableModels`.
        /// Populated when the agent reports models (cyril enables the
        /// `unstable_session_model` ACP feature). Empty otherwise.
        available_models: Vec<ModelInfo>,
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

/// A trust tier option from `_meta.trustOptions[]` on permission requests.
/// Represents a level of persistent trust the user can grant (e.g. "Full command",
/// "Base command") with pre-built regex patterns for storage.
#[derive(Debug, Clone)]
pub struct TrustOption {
    pub label: String,
    pub display: String,
    pub setting_key: String,
    pub patterns: Vec<String>,
}

/// A request from the agent that needs user approval.
/// NOT Clone — owns a oneshot sender for the response.
#[derive(Debug)]
pub struct PermissionRequest {
    pub tool_call: ToolCall,
    pub message: String,
    pub options: Vec<PermissionOption>,
    pub trust_options: Vec<TrustOption>,
    pub responder: tokio::sync::oneshot::Sender<PermissionResponse>,
}

/// The semantic kind of a permission option. Mirrors `acp::PermissionOptionKind`.
/// Replies carry the picked option's id (`PermissionResponse::Selected`), not
/// the kind; the kind drives UI concerns — the AllowAlways trust-phase
/// transition and destructive-option styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

/// Unique identifier of a permission option within one request. Newtype
/// preventing labels or other strings being passed where the wire optionId
/// belongs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionOptionId(String);

impl PermissionOptionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PermissionOptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An option in a permission request dialog.
#[derive(Debug, Clone)]
pub struct PermissionOption {
    pub id: PermissionOptionId,
    pub label: String,
    pub kind: PermissionOptionKind,
    pub is_destructive: bool,
}

/// The user's response to a permission request.
#[derive(Debug, Clone)]
pub enum PermissionResponse {
    /// The user picked a specific option from the request's option list.
    ///
    /// `option_id` must name an option from the originating request; the
    /// converter warns at runtime if it doesn't (a foreign id would silently
    /// answer the agent with something it never offered).
    Selected {
        option_id: PermissionOptionId,
        /// Trust tier label from phase-2 selection (v2 `AllowAlways` flow).
        trust_option: Option<String>,
    },
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
    /// Request the agent switch to the given model via the standard ACP
    /// `session/set_model` method. Infrastructure-only today: `/model`
    /// currently routes through `ExecuteCommand` because Kiro does not
    /// advertise `session/set_model` in its capabilities.
    SetModel {
        model_id: String,
    },
    ExtMethod {
        method: String,
        params: serde_json::Value,
    },
    /// Query Kiro's on-disk settings snapshot via `kiro.dev/settings/list`.
    /// Wire request takes empty `{}` params (non-empty hangs the agent —
    /// see `docs/cyril-acp-coverage-vs-2.4.1.md`). The response is a flat
    /// dotted-key map mirroring `~/.kiro/settings/cli.json`.
    ///
    /// Read-only. There is intentionally no `SetSetting` BridgeCommand:
    /// `kiro.dev/settings/set` is dead wire surface (tui.js has the name
    /// in its constants table with zero call sites; the TUI mutates
    /// settings by writing the cli.json file directly).
    ListSettings,
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
    /// Queue a mid-turn steer (Kiro 2.7.0+; ROADMAP K1a). Sent as an awaited
    /// `_session/steer` ExtRequest. -32601 marks the session unsupported.
    SteerSession {
        session_id: SessionId,
        message: String,
    },
    /// Drop the queued steer before pickup, via `_session/steer/clear`. Like
    /// `SteerSession`, a session already marked unsupported (prior -32601) is
    /// skipped silently — the bridge never re-sends on an unsupported session.
    ClearSteering {
        session_id: SessionId,
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
            effort: Some(EffortLevel::High),
        };
        if let Notification::MetadataUpdated {
            context_usage,
            metering,
            tokens,
            effort,
        } = n
        {
            assert!((context_usage.percentage() - 75.0).abs() < f64::EPSILON);
            assert!(metering.is_none());
            assert!(tokens.is_none());
            assert_eq!(effort, Some(EffortLevel::High));
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
            id: PermissionOptionId::new("allow_once"),
            label: "Allow Once".into(),
            kind: PermissionOptionKind::AllowOnce,
            is_destructive: false,
        };
        assert_eq!(opt.id.as_str(), "allow_once");
        assert_eq!(opt.kind, PermissionOptionKind::AllowOnce);
        assert!(!opt.is_destructive);
    }

    #[test]
    fn permission_response_variants() {
        let responses = [
            PermissionResponse::Selected {
                option_id: PermissionOptionId::new("opt-1"),
                trust_option: None,
            },
            PermissionResponse::Cancel,
        ];
        assert_eq!(responses.len(), 2);
    }

    #[test]
    fn notification_user_message() {
        use crate::types::message::UserMessage;
        let n = Notification::UserMessage(UserMessage {
            text: "what does this do?".into(),
            is_streaming: true,
        });
        assert!(matches!(n, Notification::UserMessage(_)));
    }

    #[test]
    fn bridge_command_set_model() {
        let cmd = BridgeCommand::SetModel {
            model_id: "claude-opus-4.6".into(),
        };
        assert!(matches!(cmd, BridgeCommand::SetModel { .. }));
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

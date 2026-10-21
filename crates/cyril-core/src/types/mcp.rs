//! Session-bound MCP management. Catalogs and external previews are immutable data,
//! not tools or model instructions. Wire decoding belongs to the KAS converter.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::{SessionId, ToolCallId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRequest {
    pub session_id: SessionId,
    pub server_name: String,
    pub operation: McpOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpOperation {
    Reset {
        start_oauth: bool,
    },
    GetPrompt {
        name: String,
        arguments: BTreeMap<String, String>,
    },
    GetResource {
        uri: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpState {
    pub session_id: Option<SessionId>,
    /// None is unreported, whereas Some with no servers is authoritative empty.
    pub snapshot: Option<Arc<McpSnapshot>>,
    pub policy: Option<McpPolicy>,
    /// A rejected status frame never replaces the last valid snapshot.
    pub diagnostic: Option<String>,
    /// Passive URLs survive transient reconnect status; only connected/removal resolves them.
    pub authorization_urls: BTreeMap<String, String>,
    pub pending: BTreeMap<String, McpPending>,
    pub outcomes: BTreeMap<String, McpOutcome>,
    pub elicitation: Option<McpElicitationNotice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSnapshot {
    pub session_id: SessionId,
    pub servers: Vec<McpServer>,
    pub access_mode: Option<McpAccessMode>,
    pub registry_servers: Option<Vec<McpRegistryServer>>,
    pub access_mode_filtered_servers: Option<Vec<String>>,
    pub unresolved_registry_servers: Option<Vec<String>>,
    pub provenance: Option<McpProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpAccessMode {
    Registry,
    NonRegistry,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRegistryServer {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
}

/// Preserve the complete resource descriptor as display data, including future
/// fields. Consumers may display `details`, never parse it to make decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpProvenance {
    pub resource_type: Option<String>,
    pub origin: Option<String>,
    pub krn: Option<String>,
    pub layer: Option<String>,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    pub auth_type: Option<String>,
    pub status: McpServerStatus,
    pub provenance: Option<McpProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServerStatus {
    Connecting,
    Connected {
        catalog: Arc<McpCatalog>,
    },
    Failed {
        failed_authorization: bool,
        authorization_url: Option<String>,
        error_message: String,
        error_code: Option<String>,
    },
    Disabled,
    Unknown {
        status: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct McpCatalog {
    pub tools: Vec<McpTool>,
    pub prompts: Vec<McpPrompt>,
    pub resources: Vec<McpResource>,
    pub resource_templates: Vec<McpResourceTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub disabled: bool,
    /// Read-only schema text. No UI-side JSON interpretation is needed.
    pub input_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPrompt {
    /// Canonical KAS `promptName`, used for selection and getPrompt requests.
    pub name: String,
    /// Advertised display label, retained only when it differs from `name`.
    pub title: Option<String>,
    pub description: Option<String>,
    pub arguments: Vec<McpPromptArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpResourceTemplate {
    pub uri_template: String,
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpPolicy {
    ApiFailure,
    AdminDisabled,
    NoEndpoint,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpPending {
    pub id: u64,
    pub request: McpRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOutcome {
    pub id: u64,
    pub request: McpRequest,
    pub result: Result<McpOperationResult, McpOperationError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpOperationResult {
    /// RPC acknowledgement only; authoritative status owns connection state.
    Acknowledged,
    Preview(Arc<McpPreview>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOperationError {
    pub kind: McpErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpErrorKind {
    SessionChanged,
    ServerMissing,
    Disconnected,
    Policy,
    EntryMissing,
    InvalidArguments,
    AlreadyPending,
    Malformed,
    Rejected,
    Transport,
    Timeout,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpPreview {
    Prompt {
        description: Option<String>,
        messages: Vec<McpMessage>,
    },
    Resource {
        contents: Vec<McpContent>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpMessage {
    pub role: String,
    pub content: McpContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpContent {
    Text {
        text: String,
        mime_type: Option<String>,
        uri: Option<String>,
    },
    NonText {
        kind: String,
        mime_type: Option<String>,
        uri: Option<String>,
        name: Option<String>,
        description: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpElicitationMode {
    Form,
    Url,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpElicitationNotice {
    pub session_id: SessionId,
    pub tool_call_id: ToolCallId,
    pub mode: McpElicitationMode,
    pub message: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpUpdate {
    Snapshot(Arc<McpSnapshot>),
    /// Serial promotion of passive URLs retained before SessionCreated.
    AuthorizationUrls {
        session_id: SessionId,
        urls: BTreeMap<String, String>,
    },
    Policy {
        session_id: SessionId,
        policy: McpPolicy,
    },
    Diagnostic {
        session_id: SessionId,
        message: String,
    },
    Pending(McpPending),
    Outcome(McpOutcome),
    ElicitationDeclined(McpElicitationNotice),
}

impl McpUpdate {
    pub fn session_id(&self) -> &SessionId {
        match self {
            Self::Snapshot(snapshot) => &snapshot.session_id,
            Self::Policy { session_id, .. } | Self::Diagnostic { session_id, .. } => session_id,
            Self::AuthorizationUrls { session_id, .. } => session_id,
            Self::Pending(pending) => &pending.request.session_id,
            Self::Outcome(outcome) => &outcome.request.session_id,
            Self::ElicitationDeclined(notice) => &notice.session_id,
        }
    }
}

impl McpState {
    pub fn for_session(session_id: SessionId) -> Self {
        Self {
            session_id: Some(session_id),
            ..Self::default()
        }
    }

    /// Both controller and serial mediator use the same pure reducer. Foreign
    /// sessions cannot change state even if a caller forgets its routing guard.
    pub fn apply(&mut self, update: &McpUpdate) -> bool {
        if self.session_id.as_ref() != Some(update.session_id()) {
            return false;
        }
        match update {
            McpUpdate::Snapshot(snapshot) => {
                // KAS synchronously empties its desired pool while governance is off.
                // Only known active states contradict its api/admin-disabled notice.
                if matches!(
                    &self.policy,
                    Some(McpPolicy::ApiFailure | McpPolicy::AdminDisabled)
                ) && snapshot.servers.iter().any(|server| {
                    matches!(
                        &server.status,
                        McpServerStatus::Connecting
                            | McpServerStatus::Connected { .. }
                            | McpServerStatus::Failed { .. }
                    )
                }) {
                    self.policy = None;
                }
                self.snapshot = Some(Arc::clone(snapshot));
                self.diagnostic = None;
                self.authorization_urls.retain(|name, _| {
                    snapshot.servers.iter().any(|server| {
                        &server.name == name
                            && !matches!(server.status, McpServerStatus::Connected { .. })
                    })
                });
                for server in &snapshot.servers {
                    if let McpServerStatus::Failed {
                        failed_authorization: true,
                        authorization_url: Some(url),
                        ..
                    } = &server.status
                    {
                        self.authorization_urls
                            .insert(server.name.clone(), url.clone());
                    }
                }
            }
            McpUpdate::AuthorizationUrls { urls, .. } => self.authorization_urls = urls.clone(),
            McpUpdate::Policy { policy, .. } => self.policy = Some(policy.clone()),
            McpUpdate::Diagnostic { message, .. } => self.diagnostic = Some(message.clone()),
            McpUpdate::Pending(pending) => {
                self.outcomes.remove(&pending.request.server_name);
                self.pending
                    .insert(pending.request.server_name.clone(), pending.clone());
            }
            McpUpdate::Outcome(outcome) => {
                let name = &outcome.request.server_name;
                if self
                    .pending
                    .get(name)
                    .is_some_and(|pending| pending.id != outcome.id)
                {
                    return false;
                }
                self.pending.remove(name);
                self.outcomes.insert(name.clone(), outcome.clone());
            }
            McpUpdate::ElicitationDeclined(notice) => self.elicitation = Some(notice.clone()),
        }
        true
    }

    pub fn validate(&self, request: &McpRequest) -> Result<(), McpOperationError> {
        let error = |kind, message: String| McpOperationError { kind, message };
        if self.session_id.as_ref() != Some(&request.session_id) {
            return Err(error(
                McpErrorKind::SessionChanged,
                "The original MCP session is no longer current. Reopen /mcp.".into(),
            ));
        }
        if let Some(policy) = &self.policy {
            return Err(error(
                McpErrorKind::Policy,
                format!("MCP is disabled by governance: {policy:?}."),
            ));
        }
        if self.pending.contains_key(&request.server_name) {
            return Err(error(
                McpErrorKind::AlreadyPending,
                "This server already has an MCP operation in flight.".into(),
            ));
        }
        let server = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot
                    .servers
                    .iter()
                    .find(|server| server.name == request.server_name)
            })
            .ok_or_else(|| {
                error(
                    McpErrorKind::ServerMissing,
                    "The original server is not in the current MCP snapshot. Reopen /mcp.".into(),
                )
            })?;
        if matches!(request.operation, McpOperation::Reset { .. }) {
            return if matches!(
                server.status,
                McpServerStatus::Disabled | McpServerStatus::Unknown { .. }
            ) {
                Err(error(
                    McpErrorKind::Disconnected,
                    "This server has no known connectable configuration.".into(),
                ))
            } else {
                Ok(())
            };
        }
        let McpServerStatus::Connected { catalog } = &server.status else {
            return Err(error(
                McpErrorKind::Disconnected,
                "The original MCP server is no longer connected. Reconnect it first.".into(),
            ));
        };
        match &request.operation {
            McpOperation::GetPrompt { name, arguments } => {
                let prompt = catalog
                    .prompts
                    .iter()
                    .find(|prompt| &prompt.name == name)
                    .ok_or_else(|| {
                        error(
                            McpErrorKind::EntryMissing,
                            "This prompt is no longer advertised by the original server.".into(),
                        )
                    })?;
                for argument in &prompt.arguments {
                    if argument.required && !arguments.contains_key(&argument.name) {
                        return Err(error(
                            McpErrorKind::InvalidArguments,
                            format!("Required argument {:?} is absent.", argument.name),
                        ));
                    }
                }
                if let Some(name) = arguments.keys().find(|name| {
                    !prompt
                        .arguments
                        .iter()
                        .any(|argument| &argument.name == *name)
                }) {
                    return Err(error(
                        McpErrorKind::InvalidArguments,
                        format!("Argument {name:?} is not advertised by this prompt."),
                    ));
                }
            }
            McpOperation::GetResource { uri } => {
                if !catalog
                    .resources
                    .iter()
                    .any(|resource| &resource.uri == uri)
                {
                    return Err(error(
                        McpErrorKind::EntryMissing,
                        "This resource URI is no longer advertised by the original server.".into(),
                    ));
                }
            }
            McpOperation::Reset { .. } => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;

//! What a KAS permission request asks for, parsed once at the KAS boundary so
//! no raw JSON reaches the App or the review policy.

use std::path::PathBuf;

/// The capability a permission request asks for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Capability {
    FsRead,
    FsWrite,
    StrReplace,
    /// `shell`, `execute` and `execute_bash`.
    Shell,
    /// Anything else, including an absent capability: always denied.
    Unrecognized(String),
}

impl Capability {
    pub fn parse(name: &str) -> Self {
        match name {
            "fs_read" => Self::FsRead,
            "fs_write" => Self::FsWrite,
            "str_replace" => Self::StrReplace,
            "shell" | "execute" | "execute_bash" => Self::Shell,
            other => Self::Unrecognized(other.to_owned()),
        }
    }
}

/// `_meta.kiro.{toolId, command, consent{capability, resource, workspaceRoot,
/// triggeringResource, askType}}` of a KAS `session/request_permission`, plus
/// the commands the tool call's `rawInput` carries (`command` / `cmd`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionConsent {
    capability: Capability,
    /// `consent.capability` exactly as sent (non-empty), before the tool-id
    /// fallback and the normalisation `capability` applies. "Always allow"
    /// keys its choices on these exact names, as Kiro's TUI does (cyril-auxt).
    wire_capability: Option<String>,
    resource: Option<String>,
    triggering_resource: Option<String>,
    explicit_ask: bool,
    workspace_root: Option<PathBuf>,
    tool_id: Option<String>,
    command: Option<String>,
    raw_commands: Vec<String>,
}

impl PermissionConsent {
    /// The capability is `consent.capability`, or the tool id when the
    /// consent names none.
    pub fn new(
        capability: Option<&str>,
        resource: Option<String>,
        workspace_root: Option<PathBuf>,
        tool_id: Option<String>,
        command: Option<String>,
        raw_commands: Vec<String>,
    ) -> Self {
        let wire_capability = capability.filter(|name| !name.is_empty());
        let named = wire_capability.or(tool_id.as_deref()).unwrap_or_default();
        Self {
            capability: Capability::parse(named),
            wire_capability: wire_capability.map(str::to_owned),
            resource,
            triggering_resource: None,
            explicit_ask: false,
            workspace_root,
            tool_id,
            command,
            raw_commands,
        }
    }

    /// `consent.triggeringResource`: the resource that raised the ask when it
    /// differs from the one a grant covers.
    pub fn with_triggering_resource(mut self, triggering_resource: Option<String>) -> Self {
        self.triggering_resource = triggering_resource;
        self
    }

    /// `consent.askType == "explicit"`: the agent wants this exact call
    /// confirmed, so no folder grant is offered.
    pub fn with_explicit_ask(mut self, explicit_ask: bool) -> Self {
        self.explicit_ask = explicit_ask;
        self
    }

    pub fn capability(&self) -> &Capability {
        &self.capability
    }

    /// `consent.capability` as sent, when non-empty.
    pub fn wire_capability(&self) -> Option<&str> {
        self.wire_capability.as_deref()
    }

    pub fn triggering_resource(&self) -> Option<&str> {
        self.triggering_resource.as_deref()
    }

    pub fn is_explicit_ask(&self) -> bool {
        self.explicit_ask
    }

    pub fn resource(&self) -> Option<&str> {
        self.resource.as_deref()
    }

    pub fn workspace_root(&self) -> Option<&std::path::Path> {
        self.workspace_root.as_deref()
    }

    pub fn tool_id(&self) -> Option<&str> {
        self.tool_id.as_deref()
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    /// `rawInput.command` and `rawInput.cmd`, when present.
    pub fn raw_commands(&self) -> &[String] {
        &self.raw_commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_falls_back_to_the_tool_id() {
        let consent = |capability: Option<&str>, tool: Option<&str>| {
            PermissionConsent::new(
                capability,
                None,
                None,
                tool.map(str::to_owned),
                None,
                Vec::new(),
            )
        };
        assert_eq!(
            consent(Some("fs_read"), Some("fs_write")).capability(),
            &Capability::FsRead
        );
        assert_eq!(
            consent(None, Some("execute_bash")).capability(),
            &Capability::Shell
        );
        assert_eq!(
            consent(Some(""), Some("fs_write")).capability(),
            &Capability::FsWrite
        );
        assert_eq!(
            consent(Some("web_fetch"), None).capability(),
            &Capability::Unrecognized("web_fetch".to_owned())
        );
        assert_eq!(
            consent(None, None).capability(),
            &Capability::Unrecognized(String::new())
        );
    }
}

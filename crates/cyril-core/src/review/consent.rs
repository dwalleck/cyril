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

/// `_meta.kiro.{toolId, command, consent{capability, resource, workspaceRoot}}`
/// of a KAS `session/request_permission`, plus the commands the tool call's
/// `rawInput` carries (`command` / `cmd`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionConsent {
    capability: Capability,
    resource: Option<String>,
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
        let named = capability
            .filter(|name| !name.is_empty())
            .or(tool_id.as_deref())
            .unwrap_or_default();
        Self {
            capability: Capability::parse(named),
            resource,
            workspace_root,
            tool_id,
            command,
            raw_commands,
        }
    }

    pub fn capability(&self) -> &Capability {
        &self.capability
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

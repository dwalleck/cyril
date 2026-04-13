/// LSP server status as reported by Kiro's code intelligence.
///
/// `Unknown(String)` captures any status string not recognized by the current
/// parser, allowing forward-compatible deserialization without panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStatus {
    Initialized,
    Initializing,
    Failed,
    Unknown(String),
}

/// A single LSP server entry from the /code panel response.
///
/// Fields are public — this is a read-only display DTO produced exclusively
/// by `CodeCommandResponse::from_json`. Same convention as `CommandOption`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerInfo {
    pub name: String,
    pub languages: Vec<String>,
    /// `None` when the status field is absent from the protocol response.
    pub status: Option<LspStatus>,
    pub init_duration_ms: Option<u64>,
}

/// Parsed data from a /code status response.
///
/// Fields are public — this is a read-only display DTO produced exclusively
/// by `CodeCommandResponse::from_json`. Same convention as `CommandOption`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodePanelData {
    pub status: LspStatus,
    pub message: Option<String>,
    pub warning: Option<String>,
    pub root_path: Option<String>,
    pub detected_languages: Vec<String>,
    pub project_markers: Vec<String>,
    pub config_path: Option<String>,
    /// Parsed but not currently rendered — reserved for future help link.
    pub doc_url: Option<String>,
    pub lsps: Vec<LspServerInfo>,
}

/// The three shapes a /code CommandExecuted response can take.
#[derive(Debug, Clone)]
pub enum CodeCommandResponse {
    /// Show the status panel overlay.
    Panel(CodePanelData),
    /// Auto-send a prompt to the agent.
    Prompt { text: String, label: Option<String> },
    /// Unknown shape — fall through to generic formatting.
    Unknown(serde_json::Value),
}

impl CodeCommandResponse {
    /// Parse a `CommandExecuted` response JSON for the `/code` command.
    ///
    /// Routes by data shape:
    /// - `data.executePrompt` exists → Prompt (checked first — takes priority)
    /// - `data.status` exists → Panel
    /// - anything else → Unknown
    pub fn from_json(response: &serde_json::Value) -> Self {
        let data = match response.get("data") {
            Some(d) if !d.is_null() => d,
            _ => return Self::Unknown(response.clone()),
        };

        // Prompt path — check first (takes priority if both shapes present)
        if let Some(prompt) = data.get("executePrompt").and_then(|p| p.as_str()) {
            return Self::Prompt {
                text: prompt.to_string(),
                label: json_str(data, "label"),
            };
        }

        // Panel path
        if let Some(status_str) = data.get("status").and_then(|s| s.as_str()) {
            let status = parse_lsp_status(status_str);
            let lsps = data
                .get("lsps")
                .and_then(|l| l.as_array())
                .map(|arr| arr.iter().filter_map(parse_lsp_server).collect())
                .unwrap_or_default();

            return Self::Panel(CodePanelData {
                status,
                message: json_str(data, "message"),
                warning: json_str(data, "warning"),
                root_path: json_str(data, "rootPath"),
                detected_languages: json_str_array(data, "detectedLanguages"),
                project_markers: json_str_array(data, "projectMarkers"),
                config_path: json_str(data, "configPath"),
                doc_url: json_str(data, "docUrl"),
                lsps,
            });
        }

        Self::Unknown(response.clone())
    }
}

/// Extract an optional string field from a JSON object.
fn json_str(obj: &serde_json::Value, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Extract an array-of-strings field, defaulting to empty if absent or wrong type.
fn json_str_array(obj: &serde_json::Value, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_lsp_status(s: &str) -> LspStatus {
    match s {
        "initialized" => LspStatus::Initialized,
        "initializing" => LspStatus::Initializing,
        "failed" => LspStatus::Failed,
        other => LspStatus::Unknown(other.to_string()),
    }
}

fn parse_lsp_server(value: &serde_json::Value) -> Option<LspServerInfo> {
    let name = match value.get("name").and_then(|n| n.as_str()) {
        Some(n) => n.to_string(),
        None => {
            tracing::warn!(entry = %value, "LSP server entry missing required `name` — skipping");
            return None;
        }
    };
    let languages = json_str_array(value, "languages");
    let status = value
        .get("status")
        .and_then(|s| s.as_str())
        .map(parse_lsp_status);
    let init_duration_ms = value.get("initDurationMs").and_then(|d| d.as_u64());

    Some(LspServerInfo {
        name,
        languages,
        status,
        init_duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_panel_response() {
        let response = json!({
            "success": true,
            "message": "Code intelligence status",
            "data": {
                "status": "initialized",
                "message": "LSP servers ready",
                "rootPath": "/home/user/project",
                "detectedLanguages": ["rust"],
                "projectMarkers": ["Cargo.toml"],
                "configPath": ".kiro/settings/lsp.json",
                "docUrl": "https://kiro.dev/docs/cli/code-intelligence/",
                "lsps": [
                    {
                        "name": "rust-analyzer",
                        "languages": ["rust"],
                        "status": "initialized",
                        "initDurationMs": 44
                    }
                ]
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.status, LspStatus::Initialized);
                assert_eq!(data.detected_languages, vec!["rust"]);
                assert_eq!(data.lsps.len(), 1);
                assert_eq!(data.lsps[0].name, "rust-analyzer");
                assert_eq!(data.lsps[0].status, Some(LspStatus::Initialized));
                assert_eq!(data.lsps[0].init_duration_ms, Some(44));
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn parse_prompt_response() {
        let response = json!({
            "success": true,
            "data": {
                "executePrompt": "Analyze the codebase...",
                "label": "Code Summary"
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Prompt { text, label } => {
                assert_eq!(text, "Analyze the codebase...");
                assert_eq!(label, Some("Code Summary".into()));
            }
            other => panic!("Expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_response() {
        let response = json!({
            "success": true,
            "message": "Something else happened"
        });
        let result = CodeCommandResponse::from_json(&response);
        assert!(matches!(result, CodeCommandResponse::Unknown(_)));
    }

    #[test]
    fn parse_initializing_status() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initializing",
                "message": "Starting LSP servers...",
                "detectedLanguages": [],
                "projectMarkers": [],
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.status, LspStatus::Initializing);
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn parse_warning_field() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initialized",
                "warning": "pyright not found on PATH",
                "detectedLanguages": ["python"],
                "projectMarkers": ["requirements.txt"],
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.warning, Some("pyright not found on PATH".into()));
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn prompt_takes_priority_over_panel() {
        let response = json!({
            "success": true,
            "data": {
                "executePrompt": "Do something...",
                "status": "initialized",
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        assert!(matches!(result, CodeCommandResponse::Prompt { .. }));
    }

    #[test]
    fn malformed_lsp_entry_skipped_valid_retained() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initialized",
                "detectedLanguages": [],
                "projectMarkers": [],
                "lsps": [
                    { "name": "rust-analyzer", "languages": ["rust"], "status": "initialized" },
                    { "languages": ["python"] },
                    { "name": "gopls", "languages": ["go"], "status": "failed" }
                ]
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.lsps.len(), 2, "malformed entry should be skipped");
                assert_eq!(data.lsps[0].name, "rust-analyzer");
                assert_eq!(data.lsps[1].name, "gopls");
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn lsp_entry_missing_status_has_none() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initialized",
                "detectedLanguages": [],
                "projectMarkers": [],
                "lsps": [
                    { "name": "rust-analyzer", "languages": ["rust"] }
                ]
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.lsps.len(), 1);
                assert_eq!(data.lsps[0].status, None, "missing status should be None");
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn prompt_without_label() {
        let response = json!({
            "success": true,
            "data": {
                "executePrompt": "Analyze the code..."
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Prompt { text, label } => {
                assert_eq!(text, "Analyze the code...");
                assert_eq!(label, None);
            }
            other => panic!("Expected Prompt, got {other:?}"),
        }
    }
}

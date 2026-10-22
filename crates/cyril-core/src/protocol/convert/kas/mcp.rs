//! Exact KAS MCP notification and management-result boundary (0.66.43).

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::types::mcp::*;
use crate::types::{Notification, SessionId};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    session_id: String,
    servers: Vec<Value>,
    access_mode: Option<String>,
    registry_servers: Option<Vec<RegistryServer>>,
    access_mode_filtered_servers: Option<Vec<String>>,
    unresolved_registry_servers: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct RegistryServer {
    name: String,
    version: Option<String>,
    description: Option<String>,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Server {
    name: String,
    status: String,
    auth_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Failed {
    failed_authorization: bool,
    authorization_url: Option<String>,
    error_message: String,
    error_code: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    tools: Vec<Tool>,
    prompts: Vec<Prompt>,
    resources: Vec<Resource>,
    resource_templates: Vec<ResourceTemplate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tool {
    name: String,
    description: Option<String>,
    disabled: bool,
    input_schema: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Prompt {
    name: String,
    prompt_name: String,
    description: Option<String>,
    #[serde(default)]
    arguments: Vec<Argument>,
}

#[derive(Deserialize)]
struct Argument {
    name: String,
    description: Option<String>,
    #[serde(default)]
    required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resource {
    uri: String,
    name: String,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceTemplate {
    uri_template: String,
    name: String,
    title: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceProvenance {
    resource_type: Option<String>,
    source: Option<ResourceSource>,
    krn: Option<String>,
    layer: Option<String>,
}

#[derive(Deserialize)]
struct ResourceSource {
    origin: Option<String>,
}

pub(crate) fn convert(method: &str, params: &Value) -> crate::Result<Option<Notification>> {
    if !matches!(method, "kiro/mcp/status" | "kiro/mcp/governance_disabled") {
        return Ok(None);
    }
    let session_id = match required_text(params, "sessionId") {
        Ok(session_id) => SessionId::new(session_id),
        Err(message) => {
            return Ok(Some(Notification::BridgeError {
                operation: "MCP status".into(),
                message: format!("{message}; last known MCP state is unchanged."),
            }));
        }
    };
    let decoded = match method {
        "kiro/mcp/status" => {
            snapshot(params).map(|snapshot| McpUpdate::Snapshot(Arc::new(snapshot)))
        }
        _ => required_text(params, "reason").map(|reason| McpUpdate::Policy {
            session_id: session_id.clone(),
            policy: match reason {
                "api_failure" => McpPolicy::ApiFailure,
                "admin_disabled" => McpPolicy::AdminDisabled,
                "no_endpoint" => McpPolicy::NoEndpoint,
                other => McpPolicy::Unknown(other.to_owned()),
            },
        }),
    };
    let update = decoded.unwrap_or_else(|message| McpUpdate::Diagnostic {
        session_id,
        message,
    });
    Ok(Some(Notification::McpUpdate(update)))
}

fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value).map_err(|error| format!("Malformed MCP data: {error}"))
}

fn required_text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("Malformed MCP data: {key} must be a nonempty string"))
}

fn provenance(value: &Value) -> Result<Option<McpProvenance>, String> {
    let Some(resource) = value
        .get("_meta")
        .and_then(|meta| meta.get("kiro"))
        .and_then(|kiro| kiro.get("resource"))
    else {
        return Ok(None);
    };
    if !resource.is_object() {
        return Err("Malformed MCP resource provenance: expected object".into());
    }
    if resource
        .get("source")
        .is_some_and(|source| !source.is_null() && !source.is_object())
    {
        return Err(
            "Malformed MCP resource provenance: source must be an object when non-null".into(),
        );
    }
    let wire = ResourceProvenance::deserialize(resource)
        .map_err(|error| format!("Malformed MCP resource provenance: {error}"))?;
    Ok(Some(McpProvenance {
        resource_type: wire.resource_type,
        origin: wire.source.and_then(|source| source.origin),
        krn: wire.krn,
        layer: wire.layer,
        details: serde_json::to_string_pretty(resource)
            .map_err(|error| format!("Malformed MCP resource descriptor: {error}"))?,
    }))
}

fn snapshot(value: &Value) -> Result<McpSnapshot, String> {
    let wire: Status = decode(value)?;
    if wire.session_id.is_empty() {
        return Err("Malformed MCP status: empty sessionId".into());
    }
    let mut names = BTreeSet::new();
    let mut servers = Vec::with_capacity(wire.servers.len());
    for value in wire.servers {
        let wire: Server = decode(&value)?;
        if wire.name.is_empty() || !names.insert(wire.name.clone()) {
            return Err("Malformed MCP status: empty or duplicate server name".into());
        }
        let status = match wire.status.as_str() {
            "connecting" => McpServerStatus::Connecting,
            "connected" => {
                let catalog: Catalog = decode(&value)?;
                McpServerStatus::Connected {
                    catalog: Arc::new(McpCatalog {
                        tools: catalog
                            .tools
                            .into_iter()
                            .map(|tool| McpTool {
                                name: tool.name,
                                description: tool.description,
                                disabled: tool.disabled,
                                input_schema: Value::Object(tool.input_schema).to_string(),
                            })
                            .collect(),
                        prompts: catalog
                            .prompts
                            .into_iter()
                            .map(|prompt| {
                                let title =
                                    (prompt.name != prompt.prompt_name).then_some(prompt.name);
                                McpPrompt {
                                    name: prompt.prompt_name,
                                    title,
                                    description: prompt.description,
                                    arguments: prompt
                                        .arguments
                                        .into_iter()
                                        .map(|argument| McpPromptArgument {
                                            name: argument.name,
                                            description: argument.description,
                                            required: argument.required,
                                        })
                                        .collect(),
                                }
                            })
                            .collect(),
                        resources: catalog
                            .resources
                            .into_iter()
                            .map(|resource| McpResource {
                                uri: resource.uri,
                                name: resource.name,
                                title: resource.title,
                                description: resource.description,
                                mime_type: resource.mime_type,
                            })
                            .collect(),
                        resource_templates: catalog
                            .resource_templates
                            .into_iter()
                            .map(|resource| McpResourceTemplate {
                                uri_template: resource.uri_template,
                                name: resource.name,
                                title: resource.title,
                                description: resource.description,
                                mime_type: resource.mime_type,
                            })
                            .collect(),
                    }),
                }
            }
            "failed" => {
                let failed: Failed = decode(&value)?;
                McpServerStatus::Failed {
                    failed_authorization: failed.failed_authorization,
                    authorization_url: failed.authorization_url,
                    error_message: failed.error_message,
                    error_code: failed.error_code,
                }
            }
            "disabled" => McpServerStatus::Disabled,
            status if !status.is_empty() => McpServerStatus::Unknown {
                status: status.to_owned(),
            },
            _ => return Err("Malformed MCP status: empty server status".into()),
        };
        servers.push(McpServer {
            name: wire.name,
            auth_type: wire.auth_type,
            status,
            provenance: provenance(&value)?,
        });
    }
    Ok(McpSnapshot {
        session_id: SessionId::new(wire.session_id),
        servers,
        access_mode: wire.access_mode.map(|mode| match mode.as_str() {
            "registry" => McpAccessMode::Registry,
            "nonRegistry" => McpAccessMode::NonRegistry,
            _ => McpAccessMode::Unknown(mode),
        }),
        registry_servers: wire.registry_servers.map(|servers| {
            servers
                .into_iter()
                .map(|server| McpRegistryServer {
                    name: server.name,
                    version: server.version,
                    description: server.description,
                    enabled: server.enabled,
                })
                .collect()
        }),
        access_mode_filtered_servers: wire.access_mode_filtered_servers,
        unresolved_registry_servers: wire.unresolved_registry_servers,
        provenance: provenance(value)?,
    })
}

pub(crate) fn result(
    operation: &McpOperation,
    value: Value,
) -> Result<McpOperationResult, McpOperationError> {
    let malformed = |message| McpOperationError {
        kind: McpErrorKind::Malformed,
        message,
    };
    if value.get("isError") == Some(&Value::Bool(true))
        || value.get("success") == Some(&Value::Bool(false))
        || value.get("error").is_some()
    {
        return Err(McpOperationError {
            kind: McpErrorKind::Rejected,
            message: format!("MCP operation refused: {value}"),
        });
    }
    match operation {
        McpOperation::Reset { .. } => {
            if value.get("success") == Some(&Value::Bool(true)) {
                Ok(McpOperationResult::Acknowledged)
            } else {
                Err(malformed(
                    "MCP reset response did not acknowledge success.".into(),
                ))
            }
        }
        McpOperation::GetPrompt { .. } => {
            let messages = value
                .get("messages")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    malformed("MCP prompt response is missing its messages array.".into())
                })?;
            let description = optional_text(&value, "description").map_err(malformed)?;
            let mut converted = Vec::with_capacity(messages.len());
            for message in messages {
                let role = required_text(message, "role")
                    .map_err(malformed)?
                    .to_owned();
                let content = message
                    .get("content")
                    .ok_or_else(|| malformed("MCP prompt message has no content.".into()))?;
                converted.push(McpMessage {
                    role,
                    content: content_value(content, false).map_err(malformed)?,
                });
            }
            Ok(McpOperationResult::Preview(Arc::new(McpPreview::Prompt {
                description,
                messages: converted,
            })))
        }
        McpOperation::GetResource { .. } => {
            let contents = value
                .get("contents")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    malformed("MCP resource response is missing its contents array.".into())
                })?;
            let contents = contents
                .iter()
                .map(|value| content_value(value, true))
                .collect::<Result<Vec<_>, _>>()
                .map_err(malformed)?;
            Ok(McpOperationResult::Preview(Arc::new(
                McpPreview::Resource { contents },
            )))
        }
    }
}

fn optional_text(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!(
            "Malformed MCP content: {key} must be a string when present"
        )),
    }
}

fn content_value(value: &Value, resource: bool) -> Result<McpContent, String> {
    if !value.is_object() {
        return Err("Malformed MCP content: expected an object".into());
    }
    let uri = optional_text(value, "uri")?;
    let mime_type = optional_text(value, "mimeType")?;
    if resource {
        required_text(value, "uri")?;
        if let Some(text) = optional_text(value, "text")? {
            return Ok(McpContent::Text {
                text,
                mime_type,
                uri,
            });
        }
        if value.get("blob").and_then(Value::as_str).is_none() {
            return Err("Malformed MCP resource content: expected text or blob.".into());
        }
        return Ok(McpContent::NonText {
            kind: "blob".into(),
            mime_type,
            uri,
            name: None,
            description: None,
        });
    }
    let kind = required_text(value, "type")?;
    if kind == "text" {
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "Malformed MCP text content: expected text string.".to_owned())?;
        return Ok(McpContent::Text {
            text: text.to_owned(),
            mime_type,
            uri,
        });
    }
    if kind == "resource" {
        let embedded = value
            .get("resource")
            .ok_or_else(|| "Malformed embedded MCP resource.".to_owned())?;
        return content_value(embedded, true);
    }
    if matches!(kind, "image" | "audio") {
        if value.get("data").and_then(Value::as_str).is_none() {
            return Err(format!("Malformed MCP {kind} content: missing data"));
        }
        required_text(value, "mimeType")?;
    }
    if kind == "resource_link" {
        required_text(value, "uri")?;
        required_text(value, "name")?;
    }
    Ok(McpContent::NonText {
        kind: kind.to_owned(),
        mime_type,
        uri,
        name: optional_text(value, "name")?,
        description: optional_text(value, "description")?,
    })
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;

use std::cell::RefCell;
use std::collections::HashMap;

use agent_client_protocol as acp;

use crate::types::*;

pub(crate) fn to_tool_kind(kind: agent_client_protocol::ToolKind) -> ToolKind {
    match kind {
        agent_client_protocol::ToolKind::Read => ToolKind::Read,
        agent_client_protocol::ToolKind::Edit
        | agent_client_protocol::ToolKind::Delete
        | agent_client_protocol::ToolKind::Move => ToolKind::Write,
        agent_client_protocol::ToolKind::Execute => ToolKind::Execute,
        agent_client_protocol::ToolKind::Search => ToolKind::Search,
        agent_client_protocol::ToolKind::Think => ToolKind::Think,
        agent_client_protocol::ToolKind::Fetch => ToolKind::Fetch,
        _ => ToolKind::Other,
    }
}

pub(crate) fn to_stop_reason(reason: agent_client_protocol::StopReason) -> StopReason {
    match reason {
        agent_client_protocol::StopReason::EndTurn => StopReason::EndTurn,
        agent_client_protocol::StopReason::MaxTokens => StopReason::MaxTokens,
        agent_client_protocol::StopReason::MaxTurnRequests => StopReason::MaxTurnRequests,
        agent_client_protocol::StopReason::Refusal => StopReason::Refusal,
        agent_client_protocol::StopReason::Cancelled => StopReason::Cancelled,
        _ => {
            tracing::warn!(?reason, "unknown StopReason variant, defaulting to EndTurn");
            StopReason::EndTurn
        }
    }
}

pub(crate) fn to_tool_call_status(status: agent_client_protocol::ToolCallStatus) -> ToolCallStatus {
    match status {
        agent_client_protocol::ToolCallStatus::InProgress => ToolCallStatus::InProgress,
        agent_client_protocol::ToolCallStatus::Pending => ToolCallStatus::Pending,
        agent_client_protocol::ToolCallStatus::Completed => ToolCallStatus::Completed,
        _ => ToolCallStatus::Failed,
    }
}

pub(crate) fn to_tool_call(
    acp_call: &agent_client_protocol::ToolCall,
    cached_inputs: &std::collections::HashMap<String, serde_json::Value>,
) -> ToolCall {
    let id_str = acp_call.tool_call_id.to_string();

    let content = convert_tool_call_content(&acp_call.content);
    let locations = convert_tool_call_locations(&acp_call.locations);

    ToolCall::new(
        ToolCallId::new(id_str.clone()),
        acp_call.title.clone(),
        to_tool_kind(acp_call.kind),
        to_tool_call_status(acp_call.status),
        cached_inputs
            .get(&id_str)
            .cloned()
            .or_else(|| acp_call.raw_input.clone()),
    )
    .with_content(content)
    .with_locations(locations)
    .with_raw_output(acp_call.raw_output.clone())
}

/// Convert ACP tool call content to our internal representation.
fn convert_tool_call_content(acp_content: &[acp::ToolCallContent]) -> Vec<ToolCallContent> {
    acp_content
        .iter()
        .filter_map(|c| match c {
            acp::ToolCallContent::Diff(diff) => Some(ToolCallContent::Diff {
                path: diff.path.to_string_lossy().to_string(),
                old_text: diff.old_text.clone(),
                new_text: diff.new_text.clone(),
            }),
            acp::ToolCallContent::Content(content) => {
                if let acp::ContentBlock::Text(ref text) = content.content {
                    Some(ToolCallContent::Text(text.text.clone()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect()
}

/// Convert ACP tool call locations to our internal representation.
fn convert_tool_call_locations(acp_locations: &[acp::ToolCallLocation]) -> Vec<ToolCallLocation> {
    acp_locations
        .iter()
        .map(|loc| ToolCallLocation {
            path: loc.path.to_string_lossy().to_string(),
            line: loc.line,
        })
        .collect()
}

/// Parse a single subagent entry from a `subagent/list_update` JSON array element.
fn parse_subagent_entry(v: &serde_json::Value) -> Option<SubagentInfo> {
    let session_id = match v
        .get("sessionId")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(id) => SessionId::new(id),
        None => {
            tracing::warn!("subagent entry missing or empty sessionId, skipping");
            return None;
        }
    };
    let session_name = v
        .get("sessionName")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            tracing::warn!("subagent entry missing sessionName");
            "(unknown)"
        });
    let agent_name = v
        .get("agentName")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            tracing::warn!("subagent entry missing agentName");
            "(unknown)"
        });
    let initial_query = v
        .get("initialQuery")
        .and_then(|q| q.as_str())
        .unwrap_or_default();

    let status_obj = v.get("status");
    if status_obj.is_none() {
        tracing::warn!("subagent entry missing status field, defaulting to Working");
    }
    let status_type = status_obj
        .and_then(|s| s.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("working");
    let working_message = status_obj
        .and_then(|s| s.get("message"))
        .and_then(|m| m.as_str())
        .map(String::from);
    let status = match status_type {
        "working" => SubagentStatus::Working {
            message: working_message,
        },
        "terminated" => SubagentStatus::Terminated,
        other => {
            tracing::warn!(
                status = other,
                "unknown subagent status type, treating as Working"
            );
            SubagentStatus::Working {
                message: working_message,
            }
        }
    };

    let group = v.get("group").and_then(|g| g.as_str()).map(String::from);
    let role = v.get("role").and_then(|r| r.as_str()).map(String::from);
    let depends_on = parse_string_array(v, "dependsOn");

    Some(SubagentInfo::new(
        session_id,
        session_name,
        agent_name,
        initial_query,
        status,
        group,
        role,
        depends_on,
    ))
}

/// Parse a single pending stage entry from a `subagent/list_update` JSON array element.
fn parse_pending_stage(v: &serde_json::Value) -> Option<PendingStage> {
    let name = match v
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(n) => n,
        None => {
            tracing::warn!("pending stage entry missing or empty name, skipping");
            return None;
        }
    };
    let agent_name = v
        .get("agentName")
        .and_then(|n| n.as_str())
        .map(String::from);
    let group = v.get("group").and_then(|g| g.as_str()).map(String::from);
    let role = v.get("role").and_then(|r| r.as_str()).map(String::from);
    let depends_on = parse_string_array(v, "dependsOn");

    Some(PendingStage::new(name, agent_name, group, role, depends_on))
}

/// Extract a string array from a JSON value by key. Returns empty vec on missing/malformed.
fn parse_string_array(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn to_ext_notification(
    method: &str,
    params: &serde_json::Value,
) -> crate::Result<Option<Notification>> {
    match method {
        "kiro.dev/metadata" => {
            let pct = match params
                .get("contextUsagePercentage")
                .and_then(|v| v.as_f64())
            {
                Some(v) => v,
                None => {
                    tracing::warn!(
                        "kiro.dev/metadata missing or non-numeric contextUsagePercentage"
                    );
                    0.0
                }
            };

            let metering = params
                .get("meteringUsage")
                .and_then(|m| m.as_array())
                .and_then(|arr| {
                    let credits: f64 = arr
                        .iter()
                        .filter_map(|u| u.get("value").and_then(|v| v.as_f64()))
                        .sum();
                    if credits > 0.0 {
                        let duration_ms = params.get("turnDurationMs").and_then(|d| d.as_u64());
                        Some(TurnMetering::new(credits, duration_ms))
                    } else {
                        None
                    }
                });

            let tokens = {
                let input = params.get("inputTokens").and_then(|v| v.as_u64());
                let output = params.get("outputTokens").and_then(|v| v.as_u64());
                let cached = params.get("cachedTokens").and_then(|v| v.as_u64());
                match (input, output) {
                    (Some(i), Some(o)) => Some(TokenCounts::new(i, o, cached)),
                    _ => None,
                }
            };

            Ok(Some(Notification::MetadataUpdated {
                context_usage: ContextUsage::new(pct),
                metering,
                tokens,
            }))
        }
        "kiro.dev/compaction/status" => {
            let message = if let Some(status) = params.get("status") {
                let status_type = status
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                match status_type {
                    "started" => "Compacting conversation context...".to_string(),
                    "completed" => {
                        let summary = params
                            .get("summary")
                            .and_then(|s| s.as_str())
                            .unwrap_or("done");
                        format!("Compaction completed: {summary}")
                    }
                    "failed" => {
                        let error = status
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("unknown error");
                        format!("Compaction failed: {error}")
                    }
                    other => format!("Compaction: {other}"),
                }
            } else {
                match params
                    .get("message")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    Some(msg) => msg.to_string(),
                    None => {
                        tracing::warn!(
                            "kiro.dev/compaction/status: no status object or message field"
                        );
                        return Ok(None);
                    }
                }
            };
            Ok(Some(Notification::CompactionStatus { message }))
        }
        "kiro.dev/clear/status" => {
            let message = match params
                .get("message")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(msg) => msg.to_string(),
                None => {
                    tracing::debug!("kiro.dev/clear/status: empty or missing message");
                    return Ok(None);
                }
            };
            Ok(Some(Notification::ClearStatus { message }))
        }
        "kiro.dev/agent/switched" => {
            let name = params
                .get("agentName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/agent/switched: missing or empty agentName");
                    "unknown"
                })
                .to_string();
            let welcome = params
                .get("welcomeMessage")
                .and_then(|v| v.as_str())
                .map(String::from);
            let previous_agent = params
                .get("previousAgentName")
                .and_then(|v| v.as_str())
                .map(String::from);
            let model = params
                .get("model")
                .and_then(|v| v.as_str())
                .map(String::from);
            Ok(Some(Notification::AgentSwitched {
                name,
                welcome,
                previous_agent,
                model,
            }))
        }
        "kiro.dev/commands/available" => {
            // Parse the commands list from the payload.
            // Kiro sends: {"commands": [...]} or {"availableCommands": [...]} or just [...]
            let commands_value = params
                .get("commands")
                .or_else(|| params.get("availableCommands"))
                .unwrap_or(params);

            let commands = if let Some(arr) = commands_value.as_array() {
                arr.iter()
                    .filter_map(|v| {
                        let raw_name = v
                            .get("name")
                            .or_else(|| v.get("command"))
                            .and_then(|n| n.as_str())?;
                        let name = raw_name.strip_prefix('/').unwrap_or(raw_name);
                        let label = v.get("label").and_then(|l| l.as_str()).unwrap_or(name);
                        let description = v
                            .get("description")
                            .and_then(|d| d.as_str())
                            .map(String::from);

                        let meta = v.get("meta");
                        let is_selection = meta
                            .and_then(|m| m.get("inputType"))
                            .and_then(|t| t.as_str())
                            == Some("selection");
                        let is_local = meta
                            .and_then(|m| m.get("local"))
                            .and_then(|l| l.as_bool())
                            .unwrap_or(false);

                        // Backward compat: hasOptions field OR selection inputType
                        let has_options = is_selection
                            || v.get("hasOptions")
                                .and_then(|h| h.as_bool())
                                .unwrap_or(false);

                        Some(CommandInfo::new(
                            name,
                            label,
                            description,
                            has_options,
                            is_selection,
                            is_local,
                        ))
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let prompts = params
                .get("prompts")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            let name = v.get("name")?.as_str()?;
                            let description = v
                                .get("description")
                                .and_then(|d| d.as_str())
                                .map(String::from);
                            let server_name = v
                                .get("serverName")
                                .and_then(|s| s.as_str())
                                .map(String::from);
                            let arguments = v
                                .get("arguments")
                                .and_then(|a| a.as_array())
                                .map(|args| {
                                    args.iter()
                                        .filter_map(|arg| {
                                            let arg_name = arg.get("name")?.as_str()?;
                                            let required = arg
                                                .get("required")
                                                .and_then(|r| r.as_bool())
                                                .unwrap_or(false);
                                            let desc = arg
                                                .get("description")
                                                .and_then(|d| d.as_str())
                                                .map(String::from);
                                            Some(PromptArgument::new(arg_name, desc, required))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            Some(PromptInfo::new(name, description, server_name, arguments))
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(Some(Notification::CommandsUpdated { commands, prompts }))
        }
        "kiro.dev/session/update" => {
            let update = params.get("update");
            let session_update = update
                .and_then(|u| u.get("sessionUpdate"))
                .and_then(|s| s.as_str());
            match session_update {
                Some("tool_call_chunk") => {
                    let tool_call_id = match update
                        .and_then(|u| u.get("toolCallId"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        Some(id) => id.to_string(),
                        None => {
                            tracing::warn!("tool_call_chunk missing or empty toolCallId, dropping");
                            return Ok(None);
                        }
                    };
                    let title = update
                        .and_then(|u| u.get("title"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let kind = update
                        .and_then(|u| u.get("kind"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let ext_session_id = params
                        .get("sessionId")
                        .and_then(|s| s.as_str())
                        .filter(|s| !s.is_empty())
                        .map(SessionId::new);
                    Ok(Some(Notification::ToolCallChunk {
                        tool_call_id: ToolCallId::new(tool_call_id),
                        title,
                        kind,
                        session_id: ext_session_id,
                    }))
                }
                Some(other) => {
                    tracing::debug!(variant = other, "unhandled kiro.dev/session/update variant");
                    Err(crate::Error::from_kind(crate::ErrorKind::Protocol {
                        message: format!("unhandled session/update variant: {other}"),
                    }))
                }
                None => {
                    tracing::debug!("kiro.dev/session/update missing sessionUpdate field");
                    Err(crate::Error::from_kind(crate::ErrorKind::Protocol {
                        message: "missing sessionUpdate field".into(),
                    }))
                }
            }
        }
        "kiro.dev/error/rate_limit" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Rate limit exceeded")
                .to_string();
            Ok(Some(Notification::RateLimited { message }))
        }
        "kiro.dev/mcp/server_init_failure" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/mcp/server_init_failure: missing or empty serverName");
                    "unknown"
                })
                .to_string();
            let error = params
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::McpServerInitFailure {
                server_name,
                error,
            }))
        }
        "kiro.dev/mcp/oauth_request" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/mcp/oauth_request: missing or empty serverName");
                    "unknown"
                })
                .to_string();
            let url = params
                .get("oauthUrl")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("")
                .to_string();
            if url.is_empty() {
                tracing::warn!("mcp/oauth_request missing oauthUrl");
                Ok(None)
            } else {
                Ok(Some(Notification::McpOAuthRequest { server_name, url }))
            }
        }
        "kiro.dev/mcp/server_initialized" => {
            let server_name = params
                .get("serverName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/mcp/server_initialized: missing or empty serverName");
                    "unknown"
                })
                .to_string();
            Ok(Some(Notification::McpServerInitialized { server_name }))
        }
        "kiro.dev/agent/not_found" => {
            let requested = params
                .get("requestedAgent")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/agent/not_found: missing or empty requestedAgent");
                    "unknown"
                })
                .to_string();
            let fallback = params
                .get("fallbackAgent")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::AgentNotFound {
                requested,
                fallback,
            }))
        }
        "kiro.dev/agent/config_error" => {
            let path = params
                .get("path")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/agent/config_error: missing or empty path");
                    "(unknown path)"
                })
                .to_string();
            let error = params
                .get("error")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/agent/config_error: missing or empty error");
                    "(no detail)"
                })
                .to_string();
            Ok(Some(Notification::AgentConfigError { path, error }))
        }
        "kiro.dev/model/not_found" => {
            let requested = params
                .get("requestedModel")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    tracing::warn!("kiro.dev/model/not_found: missing or empty requestedModel");
                    "unknown"
                })
                .to_string();
            let fallback = params
                .get("fallbackModel")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            Ok(Some(Notification::ModelNotFound {
                requested,
                fallback,
            }))
        }
        "kiro.dev/subagent/list_update" => {
            let subagents_raw = params.get("subagents").and_then(|s| s.as_array());
            if subagents_raw.is_none() {
                tracing::warn!("subagent/list_update missing subagents array");
            }
            let subagents = subagents_raw
                .map(|arr| arr.iter().filter_map(parse_subagent_entry).collect())
                .unwrap_or_default();

            let pending_stages_raw = params.get("pendingStages").and_then(|s| s.as_array());
            if pending_stages_raw.is_none() && subagents_raw.is_some() {
                tracing::warn!("subagent/list_update missing pendingStages array");
            }
            let pending_stages = pending_stages_raw
                .map(|arr| arr.iter().filter_map(parse_pending_stage).collect())
                .unwrap_or_default();

            Ok(Some(Notification::SubagentListUpdated {
                subagents,
                pending_stages,
            }))
        }
        "kiro.dev/session/inbox_notification" => {
            let session_id = match params
                .get("sessionId")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(id) => SessionId::new(id),
                None => {
                    tracing::warn!("inbox_notification missing or empty sessionId, dropping");
                    return Ok(None);
                }
            };
            let message_count = match params.get("messageCount").and_then(|m| m.as_u64()) {
                Some(n) => n as u32,
                None => {
                    tracing::warn!("inbox_notification missing messageCount, defaulting to 0");
                    0
                }
            };
            let escalation_count = match params.get("escalationCount").and_then(|e| e.as_u64()) {
                Some(n) => n as u32,
                None => {
                    tracing::warn!("inbox_notification missing escalationCount, defaulting to 0");
                    0
                }
            };
            let senders = params
                .get("senders")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            Ok(Some(Notification::InboxNotification {
                session_id,
                message_count,
                escalation_count,
                senders,
            }))
        }
        "kiro.dev/session/activity" | "kiro.dev/session/list_update" => {
            tracing::debug!(
                method,
                "multi-session notification acknowledged, not forwarded"
            );
            Ok(None)
        }
        other => {
            tracing::debug!(method = other, "unknown extension notification");
            Ok(None)
        }
    }
}

/// Build a `ToolCall` from the `ToolCallUpdate` inside a permission request,
/// enriching it with cached `raw_input` when the update doesn't carry one.
pub(crate) fn to_tool_call_from_permission(
    args: &acp::RequestPermissionRequest,
    cached: &HashMap<String, serde_json::Value>,
) -> ToolCall {
    let update = &args.tool_call;
    let id_str = update.tool_call_id.to_string();
    let title = update.fields.title.clone().unwrap_or_default();
    let kind = update
        .fields
        .kind
        .map(to_tool_kind)
        .unwrap_or(ToolKind::Other);
    let status = update
        .fields
        .status
        .map(to_tool_call_status)
        .unwrap_or(ToolCallStatus::Pending);
    let raw_input = cached
        .get(&id_str)
        .cloned()
        .or_else(|| update.fields.raw_input.clone());

    let content = update
        .fields
        .content
        .as_deref()
        .map(convert_tool_call_content)
        .unwrap_or_default();
    let locations = update
        .fields
        .locations
        .as_deref()
        .map(convert_tool_call_locations)
        .unwrap_or_default();

    ToolCall::new(ToolCallId::new(id_str), title, kind, status, raw_input)
        .with_content(content)
        .with_locations(locations)
        .with_raw_output(update.fields.raw_output.clone())
}

/// Convert ACP permission options to our internal representation.
pub(crate) fn to_permission_options(args: &acp::RequestPermissionRequest) -> Vec<PermissionOption> {
    args.options
        .iter()
        .map(|opt| {
            let is_destructive = matches!(
                opt.kind,
                acp::PermissionOptionKind::RejectOnce | acp::PermissionOptionKind::RejectAlways
            );
            PermissionOption {
                id: opt.option_id.to_string(),
                label: opt.name.clone(),
                is_destructive,
            }
        })
        .collect()
}

/// Extract a human-readable message from a permission request.
/// Falls back to the tool call title if no dedicated message field exists.
pub(crate) fn extract_permission_message(args: &acp::RequestPermissionRequest) -> String {
    args.tool_call
        .fields
        .title
        .clone()
        .unwrap_or_else(|| "Permission requested".to_string())
}

/// Convert our `PermissionResponse` back into an ACP `RequestPermissionResponse`.
/// Uses the option IDs from the original request to map our response variants.
pub(crate) fn from_permission_response(
    response: PermissionResponse,
    args: &acp::RequestPermissionRequest,
) -> acp::RequestPermissionResponse {
    let outcome = match response {
        PermissionResponse::Cancel => acp::RequestPermissionOutcome::Cancelled,
        PermissionResponse::AllowOnce => {
            let option_id = find_option_id(args, acp::PermissionOptionKind::AllowOnce);
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id))
        }
        PermissionResponse::AllowAlways => {
            let option_id = find_option_id(args, acp::PermissionOptionKind::AllowAlways);
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id))
        }
        PermissionResponse::Reject => {
            let option_id = find_option_id(args, acp::PermissionOptionKind::RejectOnce);
            acp::RequestPermissionOutcome::Selected(acp::SelectedPermissionOutcome::new(option_id))
        }
    };
    acp::RequestPermissionResponse::new(outcome)
}

/// Find the option ID for a given permission kind in the request.
/// Falls back to the first option ID if the exact kind isn't found.
fn find_option_id(
    args: &acp::RequestPermissionRequest,
    target_kind: acp::PermissionOptionKind,
) -> acp::PermissionOptionId {
    if let Some(opt) = args.options.iter().find(|o| o.kind == target_kind) {
        return opt.option_id.clone();
    }

    tracing::warn!(
        ?target_kind,
        "permission option kind not found, falling back to first available option"
    );

    args.options
        .first()
        .map(|o| o.option_id.clone())
        .unwrap_or_else(|| {
            tracing::error!("no permission options available, fabricating allow_once ID");
            acp::PermissionOptionId::new("allow_once")
        })
}

/// Cache `raw_input` from tool call and tool call update notifications,
/// keyed by tool call ID. Permission requests arrive without `raw_input`,
/// so the client looks it up from this cache.
pub(crate) fn cache_tool_call_input(
    args: &acp::SessionNotification,
    cache: &RefCell<HashMap<String, serde_json::Value>>,
) {
    match &args.update {
        acp::SessionUpdate::ToolCall(tc) => {
            if let Some(ref raw_input) = tc.raw_input {
                cache
                    .borrow_mut()
                    .insert(tc.tool_call_id.to_string(), raw_input.clone());
            }
        }
        acp::SessionUpdate::ToolCallUpdate(update) => {
            if let Some(ref raw_input) = update.fields.raw_input {
                cache
                    .borrow_mut()
                    .insert(update.tool_call_id.to_string(), raw_input.clone());
            }
        }
        _ => {}
    }
}

/// Convert an ACP `SessionNotification` to our internal `Notification`.
/// Returns `None` for update types we don't surface to the UI.
pub(crate) fn session_update_to_notification(
    args: &acp::SessionNotification,
    cached_inputs: &HashMap<String, serde_json::Value>,
) -> Option<Notification> {
    match &args.update {
        acp::SessionUpdate::AgentMessageChunk(chunk) => {
            if let acp::ContentBlock::Text(ref text) = chunk.content {
                Some(Notification::AgentMessage(AgentMessage {
                    text: text.text.clone(),
                    is_streaming: true,
                }))
            } else {
                None
            }
        }
        acp::SessionUpdate::AgentThoughtChunk(chunk) => {
            if let acp::ContentBlock::Text(ref text) = chunk.content {
                Some(Notification::AgentThought(AgentThought {
                    text: text.text.clone(),
                }))
            } else {
                None
            }
        }
        acp::SessionUpdate::ToolCall(tc) => Some(Notification::ToolCallStarted(to_tool_call(
            tc,
            cached_inputs,
        ))),
        acp::SessionUpdate::ToolCallUpdate(update) => {
            let id_str = update.tool_call_id.to_string();
            let title = update.fields.title.clone().unwrap_or_default();
            let kind = update
                .fields
                .kind
                .map(to_tool_kind)
                .unwrap_or(ToolKind::Other);
            let status = update
                .fields
                .status
                .map(to_tool_call_status)
                .unwrap_or(ToolCallStatus::Pending);
            let raw_input = cached_inputs
                .get(&id_str)
                .cloned()
                .or_else(|| update.fields.raw_input.clone());

            let content = update
                .fields
                .content
                .as_deref()
                .map(convert_tool_call_content)
                .unwrap_or_default();
            let locations = update
                .fields
                .locations
                .as_deref()
                .map(convert_tool_call_locations)
                .unwrap_or_default();

            Some(Notification::ToolCallUpdated(
                ToolCall::new(ToolCallId::new(id_str), title, kind, status, raw_input)
                    .with_content(content)
                    .with_locations(locations)
                    .with_raw_output(update.fields.raw_output.clone()),
            ))
        }
        acp::SessionUpdate::Plan(plan) => {
            let entries = plan
                .entries
                .iter()
                .map(|e| {
                    let status = match e.status {
                        acp::PlanEntryStatus::Pending => PlanEntryStatus::Pending,
                        acp::PlanEntryStatus::InProgress => PlanEntryStatus::InProgress,
                        acp::PlanEntryStatus::Completed => PlanEntryStatus::Completed,
                        _ => PlanEntryStatus::Failed,
                    };
                    let priority = match e.priority {
                        acp::PlanEntryPriority::High => PlanEntryPriority::High,
                        acp::PlanEntryPriority::Medium => PlanEntryPriority::Medium,
                        acp::PlanEntryPriority::Low => PlanEntryPriority::Low,
                        _ => PlanEntryPriority::Medium,
                    };
                    PlanEntry::new(e.content.clone(), status, priority)
                })
                .collect();
            Some(Notification::PlanUpdated(Plan::new(entries)))
        }
        acp::SessionUpdate::CurrentModeUpdate(mode) => Some(Notification::ModeChanged {
            mode_id: mode.current_mode_id.to_string(),
        }),
        acp::SessionUpdate::ConfigOptionUpdate(update) => {
            let options = update
                .config_options
                .iter()
                .filter_map(|opt| match &opt.kind {
                    acp::SessionConfigKind::Select(select) => {
                        let values = match &select.options {
                            acp::SessionConfigSelectOptions::Ungrouped(flat) => {
                                flat.iter().map(|v| v.value.to_string()).collect()
                            }
                            acp::SessionConfigSelectOptions::Grouped(groups) => groups
                                .iter()
                                .flat_map(|g| g.options.iter().map(|v| v.value.to_string()))
                                .collect(),
                            _ => Vec::new(),
                        };
                        Some(ConfigOption {
                            key: opt.id.to_string(),
                            label: opt.name.clone(),
                            value: Some(select.current_value.to_string()),
                            options: values,
                        })
                    }
                    _ => None,
                })
                .collect();
            Some(Notification::ConfigOptionsUpdated(options))
        }
        acp::SessionUpdate::AvailableCommandsUpdate(update) => {
            let commands = update
                .available_commands
                .iter()
                .map(|cmd| {
                    CommandInfo::new(
                        cmd.name.clone(),
                        cmd.description.clone(),
                        None::<String>,
                        cmd.input.is_some(),
                        false,
                        false,
                    )
                })
                .collect();
            Some(Notification::CommandsUpdated {
                commands,
                prompts: Vec::new(),
            })
        }
        _ => {
            tracing::debug!("unhandled session update variant");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_tool_kind_read() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Read),
            ToolKind::Read
        );
    }

    #[test]
    fn to_tool_kind_edit_maps_to_write() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Edit),
            ToolKind::Write
        );
    }

    #[test]
    fn to_tool_kind_delete_maps_to_write() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Delete),
            ToolKind::Write
        );
    }

    #[test]
    fn to_tool_kind_move_maps_to_write() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Move),
            ToolKind::Write
        );
    }

    #[test]
    fn to_tool_kind_execute() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Execute),
            ToolKind::Execute
        );
    }

    #[test]
    fn to_tool_kind_other() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Other),
            ToolKind::Other
        );
    }

    #[test]
    fn to_tool_kind_search() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Search),
            ToolKind::Search
        );
    }

    #[test]
    fn to_tool_kind_think() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Think),
            ToolKind::Think
        );
    }

    #[test]
    fn to_tool_kind_fetch() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Fetch),
            ToolKind::Fetch
        );
    }

    #[test]
    fn to_tool_call_status_in_progress() {
        assert_eq!(
            to_tool_call_status(agent_client_protocol::ToolCallStatus::InProgress),
            ToolCallStatus::InProgress
        );
    }

    #[test]
    fn to_tool_call_status_pending() {
        assert_eq!(
            to_tool_call_status(agent_client_protocol::ToolCallStatus::Pending),
            ToolCallStatus::Pending
        );
    }

    #[test]
    fn to_tool_call_status_completed() {
        assert_eq!(
            to_tool_call_status(agent_client_protocol::ToolCallStatus::Completed),
            ToolCallStatus::Completed
        );
    }

    #[test]
    fn to_tool_call_status_failed() {
        assert_eq!(
            to_tool_call_status(agent_client_protocol::ToolCallStatus::Failed),
            ToolCallStatus::Failed
        );
    }

    #[test]
    fn to_ext_notification_unknown_method_returns_none() {
        let result = to_ext_notification("unknown.method", &serde_json::json!({}));
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn to_ext_notification_metadata() {
        let params = serde_json::json!({"contextUsagePercentage": 75.0});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::MetadataUpdated {
            context_usage,
            metering,
            tokens,
        })) = result
        {
            assert!((context_usage.percentage() - 75.0).abs() < f64::EPSILON);
            assert!(metering.is_none());
            assert!(tokens.is_none());
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn parse_metadata_with_metering() {
        let params = serde_json::json!({
            "sessionId": "s1",
            "contextUsagePercentage": 7.11,
            "meteringUsage": [
                {"unit": "credit", "unitPlural": "credits", "value": 0.018}
            ],
            "turnDurationMs": 1948
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated {
            context_usage,
            metering,
            ..
        })) = result
        {
            assert!((context_usage.percentage() - 7.11).abs() < 0.01);
            let m = metering.unwrap();
            assert!((m.credits() - 0.018).abs() < 0.001);
            assert_eq!(m.duration_ms(), Some(1948));
        } else {
            panic!("expected MetadataUpdated, got {:?}", result);
        }
    }

    #[test]
    fn parse_metadata_without_metering() {
        let params = serde_json::json!({
            "sessionId": "s1",
            "contextUsagePercentage": 2.28
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated {
            metering, tokens, ..
        })) = result
        {
            assert!(metering.is_none());
            assert!(tokens.is_none());
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn parse_metadata_with_tokens() {
        let params = serde_json::json!({
            "contextUsagePercentage": 15.0,
            "inputTokens": 1500,
            "outputTokens": 300,
            "cachedTokens": 200
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { tokens, .. })) = result {
            let t = tokens.expect("tokens should be present");
            assert_eq!(t.input(), 1500);
            assert_eq!(t.output(), 300);
            assert_eq!(t.cached(), Some(200));
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn parse_metadata_with_partial_tokens() {
        let params = serde_json::json!({
            "contextUsagePercentage": 15.0,
            "inputTokens": 1500
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { tokens, .. })) = result {
            assert!(tokens.is_none(), "partial tokens should produce None");
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_legacy() {
        let params = serde_json::json!({"message": "50% done"});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { message })) = result {
            assert_eq!(message, "50% done");
        } else {
            panic!("expected CompactionStatus");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_started() {
        let params = serde_json::json!({"status": {"type": "started"}});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { message })) = result {
            assert!(message.contains("Compacting"), "got: {message}");
        } else {
            panic!("expected CompactionStatus");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_failed() {
        let params = serde_json::json!({"status": {"type": "failed", "error": "out of memory"}});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { message })) = result {
            assert!(message.contains("out of memory"), "got: {message}");
        } else {
            panic!("expected CompactionStatus");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_completed() {
        let params =
            serde_json::json!({"status": {"type": "completed"}, "summary": "3 turns removed"});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { message })) = result {
            assert!(message.contains("3 turns removed"), "got: {message}");
        } else {
            panic!("expected CompactionStatus");
        }
    }

    #[test]
    fn to_ext_notification_clear_status() {
        let params = serde_json::json!({"message": "cleared"});
        let result = to_ext_notification("kiro.dev/clear/status", &params);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(Some(Notification::ClearStatus { .. }))));
    }

    #[test]
    fn to_ext_notification_agent_switched() {
        let params = serde_json::json!({"agentName": "code-agent", "welcomeMessage": "Hello!"});
        let result = to_ext_notification("kiro.dev/agent/switched", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::AgentSwitched { name, welcome, .. })) = result {
            assert_eq!(name, "code-agent");
            assert_eq!(welcome.as_deref(), Some("Hello!"));
        } else {
            panic!("expected AgentSwitched");
        }
    }

    #[test]
    fn to_tool_call_uses_cached_input_when_available() {
        let acp_call = agent_client_protocol::ToolCall::new("tc_1", "Read file")
            .kind(agent_client_protocol::ToolKind::Read)
            .status(agent_client_protocol::ToolCallStatus::InProgress)
            .raw_input(serde_json::json!({"path": "original.rs"}));

        let mut cached = std::collections::HashMap::new();
        cached.insert("tc_1".to_string(), serde_json::json!({"path": "cached.rs"}));

        let result = to_tool_call(&acp_call, &cached);
        assert_eq!(result.id().as_str(), "tc_1");
        assert_eq!(
            result.raw_input(),
            Some(&serde_json::json!({"path": "cached.rs"}))
        );
    }

    #[test]
    fn to_tool_call_falls_back_to_raw_input() {
        let acp_call = agent_client_protocol::ToolCall::new("tc_2", "Execute command")
            .kind(agent_client_protocol::ToolKind::Execute)
            .status(agent_client_protocol::ToolCallStatus::Completed)
            .raw_input(serde_json::json!({"cmd": "ls"}));

        let cached = std::collections::HashMap::new();
        let result = to_tool_call(&acp_call, &cached);
        assert_eq!(result.raw_input(), Some(&serde_json::json!({"cmd": "ls"})));
    }

    #[test]
    fn to_ext_notification_commands_available_with_commands_key() {
        let params = serde_json::json!({
            "commands": [
                {"name": "model", "label": "Switch model", "description": "Change model", "hasOptions": true},
                {"name": "compact", "label": "Compact", "hasOptions": false}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert_eq!(cmds.len(), 2);
            assert_eq!(cmds[0].name(), "model");
            assert_eq!(cmds[0].label(), "Switch model");
            assert_eq!(cmds[0].description(), Some("Change model"));
            assert!(cmds[0].has_options());
            assert_eq!(cmds[1].name(), "compact");
            assert!(!cmds[1].has_options());
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn to_ext_notification_commands_available_with_available_commands_key() {
        let params = serde_json::json!({
            "availableCommands": [
                {"name": "tools", "label": "Show tools"}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert_eq!(cmds.len(), 1);
            assert_eq!(cmds[0].name(), "tools");
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn to_ext_notification_commands_available_empty_payload() {
        let params = serde_json::json!({});
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert!(cmds.is_empty());
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn to_ext_notification_session_update_tool_call_chunk() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "tool_call_chunk",
                "toolCallId": "tc_123",
                "title": "reading main.rs",
                "kind": "read"
            }
        });
        let result = to_ext_notification("kiro.dev/session/update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::ToolCallChunk {
            tool_call_id,
            title,
            kind,
            session_id,
        })) = result
        {
            assert_eq!(tool_call_id.as_str(), "tc_123");
            assert_eq!(title, "reading main.rs");
            assert_eq!(kind, "read");
            assert!(session_id.is_none());
        } else {
            panic!("expected ToolCallChunk");
        }
    }

    #[test]
    fn to_ext_notification_session_update_unknown_variant_returns_error() {
        let params = serde_json::json!({
            "update": {
                "sessionUpdate": "some_future_variant"
            }
        });
        let result = to_ext_notification("kiro.dev/session/update", &params);
        assert!(result.is_err());
    }

    #[test]
    fn to_ext_notification_session_update_missing_session_update_field() {
        let params = serde_json::json!({"update": {}});
        let result = to_ext_notification("kiro.dev/session/update", &params);
        assert!(result.is_err());
    }

    /// Helper to build a `RequestPermissionRequest` with given option kinds.
    fn make_permission_request(
        options: Vec<(
            &'static str,
            &'static str,
            agent_client_protocol::PermissionOptionKind,
        )>,
    ) -> acp::RequestPermissionRequest {
        let tool_call_update = acp::ToolCallUpdate::new(
            "tc_perm",
            acp::ToolCallUpdateFields::new()
                .title("Run command")
                .kind(acp::ToolKind::Execute)
                .status(acp::ToolCallStatus::Pending),
        );
        let perm_options: Vec<acp::PermissionOption> = options
            .into_iter()
            .map(|(id, name, kind)| acp::PermissionOption::new(id, name, kind))
            .collect();
        acp::RequestPermissionRequest::new("sess_1", tool_call_update, perm_options)
    }

    #[test]
    fn find_option_id_exact_match() {
        let req = make_permission_request(vec![
            ("opt_allow", "Yes", acp::PermissionOptionKind::AllowOnce),
            (
                "opt_always",
                "Always",
                acp::PermissionOptionKind::AllowAlways,
            ),
            ("opt_reject", "No", acp::PermissionOptionKind::RejectOnce),
        ]);

        let allow_id = find_option_id(&req, acp::PermissionOptionKind::AllowOnce);
        assert_eq!(allow_id.to_string(), "opt_allow");

        let reject_id = find_option_id(&req, acp::PermissionOptionKind::RejectOnce);
        assert_eq!(reject_id.to_string(), "opt_reject");
    }

    #[test]
    fn find_option_id_fallback_to_first() {
        let req = make_permission_request(vec![(
            "opt_allow",
            "Yes",
            acp::PermissionOptionKind::AllowOnce,
        )]);

        // RejectOnce doesn't exist, should fall back to first option (AllowOnce)
        let id = find_option_id(&req, acp::PermissionOptionKind::RejectOnce);
        assert_eq!(id.to_string(), "opt_allow");
    }

    #[test]
    fn from_permission_response_allow_once() {
        let req = make_permission_request(vec![
            ("opt_allow", "Yes", acp::PermissionOptionKind::AllowOnce),
            (
                "opt_always",
                "Always",
                acp::PermissionOptionKind::AllowAlways,
            ),
            ("opt_reject", "No", acp::PermissionOptionKind::RejectOnce),
        ]);

        let resp = from_permission_response(PermissionResponse::AllowOnce, &req);
        if let acp::RequestPermissionOutcome::Selected(selected) = resp.outcome {
            assert_eq!(selected.option_id.to_string(), "opt_allow");
        } else {
            panic!("expected Selected outcome");
        }
    }

    #[test]
    fn from_permission_response_cancel() {
        let req = make_permission_request(vec![(
            "opt_allow",
            "Yes",
            acp::PermissionOptionKind::AllowOnce,
        )]);

        let resp = from_permission_response(PermissionResponse::Cancel, &req);
        assert!(matches!(
            resp.outcome,
            acp::RequestPermissionOutcome::Cancelled
        ));
    }

    #[test]
    fn to_ext_notification_commands_strips_slash_prefix() {
        let params = serde_json::json!({
            "commands": [
                {"name": "/model", "description": "Switch model", "meta": {"inputType": "selection"}}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert_eq!(cmds[0].name(), "model", "leading / should be stripped");
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn to_ext_notification_commands_parses_selection_type() {
        let params = serde_json::json!({
            "commands": [
                {"name": "/model", "description": "Switch model", "meta": {"inputType": "selection"}},
                {"name": "/compact", "description": "Compact context"}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert!(cmds[0].is_selection(), "/model should be selection");
            assert!(!cmds[1].is_selection(), "/compact should not be selection");
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn parse_rate_limit_error() {
        let params = serde_json::json!({
            "message": "Rate limit exceeded. Please wait before retrying."
        });
        let result = to_ext_notification("kiro.dev/error/rate_limit", &params);
        if let Ok(Some(Notification::RateLimited { message })) = result {
            assert!(message.contains("Rate limit"));
        } else {
            panic!("expected RateLimited, got {:?}", result);
        }
    }

    #[test]
    fn parse_rate_limit_error_missing_message() {
        let params = serde_json::json!({});
        let result = to_ext_notification("kiro.dev/error/rate_limit", &params);
        if let Ok(Some(Notification::RateLimited { message })) = result {
            assert!(!message.is_empty());
        } else {
            panic!("expected RateLimited");
        }
    }

    #[test]
    fn parse_mcp_server_init_failure() {
        let params = serde_json::json!({
            "serverName": "my-mcp",
            "error": "connection refused"
        });
        let result = to_ext_notification("kiro.dev/mcp/server_init_failure", &params);
        if let Ok(Some(Notification::McpServerInitFailure { server_name, error })) = result {
            assert_eq!(server_name, "my-mcp");
            assert_eq!(error.as_deref(), Some("connection refused"));
        } else {
            panic!("expected McpServerInitFailure, got {:?}", result);
        }
    }

    #[test]
    fn parse_mcp_server_init_failure_no_error() {
        let params = serde_json::json!({ "serverName": "my-mcp" });
        let result = to_ext_notification("kiro.dev/mcp/server_init_failure", &params);
        if let Ok(Some(Notification::McpServerInitFailure { server_name, error })) = result {
            assert_eq!(server_name, "my-mcp");
            assert!(error.is_none());
        } else {
            panic!("expected McpServerInitFailure");
        }
    }

    #[test]
    fn parse_mcp_oauth_request() {
        let params = serde_json::json!({
            "serverName": "github-mcp",
            "oauthUrl": "https://github.com/login/oauth/authorize?client_id=abc"
        });
        let result = to_ext_notification("kiro.dev/mcp/oauth_request", &params);
        if let Ok(Some(Notification::McpOAuthRequest { server_name, url })) = result {
            assert_eq!(server_name, "github-mcp");
            assert!(url.starts_with("https://"));
        } else {
            panic!("expected McpOAuthRequest, got {:?}", result);
        }
    }

    #[test]
    fn parse_mcp_oauth_request_missing_url() {
        let params = serde_json::json!({ "serverName": "github-mcp" });
        let result = to_ext_notification("kiro.dev/mcp/oauth_request", &params);
        assert!(
            matches!(result, Ok(None)),
            "missing oauthUrl should return None"
        );
    }

    #[test]
    fn parse_mcp_server_initialized() {
        let params = serde_json::json!({ "serverName": "github-mcp" });
        let result = to_ext_notification("kiro.dev/mcp/server_initialized", &params);
        if let Ok(Some(Notification::McpServerInitialized { server_name })) = result {
            assert_eq!(server_name, "github-mcp");
        } else {
            panic!("expected McpServerInitialized, got {:?}", result);
        }
    }

    // --- convert_tool_call_content tests ---

    #[test]
    fn convert_tool_call_content_diff() {
        let diff = acp::Diff::new("src/main.rs", "new code").old_text("old code");
        let acp_content = vec![acp::ToolCallContent::Diff(diff)];
        let result = convert_tool_call_content(&acp_content);
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            ToolCallContent::Diff {
                path,
                old_text,
                new_text,
            } if path == "src/main.rs"
                && old_text.as_deref() == Some("old code")
                && new_text == "new code"
        ));
    }

    #[test]
    fn convert_tool_call_content_empty() {
        let result = convert_tool_call_content(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn convert_tool_call_content_text_via_content_block() {
        let text_block = acp::ContentBlock::from("hello world");
        let acp_content = vec![acp::ToolCallContent::Content(acp::Content::new(text_block))];
        let result = convert_tool_call_content(&acp_content);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ToolCallContent::Text(t) if t == "hello world"));
    }

    // --- convert_tool_call_locations tests ---

    #[test]
    fn convert_tool_call_locations_basic() {
        let loc = acp::ToolCallLocation::new("src/lib.rs").line(42u32);
        let result = convert_tool_call_locations(&[loc]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "src/lib.rs");
        assert_eq!(result[0].line, Some(42));
    }

    #[test]
    fn convert_tool_call_locations_without_line() {
        let loc = acp::ToolCallLocation::new("Cargo.toml");
        let result = convert_tool_call_locations(&[loc]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "Cargo.toml");
        assert!(result[0].line.is_none());
    }

    #[test]
    fn convert_tool_call_locations_empty() {
        let result = convert_tool_call_locations(&[]);
        assert!(result.is_empty());
    }

    // --- cache_tool_call_input tests ---

    #[test]
    fn cache_tool_call_input_from_tool_call() {
        let cache = RefCell::new(HashMap::new());
        let tc = acp::ToolCall::new("tc_1", "Read file")
            .raw_input(serde_json::json!({"path": "test.rs"}));
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("sess"),
            acp::SessionUpdate::ToolCall(tc),
        );
        cache_tool_call_input(&notification, &cache);
        let borrowed = cache.borrow();
        assert!(borrowed.contains_key("tc_1"));
        assert_eq!(borrowed["tc_1"], serde_json::json!({"path": "test.rs"}));
    }

    #[test]
    fn cache_tool_call_input_ignores_non_tool_updates() {
        let cache = RefCell::new(HashMap::new());
        let chunk = acp::ContentChunk::new(acp::ContentBlock::from("hello"));
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("sess"),
            acp::SessionUpdate::AgentMessageChunk(chunk),
        );
        cache_tool_call_input(&notification, &cache);
        assert!(cache.borrow().is_empty());
    }

    #[test]
    fn cache_tool_call_input_from_tool_call_update() {
        let cache = RefCell::new(HashMap::new());
        let update = acp::ToolCallUpdate::new(
            "tc_2",
            acp::ToolCallUpdateFields::new().raw_input(serde_json::json!({"cmd": "ls"})),
        );
        let notification = acp::SessionNotification::new(
            acp::SessionId::new("sess"),
            acp::SessionUpdate::ToolCallUpdate(update),
        );
        cache_tool_call_input(&notification, &cache);
        let borrowed = cache.borrow();
        assert!(borrowed.contains_key("tc_2"));
        assert_eq!(borrowed["tc_2"], serde_json::json!({"cmd": "ls"}));
    }

    #[test]
    fn to_ext_notification_commands_parses_local_flag() {
        let params = serde_json::json!({
            "commands": [
                {"name": "/quit", "description": "Quit", "meta": {"local": true}},
                {"name": "/compact", "description": "Compact"}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Some(Notification::CommandsUpdated { commands: cmds, .. })) = result {
            assert!(cmds[0].is_local(), "/quit should be local");
            assert!(!cmds[1].is_local(), "/compact should not be local");
        } else {
            panic!("expected CommandsUpdated");
        }
    }

    #[test]
    fn parse_agent_not_found() {
        let params = serde_json::json!({
            "requestedAgent": "code-reviewer",
            "fallbackAgent": "default"
        });
        let result = to_ext_notification("kiro.dev/agent/not_found", &params);
        if let Ok(Some(Notification::AgentNotFound {
            requested,
            fallback,
        })) = result
        {
            assert_eq!(requested, "code-reviewer");
            assert_eq!(fallback.as_deref(), Some("default"));
        } else {
            panic!("expected AgentNotFound, got {:?}", result);
        }
    }

    #[test]
    fn parse_agent_config_error() {
        let params = serde_json::json!({
            "path": ".kiro/agents/broken.md",
            "error": "invalid YAML frontmatter"
        });
        let result = to_ext_notification("kiro.dev/agent/config_error", &params);
        if let Ok(Some(Notification::AgentConfigError { path, error })) = result {
            assert_eq!(path, ".kiro/agents/broken.md");
            assert_eq!(error, "invalid YAML frontmatter");
        } else {
            panic!("expected AgentConfigError, got {:?}", result);
        }
    }

    #[test]
    fn parse_model_not_found() {
        let params = serde_json::json!({
            "requestedModel": "claude-opus-5",
            "fallbackModel": "claude-sonnet-4"
        });
        let result = to_ext_notification("kiro.dev/model/not_found", &params);
        if let Ok(Some(Notification::ModelNotFound {
            requested,
            fallback,
        })) = result
        {
            assert_eq!(requested, "claude-opus-5");
            assert_eq!(fallback.as_deref(), Some("claude-sonnet-4"));
        } else {
            panic!("expected ModelNotFound, got {:?}", result);
        }
    }

    #[test]
    fn multi_session_notifications_acknowledged_not_forwarded() {
        for method in ["kiro.dev/session/activity", "kiro.dev/session/list_update"] {
            let result = to_ext_notification(method, &serde_json::json!({}));
            assert!(
                matches!(result, Ok(None)),
                "{method} should return Ok(None), got {result:?}"
            );
        }
    }

    #[test]
    fn parse_subagent_list_update_with_active_subagents() {
        let params = serde_json::json!({
            "subagents": [{
                "sessionId": "b49d53d1-a42a-4ef6-a173-a6224e8e6fcd",
                "sessionName": "code-reviewer",
                "agentName": "code-reviewer",
                "initialQuery": "Review the code changes",
                "status": { "type": "working", "message": "Running" },
                "group": "crew-Review code changes",
                "role": "code-reviewer",
                "dependsOn": []
            }],
            "pendingStages": [{
                "name": "summary-writer",
                "agentName": "summary-writer",
                "group": "crew-Review code changes",
                "role": "summary-writer",
                "dependsOn": ["code-reviewer"]
            }]
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::SubagentListUpdated {
            subagents,
            pending_stages,
        })) = result
        {
            assert_eq!(subagents.len(), 1);
            assert_eq!(subagents[0].session_name(), "code-reviewer");
            assert!(subagents[0].is_working());
            assert_eq!(subagents[0].group(), Some("crew-Review code changes"));
            assert_eq!(pending_stages.len(), 1);
            assert_eq!(pending_stages[0].name(), "summary-writer");
            assert_eq!(pending_stages[0].depends_on(), &["code-reviewer"]);
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_subagent_list_update_empty() {
        let params = serde_json::json!({
            "subagents": [],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::SubagentListUpdated {
            subagents,
            pending_stages,
        })) = result
        {
            assert!(subagents.is_empty());
            assert!(pending_stages.is_empty());
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_subagent_list_update_terminated_status() {
        let params = serde_json::json!({
            "subagents": [{
                "sessionId": "s1",
                "sessionName": "reviewer",
                "agentName": "reviewer",
                "initialQuery": "review",
                "status": { "type": "terminated" },
                "group": null,
                "role": null,
                "dependsOn": []
            }],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::SubagentListUpdated { subagents, .. })) = result {
            assert!(!subagents[0].is_working());
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_inbox_notification() {
        let params = serde_json::json!({
            "sessionId": "874046d5-c7ab-47a7-86c5-b15cece1379a",
            "sessionName": "main",
            "messageCount": 2,
            "escalationCount": 0,
            "senders": ["subagent"]
        });
        let result = to_ext_notification("kiro.dev/session/inbox_notification", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::InboxNotification {
            session_id,
            message_count,
            escalation_count,
            senders,
        })) = result
        {
            assert_eq!(session_id.as_str(), "874046d5-c7ab-47a7-86c5-b15cece1379a");
            assert_eq!(message_count, 2);
            assert_eq!(escalation_count, 0);
            assert_eq!(senders, vec!["subagent"]);
        } else {
            panic!("expected InboxNotification");
        }
    }

    #[test]
    fn parse_tool_call_chunk_with_session_id() {
        let params = serde_json::json!({
            "sessionId": "b49d53d1-subagent",
            "update": {
                "sessionUpdate": "tool_call_chunk",
                "toolCallId": "tc-1",
                "title": "read",
                "kind": "read"
            }
        });
        let result = to_ext_notification("kiro.dev/session/update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::ToolCallChunk { session_id, .. })) = result {
            assert_eq!(
                session_id.as_ref().map(|s| s.as_str()),
                Some("b49d53d1-subagent")
            );
        } else {
            panic!("expected ToolCallChunk with session_id");
        }
    }

    #[test]
    fn parse_tool_call_chunk_empty_session_id_treated_as_none() {
        let params = serde_json::json!({
            "sessionId": "",
            "update": {
                "sessionUpdate": "tool_call_chunk",
                "toolCallId": "tc-2",
                "title": "read",
                "kind": "read"
            }
        });
        let result = to_ext_notification("kiro.dev/session/update", &params);
        assert!(result.is_ok());
        if let Ok(Some(Notification::ToolCallChunk { session_id, .. })) = result {
            assert!(session_id.is_none(), "empty sessionId should be None");
        } else {
            panic!("expected ToolCallChunk");
        }
    }

    #[test]
    fn parse_subagent_list_update_missing_session_id_skips_entry() {
        let params = serde_json::json!({
            "subagents": [
                {
                    "sessionName": "no-id",
                    "agentName": "no-id",
                    "initialQuery": "query",
                    "status": { "type": "working", "message": "Running" },
                    "dependsOn": []
                },
                {
                    "sessionId": "s2",
                    "sessionName": "has-id",
                    "agentName": "has-id",
                    "initialQuery": "query",
                    "status": { "type": "working", "message": "Running" },
                    "dependsOn": []
                }
            ],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        if let Ok(Some(Notification::SubagentListUpdated { subagents, .. })) = result {
            assert_eq!(subagents.len(), 1);
            assert_eq!(subagents[0].session_name(), "has-id");
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_subagent_list_update_multiple_subagents() {
        let params = serde_json::json!({
            "subagents": [
                {
                    "sessionId": "s1",
                    "sessionName": "reviewer",
                    "agentName": "code-reviewer",
                    "initialQuery": "review code",
                    "status": { "type": "working", "message": "Reading files" },
                    "group": "crew-Review",
                    "role": "code-reviewer",
                    "dependsOn": []
                },
                {
                    "sessionId": "s2",
                    "sessionName": "analyzer",
                    "agentName": "pr-test-analyzer",
                    "initialQuery": "analyze tests",
                    "status": { "type": "terminated" },
                    "group": "crew-Review",
                    "role": "pr-test-analyzer",
                    "dependsOn": []
                }
            ],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        if let Ok(Some(Notification::SubagentListUpdated { subagents, .. })) = result {
            assert_eq!(subagents.len(), 2);
            assert!(subagents[0].is_working());
            assert!(!subagents[1].is_working());
            assert_eq!(subagents[0].session_name(), "reviewer");
            assert_eq!(subagents[1].session_name(), "analyzer");
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_subagent_working_status_without_message() {
        let params = serde_json::json!({
            "subagents": [{
                "sessionId": "s1",
                "sessionName": "reviewer",
                "agentName": "reviewer",
                "initialQuery": "review",
                "status": { "type": "working" },
                "group": null,
                "role": null,
                "dependsOn": []
            }],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        if let Ok(Some(Notification::SubagentListUpdated { subagents, .. })) = result {
            assert!(subagents[0].is_working());
            if let SubagentStatus::Working { message } = subagents[0].status() {
                assert!(message.is_none());
            } else {
                panic!("expected Working status");
            }
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_subagent_unknown_status_type_defaults_to_working() {
        let params = serde_json::json!({
            "subagents": [{
                "sessionId": "s1",
                "sessionName": "reviewer",
                "agentName": "reviewer",
                "initialQuery": "review",
                "status": { "type": "suspended", "message": "Paused" },
                "group": null,
                "role": null,
                "dependsOn": []
            }],
            "pendingStages": []
        });
        let result = to_ext_notification("kiro.dev/subagent/list_update", &params);
        if let Ok(Some(Notification::SubagentListUpdated { subagents, .. })) = result {
            assert!(subagents[0].is_working());
            if let SubagentStatus::Working { message } = subagents[0].status() {
                assert_eq!(message.as_deref(), Some("Paused"));
            } else {
                panic!("expected Working status");
            }
        } else {
            panic!("expected SubagentListUpdated");
        }
    }

    #[test]
    fn parse_inbox_notification_missing_session_id_returns_none() {
        let params = serde_json::json!({
            "messageCount": 1,
            "escalationCount": 0,
            "senders": ["subagent"]
        });
        let result = to_ext_notification("kiro.dev/session/inbox_notification", &params);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn parse_commands_available_with_prompts() {
        let params = serde_json::json!({
            "commands": [
                { "name": "/help", "description": "Show help" }
            ],
            "prompts": [
                {
                    "name": "review-pr",
                    "description": "Review a PR",
                    "serverName": "file-prompts",
                    "arguments": [
                        { "name": "branch", "required": true },
                        { "name": "scope", "required": false }
                    ]
                }
            ],
            "tools": [],
            "mcpServers": []
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Some(Notification::CommandsUpdated { commands, prompts })) = result {
            assert_eq!(commands.len(), 1);
            assert_eq!(prompts.len(), 1);
            assert_eq!(prompts[0].name(), "review-pr");
            assert_eq!(prompts[0].arguments().len(), 2);
            assert!(prompts[0].arguments()[0].required());
            assert!(!prompts[0].arguments()[1].required());
            assert_eq!(prompts[0].argument_hints(), "<branch> [scope]");
        } else {
            panic!("expected CommandsUpdated, got {:?}", result);
        }
    }

    #[test]
    fn parse_commands_available_no_prompts() {
        let params = serde_json::json!({
            "commands": [{ "name": "/help", "description": "Show help" }]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Some(Notification::CommandsUpdated { prompts, .. })) = result {
            assert!(prompts.is_empty());
        } else {
            panic!("expected CommandsUpdated");
        }
    }
}

//! Kiro-specific extension parsing.
//!
//! `kiro.dev/*` methods, subagent metadata, and other Kiro-only ACP
//! extensions live here. Generic ACP conversion stays in `super`.
//!
//! Note: on the wire these methods appear with a leading underscore
//! (`_kiro.dev/...`) because the public `agent-client-protocol` crate
//! auto-prefixes outbound extension method names with `_` and auto-strips
//! them inbound. The names without underscore are the canonical form.

use crate::types::*;

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

    Some(
        SubagentInfo::new(session_id, session_name, agent_name, initial_query, status)
            .with_group(group)
            .with_role(role)
            .with_depends_on(depends_on),
    )
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
            let Some(status) = params.get("status") else {
                tracing::warn!(
                    "kiro.dev/compaction/status: missing status object, dropping notification"
                );
                return Ok(None);
            };
            let status_type = status.get("type").and_then(|t| t.as_str());
            let (phase, summary) = match status_type {
                Some("started") => (CompactionPhase::Started, None),
                Some("completed") => {
                    let summary = params
                        .get("summary")
                        .and_then(|s| s.as_str())
                        .map(String::from);
                    (CompactionPhase::Completed, summary)
                }
                Some("failed") => {
                    let error = status
                        .get("error")
                        .and_then(|e| e.as_str())
                        .map(String::from);
                    (CompactionPhase::Failed { error }, None)
                }
                Some(other) => {
                    tracing::warn!(
                        status_type = other,
                        "kiro.dev/compaction/status: unknown status type, dropping"
                    );
                    return Ok(None);
                }
                None => {
                    tracing::warn!(
                        "kiro.dev/compaction/status: status object missing `type` field"
                    );
                    return Ok(None);
                }
            };
            Ok(Some(Notification::CompactionStatus { phase, summary }))
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

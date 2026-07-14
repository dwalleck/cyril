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

    // Pipeline stage metadata (Kiro 2.5.0+). `name` is the stage name (may be
    // null/absent, and distinct in meaning from sessionName — though for
    // pipeline stages the two may carry the same value); `createdAtMs` is the
    // stage creation time.
    let stage_name = v
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let created_at_ms = v.get("createdAtMs").and_then(|c| c.as_u64());

    // Review-loop progress (Kiro 2.5.0+). Only model loop_state when the stage
    // actually has a loop — `hasLoop: false` leaves it None so "looping" and
    // "not looping" can't be confused (the iteration/max counters, if present,
    // are meaningless when hasLoop is false).
    let loop_state = if v.get("hasLoop").and_then(|h| h.as_bool()).unwrap_or(false) {
        // hasLoop is true, so both counters are expected. If either is missing
        // or non-numeric the frame is corrupt — log and emit no badge rather
        // than defaulting to 0 and rendering a misleading "↻ 1/0".
        match (
            v.get("loopIteration").and_then(|i| i.as_u64()),
            v.get("loopMaxIterations").and_then(|m| m.as_u64()),
        ) {
            (Some(iteration), Some(max_iterations)) => {
                let state = LoopState::new(iteration as u32, max_iterations as u32);
                if state.is_none() {
                    tracing::warn!(
                        session_id = %session_id,
                        session_name,
                        "subagent list_update has hasLoop:true but loopMaxIterations is 0, omitting loop badge"
                    );
                }
                state
            }
            _ => {
                tracing::warn!(
                    session_id = %session_id,
                    session_name,
                    "subagent list_update has hasLoop:true but missing/invalid loop counters, omitting loop badge"
                );
                None
            }
        }
    } else {
        None
    };

    Some(
        SubagentInfo::new(session_id, session_name, agent_name, initial_query, status)
            .with_group(group)
            .with_role(role)
            .with_depends_on(depends_on)
            .with_stage_name(stage_name)
            .with_created_at_ms(created_at_ms)
            .with_loop_state(loop_state),
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

            // Preserve zero-credit turns rather than filtering them out — a
            // turn with `meteringUsage: [{value: 0.0, ...}]` (cached
            // response, free tier, etc.) is semantically distinct from a
            // turn with the field omitted entirely. The UI layer decides
            // whether to display 0.0 or hide it.
            let metering = params
                .get("meteringUsage")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    let credits: f64 = arr
                        .iter()
                        .filter_map(|u| u.get("value").and_then(|v| v.as_f64()))
                        .sum();
                    let duration_ms = params.get("turnDurationMs").and_then(|d| d.as_u64());
                    TurnMetering::new(credits, duration_ms)
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

            // Thinking-effort level (Kiro 2.5.0+), present only under thinking
            // models. `EffortLevel::from_wire` maps the closed wire set and
            // returns None for empty/unrecognized values, so "" never reaches
            // the UI as a level. An absent field is normal (non-thinking model);
            // a present-but-non-string field is corrupt, so warn rather than
            // silently degrading to None (distinguish missing from corrupt).
            let effort = match params.get("effort") {
                None => None,
                Some(e) => match e.as_str() {
                    Some(s) => EffortLevel::from_wire(s),
                    None => {
                        tracing::warn!(
                            effort = ?e,
                            "metadata `effort` present but not a string, ignoring"
                        );
                        None
                    }
                },
            };

            Ok(Some(Notification::MetadataUpdated {
                context_usage: ContextUsage::new(pct),
                metering,
                tokens,
                effort,
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
            // For steering-echo logs only ("<unknown>" if the envelope omits it).
            let session_id = params
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
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
                // Queue-steering echoes, OLD v2 dialect (Kiro 2.7.0+; ROADMAP K1a;
                // live until the 2026-06/07 backend rollout, kept as rollback
                // insurance — cyril-vgcm D1). They ride the SAME wire method as
                // tool_call_chunk — `_kiro.dev/session/update`, delivered here as
                // `kiro.dev/session/update` after the ACP library strips the single
                // leading `_` ext-prefix. Always emit so the queue counter
                // transitions; a missing payload field degrades only the (K1b)
                // display text and becomes `None`, never a `Some("")` sentinel.
                // This dialect carries no message ids.
                Some("steering_queued") => {
                    let message = update
                        .and_then(|u| u.get("message"))
                        .and_then(|v| v.as_str());
                    if message.is_none() {
                        tracing::warn!(
                            session_id,
                            "steering_queued missing message field; counting with no text"
                        );
                    }
                    Ok(Some(Notification::SteeringQueued {
                        message: message.map(str::to_string),
                        message_id: None,
                    }))
                }
                Some("steering_consumed") => {
                    let content = update
                        .and_then(|u| u.get("content"))
                        .and_then(|v| v.as_str());
                    if content.is_none() {
                        tracing::warn!(
                            session_id,
                            "steering_consumed missing content field; decrementing with no text"
                        );
                    }
                    Ok(Some(Notification::SteeringConsumed {
                        content: content.map(str::to_string),
                        message_id: None,
                    }))
                }
                Some("steering_cleared") => Ok(Some(Notification::SteeringCleared {
                    message_ids: Vec::new(),
                })),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    // Slice B / design claims 1-6 + cyril-c1qe. Steering rides wire method
    // `_kiro.dev/session/update`; the ACP library strips the single leading `_`
    // before delivery, so the converter receives the STRIPPED form below and
    // steering shares tool_call_chunk's `kiro.dev/session/update` arm.
    const STEER_METHOD: &str = "kiro.dev/session/update";

    #[test]
    fn steering_queued_converts() {
        // L25 shape, plus a stray `content` to prove we read `message`, not it.
        let params = json!({
            "sessionId": "2dc3c608",
            "update": {"sessionUpdate": "steering_queued", "message": "stop now", "content": "WRONG"}
        });
        assert!(matches!(
            to_ext_notification(STEER_METHOD, &params),
            Ok(Some(Notification::SteeringQueued { message, message_id: None }))
                if message.as_deref() == Some("stop now")
        ));
    }

    #[test]
    fn steering_consumed_converts() {
        let params = json!({
            "sessionId": "2dc3c608",
            "update": {"sessionUpdate": "steering_consumed", "content": "stop now"}
        });
        assert!(matches!(
            to_ext_notification(STEER_METHOD, &params),
            Ok(Some(Notification::SteeringConsumed { content, message_id: None }))
                if content.as_deref() == Some("stop now")
        ));
    }

    #[test]
    fn steering_cleared_converts() {
        // L120: payload-free frame must still produce the notification. The old
        // dialect carries no ids — empty `message_ids` means "everything queued"
        // (cyril-vgcm C4/C7 contract).
        let params = json!({
            "sessionId": "2dc3c608",
            "update": {"sessionUpdate": "steering_cleared", "foo": 1}
        });
        assert!(matches!(
            to_ext_notification(STEER_METHOD, &params),
            Ok(Some(Notification::SteeringCleared { message_ids })) if message_ids.is_empty()
        ));
    }

    #[test]
    fn steering_missing_field_still_emits_with_none() {
        // C1 regression (PR #18 review): a queued/consumed echo with no payload
        // field must STILL emit the notification (so the depth counter transitions
        // — a dropped decrement would permanently inflate the queue). The absent
        // field becomes `None`, never a `Some("")` sentinel.
        let q = json!({"update": {"sessionUpdate": "steering_queued"}});
        assert!(matches!(
            to_ext_notification(STEER_METHOD, &q),
            Ok(Some(Notification::SteeringQueued {
                message: None,
                message_id: None,
            }))
        ));
        let c = json!({"update": {"sessionUpdate": "steering_consumed"}});
        assert!(matches!(
            to_ext_notification(STEER_METHOD, &c),
            Ok(Some(Notification::SteeringConsumed {
                content: None,
                message_id: None,
            }))
        ));
    }

    #[test]
    fn steering_unknown_variant_errs() {
        // Steering now shares the `kiro.dev/session/update` arm with tool_call_chunk,
        // which Errs on a genuinely unknown sub-variant (existing behavior, preserved).
        // A future steering variant we don't model yet is logged + dropped via the Err.
        let unknown = json!({"update": {"sessionUpdate": "steering_paused"}});
        assert!(to_ext_notification(STEER_METHOD, &unknown).is_err());
        let missing = json!({"update": {}});
        assert!(to_ext_notification(STEER_METHOD, &missing).is_err());
    }

    #[test]
    fn steering_rides_stripped_method_not_underscore() {
        // Regression fence for cyril-c1qe: the ACP lib strips the single leading `_`
        // from ext methods, so steering echoes (wire `_kiro.dev/session/update`) reach
        // the converter as `kiro.dev/session/update`. K1a keyed a dead arm on the raw
        // underscore form. Assert the STRIPPED method handles steering, and the raw
        // underscore form is NOT a steering match (it falls to the catch-all).
        let frame = json!({"update": {"sessionUpdate": "steering_queued", "message": "x"}});
        assert!(matches!(
            to_ext_notification("kiro.dev/session/update", &frame),
            Ok(Some(Notification::SteeringQueued { .. }))
        ));
        assert!(matches!(
            to_ext_notification("_kiro.dev/session/update", &frame),
            Ok(None)
        ));
    }

    #[test]
    fn parse_subagent_entry_extracts_review_loop_fields() {
        // A looping reviewer stage mid-loop (Kiro 2.5.0 review-loop wire shape).
        let v = json!({
            "sessionId": "abc",
            "sessionName": "checker",
            "agentName": "kiro_default",
            "initialQuery": "review the draft",
            "status": {"type": "working", "message": "Running"},
            "name": "checker",
            "createdAtMs": 1_780_023_672_042u64,
            "hasLoop": true,
            "loopIteration": 1,
            "loopMaxIterations": 2,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.session_name(), "checker");
        assert_eq!(info.stage_name(), Some("checker"));
        assert_eq!(info.created_at_ms(), Some(1_780_023_672_042));
        assert_eq!(info.loop_state(), LoopState::new(1, 2));
    }

    #[test]
    fn parse_subagent_entry_no_loop_when_has_loop_false() {
        // A non-looping stage carries the loop counters but hasLoop=false;
        // they must NOT produce a loop_state.
        let v = json!({
            "sessionId": "def",
            "sessionName": "writer",
            "agentName": "kiro_default",
            "initialQuery": "write the draft",
            "status": {"type": "working"},
            "hasLoop": false,
            "loopIteration": 0,
            "loopMaxIterations": 0,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.loop_state(), None);
        assert_eq!(info.stage_name(), None);
        assert_eq!(info.created_at_ms(), None);
    }

    #[test]
    fn parse_subagent_entry_no_loop_fields_at_all() {
        // Pre-2.5.0 shape: no loop keys present at all → no loop_state.
        let v = json!({
            "sessionId": "ghi",
            "sessionName": "legacy",
            "agentName": "kiro_default",
            "initialQuery": "q",
            "status": {"type": "working"},
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.loop_state(), None);
    }

    #[test]
    fn parse_subagent_entry_no_loop_when_has_loop_true_but_counters_missing() {
        // Corrupt frame: hasLoop:true but the counters are absent. Must NOT
        // default to 0/0 and render a misleading "↻ 1/0" — emit no loop_state.
        let v = json!({
            "sessionId": "jkl",
            "sessionName": "checker",
            "agentName": "kiro_default",
            "initialQuery": "review",
            "status": {"type": "working"},
            "hasLoop": true,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.loop_state(), None);
    }

    #[test]
    fn parse_subagent_entry_no_loop_when_counters_non_numeric() {
        // Corrupt frame: hasLoop:true but a counter is the wrong JSON type
        // (string instead of number). `as_u64()` yields None, so this must be
        // treated as corrupt and emit no loop_state — not silently become 1/0.
        let v = json!({
            "sessionId": "pqr",
            "sessionName": "checker",
            "agentName": "kiro_default",
            "initialQuery": "review",
            "status": {"type": "working"},
            "hasLoop": true,
            "loopIteration": "two",
            "loopMaxIterations": 2,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.loop_state(), None);
    }

    #[test]
    fn parse_subagent_entry_no_loop_when_max_is_zero() {
        // Corrupt frame: hasLoop:true with a zero cap. LoopState::new rejects
        // max == 0 (a zero-cap loop would render a misleading "↻ 1/0"), so the
        // parser routes this to None via the zero-cap warn branch.
        let v = json!({
            "sessionId": "stu",
            "sessionName": "checker",
            "agentName": "kiro_default",
            "initialQuery": "review",
            "status": {"type": "working"},
            "hasLoop": true,
            "loopIteration": 0,
            "loopMaxIterations": 0,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.loop_state(), None);
    }

    #[test]
    fn parse_subagent_entry_clamps_over_cap_iteration() {
        // A well-formed but over-cap frame (iteration >= max) is a Kiro-
        // semantics question, not corruption: LoopState clamps iteration to
        // max-1 so the badge never renders past the cap (here "↻ 2/2").
        let v = json!({
            "sessionId": "vwx",
            "sessionName": "checker",
            "agentName": "kiro_default",
            "initialQuery": "review",
            "status": {"type": "working"},
            "hasLoop": true,
            "loopIteration": 99,
            "loopMaxIterations": 2,
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        let loop_state = info.loop_state().expect("over-cap frame still loops");
        assert_eq!(loop_state.iteration(), 1, "iteration clamped to max-1");
        assert_eq!(loop_state.display_iteration(), 2, "displays clamped 2/2");
        assert_eq!(loop_state.max_iterations(), 2);
    }

    #[test]
    fn parse_subagent_entry_empty_stage_name_is_none() {
        // An empty `name` string must not reach the UI as a blank stage label.
        let v = json!({
            "sessionId": "mno",
            "sessionName": "writer",
            "agentName": "kiro_default",
            "initialQuery": "q",
            "status": {"type": "working"},
            "name": "",
        });
        let info = parse_subagent_entry(&v).expect("entry should parse");
        assert_eq!(info.stage_name(), None);
    }
}

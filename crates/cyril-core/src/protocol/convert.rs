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
        None,
        to_tool_kind(acp_call.kind),
        to_tool_call_status(acp_call.status),
        cached_inputs
            .get(&id_str)
            .cloned()
            .or_else(|| acp_call.raw_input.clone()),
    )
    .with_content(content)
    .with_locations(locations)
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

pub(crate) fn to_ext_notification(
    method: &str,
    params: &serde_json::Value,
) -> crate::Result<Notification> {
    match method {
        "kiro.dev/metadata" => {
            let pct = params
                .get("contextUsagePercentage")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            Ok(Notification::ContextUsageUpdated(ContextUsage::new(pct)))
        }
        "kiro.dev/compaction/status" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Notification::CompactionStatus { message })
        }
        "kiro.dev/clear/status" => {
            let message = params
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok(Notification::ClearStatus { message })
        }
        "kiro.dev/agent/switched" => {
            let name = params
                .get("agentName")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let welcome = params
                .get("welcomeMessage")
                .and_then(|v| v.as_str())
                .map(String::from);
            Ok(Notification::AgentSwitched { name, welcome })
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

            Ok(Notification::CommandsUpdated(commands))
        }
        "kiro.dev/session/update" => {
            let update = params.get("update");
            let session_update = update
                .and_then(|u| u.get("sessionUpdate"))
                .and_then(|s| s.as_str());
            match session_update {
                Some("tool_call_chunk") => {
                    let tool_call_id = update
                        .and_then(|u| u.get("toolCallId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
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
                    Ok(Notification::ToolCallChunk {
                        tool_call_id,
                        title,
                        kind,
                    })
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
        other => {
            tracing::debug!(method = other, "unknown extension notification");
            Err(crate::Error::from_kind(crate::ErrorKind::Protocol {
                message: format!("unknown extension: {other}"),
            }))
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

    ToolCall::new(
        ToolCallId::new(id_str),
        title,
        None,
        kind,
        status,
        raw_input,
    )
    .with_content(content)
    .with_locations(locations)
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
                ToolCall::new(
                    ToolCallId::new(id_str),
                    title,
                    None,
                    kind,
                    status,
                    raw_input,
                )
                .with_content(content)
                .with_locations(locations),
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
                        _ => PlanEntryStatus::Pending,
                    };
                    PlanEntry::new(e.content.clone(), status)
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
            Some(Notification::CommandsUpdated(commands))
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
    fn to_ext_notification_unknown_method_returns_error() {
        let result = to_ext_notification("unknown.method", &serde_json::json!({}));
        match result {
            Err(ref e) => assert!(matches!(e.kind(), crate::ErrorKind::Protocol { .. })),
            Ok(_) => panic!("expected error for unknown method"),
        }
    }

    #[test]
    fn to_ext_notification_metadata() {
        let params = serde_json::json!({"contextUsagePercentage": 75.0});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        assert!(result.is_ok());
        if let Ok(Notification::ContextUsageUpdated(usage)) = result {
            assert!((usage.percentage() - 75.0).abs() < f64::EPSILON);
        } else {
            panic!("expected ContextUsageUpdated");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status() {
        let params = serde_json::json!({"message": "50% done"});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(Notification::CompactionStatus { .. })));
    }

    #[test]
    fn to_ext_notification_clear_status() {
        let params = serde_json::json!({"message": "cleared"});
        let result = to_ext_notification("kiro.dev/clear/status", &params);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(Notification::ClearStatus { .. })));
    }

    #[test]
    fn to_ext_notification_agent_switched() {
        let params = serde_json::json!({"agentName": "code-agent", "welcomeMessage": "Hello!"});
        let result = to_ext_notification("kiro.dev/agent/switched", &params);
        assert!(result.is_ok());
        if let Ok(Notification::AgentSwitched { name, welcome }) = result {
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
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
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
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
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
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
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
        if let Ok(Notification::ToolCallChunk {
            tool_call_id,
            title,
            kind,
        }) = result
        {
            assert_eq!(tool_call_id, "tc_123");
            assert_eq!(title, "reading main.rs");
            assert_eq!(kind, "read");
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

    #[test]
    fn to_ext_notification_commands_strips_slash_prefix() {
        let params = serde_json::json!({
            "commands": [
                {"name": "/model", "description": "Switch model", "meta": {"inputType": "selection"}}
            ]
        });
        let result = to_ext_notification("kiro.dev/commands/available", &params);
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
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
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
            assert!(cmds[0].is_selection(), "/model should be selection");
            assert!(!cmds[1].is_selection(), "/compact should not be selection");
        } else {
            panic!("expected CommandsUpdated");
        }
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
        if let Ok(Notification::CommandsUpdated(cmds)) = result {
            assert!(cmds[0].is_local(), "/quit should be local");
            assert!(!cmds[1].is_local(), "/compact should not be local");
        } else {
            panic!("expected CommandsUpdated");
        }
    }
}

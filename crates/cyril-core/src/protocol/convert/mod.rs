use std::cell::RefCell;
use std::collections::HashMap;

use agent_client_protocol as acp;

use crate::types::*;

pub(crate) mod kiro;

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
        agent_client_protocol::ToolKind::SwitchMode => ToolKind::SwitchMode,
        _ => ToolKind::Other,
    }
}

/// Convert an ACP `SessionMode` into cyril's domain type, lifting the
/// Kiro-specific `_meta.welcomeMessage` field out of the `_meta` bag.
pub(crate) fn to_session_mode(mode: &acp::SessionMode) -> SessionMode {
    let welcome = mode.meta.as_ref().and_then(|m| {
        m.get("welcomeMessage").and_then(|v| match v.as_str() {
            Some(s) => Some(s.to_string()),
            None => {
                tracing::warn!(
                    mode_id = %mode.id,
                    value = ?v,
                    "_meta.welcomeMessage present but not a string, ignoring"
                );
                None
            }
        })
    });
    SessionMode::new(
        ModeId::new(mode.id.to_string()),
        mode.name.clone(),
        mode.description.clone(),
    )
    .with_welcome_message(welcome)
}

/// Convert an ACP `ModelInfo` into cyril's domain type.
pub(crate) fn to_model_info(info: &acp::ModelInfo) -> ModelInfo {
    ModelInfo::new(
        ModelId::new(info.model_id.to_string()),
        info.name.clone(),
        info.description.clone(),
    )
}

/// Build a `SessionCreated` notification from the mode/model state returned
/// by `session/new` or `session/load`. Consolidates the ACP→cyril conversion
/// in one place alongside the per-item converters it calls.
pub(crate) fn session_created_from_response(
    session_id: String,
    modes: Option<&acp::SessionModeState>,
    models: Option<&acp::SessionModelState>,
) -> Notification {
    let current_mode = modes.map(|m| ModeId::new(m.current_mode_id.to_string()));
    let available_modes: Vec<SessionMode> = modes
        .map(|m| m.available_modes.iter().map(to_session_mode).collect())
        .unwrap_or_default();
    let current_model = models.map(|m| m.current_model_id.to_string());
    let available_models: Vec<ModelInfo> = models
        .map(|m| m.available_models.iter().map(to_model_info).collect())
        .unwrap_or_default();
    Notification::SessionCreated {
        session_id: SessionId::new(session_id),
        current_mode,
        current_model,
        available_modes,
        available_models,
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
            let kind = match opt.kind {
                acp::PermissionOptionKind::AllowOnce => PermissionOptionKind::AllowOnce,
                acp::PermissionOptionKind::AllowAlways => PermissionOptionKind::AllowAlways,
                acp::PermissionOptionKind::RejectOnce => PermissionOptionKind::RejectOnce,
                acp::PermissionOptionKind::RejectAlways => PermissionOptionKind::RejectAlways,
                _ => {
                    tracing::warn!(
                        ?opt.kind,
                        "unknown PermissionOptionKind variant; defaulting to RejectOnce"
                    );
                    PermissionOptionKind::RejectOnce
                }
            };
            let is_destructive = matches!(
                kind,
                PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
            );
            PermissionOption {
                id: opt.option_id.to_string(),
                label: opt.name.clone(),
                kind,
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
        PermissionResponse::RejectAlways => {
            let option_id = find_option_id(args, acp::PermissionOptionKind::RejectAlways);
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
        acp::SessionUpdate::UserMessageChunk(chunk) => {
            if let acp::ContentBlock::Text(ref text) = chunk.content {
                Some(Notification::UserMessage(UserMessage {
                    text: text.text.clone(),
                    is_streaming: true,
                }))
            } else {
                None
            }
        }
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
            mode_id: ModeId::new(mode.current_mode_id.to_string()),
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
        acp::SessionUpdate::UsageUpdate(usage) => {
            tracing::info!(
                used = usage.used,
                size = usage.size,
                has_cost = usage.cost.is_some(),
                "received ACP UsageUpdate (unstable_session_usage)"
            );
            Some(Notification::UsageUpdated {
                used: usage.used,
                size: usage.size,
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::kiro::*;
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
    fn parse_metadata_with_zero_credit_metering_preserved() {
        // Regression: the parser previously dropped meteringUsage entries
        // that summed to 0.0 credits, conflating them with the
        // metering-field-absent case. Zero-cost turns (cached responses,
        // free tier) should now flow through as Some(TurnMetering(0.0)).
        let params = serde_json::json!({
            "sessionId": "s1",
            "contextUsagePercentage": 1.5,
            "meteringUsage": [
                {"unit": "credit", "unitPlural": "credits", "value": 0.0}
            ],
            "turnDurationMs": 12
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { metering, .. })) = result {
            let m = metering.expect("zero-credit metering should be preserved");
            assert!((m.credits() - 0.0).abs() < f64::EPSILON);
            assert_eq!(m.duration_ms(), Some(12));
        } else {
            panic!("expected MetadataUpdated, got {:?}", result);
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
    fn to_ext_notification_compaction_status_bare_message_drops() {
        // Legacy wire shape with only a flat `message` is no longer synthesized
        // into a CompactionStatus; without a `status.type` we can't classify it.
        let params = serde_json::json!({"message": "50% done"});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn to_ext_notification_compaction_status_started() {
        let params = serde_json::json!({"status": {"type": "started"}});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { phase, summary })) = result {
            assert_eq!(phase, CompactionPhase::Started);
            assert!(summary.is_none());
        } else {
            panic!("expected CompactionStatus, got {result:?}");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_failed() {
        let params = serde_json::json!({"status": {"type": "failed", "error": "out of memory"}});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { phase, summary })) = result {
            assert_eq!(
                phase,
                CompactionPhase::Failed {
                    error: Some("out of memory".into())
                }
            );
            assert!(summary.is_none());
        } else {
            panic!("expected CompactionStatus, got {result:?}");
        }
    }

    #[test]
    fn to_ext_notification_compaction_status_completed() {
        let params =
            serde_json::json!({"status": {"type": "completed"}, "summary": "3 turns removed"});
        let result = to_ext_notification("kiro.dev/compaction/status", &params);
        if let Ok(Some(Notification::CompactionStatus { phase, summary })) = result {
            assert_eq!(phase, CompactionPhase::Completed);
            assert_eq!(summary.as_deref(), Some("3 turns removed"));
        } else {
            panic!("expected CompactionStatus, got {result:?}");
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
    fn from_permission_response_reject_always() {
        let req = make_permission_request(vec![
            ("opt_allow", "Yes", acp::PermissionOptionKind::AllowOnce),
            (
                "opt_reject_always",
                "Never",
                acp::PermissionOptionKind::RejectAlways,
            ),
        ]);

        let resp = from_permission_response(PermissionResponse::RejectAlways, &req);
        if let acp::RequestPermissionOutcome::Selected(selected) = resp.outcome {
            assert_eq!(selected.option_id.to_string(), "opt_reject_always");
        } else {
            panic!("expected Selected outcome");
        }
    }

    #[test]
    fn to_tool_kind_switch_mode() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::SwitchMode),
            ToolKind::SwitchMode
        );
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

    // --- to_session_mode / to_model_info conversion tests ---

    fn acp_session_mode(id: &str, name: &str, meta: Option<acp::Meta>) -> acp::SessionMode {
        let mut m = acp::SessionMode::new(acp::SessionModeId::new(id.to_string()), name);
        m.meta = meta;
        m
    }

    #[test]
    fn to_session_mode_extracts_welcome_message_from_meta() {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "welcomeMessage".into(),
            serde_json::json!("Transform any idea..."),
        );
        let acp_mode = acp_session_mode("kiro_planner", "kiro_planner", Some(meta));
        let mode = to_session_mode(&acp_mode);
        assert_eq!(mode.welcome_message(), Some("Transform any idea..."));
        assert_eq!(mode.id().as_str(), "kiro_planner");
        assert_eq!(mode.label(), "kiro_planner");
    }

    #[test]
    fn to_session_mode_no_meta_yields_no_welcome() {
        let acp_mode = acp_session_mode("kiro_default", "kiro_default", None);
        let mode = to_session_mode(&acp_mode);
        assert_eq!(mode.welcome_message(), None);
    }

    #[test]
    fn to_session_mode_meta_without_welcome_key() {
        let mut meta = serde_json::Map::new();
        meta.insert("unrelated".into(), serde_json::json!("value"));
        let acp_mode = acp_session_mode("kiro_default", "kiro_default", Some(meta));
        let mode = to_session_mode(&acp_mode);
        assert_eq!(mode.welcome_message(), None);
    }

    #[test]
    fn to_session_mode_non_string_welcome_is_ignored() {
        // If Kiro ever ships a non-string welcomeMessage, we drop it rather
        // than panic. A warn log is emitted (not asserted here).
        let mut meta = serde_json::Map::new();
        meta.insert("welcomeMessage".into(), serde_json::json!(42));
        let acp_mode = acp_session_mode("kiro_default", "kiro_default", Some(meta));
        let mode = to_session_mode(&acp_mode);
        assert_eq!(mode.welcome_message(), None);
    }

    #[test]
    fn to_session_mode_copies_description() {
        let mut acp_mode = acp_session_mode("chat", "chat", None);
        acp_mode.description = Some("General chat".into());
        let mode = to_session_mode(&acp_mode);
        assert_eq!(mode.description(), Some("General chat"));
    }

    #[test]
    fn to_model_info_round_trip() {
        let acp_info = acp::ModelInfo::new(
            acp::ModelId::new("claude-sonnet-4".to_string()),
            "Claude Sonnet 4",
        )
        .description(Some("Fast model".to_string()));
        let info = to_model_info(&acp_info);
        assert_eq!(info.id().as_str(), "claude-sonnet-4");
        assert_eq!(info.name(), "Claude Sonnet 4");
        assert_eq!(info.description(), Some("Fast model"));
    }

    #[test]
    fn to_model_info_no_description() {
        let acp_info = acp::ModelInfo::new(acp::ModelId::new("claude-haiku".to_string()), "Haiku");
        let info = to_model_info(&acp_info);
        assert_eq!(info.description(), None);
    }

    // --- session_created_from_response helper tests ---

    fn acp_mode_with_welcome(id: &str, name: &str, welcome: Option<&str>) -> acp::SessionMode {
        let mut m = acp::SessionMode::new(acp::SessionModeId::new(id.to_string()), name);
        if let Some(w) = welcome {
            let mut meta = serde_json::Map::new();
            meta.insert("welcomeMessage".into(), serde_json::json!(w));
            m.meta = Some(meta);
        }
        m
    }

    #[test]
    fn session_created_from_response_populates_modes_and_welcome() {
        let mode_state = acp::SessionModeState::new(
            acp::SessionModeId::new("kiro_planner".to_string()),
            vec![
                acp_mode_with_welcome("kiro_default", "kiro_default", None),
                acp_mode_with_welcome(
                    "kiro_planner",
                    "kiro_planner",
                    Some("Transform any idea..."),
                ),
            ],
        );

        let notif = session_created_from_response("s1".into(), Some(&mode_state), None);
        match notif {
            Notification::SessionCreated {
                session_id,
                current_mode,
                current_model,
                available_modes,
                available_models,
            } => {
                assert_eq!(session_id.as_str(), "s1");
                assert_eq!(
                    current_mode.as_ref().map(ModeId::as_str),
                    Some("kiro_planner")
                );
                assert_eq!(current_model, None);
                assert_eq!(available_modes.len(), 2);
                assert_eq!(
                    available_modes[1].welcome_message(),
                    Some("Transform any idea...")
                );
                assert!(available_models.is_empty());
            }
            other => panic!("expected SessionCreated, got {other:?}"),
        }
    }

    #[test]
    fn session_created_from_response_populates_models() {
        let model_state = acp::SessionModelState::new(
            acp::ModelId::new("claude-sonnet-4".to_string()),
            vec![
                acp::ModelInfo::new(acp::ModelId::new("claude-sonnet-4".to_string()), "Sonnet"),
                acp::ModelInfo::new(acp::ModelId::new("claude-haiku".to_string()), "Haiku"),
            ],
        );

        let notif = session_created_from_response("s1".into(), None, Some(&model_state));
        match notif {
            Notification::SessionCreated {
                current_model,
                available_modes,
                available_models,
                ..
            } => {
                assert_eq!(current_model.as_deref(), Some("claude-sonnet-4"));
                assert!(available_modes.is_empty());
                assert_eq!(available_models.len(), 2);
                assert_eq!(available_models[0].id().as_str(), "claude-sonnet-4");
                assert_eq!(available_models[1].name(), "Haiku");
            }
            other => panic!("expected SessionCreated, got {other:?}"),
        }
    }

    #[test]
    fn session_created_from_response_none_none_yields_empty_catalogs() {
        let notif = session_created_from_response("s1".into(), None, None);
        match notif {
            Notification::SessionCreated {
                session_id,
                current_mode,
                current_model,
                available_modes,
                available_models,
            } => {
                assert_eq!(session_id.as_str(), "s1");
                assert!(current_mode.is_none());
                assert!(current_model.is_none());
                assert!(available_modes.is_empty());
                assert!(available_models.is_empty());
            }
            other => panic!("expected SessionCreated, got {other:?}"),
        }
    }
}

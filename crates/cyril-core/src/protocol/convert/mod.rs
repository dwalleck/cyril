use agent_client_protocol as acp;

use crate::types::*;

#[cfg(feature = "kas")]
pub(crate) mod kas;
pub(crate) mod kiro;
#[cfg(test)]
mod probe_j1b3;
#[cfg(test)]
mod probe_qo13;

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
/// Convert standard ACP prompt-response usage without leaking ACP types.
pub(crate) fn to_token_usage(usage: &acp::Usage) -> TokenUsage {
    TokenUsage::new(
        usage.total_tokens,
        usage.input_tokens,
        usage.output_tokens,
        usage.thought_tokens,
        usage.cached_read_tokens,
        usage.cached_write_tokens,
    )
}

fn to_money(cost: &acp::Cost) -> Option<Money> {
    match Money::try_new(cost.amount, cost.currency.clone()) {
        Ok(money) => Some(money),
        Err(error) => {
            tracing::warn!(
                amount = cost.amount,
                currency = cost.currency,
                error = %error,
                "invalid ACP cumulative usage cost, ignoring cost"
            );
            None
        }
    }
}

pub(crate) fn to_config_options(options: &[acp::SessionConfigOption]) -> Vec<ConfigOption> {
    options
        .iter()
        .filter_map(|option| match &option.kind {
            acp::SessionConfigKind::Select(select) => {
                let values = match &select.options {
                    acp::SessionConfigSelectOptions::Ungrouped(flat) => {
                        flat.iter().map(|value| value.value.to_string()).collect()
                    }
                    acp::SessionConfigSelectOptions::Grouped(groups) => groups
                        .iter()
                        .flat_map(|group| group.options.iter().map(|value| value.value.to_string()))
                        .collect(),
                    _ => Vec::new(),
                };
                Some(ConfigOption {
                    key: option.id.to_string(),
                    label: option.name.clone(),
                    value: Some(select.current_value.to_string()),
                    options: values,
                })
            }
            _ => None,
        })
        .collect()
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

pub(crate) fn to_tool_call(acp_call: &agent_client_protocol::ToolCall) -> ToolCall {
    let id_str = acp_call.tool_call_id.to_string();

    let content = convert_tool_call_content(&acp_call.content);
    let locations = convert_tool_call_locations(&acp_call.locations);

    ToolCall::new(
        ToolCallId::new(id_str),
        acp_call.title.clone(),
        to_tool_kind(acp_call.kind),
        to_tool_call_status(acp_call.status),
        acp_call.raw_input.clone(),
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

pub(crate) fn to_tool_call_update(update: &agent_client_protocol::ToolCallUpdate) -> ToolCall {
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
    let raw_input = update.fields.raw_input.clone();

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

/// Build a `ToolCall` from the `ToolCallUpdate` inside a permission request.
/// KAS sends this as a stub (no `raw_input`); callers needing the full
/// payload must join through the client's session-scoped ledger instead.
pub(crate) fn to_tool_call_from_permission(args: &acp::RequestPermissionRequest) -> ToolCall {
    to_tool_call_update(&args.tool_call)
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
                id: PermissionOptionId::new(opt.option_id.to_string()),
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

/// Extract trust options from `_meta.trustOptions[]` on the permission request.
///
/// Malformed entries are dropped (and logged) rather than materialized with
/// sentinel fields, so every returned `TrustOption` has a real `setting_key`
/// the persistence layer can act on.
pub(crate) fn extract_trust_options(args: &acp::RequestPermissionRequest) -> Vec<TrustOption> {
    let Some(meta) = &args.meta else {
        return Vec::new();
    };
    let Some(trust_options_val) = meta.get("trustOptions") else {
        return Vec::new();
    };
    let Some(arr) = trust_options_val.as_array() else {
        tracing::warn!("_meta.trustOptions is present but not an array; offering no trust tiers");
        return Vec::new();
    };
    let parsed: Vec<TrustOption> = arr.iter().filter_map(parse_trust_option).collect();
    if parsed.len() != arr.len() {
        tracing::warn!(
            raw = arr.len(),
            parsed = parsed.len(),
            "dropped malformed trust option(s) from _meta.trustOptions"
        );
    }
    parsed
}

/// Parse a single `_meta.trustOptions[]` entry. Returns `None` (so the caller
/// drops it) when a required field is absent or the wrong type — notably
/// `setting_key`, which the persistence layer keys on: an entry without it is
/// malformed, not a tier with an empty key.
fn parse_trust_option(v: &serde_json::Value) -> Option<TrustOption> {
    let label = v.get("label")?.as_str()?.to_string();
    let display = v.get("display")?.as_str()?.to_string();
    let setting_key = v
        .get("setting_key")
        .or_else(|| v.get("settingKey"))
        .and_then(|v| v.as_str())?
        .to_string();
    let patterns = v
        .get("patterns")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Some(TrustOption {
        label,
        display,
        setting_key,
        patterns,
    })
}

/// Convert our `PermissionResponse` back into an ACP `RequestPermissionResponse`.
/// `Selected` carries the picked option's id verbatim; there is no kind-based
/// re-derivation — the id IS the answer.
pub(crate) fn from_permission_response(
    response: PermissionResponse,
    args: &acp::RequestPermissionRequest,
) -> acp::RequestPermissionResponse {
    let outcome = match &response {
        PermissionResponse::Cancel => acp::RequestPermissionOutcome::Cancelled,
        PermissionResponse::Selected {
            option_id,
            trust_option,
        } => {
            // Runtime tripwire for the doc contract on `Selected`: a foreign
            // id would silently answer the agent with an option it never
            // offered, so this must survive release builds.
            if !args
                .options
                .iter()
                .any(|o| o.option_id.to_string() == option_id.as_str())
            {
                tracing::warn!(
                    option_id = %option_id,
                    "selected permission option not present in the originating request; sending as-is"
                );
            }
            let mut selected = acp::SelectedPermissionOutcome::new(acp::PermissionOptionId::new(
                option_id.as_str(),
            ));
            if let Some(label) = trust_option {
                let mut meta = serde_json::Map::new();
                meta.insert(
                    "trustOption".to_string(),
                    serde_json::Value::String(label.clone()),
                );
                selected = selected.meta(meta);
            }
            acp::RequestPermissionOutcome::Selected(selected)
        }
    };
    acp::RequestPermissionResponse::new(outcome)
}

/// Convert an ACP `SessionNotification` to our internal `Notification`.
/// Returns `None` for update types we don't surface to the UI.
pub(crate) fn session_update_to_notification(
    args: &acp::SessionNotification,
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
        acp::SessionUpdate::ToolCall(tc) => Some(Notification::ToolCallStarted(to_tool_call(tc))),
        acp::SessionUpdate::ToolCallUpdate(update) => {
            Some(Notification::ToolCallUpdated(to_tool_call_update(update)))
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
        acp::SessionUpdate::ConfigOptionUpdate(update) => Some(Notification::ConfigOptionsUpdated(
            to_config_options(&update.config_options),
        )),
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
                cost: usage.cost.as_ref().and_then(to_money),
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
            duration_ms,
            effort,
            session_id,
            refusal,
        })) = result
        {
            let ctx = context_usage.expect("context_usage should be present");
            assert!((ctx.percentage() - 75.0).abs() < f64::EPSILON);
            assert!(metering.is_none());
            assert!(tokens.is_none());
            assert!(duration_ms.is_none());
            assert_eq!(
                effort,
                EffortUpdate::Unchanged,
                "no effort field => no change"
            );
            assert!(session_id.is_none(), "no sessionId field => None (global)");
            assert!(refusal.is_none(), "plain frame carries no refusal");
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_with_effort() {
        let params = serde_json::json!({"contextUsagePercentage": 7.5, "effort": "high"});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { effort, .. })) = result {
            assert_eq!(effort, EffortUpdate::Set(EffortLevel::High));
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_unrecognized_effort_is_preserved() {
        // Backend-defined levels (cyril-1gim) must surface as `Other` and
        // render, not vanish (still logged at debug!).
        let params = serde_json::json!({"contextUsagePercentage": 7.5, "effort": "turbo"});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { effort, .. })) = result {
            assert_eq!(
                effort,
                EffortUpdate::Set(EffortLevel::Other("turbo".into()))
            );
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_empty_effort_is_unchanged() {
        // An empty effort string must not surface as a blank "◇ " toolbar badge
        // nor clear an existing one — it's the wire's "not set".
        let params = serde_json::json!({"contextUsagePercentage": 7.5, "effort": ""});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { effort, .. })) = result {
            assert_eq!(effort, EffortUpdate::Unchanged);
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_non_string_effort_is_unchanged() {
        // A present-but-non-string effort field is corrupt (warned, not silent)
        // and must neither set nor clear the badge.
        let params = serde_json::json!({"contextUsagePercentage": 7.5, "effort": 5});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { effort, .. })) = result {
            assert_eq!(effort, EffortUpdate::Unchanged);
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_null_effort_clears() {
        // tui.js checks `"effort" in e`: an explicit `effort: null` is a
        // badge-CLEAR signal (cyril-1gim), distinct from an absent field —
        // the engine can turn the badge off mid-session.
        let params = serde_json::json!({"contextUsagePercentage": 7.5, "effort": null});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated { effort, .. })) = result {
            assert_eq!(effort, EffortUpdate::Clear);
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    #[test]
    fn to_ext_notification_metadata_lone_duration_is_preserved() {
        // A duration/effort-only frame (real 2.4.1 shape: {"sessionId",
        // "turnDurationMs": 2281, "effort": "high"}) carries duration
        // without a credits aggregate. The duration is parsed independently
        // (cyril-1gim) so it can reach the turn summary; the absent
        // meteringUsage stays None (no credits fabricated at the wire).
        let params = serde_json::json!({"turnDurationMs": 2281, "effort": "high"});
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated {
            metering,
            duration_ms,
            ..
        })) = result
        {
            assert!(metering.is_none(), "no credits aggregate on this shape");
            assert_eq!(duration_ms, Some(2281), "lone duration must be preserved");
        } else {
            panic!("expected MetadataUpdated");
        }
    }

    /// Pull the refusal field out of a parsed `kiro.dev/metadata` result.
    fn refusal_of(
        result: crate::Result<Option<Notification>>,
    ) -> Option<crate::types::RefusalAlert> {
        match result {
            Ok(Some(Notification::MetadataUpdated { refusal, .. })) => refusal,
            other => panic!("expected MetadataUpdated, got {other:?}"),
        }
    }

    #[test]
    fn to_ext_notification_metadata_refusal_full() {
        // Design claim #1 (cyril-h8zb): full refusal object, subfields
        // preserved verbatim. stopReason is deliberately NOT
        // CONTENT_FILTERED — the object alone must alert (kills an
        // AND-instead-of-OR implementation).
        let params = serde_json::json!({
            "refusal": {
                "category": "unsafe",
                "explanation": "blocked by policy",
                "recommendedModel": "claude-opus"
            },
            "stopReason": "end_turn"
        });
        let alert = refusal_of(to_ext_notification("kiro.dev/metadata", &params))
            .expect("refusal object must produce an alert");
        assert_eq!(alert.category(), Some("unsafe"));
        assert_eq!(alert.explanation(), Some("blocked by policy"));
        assert_eq!(alert.recommended_model(), Some("claude-opus"));
    }

    #[test]
    fn to_ext_notification_metadata_refusal_absent_unchanged() {
        // Design claim #2 (cyril-h8zb): no refusal key + benign stopReason
        // => None (shapes a and b). Kills construct-alert-on-every-frame.
        let plain = serde_json::json!({"contextUsagePercentage": 7.5});
        assert_eq!(
            refusal_of(to_ext_notification("kiro.dev/metadata", &plain)),
            None
        );
        let benign = serde_json::json!({"contextUsagePercentage": 7.5, "stopReason": "end_turn"});
        assert_eq!(
            refusal_of(to_ext_notification("kiro.dev/metadata", &benign)),
            None
        );
    }

    #[test]
    fn to_ext_notification_metadata_content_filtered_no_object() {
        // Design claim #3 (cyril-h8zb): bare CONTENT_FILTERED alerts with
        // every subfield absent (Kiro's OR-condition, carved 2.15.0 tui.js).
        let params = serde_json::json!({"stopReason": "CONTENT_FILTERED"});
        let alert = refusal_of(to_ext_notification("kiro.dev/metadata", &params))
            .expect("bare CONTENT_FILTERED must alert");
        assert_eq!(alert.category(), None);
        assert_eq!(alert.explanation(), None);
        assert_eq!(alert.recommended_model(), None);

        // Issue addendum (2.12.3): "refusal" is a first-class stopReason zod
        // literal — tolerated alongside CONTENT_FILTERED (review fix).
        let params = serde_json::json!({"stopReason": "refusal"});
        assert!(
            refusal_of(to_ext_notification("kiro.dev/metadata", &params)).is_some(),
            "bare stopReason 'refusal' must alert"
        );
    }

    #[test]
    fn to_ext_notification_metadata_content_filtered_case_exact() {
        // The wire literal is exact — lowercase must NOT match (kills a
        // case-insensitive comparison; the backend emits the SCREAMING form).
        let params = serde_json::json!({"stopReason": "content_filtered"});
        assert_eq!(
            refusal_of(to_ext_notification("kiro.dev/metadata", &params)),
            None
        );
    }

    /// Log-capture writer (cyril-1gim idiom, hoisted for the refusal fences).
    #[derive(Clone, Default)]
    struct CaptureWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("capture lock").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Run `f` under a DEBUG-level capture subscriber; return its result and
    /// the captured log text.
    fn with_captured_logs<T>(f: impl FnOnce() -> T) -> (T, String) {
        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(capture.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let logs =
            String::from_utf8(capture.0.lock().expect("capture lock").clone()).expect("utf8 logs");
        (result, logs)
    }

    #[test]
    fn to_ext_notification_metadata_refusal_and_stop_reason_not_flagged() {
        // refusal and stopReason are recognized-and-parsed (cyril-h8zb); they
        // must never trip the unknown-key debug log reserved for genuine
        // backend additions (cyril-1gim).
        let params = serde_json::json!({
            "contextUsagePercentage": 7.5,
            "refusal": {"category": "unsafe", "explanation": "blocked", "recommendedModel": "claude-opus"},
            "stopReason": "CONTENT_FILTERED"
        });
        let (result, logs) =
            with_captured_logs(|| to_ext_notification("kiro.dev/metadata", &params));
        assert!(refusal_of(result).is_some(), "refusal must parse");
        assert!(
            !logs.contains("unrecognized top-level field"),
            "refusal/stopReason must not be flagged unknown; captured: {logs}"
        );
    }

    #[test]
    fn to_ext_notification_metadata_unknown_key_logged() {
        // A backend addition to kiro.dev/metadata must land visibly: the
        // unrecognized top-level key is named in a debug log (cyril-1gim).
        let params =
            serde_json::json!({"contextUsagePercentage": 7.5, "brandNewField": {"nested": 42}});
        let (result, logs) =
            with_captured_logs(|| to_ext_notification("kiro.dev/metadata", &params));
        assert!(result.is_ok());
        assert!(
            logs.contains("brandNewField"),
            "unknown key must be named in the debug log; captured: {logs}"
        );
    }

    #[test]
    fn to_ext_notification_metadata_refusal_partial_matrix() {
        // Design claim #4 (cyril-h8zb): each present, non-empty, string-typed
        // subfield is preserved; everything else is None; no panic. The empty
        // object still alerts (JS `if(r||…)` — `{}` is truthy).
        type PartialCase = (
            serde_json::Value,
            Option<&'static str>,
            Option<&'static str>,
            Option<&'static str>,
        );
        let cases: &[PartialCase] = &[
            (
                serde_json::json!({"explanation": "why"}),
                None,
                Some("why"),
                None,
            ),
            (
                serde_json::json!({"category": "unsafe"}),
                Some("unsafe"),
                None,
                None,
            ),
            (
                serde_json::json!({"recommendedModel": "m1"}),
                None,
                None,
                Some("m1"),
            ),
            (serde_json::json!({}), None, None, None),
            (serde_json::json!({"explanation": ""}), None, None, None),
            (
                serde_json::json!({"explanation": null, "category": "c"}),
                Some("c"),
                None,
                None,
            ),
        ];
        for (obj, category, explanation, recommended) in cases {
            let params = serde_json::json!({"refusal": obj});
            let alert = refusal_of(to_ext_notification("kiro.dev/metadata", &params))
                .unwrap_or_else(|| panic!("refusal object {obj} must alert"));
            assert_eq!(alert.category(), *category, "category for {obj}");
            assert_eq!(alert.explanation(), *explanation, "explanation for {obj}");
            assert_eq!(
                alert.recommended_model(),
                *recommended,
                "recommendedModel for {obj}"
            );
        }
    }

    #[test]
    fn to_ext_notification_metadata_refusal_corrupt_object() {
        // Design claim #5 (cyril-h8zb), shapes i and j: a non-object refusal
        // warns and is ignored — but alert-worthiness via CONTENT_FILTERED
        // survives independently (kills corrupt-aborts-the-branch).
        let alone = serde_json::json!({"refusal": 5});
        let (result, logs) =
            with_captured_logs(|| to_ext_notification("kiro.dev/metadata", &alone));
        assert_eq!(
            refusal_of(result),
            None,
            "corrupt refusal alone must not alert"
        );
        assert!(
            logs.contains("not an object"),
            "corrupt refusal must warn; captured: {logs}"
        );

        let with_stop = serde_json::json!({"refusal": "x", "stopReason": "CONTENT_FILTERED"});
        let alert = refusal_of(to_ext_notification("kiro.dev/metadata", &with_stop))
            .expect("CONTENT_FILTERED must still alert past a corrupt refusal");
        assert_eq!(alert.explanation(), None);
    }

    #[test]
    fn to_ext_notification_metadata_refusal_corrupt_subfield_and_stop_reason() {
        // Design claim #5 (cyril-h8zb), shapes h and k: a wrong-typed subfield
        // warns and drops to None while siblings survive; a wrong-typed
        // stopReason warns and is treated as absent.
        let params = serde_json::json!({"refusal": {"explanation": 42, "category": "unsafe"}});
        let (result, logs) =
            with_captured_logs(|| to_ext_notification("kiro.dev/metadata", &params));
        let alert = refusal_of(result).expect("object with corrupt subfield still alerts");
        assert_eq!(
            alert.category(),
            Some("unsafe"),
            "sibling subfield survives"
        );
        assert_eq!(alert.explanation(), None, "corrupt subfield drops to None");
        assert!(
            logs.contains("subfield not a string"),
            "must warn; captured: {logs}"
        );

        let bad_stop = serde_json::json!({"stopReason": 17});
        let (result, logs) =
            with_captured_logs(|| to_ext_notification("kiro.dev/metadata", &bad_stop));
        assert_eq!(
            refusal_of(result),
            None,
            "corrupt stopReason is absent, not alert"
        );
        assert!(logs.contains("not a string"), "must warn; captured: {logs}");
    }

    #[test]
    fn to_ext_notification_metadata_refusal_preserves_existing_fields() {
        // Design claim #6 / AC1 (cyril-h8zb): a refusal-bearing kitchen-sink
        // frame parses every sibling field identically to the refusal-free
        // control (kills parse-order/consumption bugs).
        let control = serde_json::json!({
            "sessionId": "sub-1",
            "contextUsagePercentage": 42.5,
            "meteringUsage": [{"unit": "credit", "unitPlural": "credits", "value": 0.018}],
            "turnDurationMs": 1948,
            "inputTokens": 100, "outputTokens": 50, "cachedTokens": 25,
            "effort": "high"
        });
        let mut with_refusal = control.clone();
        with_refusal["refusal"] = serde_json::json!(
            {"category": "unsafe", "explanation": "blocked", "recommendedModel": "claude-opus"}
        );
        with_refusal["stopReason"] = serde_json::json!("CONTENT_FILTERED");

        let assert_siblings = |params: &serde_json::Value, want_refusal: bool| {
            let result = to_ext_notification("kiro.dev/metadata", params);
            if let Ok(Some(Notification::MetadataUpdated {
                context_usage,
                metering,
                tokens,
                duration_ms,
                effort,
                session_id,
                refusal,
            })) = result
            {
                let ctx = context_usage.expect("context present");
                assert!((ctx.percentage() - 42.5).abs() < f64::EPSILON);
                let m = metering.expect("metering present");
                assert!((m.credits().expect("credits") - 0.018).abs() < 0.001);
                assert_eq!(m.duration_ms(), Some(1948));
                assert_eq!(duration_ms, Some(1948));
                let t = tokens.expect("tokens present");
                assert_eq!((t.input(), t.output(), t.cached()), (100, 50, Some(25)));
                assert_eq!(effort, EffortUpdate::Set(crate::types::EffortLevel::High));
                assert_eq!(session_id, Some(SessionId::new("sub-1")));
                assert_eq!(refusal.is_some(), want_refusal);
            } else {
                panic!("expected MetadataUpdated, got {result:?}");
            }
        };
        assert_siblings(&control, false);
        assert_siblings(&with_refusal, true);
    }

    #[test]
    fn to_ext_notification_metadata_refusal_keeps_session_scope() {
        // Design claim #11 (cyril-h8zb): the sessionId routing tag survives
        // alongside a refusal — a subagent refusal frame stays divertible.
        let params = serde_json::json!({
            "sessionId": "subagent-7",
            "refusal": {"explanation": "blocked"}
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated {
            session_id,
            refusal,
            ..
        })) = result
        {
            assert_eq!(session_id, Some(SessionId::new("subagent-7")));
            assert!(refusal.is_some());
        } else {
            panic!("expected MetadataUpdated, got {result:?}");
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
            let ctx = context_usage.expect("context_usage should be present");
            assert!((ctx.percentage() - 7.11).abs() < 0.01);
            let m = metering.unwrap();
            assert!((m.credits().unwrap() - 0.018).abs() < 0.001);
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
            metering,
            tokens,
            session_id,
            ..
        })) = result
        {
            assert!(metering.is_none());
            assert!(tokens.is_none());
            assert_eq!(
                session_id,
                Some(SessionId::new("s1")),
                "params-level sessionId must be extracted (cyril-fh06)"
            );
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
            assert_eq!(m.credits(), Some(0.0));
            assert_eq!(m.duration_ms(), Some(12));
        } else {
            panic!("expected MetadataUpdated, got {:?}", result);
        }
    }

    #[test]
    fn parse_metadata_preserves_unlike_metering_units() {
        let params = serde_json::json!({
            "meteringUsage": [
                {"unit": "credit", "unitPlural": "credits", "value": 0.25},
                {"unit": "request", "unitPlural": "requests", "value": 2.0},
                {"unit": "credit", "unitPlural": "credits", "value": -1.0}
            ]
        });
        let notification = to_ext_notification("kiro.dev/metadata", &params)
            .expect("conversion succeeds")
            .expect("metadata converts");
        let Notification::MetadataUpdated {
            metering: Some(metering),
            ..
        } = notification
        else {
            panic!("expected metering metadata");
        };
        assert_eq!(
            metering
                .charges()
                .iter()
                .map(|charge| (charge.unit(), charge.amount()))
                .collect::<Vec<_>>(),
            vec![("credit", 0.25), ("request", 2.0)]
        );
        assert_eq!(metering.credits(), Some(0.25));
    }

    #[test]
    fn metering_absence_parser_does_not_fabricate_zero() {
        for metering_usage in [
            serde_json::json!([]),
            serde_json::json!([{"unit": "credit"}]),
        ] {
            let params = serde_json::json!({
                "meteringUsage": metering_usage,
                "turnDurationMs": 12
            });
            let notification = to_ext_notification("kiro.dev/metadata", &params)
                .expect("conversion should succeed")
                .expect("metadata frame should convert");
            let Notification::MetadataUpdated {
                metering,
                duration_ms,
                ..
            } = notification
            else {
                panic!("expected MetadataUpdated");
            };
            assert!(
                metering.is_none(),
                "metering without a numeric value must not become explicit zero"
            );
            assert_eq!(
                duration_ms,
                Some(12),
                "duration remains independently available"
            );
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
    fn metadata_without_context_retains_last_context() {
        // Replays the captured 2.4.1 wire shape (trace-2.4.1-multi-subagent.jsonl):
        // mid-turn frames like {"sessionId": …, "turnDurationMs": 2281, "effort": "high"}
        // omit contextUsagePercentage entirely. Such a frame must leave the
        // last known context usage unchanged — not stamp it to 0.0.
        let mut ctrl = crate::session::SessionController::new();

        let with_context = serde_json::json!({"contextUsagePercentage": 42.0});
        let n = to_ext_notification("kiro.dev/metadata", &with_context)
            .expect("conversion should succeed")
            .expect("metadata frame should convert");
        ctrl.apply_notification(&n);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(-1.0) - 42.0).abs()
                < f64::EPSILON,
            "frame WITH contextUsagePercentage must update context usage"
        );

        let without_context = serde_json::json!({
            "sessionId": "03ddba37-dc26-48fc-acad-0c35e4b2597b",
            "turnDurationMs": 2281,
            "effort": "high"
        });
        let n = to_ext_notification("kiro.dev/metadata", &without_context)
            .expect("conversion should succeed")
            .expect("metadata frame should convert");
        ctrl.apply_notification(&n);
        assert!(
            (ctrl.context_usage().map(|u| u.percentage()).unwrap_or(-1.0) - 42.0).abs()
                < f64::EPSILON,
            "frame WITHOUT contextUsagePercentage must retain the last context usage, got {:?}",
            ctrl.context_usage()
        );
    }

    #[test]
    fn metadata_without_context_still_applies_metering_and_effort() {
        // A context-less frame is not an empty frame: metering, duration, and
        // effort it carries must still flow through unchanged.
        let params = serde_json::json!({
            "sessionId": "s1",
            "meteringUsage": [
                {"unit": "credit", "unitPlural": "credits", "value": 0.018}
            ],
            "turnDurationMs": 2281,
            "effort": "high"
        });
        let result = to_ext_notification("kiro.dev/metadata", &params);
        if let Ok(Some(Notification::MetadataUpdated {
            metering, effort, ..
        })) = result
        {
            let m = metering.expect("metering on a context-less frame must be preserved");
            assert!((m.credits().unwrap() - 0.018).abs() < 0.001);
            assert_eq!(m.duration_ms(), Some(2281));
            assert_eq!(effort, EffortUpdate::Set(EffortLevel::High));
        } else {
            panic!("expected MetadataUpdated, got {:?}", result);
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
    fn to_tool_call_carries_frame_raw_input() {
        let acp_call = agent_client_protocol::ToolCall::new("tc_1", "Read file")
            .kind(agent_client_protocol::ToolKind::Read)
            .status(agent_client_protocol::ToolCallStatus::InProgress)
            .raw_input(serde_json::json!({"path": "original.rs"}));

        let result = to_tool_call(&acp_call);
        assert_eq!(result.id().as_str(), "tc_1");
        assert_eq!(
            result.raw_input(),
            Some(&serde_json::json!({"path": "original.rs"}))
        );
    }

    #[test]
    fn to_tool_call_update_carries_frame_raw_input() {
        let update = agent_client_protocol::ToolCallUpdate::new(
            "tc_2",
            agent_client_protocol::ToolCallUpdateFields::new()
                .kind(agent_client_protocol::ToolKind::Execute)
                .status(agent_client_protocol::ToolCallStatus::Completed)
                .raw_input(serde_json::json!({"cmd": "ls"})),
        );
        let result = to_tool_call_update(&update);
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
    fn from_permission_response_selected_exact_id_without_meta() {
        // cyril-qo13 C2: the reply carries the exact picked id, and when no
        // trust label was chosen the serialized outcome has NO `_meta` key at
        // all (a `"_meta": null` would differ from the reference client bytes).
        let req = make_permission_request(vec![
            ("q-option-0", "First", acp::PermissionOptionKind::AllowOnce),
            ("q-option-1", "Second", acp::PermissionOptionKind::AllowOnce),
            ("q-option-2", "Third", acp::PermissionOptionKind::AllowOnce),
        ]);

        let resp = from_permission_response(
            PermissionResponse::Selected {
                option_id: PermissionOptionId::new("q-option-1"),
                trust_option: None,
            },
            &req,
        );
        let json = serde_json::to_value(&resp).expect("response serializes");
        assert_eq!(json["outcome"]["optionId"], "q-option-1");
        assert!(
            json["outcome"].get("_meta").is_none(),
            "no-trust reply must not carry a _meta key: {json}"
        );
    }

    #[test]
    fn from_permission_response_selected_trust_label_verbatim() {
        // cyril-qo13 C2/C4: a phase-2 trust label — spaces, em-dash, parens —
        // lands verbatim under `_meta.trustOption` (v2 echo, unchanged shape).
        let req = make_permission_request(vec![
            ("accept", "Allow", acp::PermissionOptionKind::AllowOnce),
            (
                "always-accept",
                "Always",
                acp::PermissionOptionKind::AllowAlways,
            ),
        ]);

        let label = "Allow similar commands — ripgrep (rg …)";
        let resp = from_permission_response(
            PermissionResponse::Selected {
                option_id: PermissionOptionId::new("always-accept"),
                trust_option: Some(label.to_string()),
            },
            &req,
        );
        let json = serde_json::to_value(&resp).expect("response serializes");
        assert_eq!(json["outcome"]["optionId"], "always-accept");
        assert_eq!(json["outcome"]["_meta"]["trustOption"], label);
    }

    #[test]
    fn from_permission_response_selected_foreign_id_warns_and_sends_as_is() {
        // cyril-qo13 doc contract on `Selected.option_id` (load-bearing): a
        // foreign id still converts — the UI can only pick real options, so
        // this is state corruption — but the release-surviving warn must fire.
        #[derive(Clone, Default)]
        struct CaptureWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

        impl std::io::Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("capture lock").extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
            type Writer = CaptureWriter;
            fn make_writer(&'a self) -> Self::Writer {
                self.clone()
            }
        }

        let req = make_permission_request(vec![(
            "opt_allow",
            "Yes",
            acp::PermissionOptionKind::AllowOnce,
        )]);

        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .finish();
        let resp = tracing::subscriber::with_default(subscriber, || {
            from_permission_response(
                PermissionResponse::Selected {
                    option_id: PermissionOptionId::new("not-an-offered-option"),
                    trust_option: None,
                },
                &req,
            )
        });

        let json = serde_json::to_value(&resp).expect("response serializes");
        assert_eq!(json["outcome"]["optionId"], "not-an-offered-option");
        let logs =
            String::from_utf8(capture.0.lock().expect("capture lock").clone()).expect("utf8 logs");
        assert!(
            logs.contains("not present in the originating request"),
            "foreign-id warn must fire; captured logs: {logs}"
        );
    }

    #[test]
    fn extract_trust_options_parses_shell_payload() {
        let meta: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
            r#"{
                "trustOptions": [
                    {
                        "label": "Full command",
                        "display": "echo hello",
                        "setting_key": "allowedCommands",
                        "patterns": ["echo hello"]
                    },
                    {
                        "label": "Base command",
                        "display": "echo *",
                        "setting_key": "allowedCommands",
                        "patterns": ["echo( .*)?"]
                    }
                ]
            }"#,
        )
        .unwrap();

        let fields = acp::ToolCallUpdateFields::new();
        let args = acp::RequestPermissionRequest::new(
            "s1",
            acp::ToolCallUpdate::new("tc_1", fields),
            vec![],
        )
        .meta(meta);

        let opts = extract_trust_options(&args);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].label, "Full command");
        assert_eq!(opts[0].display, "echo hello");
        assert_eq!(opts[0].setting_key, "allowedCommands");
        assert_eq!(opts[0].patterns, vec!["echo hello"]);
        assert_eq!(opts[1].label, "Base command");
        assert_eq!(opts[1].patterns, vec!["echo( .*)?"]);
    }

    #[test]
    fn extract_trust_options_returns_empty_without_meta() {
        let fields = acp::ToolCallUpdateFields::new();
        let args = acp::RequestPermissionRequest::new(
            "s1",
            acp::ToolCallUpdate::new("tc_1", fields),
            vec![],
        );
        assert!(extract_trust_options(&args).is_empty());
    }

    #[test]
    fn extract_trust_options_handles_camel_case_setting_key() {
        let meta: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
            r#"{
                "trustOptions": [
                    {
                        "label": "Specific paths",
                        "display": "/tmp/file",
                        "settingKey": "runtime_read_paths",
                        "patterns": ["/tmp/file"]
                    }
                ]
            }"#,
        )
        .unwrap();

        let fields = acp::ToolCallUpdateFields::new();
        let args = acp::RequestPermissionRequest::new(
            "s1",
            acp::ToolCallUpdate::new("tc_1", fields),
            vec![],
        )
        .meta(meta);

        let opts = extract_trust_options(&args);
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].setting_key, "runtime_read_paths");
    }

    #[test]
    fn extract_trust_options_drops_entry_missing_setting_key() {
        // A well-formed tier plus one missing `setting_key`: the malformed one
        // is dropped (not materialized with an empty key) and the good one
        // survives — no sentinel reaches the persistence layer.
        let meta: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
            r#"{
                "trustOptions": [
                    {
                        "label": "Full command",
                        "display": "echo hello",
                        "setting_key": "allowedCommands",
                        "patterns": ["echo hello"]
                    },
                    {
                        "label": "No key",
                        "display": "echo *",
                        "patterns": ["echo( .*)?"]
                    }
                ]
            }"#,
        )
        .unwrap();

        let fields = acp::ToolCallUpdateFields::new();
        let args = acp::RequestPermissionRequest::new(
            "s1",
            acp::ToolCallUpdate::new("tc_1", fields),
            vec![],
        )
        .meta(meta);

        let opts = extract_trust_options(&args);
        assert_eq!(opts.len(), 1, "the setting_key-less tier must be dropped");
        assert_eq!(opts[0].label, "Full command");
        assert_eq!(opts[0].setting_key, "allowedCommands");
    }

    #[test]
    fn extract_trust_options_returns_empty_when_not_an_array() {
        // `trustOptions` present but the wrong shape → no tiers (distinct from
        // the absent-meta case, and logged at warn rather than silently empty).
        let meta: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{ "trustOptions": "not-an-array" }"#).unwrap();

        let fields = acp::ToolCallUpdateFields::new();
        let args = acp::RequestPermissionRequest::new(
            "s1",
            acp::ToolCallUpdate::new("tc_1", fields),
            vec![],
        )
        .meta(meta);

        assert!(extract_trust_options(&args).is_empty());
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

    // Slice 7 (cyril-atjw) — ACP-coverage verification spike (D9, NON-gating).
    // Confirms schema 0.11.2 deserializes the typed `session/update` frames KAS
    // emits live (captured under tests/fixtures/kas/). `SessionUpdate` is
    // `#[serde(tag = "sessionUpdate")]` with NO `#[serde(other)]` catch-all, so an
    // unknown KAS variant would hard-fail HERE — at the acp Client deser layer,
    // before convert/Engine ever runs. A future Err is a documented upgrade-trigger
    // for KAS-2a (cyril-j16p), not a code defense. The standard ACP variants
    // (tool_call, available_commands_update, …) the probe logs truncated are
    // already exercised by the v2 convert tests above via the same deser path;
    // the KAS-distinctive `session_info_update` envelope is exercised here (the
    // captured instance is a `user_message_id_assigned` sub-kind — the `turn_end`
    // sub-kind ADR-0004 keys off is a KAS-2a capture, cyril-j16p).
    #[test]
    fn schema_deserializes_captured_kas_session_updates() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kas");
        let mut checked = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("kas fixtures dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = std::fs::read_to_string(&path).expect("read fixture");
            let value: serde_json::Value = serde_json::from_str(&raw).expect("fixture is JSON");
            let variant = value["update"]["sessionUpdate"]
                .as_str()
                .unwrap_or("?")
                .to_string();
            // The exact layer the acp Client deserializes a `session/update` at.
            let parsed: Result<acp::SessionNotification, _> = serde_json::from_value(value);
            assert!(
                parsed.is_ok(),
                "schema 0.11.2 failed to deserialize captured KAS `{variant}` frame {}: {:?}\n\
                 -> no-serde(other) upgrade-trigger; record for KAS-2a (cyril-j16p)",
                path.display(),
                parsed.err(),
            );
            checked.push(variant);
        }
        checked.sort();
        assert!(
            checked.contains(&"session_info_update".to_string()),
            "the KAS-distinctive session_info_update variant must be covered; got {checked:?}"
        );
    }

    #[test]
    fn captured_omp_prompt_usage_maps_exactly() {
        let capture =
            include_str!("../../../../../experiments/conductor-spike/omp-usage-update-2turn.jsonl");
        let actual: Vec<TokenUsage> = capture
            .lines()
            .filter_map(|line| {
                let frame: serde_json::Value =
                    serde_json::from_str(line).expect("captured frame is JSON");
                let result = frame.get("msg")?.get("result")?;
                result.get("usage")?;
                let response: acp::PromptResponse =
                    serde_json::from_value(result.clone()).expect("captured prompt response");
                response.usage.as_ref().map(to_token_usage)
            })
            .collect();
        assert_eq!(
            actual,
            vec![
                TokenUsage::new(19_446, 19_428, 18, None, None, None),
                TokenUsage::new(19_464, 259, 5, None, Some(19_200), None),
            ]
        );
        assert!(
            acp::PromptResponse::new(acp::StopReason::EndTurn)
                .usage
                .is_none(),
            "absent standard usage must stay absent"
        );
    }

    #[test]
    fn captured_omp_initial_config_exposes_model() {
        let capture =
            include_str!("../../../../../experiments/conductor-spike/omp-usage-update-2turn.jsonl");
        let response = capture
            .lines()
            .find_map(|line| {
                let frame: serde_json::Value =
                    serde_json::from_str(line).expect("captured frame is JSON");
                let result = frame.get("msg")?.get("result")?;
                (result.get("sessionId").is_some() && result.get("configOptions").is_some()).then(
                    || {
                        serde_json::from_value::<acp::NewSessionResponse>(result.clone())
                            .expect("captured new-session response")
                    },
                )
            })
            .expect("capture contains session/new response");
        let options = to_config_options(
            response
                .config_options
                .as_deref()
                .expect("omp supplies initial config options"),
        );
        let model = options
            .iter()
            .find(|option| option.key == "model")
            .expect("model config option");
        assert_eq!(model.value.as_deref(), Some("openai-codex/gpt-5.6-luna"));
    }
}

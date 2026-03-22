use crate::types::*;

pub(crate) fn to_tool_kind(kind: agent_client_protocol::ToolKind) -> ToolKind {
    match kind {
        agent_client_protocol::ToolKind::Read => ToolKind::Read,
        agent_client_protocol::ToolKind::Edit
        | agent_client_protocol::ToolKind::Delete
        | agent_client_protocol::ToolKind::Move => ToolKind::Write,
        agent_client_protocol::ToolKind::Execute => ToolKind::Execute,
        _ => ToolKind::Other,
    }
}

pub(crate) fn to_tool_call_status(
    status: agent_client_protocol::ToolCallStatus,
) -> ToolCallStatus {
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
        other => Err(crate::Error::from_kind(crate::ErrorKind::Protocol {
            message: format!("unknown extension: {other}"),
        })),
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
    fn to_tool_kind_search_maps_to_other() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Search),
            ToolKind::Other
        );
    }

    #[test]
    fn to_tool_kind_think_maps_to_other() {
        assert_eq!(
            to_tool_kind(agent_client_protocol::ToolKind::Think),
            ToolKind::Other
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
        cached.insert(
            "tc_1".to_string(),
            serde_json::json!({"path": "cached.rs"}),
        );

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
        assert_eq!(
            result.raw_input(),
            Some(&serde_json::json!({"cmd": "ls"}))
        );
    }
}

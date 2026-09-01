use agent_client_protocol::UntypedMessage;

fn message(method: &str, params: serde_json::Value) -> UntypedMessage {
    match UntypedMessage::new(method, params) {
        Ok(message) => message,
        Err(error) => panic!("test message must serialize: {error}"),
    }
}

#[test]
fn unknown_session_update_fence_claims_only_undecodable_standard_updates() {
    let known = message(
        "session/update",
        serde_json::json!({
            "sessionId": "s",
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "hello"},
            },
        }),
    );
    assert!(!crate::protocol::client::is_unknown_session_update(&known));

    let future = message(
        "session/update",
        serde_json::json!({
            "sessionId": "s",
            "update": {"sessionUpdate": "future_update", "payload": 1},
        }),
    );
    assert!(crate::protocol::client::is_unknown_session_update(&future));

    let extension = message(
        "_kiro.dev/session/update",
        serde_json::json!({"not": "a standard update"}),
    );
    assert!(!crate::protocol::client::is_unknown_session_update(
        &extension
    ));

    let missing_session = message(
        "session/update",
        serde_json::json!({
            "update": {"sessionUpdate": "future_update"},
        }),
    );
    assert!(!crate::protocol::client::is_unknown_session_update(
        &missing_session
    ));

    let malformed_tag = message(
        "session/update",
        serde_json::json!({
            "sessionId": "s",
            "update": {"sessionUpdate": 42},
        }),
    );
    assert!(!crate::protocol::client::is_unknown_session_update(
        &malformed_tag
    ));

    let malformed_known = message(
        "session/update",
        serde_json::json!({
            "sessionId": "s",
            "update": {"sessionUpdate": "agent_message_chunk"},
        }),
    );
    assert!(!crate::protocol::client::is_unknown_session_update(
        &malformed_known
    ));
}

/// The tag list is a hand-maintained mirror of the schema's `SessionUpdate`
/// variants; keep it honest by constructing one instance of every stable
/// variant and asserting exact SET equality with the list — a stale or
/// misspelled entry fails here instead of silently misclassifying frames.
/// (A variant the SDK adds later needs no list entry to route correctly: it
/// decodes typed and never consults the list.)
#[test]
fn known_session_update_tags_match_the_schema() {
    use agent_client_protocol::schema::v1 as acp;

    let text = || acp::ContentChunk::new(acp::ContentBlock::from("x".to_owned()));
    let variants = vec![
        acp::SessionUpdate::UserMessageChunk(text()),
        acp::SessionUpdate::AgentMessageChunk(text()),
        acp::SessionUpdate::AgentThoughtChunk(text()),
        acp::SessionUpdate::ToolCall(acp::ToolCall::new("t1", "title")),
        acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
            "t1",
            acp::ToolCallUpdateFields::new(),
        )),
        acp::SessionUpdate::Plan(acp::Plan::new(Vec::new())),
        acp::SessionUpdate::AvailableCommandsUpdate(acp::AvailableCommandsUpdate::new(Vec::new())),
        acp::SessionUpdate::CurrentModeUpdate(acp::CurrentModeUpdate::new("m")),
        acp::SessionUpdate::ConfigOptionUpdate(acp::ConfigOptionUpdate::new(Vec::new())),
        acp::SessionUpdate::SessionInfoUpdate(acp::SessionInfoUpdate::new()),
        acp::SessionUpdate::UsageUpdate(acp::UsageUpdate::new(0, 1)),
    ];
    let mut schema_tags: Vec<String> = variants
        .iter()
        .map(|variant| {
            let value = match serde_json::to_value(variant) {
                Ok(value) => value,
                Err(error) => panic!("schema variant must serialize: {error}"),
            };
            match value
                .get("sessionUpdate")
                .and_then(serde_json::Value::as_str)
            {
                Some(tag) => tag.to_owned(),
                None => panic!("schema variant carried no sessionUpdate tag: {value}"),
            }
        })
        .collect();
    schema_tags.sort_unstable();
    let mut listed: Vec<String> = crate::protocol::client::KNOWN_SESSION_UPDATE_TAGS
        .iter()
        .map(|tag| (*tag).to_owned())
        .collect();
    listed.sort_unstable();
    assert_eq!(
        listed, schema_tags,
        "KNOWN_SESSION_UPDATE_TAGS drifted from the schema's SessionUpdate variants"
    );
}

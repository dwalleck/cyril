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

    for tag in crate::protocol::client::KNOWN_SESSION_UPDATE_TAGS {
        let catalog_entry = message(
            "session/update",
            serde_json::json!({
                "sessionId": "s",
                "update": {"sessionUpdate": tag},
            }),
        );
        assert!(
            !crate::protocol::client::is_unknown_session_update(&catalog_entry),
            "stable v1 tag {tag} must reach the typed decoder"
        );
    }

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
    let error = crate::protocol::client::malformed_session_update(&malformed_known);
    assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
    assert!(error.data.as_ref().is_some_and(|data| {
        data.to_string()
            .contains("malformed standard session/update")
    }));
}

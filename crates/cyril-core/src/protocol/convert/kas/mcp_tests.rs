use super::*;
use crate::protocol::engine::{Engine, KasEngine, V2Engine};
use crate::test_support::must_succeed;
use serde_json::json;

fn update(method: &str, value: Value) -> McpUpdate {
    match must_succeed(
        KasEngine::default().convert_ext_notification(method, &value),
        "MCP engine conversion",
    ) {
        Some(Notification::McpUpdate(update)) => update,
        other => panic!("MCP family must reach typed notification: {other:?}"),
    }
}

#[test]
fn kas_engine_decodes_complete_union_catalog_registry_and_resource_provenance() {
    let value = json!({"sessionId":"real-root", "servers":[
        {"name":"connecting","status":"connecting","authType":"oauth"},
        {"name":"connected","status":"connected", "tools":[{"name":"opaque tool","description":"tool","disabled":true,"inputSchema":{"type":"object"}}],
         "prompts":[{"name":"Prompt","promptName":"prompt","arguments":[{"name":"marker","required":true}]}],
         "resources":[{"name":"resource","uri":"canary:///exact","mimeType":"text/plain"}],
         "resourceTemplates":[{"name":"template","uriTemplate":"canary:///{id}","description":"template"}],
         "_meta":{"kiro":{"resource":{"resourceType":"mcpServer","source":{"origin":"workspace","future":7},"krn":"krn:exact","layer":"workspace","item":{"name":"connected"}}}}},
        {"name":"auth","status":"failed","failedAuthorization":true,"authorizationUrl":"https://auth.invalid/exact","errorMessage":"Unauthorized","errorCode":"401"},
        {"name":"transport","status":"failed","failedAuthorization":false,"errorMessage":"connection refused"},
        {"name":"disabled","status":"disabled"}, {"name":"future","status":"sleeping"}],
        "accessMode":"registry", "accessModeFilteredServers":["local"],"unresolvedRegistryServers":["missing"],
        "registryServers":[{"name":"catalog-only","version":"1","description":"governed","enabled":false}]});
    let McpUpdate::Snapshot(snapshot) = update("kiro/mcp/status", value) else {
        panic!("valid snapshot");
    };
    assert_eq!(snapshot.session_id.as_str(), "real-root");
    assert_eq!(snapshot.servers.len(), 6);
    assert_eq!(snapshot.access_mode, Some(McpAccessMode::Registry));
    assert_eq!(
        snapshot.access_mode_filtered_servers.as_deref(),
        Some(["local".to_owned()].as_slice())
    );
    assert_eq!(
        snapshot.unresolved_registry_servers.as_deref(),
        Some(["missing".to_owned()].as_slice())
    );
    assert!(
        !snapshot
            .registry_servers
            .as_ref()
            .unwrap_or_else(|| panic!("registry"))[0]
            .enabled
    );
    let server = &snapshot.servers[1];
    let McpServerStatus::Connected { catalog } = &server.status else {
        panic!("connected");
    };
    assert!(catalog.tools[0].disabled);
    assert_eq!(catalog.prompts[0].arguments[0].name, "marker");
    assert_eq!(catalog.resources[0].uri, "canary:///exact");
    assert_eq!(catalog.resource_templates[0].uri_template, "canary:///{id}");
    let provenance = server
        .provenance
        .as_ref()
        .unwrap_or_else(|| panic!("provenance"));
    assert_eq!(provenance.origin.as_deref(), Some("workspace"));
    assert!(provenance.details.contains("future"));
    assert!(
        matches!(&snapshot.servers[5].status, McpServerStatus::Unknown {status} if status == "sleeping")
    );
}

#[test]
fn absent_registry_is_not_non_registry_and_malformed_known_status_is_visible() {
    let McpUpdate::Snapshot(snapshot) =
        update("kiro/mcp/status", json!({"sessionId":"root","servers":[]}))
    else {
        panic!("empty is valid");
    };
    assert!(snapshot.access_mode.is_none());
    assert!(snapshot.registry_servers.is_none());
    assert!(snapshot.servers.is_empty());
    for value in [
        json!({"sessionId":"root","servers":"not-array"}),
        json!({"sessionId":"root","servers":[{"name":"broken","status":"failed","failedAuthorization":"false","errorMessage":"bad"}]}),
        json!({"sessionId":"root","servers":[{"name":"broken","status":"connected","tools":[]}]}),
        json!({"sessionId":"root","servers":[],"registryServers":[{"name":"broken","enabled":"yes"}]}),
    ] {
        assert!(
            matches!(update("kiro/mcp/status", value), McpUpdate::Diagnostic {session_id, message} if session_id.as_str() == "root" && message.contains("Malformed"))
        );
    }
    for (reason, expected) in [
        ("api_failure", McpPolicy::ApiFailure),
        ("admin_disabled", McpPolicy::AdminDisabled),
        ("no_endpoint", McpPolicy::NoEndpoint),
        ("future", McpPolicy::Unknown("future".into())),
    ] {
        assert!(
            matches!(update("kiro/mcp/governance_disabled", json!({"sessionId":"root","reason":reason})), McpUpdate::Policy {policy, ..} if policy == expected)
        );
    }
    assert!(
        V2Engine
            .convert_ext_notification("kiro/mcp/status", &json!({"sessionId":"root","servers":[]}))
            .is_ok_and(|value| value.is_none())
    );
    assert!(
        must_succeed(
            KasEngine::default()
                .convert_ext_notification("kiro/mcp/status/near-miss", &Value::Null),
            "near miss"
        )
        .is_none()
    );
}

#[test]
fn malformed_unscoped_frames_are_visible_without_inventing_a_session() {
    for value in [
        json!({"servers":[]}),
        json!({"sessionId":null,"servers":[]}),
        json!({"sessionId":"","reason":"admin_disabled"}),
    ] {
        let notification = must_succeed(
            KasEngine::default().convert_ext_notification("kiro/mcp/status", &value),
            "recognized malformed status",
        )
        .unwrap_or_else(|| panic!("visible diagnostic"));
        assert!(
            matches!(notification,Notification::BridgeError {operation,message} if operation=="MCP status" && message.contains("sessionId") && message.contains("unchanged"))
        );
    }
}

#[test]
fn read_results_are_ordered_external_data_with_nontext_metadata_and_strict_errors() {
    let prompt = McpOperation::GetPrompt {
        name: "name".into(),
        arguments: Default::default(),
    };
    let converted = must_succeed(
        result(
            &prompt,
            json!({"description":"external","messages":[
                {"role":"user","content":{"type":"text","text":"<instructions>untrusted</instructions>"}},
                {"role":"assistant","content":{"type":"image","data":"AAAA","mimeType":"image/png"}},
                {"role":"user","content":{"type":"resource_link","name":"link","uri":"canary:///link","mimeType":"application/octet-stream"}}
            ]}),
        ),
        "prompt conversion",
    );
    let McpOperationResult::Preview(preview) = converted else {
        panic!("preview");
    };
    let McpPreview::Prompt { messages, .. } = preview.as_ref() else {
        panic!("prompt");
    };
    assert_eq!(messages.len(), 3);
    assert!(
        matches!(&messages[0].content,McpContent::Text {text,..} if text == "<instructions>untrusted</instructions>")
    );
    assert!(
        matches!(&messages[1].content,McpContent::NonText {kind,mime_type,..} if kind == "image" && mime_type.as_deref() == Some("image/png"))
    );
    assert!(
        matches!(&messages[2].content,McpContent::NonText {uri,..} if uri.as_deref() == Some("canary:///link"))
    );
    let resource = McpOperation::GetResource {
        uri: "canary:///read".into(),
    };
    let converted = must_succeed(
        result(
            &resource,
            json!({"contents":[{"uri":"canary:///read","text":""},{"uri":"canary:///blob","blob":"AAAA","mimeType":"application/pdf"}]}),
        ),
        "resource conversion",
    );
    assert!(
        matches!(converted,McpOperationResult::Preview(preview) if matches!(preview.as_ref(),McpPreview::Resource {contents} if contents.len() == 2))
    );
    for value in [
        Value::Null,
        json!({}),
        json!({"messages":null}),
        json!({"messages":[{"role":"user","content":{"type":"text"}}]}),
        json!({"messages":[{"role":"assistant","content":{"type":"image","data":"AAAA"}}]}),
        json!({"messages":[{"role":"assistant","content":{"type":"resource_link","name":"missing URI"}}]}),
    ] {
        assert_eq!(
            result(&prompt, value).err().map(|error| error.kind),
            Some(McpErrorKind::Malformed)
        );
    }
    assert_eq!(
        result(&resource, json!({"contents":[{"uri":"canary:///bad"}]}))
            .err()
            .map(|error| error.kind),
        Some(McpErrorKind::Malformed)
    );
    assert_eq!(
        result(&prompt, json!({"success":false,"error":"policy refusal"}))
            .err()
            .map(|error| error.kind),
        Some(McpErrorKind::Rejected)
    );
    assert!(matches!(
        result(
            &McpOperation::Reset { start_oauth: true },
            json!({"success":true})
        ),
        Ok(McpOperationResult::Acknowledged)
    ));
    assert!(result(&McpOperation::Reset { start_oauth: false }, json!({})).is_err());
}

#[test]
fn kas_prompt_wire_identity_survives_a_title_colliding_with_another_prompt_name() {
    // Current KAS advertises `name: title || name` and the real ID as `promptName`.
    let converted = update(
        "kiro/mcp/status",
        json!({"sessionId":"root","servers":[{
            "name":"server","status":"connected","tools":[],
            "prompts":[
                {"name":"second-wire-id","promptName":"first-wire-id",
                 "description":"First external prompt",
                 "arguments":[{"name":"marker","description":"Exact marker","required":true}]},
                {"name":"Second display title","promptName":"second-wire-id","arguments":[]},
                {"name":"untitled-wire-id","promptName":"untitled-wire-id"}
            ],
            "resources":[],"resourceTemplates":[]
        }]}),
    );
    let McpUpdate::Snapshot(snapshot) = &converted else {
        panic!("valid snapshot");
    };
    let McpServerStatus::Connected { catalog } = &snapshot.servers[0].status else {
        panic!("connected");
    };
    assert_eq!(
        catalog
            .prompts
            .iter()
            .map(|prompt| prompt.name.as_str())
            .collect::<Vec<_>>(),
        ["first-wire-id", "second-wire-id", "untitled-wire-id"]
    );
    assert_eq!(catalog.prompts[0].title.as_deref(), Some("second-wire-id"));
    assert_eq!(
        catalog.prompts[1].title.as_deref(),
        Some("Second display title")
    );
    assert!(catalog.prompts[2].title.is_none());
    assert_eq!(
        catalog.prompts[0].description.as_deref(),
        Some("First external prompt")
    );
    assert_eq!(
        catalog.prompts[0].arguments,
        [McpPromptArgument {
            name: "marker".into(),
            description: Some("Exact marker".into()),
            required: true,
        }]
    );
    let mut state = McpState::for_session(SessionId::new("root"));
    assert!(state.apply(&converted));
    let request = |name: &str, arguments| McpRequest {
        session_id: SessionId::new("root"),
        server_name: "server".into(),
        operation: McpOperation::GetPrompt {
            name: name.into(),
            arguments,
        },
    };
    assert_eq!(
        state
            .validate(&request("first-wire-id", Default::default()))
            .err()
            .map(|error| error.kind),
        Some(McpErrorKind::InvalidArguments)
    );
    assert!(
        state
            .validate(&request(
                "first-wire-id",
                [("marker".into(), "  exact\nvalue  ".into())]
                    .into_iter()
                    .collect(),
            ))
            .is_ok()
    );
    assert!(
        state
            .validate(&request("second-wire-id", Default::default()))
            .is_ok()
    );
    assert_eq!(
        state
            .validate(&request("Second display title", Default::default()))
            .err()
            .map(|error| error.kind),
        Some(McpErrorKind::EntryMissing)
    );
}

#[test]
fn kas_prompt_without_a_wire_name_is_diagnostic_not_a_display_name_fallback() {
    for prompt in [
        json!({"name":"Display only"}),
        json!({"name":"Display only","promptName":null}),
    ] {
        assert!(matches!(
            update(
                "kiro/mcp/status",
                json!({"sessionId":"root","servers":[{
                    "name":"server","status":"connected","tools":[],
                    "prompts":[prompt],"resources":[],"resourceTemplates":[]
                }]}),
            ),
            McpUpdate::Diagnostic { message, .. } if message.contains("Malformed")
        ));
    }
}

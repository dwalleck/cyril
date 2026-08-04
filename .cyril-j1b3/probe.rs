use agent_client_protocol as acp;
use serde_json::Value;
use std::collections::HashMap;

use super::to_tool_call_from_permission;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/conductor-spike/kas-modes-dumps/bug-fix.json"
    );
    let bytes = std::fs::read(path).expect("KAS modes fixture must be readable");
    serde_json::from_slice(&bytes).expect("KAS modes fixture must be JSON")
}

#[test]
fn probe_kas_write_permission_join() {
    let data = fixture();
    let frames = data["frames"]
        .as_array()
        .expect("fixture must contain frames");
    let mut cached_inputs = HashMap::new();
    let mut permission_params = Vec::new();

    for frame in frames {
        let method = frame["method"].as_str().unwrap_or_default();
        let params = &frame["params"];
        match method {
            "session/update" => {
                let update = &params["update"];
                let kind = update["sessionUpdate"].as_str().unwrap_or_default();
                if matches!(kind, "tool_call" | "tool_call_update")
                    && let Some(raw_input) = update.get("rawInput")
                {
                    let id = update["toolCallId"].as_str().expect("tool call id");
                    cached_inputs.insert(id.to_owned(), raw_input.clone());
                }
            }
            "session/request_permission" if params["toolCall"]["title"] == "Write File" => {
                permission_params.push(params.clone());
            }
            _ => {}
        }
    }
    assert_eq!(permission_params.len(), 4, "fixture's four Write File requests");
    let mut output = Vec::new();
    for params in permission_params {
        let req: acp::RequestPermissionRequest =
            serde_json::from_value(params).expect("permission frame must parse");
        let tool_call = to_tool_call_from_permission(&req, &cached_inputs);
        let input = tool_call.raw_input().expect("joined raw_input");
        let path = input["path"].as_str().expect("joined path");
        let text = input["text"].as_str().expect("joined text");
        assert!(!path.is_empty() && !text.is_empty());
        output.push(format!(
            "id={} path={} text_bytes={}",
            tool_call.id(),
            path,
            text.len()
        ));
    }
    let output_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.cyril-j1b3/probe-output.txt"
    );
    std::fs::write(output_path, output.join("\n") + "\n").expect("probe output must be writable");
}

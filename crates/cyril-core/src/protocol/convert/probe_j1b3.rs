#![allow(clippy::expect_used)]

use agent_client_protocol as acp;
use serde_json::Value;

use crate::protocol::tool_call_ledger::ToolCallLedger;
use crate::types::{SessionId, ToolCallId};

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
    let ledger = ToolCallLedger::new();
    let mut permission_params = Vec::new();

    for frame in frames {
        let method = frame["method"].as_str().unwrap_or_default();
        let params = &frame["params"];
        match method {
            "session/update" => {
                let kind = params["update"]["sessionUpdate"]
                    .as_str()
                    .unwrap_or_default();
                if !matches!(kind, "tool_call" | "tool_call_update") {
                    continue;
                }
                let sn: acp::SessionNotification = serde_json::from_value(params.clone())
                    .expect("tool call frame must parse as acp notification");
                let session = SessionId::new(sn.session_id.to_string());
                let (kind_present, status_present) = match &sn.update {
                    acp::SessionUpdate::ToolCall(_) => (true, true),
                    acp::SessionUpdate::ToolCallUpdate(update) => {
                        (update.fields.kind.is_some(), update.fields.status.is_some())
                    }
                    _ => (false, false),
                };
                match super::session_update_to_notification(&sn) {
                    Some(crate::types::Notification::ToolCallStarted(tc))
                    | Some(crate::types::Notification::ToolCallUpdated(tc)) => {
                        ledger.merge(session, &tc, kind_present, status_present);
                    }
                    other => panic!("tool call frame must convert, got {other:?}"),
                }
            }
            "session/request_permission" if params["toolCall"]["title"] == "Write File" => {
                permission_params.push(params.clone());
            }
            _ => {}
        }
    }

    assert_eq!(
        permission_params.len(),
        4,
        "fixture's four Write File requests"
    );
    let mut output = Vec::new();
    for params in permission_params {
        let req: acp::RequestPermissionRequest =
            serde_json::from_value(params).expect("permission frame must parse");
        let session = SessionId::new(req.session_id.to_string());
        let tool_call_id = ToolCallId::new(req.tool_call.tool_call_id.to_string());
        let snapshot = ledger
            .snapshot(&session, &tool_call_id)
            .expect("ledger must hold the joined snapshot");
        let input = snapshot.raw_input().expect("joined raw_input");
        let path = input["path"].as_str().expect("joined path");
        let text = input["text"].as_str().expect("joined text");
        assert!(!path.is_empty() && !text.is_empty());
        output.push(format!(
            "id={} path={} text_bytes={}",
            snapshot.id(),
            path,
            text.len()
        ));
    }
    let output_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.cyril-j1b3/probe-output.txt"
    );
    let recorded = std::fs::read_to_string(output_path)
        .expect("committed probe recording must exist (run oracle.py to re-derive)");
    // Git may hand the recording back with CRLF on Windows runners; the
    // content comparison is on rows, not bytes.
    let recorded = recorded.replace("\r\n", "\n");
    assert_eq!(
        output.join("\n") + "\n",
        recorded,
        "joined rows must match the committed recording (oracle: .cyril-j1b3/oracle.py)"
    );
}

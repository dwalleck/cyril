//! Probe for cyril-qo13 (prove-it-prototype artifact, not a regression test).
//!
//! Replays every `session/request_permission` request from the committed KAS
//! 2.11.0 live trace through cyril's REAL pipeline:
//!   raw wire JSON -> serde parse into `acp::RequestPermissionRequest`
//!   -> `to_permission_options` (what the approval overlay displays)
//!   -> for each selectable index k: `PermissionResponse::from(options[k].kind)`
//!      (exactly what `UiState::approval_confirm` does, cyril-ui state.rs:1356)
//!   -> `from_permission_response` -> the wire optionId cyril would send.
//!
//! Independent oracle: `.cyril-qo13/oracle.py` (raw-text extraction of the
//! reference client's actual replies from the same trace — no cyril code).
//!
//! Run: cargo test -p cyril-core probe_qo13 -- --nocapture

use agent_client_protocol as acp;

use super::{from_permission_response, to_permission_options};
use crate::types::PermissionResponse;

#[test]
fn probe_qo13_replay_trace_permissions() {
    let trace_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl"
    );
    let trace = std::fs::read_to_string(trace_path)
        .unwrap_or_else(|e| panic!("trace file must exist at {trace_path}: {e}"));

    for line in trace.lines() {
        let rec: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("trace line is JSON: {e}"));
        let msg = &rec["msg"];
        if msg["method"] != "session/request_permission" {
            continue;
        }
        let req_id = &msg["id"];
        // Real parse path: the same deserialization the acp crate performs
        // when the bridge receives this request off the wire.
        let req: acp::RequestPermissionRequest = serde_json::from_value(msg["params"].clone())
            .unwrap_or_else(|e| {
                panic!("KAS params must parse as acp::RequestPermissionRequest: {e}")
            });
        let options = to_permission_options(&req);

        println!("request id={req_id} ({} options)", options.len());
        for (k, opt) in options.iter().enumerate() {
            // approval_confirm: selected index -> kind -> PermissionResponse.
            let response = PermissionResponse::from(opt.kind);
            let wire = from_permission_response(response, &req);
            let wire_json =
                serde_json::to_value(&wire).unwrap_or_else(|e| panic!("response serializes: {e}"));
            let sent = wire_json["outcome"]["optionId"]
                .as_str()
                .unwrap_or("<none>")
                .to_string();
            let verdict = if sent == opt.id { "OK" } else { "WRONG" };
            println!(
                "  pick k={k} picked_id={} cyril_sends={sent} meta={} {verdict}",
                opt.id, wire_json["outcome"]["_meta"],
            );
        }
    }
}

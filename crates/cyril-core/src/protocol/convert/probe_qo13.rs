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

/// Design-time falsifier for cyril-qo13 claim C2 (the cheapest in the
/// falsification table of `.cyril-qo13/design.md`).
///
/// An acp `SelectedPermissionOutcome` built from the NON-FIRST option id of
/// trace request 3 must serialize JSON-equal to the reference client's actual
/// reply bytes. Falsified if the acp crate injects extra fields (e.g. a null
/// `_meta`), renames `optionId`, or restricts what ids can be constructed.
#[test]
fn probe_qo13_reply_shape_matches_reference_bytes() {
    let trace_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl"
    );
    let trace = std::fs::read_to_string(trace_path)
        .unwrap_or_else(|e| panic!("trace file must exist at {trace_path}: {e}"));

    // Oracle: the reference client's reply to request id=3 (picked k=1).
    let mut reference_reply = None;
    for line in trace.lines() {
        let rec: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("trace line is JSON: {e}"));
        if rec["dir"] == "out" && rec["msg"]["id"] == 3 && rec["msg"]["result"].is_object() {
            reference_reply = Some(rec["msg"]["result"].clone());
        }
    }
    let reference_reply =
        reference_reply.unwrap_or_else(|| panic!("trace must contain the reply to id=3"));

    // The proposed design's output for pick k=1 on request 3: the exact id,
    // no kind lookup, no metadata.
    let ours = acp::RequestPermissionResponse::new(acp::RequestPermissionOutcome::Selected(
        acp::SelectedPermissionOutcome::new(acp::PermissionOptionId::new(
            "toolu_bdrk_01MYUUB44DAAYDwVc8kBxmvk-option-1",
        )),
    ));
    let ours_json =
        serde_json::to_value(&ours).unwrap_or_else(|e| panic!("response serializes: {e}"));

    assert_eq!(
        ours_json, reference_reply,
        "C2 falsified: cyril's reply encoding differs from the reference client's bytes"
    );
}

/// Design-time reachability probe for cyril-qo13 claim C7, now a sentinel:
/// acp 0.10.2's `PermissionOptionKind` has no `#[serde(other)]` catch-all, so
/// a request with an unknown option kind fails deserialization upstream of
/// cyril's code — the unknown-kind input shape is unreachable today.
///
/// If this test ever starts seeing `Ok`, an acp upgrade made the shape
/// reachable — revisit cyril-p7kp (release-audit watch item) and the dead
/// `_ =>` arm in `to_permission_options`.
#[test]
fn probe_qo13_unknown_option_kind_parse() {
    let params = serde_json::json!({
        "sessionId": "sess_x",
        "toolCall": { "toolCallId": "tc_x", "title": "Probe" },
        "options": [
            { "optionId": "opt-known", "name": "Known", "kind": "allow_once" },
            { "optionId": "opt-mystery", "name": "Mystery", "kind": "definitely_not_a_kind" }
        ]
    });
    let parsed = serde_json::from_value::<acp::RequestPermissionRequest>(params);
    assert!(
        parsed.is_err(),
        "unknown option kinds became parseable — the unknown-kind input shape \
         is now production-reachable; see cyril-p7kp: {parsed:?}"
    );
}

//! Probe artifacts and regression fences for cyril-qo13.
//!
//! Started as the prove-it-prototype probe (which demonstrated the kind-keyed
//! response collapsing every user_input pick to option-0); now the asserting
//! regression fence for design claims C1/C3: replaying every
//! `session/request_permission` from BOTH committed 2.11.0 live traces (KAS +
//! v2) plus a synthetic single-option request through cyril's REAL pipeline:
//!   raw wire JSON -> serde parse into `acp::RequestPermissionRequest`
//!   -> `to_permission_options` (what the approval overlay displays)
//!   -> for each selectable index k: `PermissionResponse::Selected` with
//!      `options[k].id` -> `from_permission_response` -> wire optionId,
//! asserting the wire optionId equals the picked option's id exactly.
//!
//! Independent oracle: `.cyril-qo13/oracle.py` (raw-text extraction of the
//! reference client's actual replies from the same traces — no cyril code);
//! the pre-fix recorded behavior lives in `.cyril-qo13/probe-output.txt`.
//!
//! Run: cargo test -p cyril-core probe_qo13

use agent_client_protocol as acp;

use super::{from_permission_response, to_permission_options};
use crate::types::PermissionResponse;

/// Replay every permission request from a raw trace (or synthetic params)
/// and assert the exact-choice property for every selectable index.
fn assert_exact_choice_for_request(req_label: &str, params: serde_json::Value) {
    // Real parse path: the same deserialization the acp crate performs when
    // the bridge receives this request off the wire.
    let req: acp::RequestPermissionRequest = serde_json::from_value(params)
        .unwrap_or_else(|e| panic!("{req_label}: params must parse as acp request: {e}"));
    let options = to_permission_options(&req);
    // user_input requests surface all options under one kind; tool approvals
    // use distinct kinds. The label localizes failures to claim C1 vs C3.
    let all_same_kind = options.windows(2).all(|w| w[0].kind == w[1].kind);
    let class = if options.len() > 1 && all_same_kind {
        "user_input/C1"
    } else {
        "tool_approval/C3"
    };

    for (k, opt) in options.iter().enumerate() {
        let wire = from_permission_response(
            PermissionResponse::Selected {
                option_id: opt.id.clone(),
                trust_option: None,
            },
            &req,
        );
        let wire_json =
            serde_json::to_value(&wire).unwrap_or_else(|e| panic!("response serializes: {e}"));
        let sent = wire_json["outcome"]["optionId"]
            .as_str()
            .unwrap_or("<none>");
        assert_eq!(
            sent,
            opt.id.as_str(),
            "[{class}] {req_label}: pick k={k} must reply with the picked option's id"
        );
        assert!(
            wire_json["outcome"].get("_meta").is_none(),
            "[{class}] {req_label}: pick k={k} must not carry _meta"
        );
    }
}

/// C1/C3 regression fence: exact-choice replies for every request in both
/// committed live traces plus a synthetic single-option request (input shape
/// S4, absent from the traces). Ids in all fixtures are non-numeric, so an
/// index-as-id bug cannot accidentally pass.
#[test]
fn probe_qo13_replay_trace_permissions() {
    for trace_file in [
        "kas-live-session-trace-2.11.0.jsonl",
        "v2-live-session-trace-2.11.0.jsonl",
    ] {
        let trace_path = format!(
            "{}/../../experiments/conductor-spike/{trace_file}",
            env!("CARGO_MANIFEST_DIR")
        );
        let trace = std::fs::read_to_string(&trace_path)
            .unwrap_or_else(|e| panic!("trace file must exist at {trace_path}: {e}"));

        let mut seen = 0usize;
        for line in trace.lines() {
            let rec: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("trace line is JSON: {e}"));
            let msg = &rec["msg"];
            if msg["method"] != "session/request_permission" {
                continue;
            }
            seen += 1;
            let label = format!("{trace_file} id={}", msg["id"]);
            assert_exact_choice_for_request(&label, msg["params"].clone());
        }
        assert!(
            seen > 0,
            "{trace_file}: expected at least one permission request — trace moved or emptied?"
        );
    }

    // Synthetic single-option request (shape S4).
    assert_exact_choice_for_request(
        "synthetic single-option",
        serde_json::json!({
            "sessionId": "sess_s4",
            "toolCall": { "toolCallId": "tc_s4", "title": "Single choice" },
            "options": [
                { "optionId": "only-choice", "name": "Only", "kind": "allow_once" }
            ]
        }),
    );
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

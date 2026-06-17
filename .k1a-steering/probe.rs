// PROBE (prove-it-prototype, K1a steering). Throwaway. Inserted into the
// `tests` mod of crates/cyril-core/src/protocol/convert/kiro.rs, run with:
//   cargo test -p cyril-core probe_steering -- --nocapture
// Then removed. Uses the REAL to_ext_notification; builds nothing from the
// feature-to-come (the new converter arm does not exist yet).
//
// Frames are copied verbatim from the captured wire log
//   experiments/conductor-spike/logs/probe-steer-goal-2.7.0.log
// lines 25 (queued), 37 (consumed), 120 (cleared) — the ORACLE.
#[test]
fn probe_steering_current_behavior() {
    let method = "_kiro.dev/session/update";
    let frames = [
        (
            "queued",
            json!({"sessionId":"2dc3c608-73e7-464c-9665-e7f5cf9af74b",
            "update":{"sessionUpdate":"steering_queued","message":"STEERING UPDATE: stop now."}}),
        ),
        (
            "consumed",
            json!({"sessionId":"2dc3c608-73e7-464c-9665-e7f5cf9af74b",
            "update":{"sessionUpdate":"steering_consumed","content":"STEERING UPDATE: stop now."}}),
        ),
        (
            "cleared",
            json!({"sessionId":"2dc3c608-73e7-464c-9665-e7f5cf9af74b",
            "update":{"sessionUpdate":"steering_cleared"}}),
        ),
    ];

    for (name, params) in &frames {
        // (1) Premise: what does TODAY's converter do with this method+frame?
        let result = to_ext_notification(method, params);
        eprintln!("[probe] {name}: to_ext_notification(_kiro.dev/session/update) = {result:?}");
        // Spec claims Ok(None) (silent drop at the outer `other =>` arm).
        // Issue claimed Err. Settle it:
        assert!(
            matches!(result, Ok(None)),
            "{name}: expected Ok(None), got {result:?}"
        );

        // (2) Data availability: the inputs the NEW arm will consume are present
        // and well-typed in the captured frame (no arm built — just field reads).
        let u = params.get("update").unwrap();
        let variant = u.get("sessionUpdate").and_then(|v| v.as_str()).unwrap();
        let payload = match variant {
            "steering_queued" => u.get("message").and_then(|v| v.as_str()),
            "steering_consumed" => u.get("content").and_then(|v| v.as_str()),
            "steering_cleared" => None, // payload-free by design
            other => panic!("unexpected variant {other}"),
        };
        let sid = params.get("sessionId").and_then(|v| v.as_str());
        eprintln!("[probe] {name}: variant={variant} payload={payload:?} sessionId={sid:?}");
    }
}

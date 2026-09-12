//! cyril-v19o C5/C7 transport fence: the KAS powers push survives the real
//! SDK2 transport, and cyril answers it with nothing.
//!
//! The converter, the engine arm and the App arms each have unit fences. What
//! none of them can prove is that `_kiro/powers/items_changed` reaches the
//! converter at all: the wire frame is an untyped extension notification that
//! has to survive ACP normalization, the SDK's `session/update` handler
//! ordering, and the mediator's ext dispatch before `KasEngine` ever sees it.
//! A frame swallowed upstream would leave every unit fence green.
//!
//! The negative half — no powers *request* is ever sent — is judged at the
//! transport boundary, where a request would have to appear as an
//! `ext:<method>` ledger entry. The `new_session` entry in the same run is the
//! positive control that makes that ledger's silence meaningful.

use super::*;

/// The push arrives unprompted, unasked for, and leaves as a typed notification.
#[cfg(feature = "kas")]
#[tokio::test]
async fn powers_push_survives_the_transport_and_draws_no_request() {
    let script = Rc::new(RefCell::new(Script {
        emit_powers_changed: true,
        wire_kas: Some(true),
        ..Script::default()
    }));
    let ledger = Rc::clone(&script);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            // Drive `NewSession` by hand rather than through `start_session`:
            // the push races the bridge's own local `UsageSessionStarted` (the
            // capture measures the push 18 ms AFTER the response, but that gap
            // is wall-clock, not a wire guarantee). What this fence owes the
            // claim is that the frame arrives with no prompt ever sent and
            // survives normalization — not which side of a local notification
            // it lands on, which nothing in the evidence pins.
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("new session command");
            let mut pushed: Option<Notification> = None;
            let mut session_id: Option<crate::types::SessionId> = None;
            let mut saw_usage = false;
            // Read until BOTH the push and `SessionCreated` have arrived, not
            // until `SessionCreated` has: the push measures 18 ms after the
            // `session/new` reply on this machine, but that gap is wall-clock,
            // not a wire guarantee, and what this fence owes the claim is that
            // the frame survives the transport — not which side of a local
            // notification it lands on. Breaking on `SessionCreated` made the
            // verdict order-dependent, so the harness's own timing decided the
            // result (review finding 18).
            for _ in 0..5 {
                if pushed.is_some() && session_id.is_some() {
                    break;
                }
                match next_notification("powers", &mut rx).await {
                    Notification::PowersChanged { powers } => {
                        pushed = Some(Notification::PowersChanged { powers });
                    }
                    usage @ Notification::UsageSessionStarted { .. } => {
                        saw_usage = true;
                        let _ = usage;
                    }
                    Notification::SessionCreated {
                        session_id: created,
                        ..
                    } => {
                        session_id = Some(created);
                    }
                    other => panic!("unexpected frame during session start: {other:?}"),
                }
            }
            assert!(
                saw_usage,
                "the bridge's own usage-session notification must still be emitted"
            );
            assert!(session_id.is_some(), "SessionCreated must arrive");
            let pushed = pushed.expect_contract("the powers push must arrive");

            match pushed {
                Notification::PowersChanged { powers } => {
                    let names: Vec<&str> = powers.iter().map(|p| p.name()).collect();
                    assert_eq!(
                        names,
                        ["aws-infrastructure-as-code", "datadog", "markdownlint"],
                        "the live capture's three powers must arrive in wire order"
                    );
                    // Fields the panel renders, checked once at this layer so a
                    // converter that stopped decoding them is caught here too.
                    let datadog = powers
                        .iter()
                        .find(|p| p.name() == "datadog")
                        .expect_contract("datadog is in the capture");
                    assert_eq!(datadog.title(), "Datadog Observability");
                    assert!(datadog.has_steering_files());
                    assert_eq!(datadog.mcp_server_names(), ["datadog"]);
                }
                other => panic!("expected the powers push, got {other:?}"),
            }

            // Positive control: the ledger records cyril→agent traffic in this
            // run, so its silence about powers below is about powers.
            let observed = ledger.borrow();
            let entries = observed.received().clone();
            assert!(
                entries.iter().any(|entry| entry == "new_session"),
                "the ledger must record this run's session creation, got {entries:?}"
            );

            // The negative: nothing cyril sent names a powers request — not the
            // declared-but-unimplemented `refresh`, not the unadvertised `list`.
            let offside: Vec<&String> = entries
                .iter()
                .filter(|entry| {
                    let lower = entry.to_ascii_lowercase();
                    lower.contains("powers") || lower.contains("items_changed")
                })
                .collect();
            assert!(
                offside.is_empty(),
                "cyril must never request powers state; it only consumes the push. \
                 Offending ledger entries: {offside:?}"
            );
        },
    )
    .await;
}

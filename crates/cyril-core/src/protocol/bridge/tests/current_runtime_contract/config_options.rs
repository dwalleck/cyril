//! cyril-vh69 C7 (wire half): `BridgeCommand::SetConfigOption` for a KAS
//! model select through the real domain mediator and SDK runtime against
//! the in-process fake agent — exactly one `session/set_config_option`
//! frame carrying the picker's `{configId, value}`, the scripted
//! `session/new` `configOptions` landing as the snapshot's
//! `ConfigOptionsUpdated` (with `_meta.kiro` effort metadata parsed), and
//! the rebuilt result catalog mapping to `ConfigOptionSet`. The confirm-
//! side chain (revalidation, effort picker, Esc) is App-level and covered
//! in `cyril/src/app.rs` tests; v2 behavior is frozen by
//! `tests/session_configuration.rs`.

use super::*;

/// The model select as raw ACP JSON: `auto` with no effort block, sonnet
/// with the frame-24 `_meta.kiro` effort shape.
fn model_catalog(current: &str) -> serde_json::Value {
    serde_json::json!([{
        "id": "model", "name": "Model", "category": "model", "type": "select",
        "currentValue": current,
        "options": [
            {"value": "auto", "name": "Auto"},
            {"value": "claude-sonnet-5.5", "name": "Claude Sonnet 5.5",
             "_meta": {"kiro": {
                "hasEffort": true,
                "effortLevels": ["low", "medium", "high", "xhigh", "max"],
                "defaultEffortLevel": "high",
             }}},
        ],
    }])
}

#[tokio::test]
async fn set_config_option_model_wire_snapshot_and_ack() {
    let script = Rc::new(RefCell::new(Script {
        new_session_config_options: Some(model_catalog("auto")),
        ..Script::default()
    }));
    script
        .borrow()
        .ext_responses
        .lock()
        .expect_contract("config responses")
        .push((
            "session/set_config_option".to_owned(),
            serde_json::json!({ "configOptions": model_catalog("claude-sonnet-5.5") }),
        ));
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permissions, _gate, _loop| async move {
            start_session(&sender, &mut rx).await;
            // The scripted session/new configOptions land right after
            // creation — the snapshot's source of truth.
            let catalog = next_notification("session catalog", &mut rx).await;
            let Notification::ConfigOptionsUpdated(options) = &catalog else {
                panic!("expected the scripted catalog, got {catalog:?}");
            };
            let model = options
                .iter()
                .find(|o| o.key == "model")
                .expect_contract("model select in the catalog");
            assert_eq!(model.value.as_deref(), Some("auto"));
            let sonnet = model
                .options
                .iter()
                .find(|o| o.value == "claude-sonnet-5.5")
                .expect_contract("sonnet is offered");
            // The `_meta.kiro` effort parse is KAS-feature-gated
            // (convert/kas.rs): under `--features kas` the metadata rides
            // the snapshot; a default build converts every value
            // effort-free by design.
            #[cfg(feature = "kas")]
            {
                let effort = sonnet
                    .effort
                    .as_ref()
                    .expect_contract("effort metadata rides the snapshot");
                assert_eq!(effort.levels().len(), 5);
                assert_eq!(effort.default_level(), Some("high"));
            }
            #[cfg(not(feature = "kas"))]
            assert!(
                sonnet.effort.is_none(),
                "default builds convert effort-free"
            );

            sender
                .send(BridgeCommand::SetConfigOption {
                    config_id: "model".into(),
                    value: "claude-sonnet-5.5".into(),
                })
                .await
                .expect_contract("send SetConfigOption");
            let ack = next_notification("model set ack", &mut rx).await;
            let Notification::ConfigOptionSet { config_id, options } = &ack else {
                panic!("expected ConfigOptionSet, got {ack:?}");
            };
            assert_eq!(config_id, "model", "the ack names the model select");
            let model = options
                .iter()
                .find(|o| o.key == "model")
                .expect_contract("rebuilt model select");
            assert_eq!(
                model.value.as_deref(),
                Some("claude-sonnet-5.5"),
                "the RETURNED currentValue is authoritative"
            );
        },
    )
    .await;

    let calls = observed.borrow().ext_calls().clone();
    assert_eq!(
        calls,
        [(
            "session/set_config_option".to_owned(),
            serde_json::json!({
                "sessionId": "fake-0",
                "configId": "model",
                "value": "claude-sonnet-5.5",
            }),
        )],
        "exactly one set_config_option frame with the picker's configId/value"
    );
}

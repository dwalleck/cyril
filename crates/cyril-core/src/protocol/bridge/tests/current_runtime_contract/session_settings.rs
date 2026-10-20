//! cyril-sm36: KAS reads its session-scoped settings (`workflows`,
//! `validation`, `fta` — settings-table `messages: ["session/new",
//! "session/load"]` in the 0.66.43 bundle) from `session/new`'s
//! `_meta.kiro.settings`, not from `initialize`. Kiro's TUI sends the same
//! settings object in both places; these fences hold cyril to that on the
//! frames the agent actually receives, and keep v2's `session/new` bare.

use super::*;

/// Ask the bridge for one session and wait until the fake agent has seen
/// its `session/new` (the notification order around it is not under test).
async fn request_session(sender: &BridgeSender, probe: &Rc<RefCell<Script>>) {
    sender
        .send(BridgeCommand::NewSession {
            cwd: std::env::temp_dir(),
        })
        .await
        .expect_contract("new session command");
    assert!(
        wait_for_received(probe, "new_session", 5).await,
        "the agent never received session/new"
    );
}

/// The value at `meta_path` in a received params object, if present.
#[cfg(feature = "kas")]
fn kiro_settings<'a>(
    params: &'a serde_json::Value,
    meta_path: &[&str],
) -> Option<&'a serde_json::Value> {
    meta_path
        .iter()
        .try_fold(params, |value, key| value.get(*key))
}

/// A KAS engine whose settings come from a `cli.json` planted under a fake
/// `KIRO_HOME` (the launch-environment path production uses; no real home is
/// read).
#[cfg(feature = "kas")]
fn kas_engine_from_cli_json(
    cli_json: &str,
) -> (tempfile::TempDir, crate::protocol::engine::KasEngine) {
    let root = tempfile::tempdir().expect_contract("temp KIRO_HOME");
    let settings_dir = root.path().join("settings");
    std::fs::create_dir_all(&settings_dir).expect_contract("settings dir");
    std::fs::write(settings_dir.join("cli.json"), cli_json).expect_contract("cli.json");
    let environment =
        crate::types::SpawnEnvironment::Replace(std::collections::BTreeMap::from([(
            "KIRO_HOME".into(),
            root.path().as_os_str().to_owned(),
        )]));
    let engine = crate::protocol::engine::KasEngine {
        settings: crate::protocol::kas::settings::settings_extra_value(&environment),
        ..Default::default()
    };
    (root, engine)
}

/// The user's `chat.enableWorkflows: true` reaches KAS on `session/new` —
/// the message its `workflows` setting is read from.
#[cfg(feature = "kas")]
#[tokio::test]
async fn kas_session_new_carries_the_workflow_gate() {
    let (_home, engine) = kas_engine_from_cli_json(r#"{"chat.enableWorkflows":true}"#);
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(engine),
        script,
        |sender, _rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            request_session(&sender, &probe).await;
            let sent = probe.borrow().new_session_params();
            let params = sent
                .first()
                .expect_contract("one session/new params object");
            let settings = kiro_settings(params, &["_meta", "kiro", "settings"])
                .expect_contract("session/new must carry _meta.kiro.settings on KAS");
            let on = serde_json::json!({ "enabled": true });
            assert_eq!(
                settings.get("workflows"),
                Some(&on),
                "workflows gate: {params}"
            );
            assert_eq!(settings.get("goal"), Some(&on), "goal gate: {params}");
        },
    )
    .await;
}

/// Capture shape: `session/new` carries exactly what `initialize` advertised
/// as settings, under `_meta.kiro` holding only `settings` — the shape Kiro's
/// 2.32.0 TUI puts on the wire (fixture extracted verbatim from
/// `kas-workflow-gateon-2.32.0.jsonl`).
#[cfg(feature = "kas")]
#[tokio::test]
async fn kas_session_new_settings_match_initialize_like_kiro() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/kas/session_new/initialize-and-session-new-gateon-2.32.0.json"
    ))
    .expect_contract("capture fixture");
    let tui_init = kiro_settings(
        &capture,
        &["initializeClientCapabilitiesMeta", "kiro", "settings"],
    )
    .expect_contract("capture initialize settings");
    let tui_new = kiro_settings(&capture, &["sessionNewMeta", "kiro", "settings"])
        .expect_contract("capture session/new settings");
    assert_eq!(
        tui_init, tui_new,
        "Kiro's TUI sends one settings object twice"
    );
    let tui_new_kiro_keys: Vec<&String> = capture["sessionNewMeta"]["kiro"]
        .as_object()
        .expect_contract("capture session/new _meta.kiro")
        .keys()
        .collect();

    let (_home, engine) = kas_engine_from_cli_json(r#"{"chat.enableWorkflows":true}"#);
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(engine),
        script,
        |sender, _rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            request_session(&sender, &probe).await;
            let initialize = probe
                .borrow()
                .initialize_params()
                .expect_contract("initialize params");
            let sent = probe.borrow().new_session_params();
            let params = sent
                .first()
                .expect_contract("one session/new params object");
            let advertised = kiro_settings(
                &initialize,
                &["clientCapabilities", "_meta", "kiro", "settings"],
            )
            .expect_contract("initialize settings");
            let session = kiro_settings(params, &["_meta", "kiro", "settings"])
                .expect_contract("session/new settings");
            assert_eq!(session, advertised, "same settings object as initialize");
            let kiro_keys: Vec<&String> = params["_meta"]["kiro"]
                .as_object()
                .expect_contract("session/new _meta.kiro")
                .keys()
                .collect();
            assert_eq!(kiro_keys, tui_new_kiro_keys, "session/new _meta.kiro shape");
        },
    )
    .await;
}

/// v2 advertises no settings at `initialize` and sends none on `session/new`.
#[tokio::test]
async fn v2_session_new_carries_no_meta() {
    let script = Rc::new(RefCell::new(Script::default()));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        |sender, _rx, _permission_rx, _gate, _loop_handle| async move {
            request_session(&sender, &probe).await;
            let sent = probe.borrow().new_session_params();
            let params = sent
                .first()
                .expect_contract("one session/new params object");
            assert_eq!(
                params.get("_meta"),
                None,
                "v2 session/new stays bare: {params}"
            );
        },
    )
    .await;
}

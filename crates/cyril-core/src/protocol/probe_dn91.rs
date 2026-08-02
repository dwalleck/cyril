//! prove-it probe (cyril-dn91): in a `--features kas` build with **V2Engine**
//! bound, which handled host callbacks answer vs refuse?
//!
//! CHARACTERIZATION of today's behavior — these assertions document the defect
//! (feature-gated, not engine-gated dispatch) and are inverted to refusals by
//! the cyril-dn91 build slices. Oracle: `.cyril-dn91/oracle.sh` (source-text
//! census of engine consults on dispatch paths — independent mechanism).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_client_protocol::{self as acp, Client as _};
use tokio::sync::mpsc;

use crate::protocol::client::{KiroClient, test_host_shell};
use crate::protocol::engine::V2Engine;
use crate::types::AgentEngine;

#[derive(Debug, PartialEq)]
enum Disposition {
    /// JSON-RPC method-not-found — the post-fix shape for un-adaptered families.
    Refused,
    /// `Ok(null)` — the protocol-default "unhandled" body (agent reads: success).
    NullDefault,
    /// A responder ran: non-null body, or a responder-specific error.
    Answered,
}

fn classify(r: &acp::Result<acp::ExtResponse>) -> Disposition {
    match r {
        Err(e) if e.code == acp::ErrorCode::MethodNotFound => Disposition::Refused,
        Err(_) => Disposition::Answered,
        Ok(resp) => match serde_json::from_str::<serde_json::Value>(resp.0.get()) {
            Ok(serde_json::Value::Null) => Disposition::NullDefault,
            _ => Disposition::Answered,
        },
    }
}

fn v2_client_in_kas_build(cwd: &std::path::Path) -> KiroClient {
    let (ntx, _nrx) = mpsc::channel(4);
    let (ptx, _prx) = mpsc::channel(1);
    // V2 binding mirrors bridge.rs::resolve_host_shell: V2 => no host shell.
    KiroClient::new(
        ntx,
        ptx,
        std::rc::Rc::new(V2Engine),
        test_host_shell(AgentEngine::V2),
        cwd,
    )
}

async fn ext(client: &KiroClient, method: &str, params: serde_json::Value) -> Disposition {
    let raw = serde_json::value::RawValue::from_string(params.to_string()).unwrap();
    let d = classify(
        &client
            .ext_method(acp::ExtRequest::new(method, raw.into()))
            .await,
    );
    eprintln!("probe_dn91: {method} => {d:?}");
    d
}

#[tokio::test]
async fn v2_bound_client_answers_auth_fs_hooks_but_advertises_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("p.txt");
    std::fs::write(&f, "dn91").unwrap();
    let client = v2_client_in_kas_build(dir.path());

    // Advertisement side: V2 advertises NO capabilities at all.
    let caps = crate::protocol::engine::client_capabilities(&V2Engine);
    assert!(!caps.fs.read_text_file && !caps.fs.write_text_file && !caps.terminal);
    assert!(caps.meta.is_none(), "V2 advertises no _meta.kiro");

    // Execution side, same binding. AUTH: the responder RUNS (real-store read →
    // token or store diagnostic; never method-not-found, never null).
    let auth = ext(
        &client,
        crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(auth, Disposition::Answered, "auth responder runs under V2");

    // TYPED FS: the override resolves and returns real file content.
    let read = client
        .read_text_file(acp::ReadTextFileRequest::new(acp::SessionId::new("s"), &f))
        .await;
    assert_eq!(read.expect("fs override answers under V2").content, "dn91");

    // _kiro/fs/* dialect: stat answers an object.
    let stat = ext(
        &client,
        crate::protocol::kas::kiro_fs::STAT_METHOD,
        serde_json::json!({"sessionId": "s", "path": f}),
    )
    .await;
    assert_eq!(
        stat,
        Disposition::Answered,
        "_kiro/fs/stat answers under V2"
    );

    // HOOKS: list serves (empty) registry; executeHook RUNS an arbitrary wire
    // command (exitCode 0 in the body proves execution, not just dispatch).
    let list = ext(
        &client,
        crate::protocol::kas::hooks::LIST_METHOD,
        serde_json::json!({"trigger": "promptSubmit"}),
    )
    .await;
    assert_eq!(list, Disposition::Answered, "hooks/list answers under V2");
    let raw = serde_json::value::RawValue::from_string(
        serde_json::json!({"hookId": "h", "hookName": "p", "command": "echo dn91-probe",
            "sessionId": "s", "userPrompt": ""})
        .to_string(),
    )
    .unwrap();
    let exec = client
        .ext_method(acp::ExtRequest::new(
            crate::protocol::kas::hooks::EXECUTE_METHOD,
            raw.into(),
        ))
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_str(exec.0.get()).unwrap();
    assert_eq!(
        body["exitCode"], 0,
        "executeHook RAN the command under V2: {body}"
    );

    // TERMINAL: the one family with an (indirect) engine gate today — V2 gets no
    // host shell, so the responder itself refuses. Distinct shape from -32601.
    let shell = ext(
        &client,
        "kiro/terminal/shell_type",
        serde_json::json!({"sessionId": "s"}),
    )
    .await;
    assert_eq!(
        shell,
        Disposition::Answered,
        "refusal comes from the responder (host-shell None), not method-not-found"
    );

    // An UNKNOWN method still falls to the protocol-default null (dcc6 F15).
    let unknown = ext(&client, "kiro/unknown/dn91", serde_json::json!({})).await;
    assert_eq!(unknown, Disposition::NullDefault);
}

/// Probe slice 3 — the cheapest falsifier for the falsifiable-design claim C7
/// (derived advertisement): today's four hand-written advertisement shapes
/// must be EXACTLY reconstructible from (host_io presence, hooks direction,
/// opaque settings extras). The expected value is a hand-assembled JSON
/// literal (independent of the constructors under test); the settings object
/// passes through opaquely since it is an engine extra, not adapter presence.
/// A mismatch means some advertisement byte is NOT determined by the adapter
/// set — and the derivation design would lose it.
#[test]
fn advertisement_is_fully_determined_by_presence_direction_extras() {
    use crate::types::kas_hooks::KasHooksMode;

    let caps_json = |mode: KasHooksMode| {
        serde_json::to_value(crate::protocol::engine::client_capabilities(
            &crate::protocol::engine::KasEngine { hooks_mode: mode },
        ))
        .unwrap()
    };

    // V2: presence all-absent, no extras → the empty capability object.
    let v2 = serde_json::to_value(crate::protocol::engine::client_capabilities(&V2Engine)).unwrap();
    assert_eq!(
        v2,
        serde_json::to_value(acp::ClientCapabilities::new()).unwrap(),
        "V2 must be byte-identical to the empty capability set"
    );

    for mode in [KasHooksMode::Off, KasHooksMode::Host, KasHooksMode::Kas] {
        let actual = caps_json(mode);
        // Opaque extra: the marshaled settings object (reads the user's
        // cli.json — environment-dependent, so it is spliced, not predicted).
        let settings = actual["_meta"]["kiro"]["settings"].clone();
        assert!(settings.is_object(), "settings extra present under KAS");

        let mut kiro = serde_json::Map::new();
        kiro.insert("settings".into(), settings);
        match mode {
            KasHooksMode::Off => {}
            KasHooksMode::Host => {
                kiro.insert("hooks".into(), serde_json::json!({"enabled": true}));
            }
            KasHooksMode::Kas => {
                kiro.insert(
                    "hooks".into(),
                    serde_json::json!({"enabled": true, "v2": true}),
                );
            }
        }
        let expected = serde_json::json!({
            "fs": {
                "readTextFile": true,
                "writeTextFile": true,
                "_meta": {"kiro": {
                    "readFile": true, "writeFile": true, "stat": true,
                    "readDirectory": true, "delete": true,
                }},
            },
            "terminal": true,
            "_meta": {"kiro": kiro},
        });
        assert_eq!(
            actual, expected,
            "KAS({mode:?}) advertisement must be exactly (host_io presence + hooks direction + settings extra)"
        );
    }
}

/// Probe slice 2 — the AC4 corner: `kas_hooks = "kas"` advertises hooks
/// {enabled, v2} as an OUTBOUND mode (agent executes hooks itself, ADR-0010),
/// and installs an empty registry inbound. Today the inbound surface still
/// SERVES: list answers `{hooks: []}` and executeHook runs an arbitrary wire
/// command — there is no direction gate, only the registry's emptiness.
#[tokio::test]
async fn kas_outbound_hooks_mode_still_serves_inbound_execution() {
    let dir = tempfile::tempdir().unwrap();
    let (ntx, _nrx) = mpsc::channel(4);
    let (ptx, _prx) = mpsc::channel(1);
    let client = KiroClient::new(
        ntx,
        ptx,
        std::rc::Rc::new(crate::protocol::engine::KasEngine {
            hooks_mode: crate::types::kas_hooks::KasHooksMode::Kas,
        }),
        test_host_shell(AgentEngine::Kas),
        dir.path(),
    );

    let list = ext(
        &client,
        crate::protocol::kas::hooks::LIST_METHOD,
        serde_json::json!({"trigger": "promptSubmit"}),
    )
    .await;
    assert_eq!(list, Disposition::Answered, "empty registry SERVES inbound");

    let raw = serde_json::value::RawValue::from_string(
        serde_json::json!({"hookId": "h", "hookName": "p", "command": "echo dn91-kas-mode",
            "sessionId": "s", "userPrompt": ""})
        .to_string(),
    )
    .unwrap();
    let exec = client
        .ext_method(acp::ExtRequest::new(
            crate::protocol::kas::hooks::EXECUTE_METHOD,
            raw.into(),
        ))
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_str(exec.0.get()).unwrap();
    assert_eq!(
        body["exitCode"], 0,
        "outbound-only hooks mode still EXECUTES inbound commands: {body}"
    );
}

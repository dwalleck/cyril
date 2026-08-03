//! prove-it probe → regression fences (cyril-dn91): in a `--features kas`
//! build, host-callback availability follows the BOUND ENGINE's adapter set.
//!
//! Began as characterization tests documenting the defect (feature-gated, not
//! engine-gated dispatch — a V2-bound client answered auth/fs/hooks and
//! executed wire commands); each build slice inverted its family's rows, and
//! the module now fences the refusal contract end-to-end. Original probe
//! evidence + oracle: `.cyril-dn91/` (findings.md, oracle.sh).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use agent_client_protocol::{self as acp, Client as _};
use tokio::sync::mpsc;

use crate::protocol::client::KiroClient;
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
        crate::protocol::client::test_host_tx(),
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
async fn v2_bound_client_refuses_every_unadapted_family() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("p.txt");
    std::fs::write(&f, "dn91").unwrap();
    let client = v2_client_in_kas_build(dir.path());

    // Advertisement side: V2 advertises NO capabilities at all.
    let caps = crate::protocol::engine::client_capabilities(&V2Engine);
    assert!(!caps.fs.read_text_file && !caps.fs.write_text_file && !caps.terminal);
    assert!(caps.meta.is_none(), "V2 advertises no _meta.kiro");

    // Execution side, same binding. AUTH: REFUSED since the dn91 auth gate —
    // V2 installs no auth adapter (was: Answered, the characterized defect).
    let auth = ext(
        &client,
        crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
        serde_json::json!({}),
    )
    .await;
    assert_eq!(auth, Disposition::Refused, "auth refused under V2 (C1)");

    // TYPED FS: REFUSED since the dn91 host-io gate (was: returned real file
    // content under V2, the characterized defect).
    let read = client
        .read_text_file(acp::ReadTextFileRequest::new(acp::SessionId::new("s"), &f))
        .await
        .expect_err("typed fs read refused under V2 (C2)");
    assert_eq!(read.code, acp::ErrorCode::MethodNotFound);

    // _kiro/fs/* dialect: REFUSED (was: stat answered an object).
    let stat = ext(
        &client,
        crate::protocol::kas::kiro_fs::STAT_METHOD,
        serde_json::json!({"sessionId": "s", "path": f}),
    )
    .await;
    assert_eq!(
        stat,
        Disposition::Refused,
        "_kiro/fs/stat refused under V2 (C3)"
    );

    // HOOKS: REFUSED since the dn91 hooks gate — V2 has no hooks capability
    // (was: list served the empty registry and executeHook RAN wire commands,
    // the characterized defect).
    let list = ext(
        &client,
        crate::protocol::kas::hooks::LIST_METHOD,
        serde_json::json!({"trigger": "promptSubmit"}),
    )
    .await;
    assert_eq!(
        list,
        Disposition::Refused,
        "hooks/list refused under V2 (C5)"
    );
    let exec = ext(
        &client,
        crate::protocol::kas::hooks::EXECUTE_METHOD,
        serde_json::json!({"hookId": "h", "hookName": "p", "command": "echo dn91-probe",
            "sessionId": "s", "userPrompt": ""}),
    )
    .await;
    assert_eq!(
        exec,
        Disposition::Refused,
        "executeHook refused under V2 (C5)"
    );

    // TERMINAL: now refused at the ADAPTER gate with -32601 (was: the one
    // family with an indirect engine gate — the registry's host-shell-None
    // responder error, a different wire shape).
    let shell = ext(
        &client,
        "kiro/terminal/shell_type",
        serde_json::json!({"sessionId": "s"}),
    )
    .await;
    assert_eq!(
        shell,
        Disposition::Refused,
        "shell_type refused at the adapter gate under V2 (C4)"
    );

    // An UNKNOWN method still falls to the protocol-default null (dcc6 F15).
    let unknown = ext(&client, "kiro/unknown/dn91", serde_json::json!({})).await;
    assert_eq!(unknown, Disposition::NullDefault);
}

/// cyril-dn91 C9 (+C14; C8's regression net): ONE walk over every engine
/// constructible in this build proving advertisement and execution
/// reachability TOGETHER (AC3 — separate tests of those two facts are
/// explicitly insufficient). Per family, the advertised bit (serialized
/// handshake capabilities) must equal the answers-bit (a real client call's
/// disposition) — two independent observation channels. Expectations are
/// derived per engine from its own data, not hardcoded per-engine tables, so
/// a future engine or mode inherits coverage. Any desync a hand-written
/// capability override could introduce (C8) fails its named cell here.
#[tokio::test]
async fn adapter_matrix_advertise_iff_answer() {
    use crate::protocol::engine::{Engine, KasEngine, client_capabilities};
    use crate::types::kas_hooks::KasHooksMode;

    let engines: Vec<(&str, std::rc::Rc<dyn Engine>)> = vec![
        ("v2", std::rc::Rc::new(V2Engine)),
        (
            "kas-off",
            std::rc::Rc::new(KasEngine {
                hooks_mode: KasHooksMode::Off,
            }),
        ),
        (
            "kas-host",
            std::rc::Rc::new(KasEngine {
                hooks_mode: KasHooksMode::Host,
            }),
        ),
        (
            "kas-kas",
            std::rc::Rc::new(KasEngine {
                hooks_mode: KasHooksMode::Kas,
            }),
        ),
    ];

    for (name, engine) in engines {
        let adapters = engine.adapters();
        let caps = client_capabilities(engine.as_ref());
        let caps_json = serde_json::to_value(&caps).unwrap();

        let (ntx, _nrx) = mpsc::channel(8);
        let (ptx, _prx) = mpsc::channel(1);
        let (mntx, _mnrx) = mpsc::channel(8);
        let dir = tempfile::tempdir().unwrap();
        // The mediation seam loads an (empty) hooks registry from the tempdir,
        // so a Host-mode engine's inbound list SERVES `{hooks:[]}`; engines
        // that don't serve inbound refuse at the client gate before dispatch.
        let client = KiroClient::new(
            ntx,
            ptx,
            engine.clone(),
            crate::protocol::client::spawn_test_mediation_at(
                crate::protocol::client::test_host_shell(AgentEngine::Kas),
                dir.path().to_path_buf(),
                true,
                mntx,
            ),
            dir.path(),
        );

        // FS: advertised read capability == a real read answers.
        let f = dir.path().join("m.txt");
        std::fs::write(&f, "x").unwrap();
        let fs_answers = client
            .read_text_file(acp::ReadTextFileRequest::new(acp::SessionId::new("s"), &f))
            .await
            .is_ok();
        assert_eq!(
            caps.fs.read_text_file, fs_answers,
            "[{name}] fs: advertised != answers"
        );

        // FS write: pair the OTHER bare-ACP fs bit with execution too (AC3 at
        // advertised-bit granularity, pre-PR review finding P1).
        let wf = dir.path().join("m-out.txt");
        let write_answers = client
            .write_text_file(acp::WriteTextFileRequest::new(
                acp::SessionId::new("s"),
                &wf,
                "w",
            ))
            .await
            .is_ok();
        assert_eq!(
            caps.fs.write_text_file, write_answers,
            "[{name}] fs write: advertised != answers"
        );

        // The `_kiro/fs/*` dialect flags (fs._meta.kiro, FS_OPS-derived):
        // stat as the representative — per-op dispatch pairing is
        // `every_advertised_fs_flag_is_dispatched`.
        let dialect_advertised =
            caps_json.pointer("/fs/_meta/kiro/stat") == Some(&serde_json::Value::Bool(true));
        let stat = ext(
            &client,
            crate::protocol::kas::kiro_fs::STAT_METHOD,
            serde_json::json!({"sessionId": "s", "path": f}),
        )
        .await;
        assert_eq!(
            dialect_advertised,
            stat == Disposition::Answered,
            "[{name}] fs dialect: advertised != answers"
        );

        // TERMINAL: advertised == shell_type answers.
        let term = ext(
            &client,
            "kiro/terminal/shell_type",
            serde_json::json!({"sessionId": "s"}),
        )
        .await;
        assert_eq!(
            caps.terminal,
            term == Disposition::Answered,
            "[{name}] terminal: advertised != answers"
        );

        // HOOKS, per direction (AC4): an inbound-serving advertisement is the
        // hooks key WITHOUT the v2 flag; `{enabled, v2}` is an outbound-client
        // declaration and must NOT be served inbound.
        let hooks_meta = caps_json.pointer("/_meta/kiro/hooks").cloned();
        let inbound_advertised = matches!(&hooks_meta, Some(h) if h.get("v2").is_none());
        let list = ext(
            &client,
            crate::protocol::kas::hooks::LIST_METHOD,
            serde_json::json!({"trigger": "promptSubmit"}),
        )
        .await;
        assert_eq!(
            inbound_advertised,
            list == Disposition::Answered,
            "[{name}] hooks: inbound advertisement != inbound serving"
        );

        // AUTH: no advertisement bit exists on the wire (KAS calls it
        // unconditionally under --auth=acp-callback) — execution must follow
        // the adapter instead.
        let auth = ext(
            &client,
            crate::protocol::kas::auth::GET_ACCESS_TOKEN_METHOD,
            serde_json::json!({}),
        )
        .await;
        assert_eq!(
            adapters.auth.is_some(),
            auth == Disposition::Answered,
            "[{name}] auth: adapter != answers"
        );

        // C14: methods outside the five families stay protocol-default null
        // under EVERY engine — the refusal net must not become refuse-by-default.
        let unknown = ext(&client, "kiro/unknown/matrix", serde_json::json!({})).await;
        assert_eq!(
            unknown,
            Disposition::NullDefault,
            "[{name}] unknown methods keep the null default (dcc6 F15)"
        );
    }
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
async fn kas_outbound_hooks_mode_refuses_inbound_serving() {
    let dir = tempfile::tempdir().unwrap();
    let (ntx, _nrx) = mpsc::channel(4);
    let (ptx, _prx) = mpsc::channel(1);
    let client = KiroClient::new(
        ntx,
        ptx,
        std::rc::Rc::new(crate::protocol::engine::KasEngine {
            hooks_mode: crate::types::kas_hooks::KasHooksMode::Kas,
        }),
        crate::protocol::client::spawn_test_mediation(crate::protocol::client::test_host_shell(
            AgentEngine::Kas,
        )),
        dir.path(),
    );

    let list = ext(
        &client,
        crate::protocol::kas::hooks::LIST_METHOD,
        serde_json::json!({"trigger": "promptSubmit"}),
    )
    .await;
    assert_eq!(
        list,
        Disposition::Refused,
        "Outbound advertises hooks but serves nothing inbound (C5/AC4)"
    );

    // executeHook must refuse BEFORE any execution: the command would create
    // a file, and the file must not exist afterwards (gate-after-execute bug).
    let marker = dir.path().join("dn91-must-not-exist");
    let raw = serde_json::value::RawValue::from_string(
        serde_json::json!({"hookId": "h", "hookName": "p",
            "command": format!("touch {}", marker.display()),
            "sessionId": "s", "userPrompt": ""})
        .to_string(),
    )
    .unwrap();
    let exec = client
        .ext_method(acp::ExtRequest::new(
            crate::protocol::kas::hooks::EXECUTE_METHOD,
            raw.into(),
        ))
        .await;
    assert_eq!(classify(&exec), Disposition::Refused, "execute refused");
    assert!(
        !marker.exists(),
        "refusal must precede execution — no side effect"
    );
}

//! Engine adapter advertisement/execution gates (cyril-dn91).
//! SDK2 request handlers enqueue typed work without capturing `Rc<dyn Engine>`;
//! the serial mediator therefore owns the execution gate. These tests pair the
//! independently derived handshake bits with that production gate.

use crate::protocol::domain_mediator::host;
use crate::protocol::engine::{Engine, KasEngine, V2Engine, client_capabilities};
use crate::protocol::kas::callbacks::HostFamily;
use crate::types::kas_hooks::KasHooksMode;

fn serialized_capabilities(engine: &dyn Engine) -> serde_json::Value {
    match serde_json::to_value(client_capabilities(engine)) {
        Ok(value) => value,
        Err(error) => panic!("client capabilities must serialize: {error}"),
    }
}

#[test]
fn adapter_matrix_advertises_if_and_only_if_the_mediator_answers() {
    let engines: Vec<(&str, Box<dyn Engine>)> = vec![
        ("v2", Box::new(V2Engine)),
        (
            "kas-off",
            Box::new(KasEngine {
                hooks_mode: KasHooksMode::Off,
            }),
        ),
        (
            "kas-host",
            Box::new(KasEngine {
                hooks_mode: KasHooksMode::Host,
            }),
        ),
        (
            "kas-kas",
            Box::new(KasEngine {
                hooks_mode: KasHooksMode::Kas,
            }),
        ),
    ];

    for (name, engine) in engines {
        let adapters = engine.adapters();
        let capabilities = client_capabilities(engine.as_ref());
        let json = serialized_capabilities(engine.as_ref());
        let host_io = host::supports(adapters, HostFamily::HostIo);
        assert_eq!(capabilities.fs.read_text_file, host_io, "{name}: fs read");
        assert_eq!(capabilities.fs.write_text_file, host_io, "{name}: fs write");
        assert_eq!(capabilities.terminal, host_io, "{name}: terminal");
        assert_eq!(
            json.pointer("/fs/_meta/kiro/stat") == Some(&serde_json::Value::Bool(true)),
            host_io,
            "{name}: Kiro fs dialect",
        );

        let hooks = json.pointer("/_meta/kiro/hooks");
        let inbound_hooks = matches!(hooks, Some(value) if value.get("v2").is_none());
        assert_eq!(
            inbound_hooks,
            host::supports(adapters, HostFamily::HooksInbound),
            "{name}: inbound hooks",
        );
        assert_eq!(
            hooks.is_some(),
            host::supports(adapters, HostFamily::HooksAny),
            "{name}: hooks in either direction",
        );
        assert_eq!(
            adapters.auth.is_some(),
            host::supports(adapters, HostFamily::Auth),
            "{name}: auth execution gate",
        );
    }
}

#[test]
fn v2_refuses_every_kas_host_family() {
    let adapters = V2Engine.adapters();
    for family in [
        HostFamily::Auth,
        HostFamily::HostIo,
        HostFamily::HooksInbound,
        HostFamily::HooksAny,
    ] {
        assert!(!host::supports(adapters, family), "v2 served {family:?}");
    }
}

#[test]
fn advertisement_is_fully_determined_by_presence_direction_and_settings() {
    let v2 = serialized_capabilities(&V2Engine);
    let empty =
        match serde_json::to_value(agent_client_protocol::schema::v1::ClientCapabilities::new()) {
            Ok(value) => value,
            Err(error) => panic!("empty capabilities must serialize: {error}"),
        };
    assert_eq!(v2, empty, "V2 must advertise the empty capability set");

    for mode in [KasHooksMode::Off, KasHooksMode::Host, KasHooksMode::Kas] {
        let actual = serialized_capabilities(&KasEngine { hooks_mode: mode });
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
                    "readFile": true,
                    "writeFile": true,
                    "stat": true,
                    "readDirectory": true,
                    "delete": true,
                }},
            },
            "terminal": true,
            "_meta": {"kiro": kiro},
        });
        assert_eq!(actual, expected, "KAS({mode:?}) capability derivation");
    }
}

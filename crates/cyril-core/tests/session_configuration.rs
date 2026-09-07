//! Standard configuration responses through the public, default V2 bridge.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{
    AgentCommand, BridgeCommand, ConfigOption, Notification, RoutedNotification, SpawnEnvironment,
};
use tokio::sync::mpsc::Receiver;
use tokio::time::timeout;

const SESSION_ID: &str = "configuration-peer";
const WAIT: Duration = Duration::from_secs(10);

fn configuration_peer(root: &Path) -> Result<AgentCommand> {
    // Resolve host version-manager shims before replacing HOME/PATH. Keep the
    // actual peer isolated with explicit interpreter arguments, not a shebang.
    let python = Command::new("python3")
        .args(["-I", "-c", "import sys; sys.stdout.write(sys.executable)"])
        .stdin(Stdio::null())
        .output()
        .context("resolve runnable host Python 3")?;
    ensure!(python.status.success(), "host Python 3 probe failed");
    let python = String::from_utf8(python.stdout).context("non-UTF-8 Python executable")?;
    ensure!(
        Path::new(&python).is_absolute(),
        "host Python 3 must report an absolute executable"
    );
    let path = root.join("configuration-peer.py");
    std::fs::write(
        &path,
        r#"import json
import signal
import sys

# No child processes, credentials, network, or inherited Python configuration.
# Even a broken bridge cannot leave this fixture alive indefinitely.
signal.alarm(30)
SESSION = "configuration-peer"
changes = 0


def send(message):
    print(json.dumps(message), flush=True)


def reply(request, result):
    send({"jsonrpc": "2.0", "id": request["id"], "result": result})


def catalog(value):
    return [{
        "id": "model", "name": "Model", "category": "model", "type": "select",
        "currentValue": value,
        "options": [
            {"value": "requested-model", "name": "Requested"},
            {"value": "server-model", "name": "Server selected"},
            {"value": "unsolicited-model", "name": "Unsolicited"},
        ],
    }]


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        reply(request, {"protocolVersion": 1, "agentCapabilities": {}, "authMethods": []})
    elif method == "session/new":
        reply(request, {"sessionId": SESSION})
    elif method == "session/set_config_option":
        params = request["params"]
        assert params["sessionId"] == SESSION
        assert params["configId"] == "model"
        assert params["value"] == "requested-model"
        changes += 1
        if changes == 1:
            grouped = catalog("server-model")
            grouped[0]["options"] = [{
                "group": "models", "name": "Models", "options": grouped[0]["options"],
            }]
            reply(request, {"configOptions": grouped})
        elif changes == 2:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {
                "sessionId": SESSION,
                "update": {"sessionUpdate": "config_option_update",
                           "configOptions": catalog("unsolicited-model")},
            }})
            # Valid JSON, invalid select currentValue. SDK DefaultOnError can
            # discard this malformed catalog; the response boundary must not.
            reply(request, {"configOptions": catalog(42)})
        elif changes == 3:
            grouped = catalog("server-model")
            grouped[0]["options"] = [{
                "group": "models", "name": "Models",
                "options": [
                    {"value": "server-model", "name": "Server selected"},
                    {"value": 42, "name": "Malformed choice"},
                ],
            }]
            reply(request, {"configOptions": grouped})
        else:
            raise AssertionError("unexpected extra configuration request")
    elif "id" in request:
        send({"jsonrpc": "2.0", "id": request["id"], "error": {
            "code": -32601, "message": "fixture only supports standard session configuration",
        }})
"#,
    )?;
    Ok(AgentCommand::new(python).with_args(vec![
        "-I".into(),
        path.to_str().context("non-UTF-8 fixture path")?.into(),
    ]))
}

async fn next_notification(rx: &mut Receiver<RoutedNotification>) -> Result<Notification> {
    Ok(rx
        .recv()
        .await
        .context("bridge notification channel closed")?
        .notification)
}

async fn configuration_outcome(rx: &mut Receiver<RoutedNotification>) -> Result<Notification> {
    loop {
        match next_notification(rx).await? {
            outcome @ (Notification::ConfigOptionSet { .. } | Notification::BridgeError { .. }) => {
                return Ok(outcome);
            }
            Notification::BridgeDisconnected { reason } => bail!("peer disconnected: {reason}"),
            _ => {}
        }
    }
}

fn assert_model(options: &[ConfigOption], expected: &str) -> Result<()> {
    let model = options
        .iter()
        .find(|option| option.key == "model")
        .context("missing returned model option")?;
    assert_eq!(model.value.as_deref(), Some(expected));
    assert_eq!(
        model.options,
        ["requested-model", "server-model", "unsolicited-model"]
    );
    Ok(())
}

#[tokio::test]
async fn standard_configuration_uses_returned_catalog_and_rejects_malformed_response() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let root = directory.path().canonicalize()?;
    let bridge = spawn_bridge(
        configuration_peer(&root)?,
        SpawnConfig {
            shell: Some("bash".into()),
            environment: SpawnEnvironment::Replace(BTreeMap::from([
                ("HOME".into(), root.clone().into_os_string()),
                ("XDG_CONFIG_HOME".into(), root.clone().into_os_string()),
                ("TMPDIR".into(), root.clone().into_os_string()),
                ("PATH".into(), "/usr/bin:/bin".into()),
            ])),
            ..SpawnConfig::default()
        },
        root.clone(),
    )?;
    let (sender, mut notifications, _permissions, _sources, completion) = bridge.split();

    // Save the result rather than panicking: even failure/timeout must drop the
    // sender and await core-owned process teardown before removing the fixture.
    let observations = timeout(WAIT, async {
        sender.send(BridgeCommand::NewSession { cwd: root }).await?;
        loop {
            match next_notification(&mut notifications).await? {
                Notification::SessionCreated { session_id, .. } => {
                    ensure!(
                        session_id.as_str() == SESSION_ID,
                        "unexpected session identity"
                    );
                    break;
                }
                Notification::BridgeError { .. } => bail!("session creation failed"),
                Notification::BridgeDisconnected { reason } => bail!("peer disconnected: {reason}"),
                _ => {}
            }
        }
        sender
            .send(BridgeCommand::SetConfigOption {
                config_id: "model".into(),
                value: "requested-model".into(),
            })
            .await?;
        let first = configuration_outcome(&mut notifications).await?;

        sender
            .send(BridgeCommand::SetConfigOption {
                config_id: "model".into(),
                value: "requested-model".into(),
            })
            .await?;
        let mut unsolicited = None;
        let mut second = None;
        // Notification and response processing may interleave. Both must be
        // observable, in distinct public event classes, regardless of order.
        while unsolicited.is_none() || second.is_none() {
            match next_notification(&mut notifications).await? {
                Notification::ConfigOptionsUpdated(options) => unsolicited = Some(options),
                outcome @ (Notification::ConfigOptionSet { .. }
                | Notification::BridgeError { .. }) => {
                    ensure!(second.is_none(), "duplicate configuration outcome");
                    second = Some(outcome);
                }
                Notification::BridgeDisconnected { reason } => bail!("peer disconnected: {reason}"),
                _ => {}
            }
        }
        sender
            .send(BridgeCommand::SetConfigOption {
                config_id: "model".into(),
                value: "requested-model".into(),
            })
            .await?;
        let grouped = configuration_outcome(&mut notifications).await?;
        Ok::<_, anyhow::Error>((first, unsolicited, second, grouped))
    })
    .await;

    drop(sender);
    timeout(WAIT, completion)
        .await
        .context("bridge teardown timed out")?
        .context("bridge completion channel closed")?;
    let (first, unsolicited, second, grouped) =
        observations.context("configuration exchange timed out")??;

    // Independent response observations: requested-model is not the server's
    // chosen value, and the unsolicited valid catalog cannot acknowledge the
    // malformed response (nor may an empty/default catalog count as success).
    match first {
        Notification::ConfigOptionSet { config_id, options } => {
            assert_eq!(config_id, "model");
            assert_model(&options, "server-model")?;
        }
        other => panic!("expected returned configuration catalog, got {other:?}"),
    }
    assert_model(
        &unsolicited.context("missing unsolicited configuration update")?,
        "unsolicited-model",
    )?;
    assert!(
        matches!(
            second,
            Some(Notification::BridgeError { operation, .. }) if operation == "set_config_option"
        ),
        "malformed configuration response must produce its typed operation failure"
    );
    assert!(
        matches!(grouped, Notification::BridgeError { operation, .. } if operation == "set_config_option"),
        "malformed grouped choices must not be silently discarded"
    );
    Ok(())
}

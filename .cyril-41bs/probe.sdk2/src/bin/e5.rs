use std::{
    collections::BTreeSet,
    env,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use agent_client_protocol::schema::{
    ProtocolVersion,
    v1::{
        ContentBlock, Implementation, InitializeRequest, NewSessionRequest, PromptRequest,
        SessionNotification, TextContent,
    },
};
use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Agent, Channel, Client, ConnectTo, ConnectionTo, Handled,
    RawJsonRpcMessage, Responder, TransportFrame, UntypedMessage, schema::v1::RequestId,
};
use anyhow::{Context, Result, bail};
use futures_util::StreamExt as _;
use rusqlite::Connection;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

#[derive(Clone)]
struct Events(Arc<Mutex<Vec<String>>>);

impl Events {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn push(&self, value: impl Into<String>) {
        self.lock().push(value.into());
    }

    fn snapshot(&self) -> Vec<String> {
        self.lock().clone()
    }
}

fn sqlite_json(value: rusqlite::types::Value) -> Result<Value> {
    let text = match value {
        rusqlite::types::Value::Text(text) => text,
        rusqlite::types::Value::Blob(bytes) => {
            String::from_utf8(bytes).context("credential row is not UTF-8")?
        }
        other => bail!("credential row has unexpected SQLite type: {other:?}"),
    };
    serde_json::from_str(&text).context("parse credential row JSON")
}

fn load_auth_response() -> Result<Value> {
    let home = env::var_os("HOME").context("HOME is unset")?;
    let db_path = PathBuf::from(home).join(".local/share/kiro-cli/data.sqlite3");
    let connection = Connection::open(&db_path)
        .with_context(|| format!("open Kiro credential database {}", db_path.display()))?;
    let token_value = connection
        .query_row(
            "select value from auth_kv where key in ('kirocli:odic:token','kirocli:social:token') order by key asc limit 1",
            [],
            |row| row.get::<_, rusqlite::types::Value>(0),
        )
        .context("read current Kiro token")?;
    let token = sqlite_json(token_value)?;
    let profile_from_token = token.get("profile_arn").and_then(Value::as_str);
    let profile = if let Some(profile) = profile_from_token {
        profile.to_owned()
    } else {
        let profile_value = connection
            .query_row(
                "select value from state where key='api.codewhisperer.profile'",
                [],
                |row| row.get::<_, rusqlite::types::Value>(0),
            )
            .context("read active Kiro profile")?;
        sqlite_json(profile_value)?
            .get("arn")
            .and_then(Value::as_str)
            .context("active Kiro profile has no arn")?
            .to_owned()
    };
    Ok(json!({
        "accessToken": token.get("access_token").context("token has no access_token")?,
        "expiresAt": token.get("expires_at").context("token has no expires_at")?,
        "profileArn": profile,
    }))
}

fn request_result(method: &str, params: &Value, auth: &Value) -> Value {
    match method {
        "_kiro/auth/getAccessToken" | "kiro/auth/getAccessToken" => auth.clone(),
        "_kiro/terminal/shell_type" | "kiro/terminal/shell_type" => {
            json!({"shellType": "bash"})
        }
        "fs/read_text_file" | "_kiro/fs/read_file" | "kiro/fs/read_file" => {
            json!({"content": "probe-content"})
        }
        "fs/write_text_file"
        | "_kiro/fs/write_file"
        | "kiro/fs/write_file"
        | "_kiro/fs/delete"
        | "kiro/fs/delete"
        | "terminal/release"
        | "terminal/kill"
        | "_kiro/hooks/executeHook"
        | "kiro/hooks/executeHook"
        | "_kiro/hooks/sessionStart"
        | "kiro/hooks/sessionStart" => json!({}),
        "_kiro/fs/read_directory" | "kiro/fs/read_directory" => json!({"entries": []}),
        "_kiro/fs/stat" | "kiro/fs/stat" => json!({"type": "file", "size": 0}),
        "terminal/create" => json!({"terminalId": "probe-terminal"}),
        "terminal/output" => {
            json!({"output": "", "truncated": false, "exitStatus": {"exitCode": 0}})
        }
        "terminal/wait_for_exit" => {
            json!({"exitStatus": {"exitCode": 0, "signal": null}})
        }
        "session/request_permission" => {
            let option_id = params
                .get("options")
                .and_then(Value::as_array)
                .and_then(|options| options.first())
                .and_then(|option| option.get("optionId"))
                .cloned();
            match option_id {
                Some(option_id) => {
                    json!({"outcome": {"outcome": "selected", "optionId": option_id}})
                }
                None => json!({"outcome": {"outcome": "cancelled"}}),
            }
        }
        "_kiro/hooks/list" | "kiro/hooks/list" => json!({"hooks": []}),
        _ => json!({}),
    }
}

macro_rules! build_client {
    ($events:expr, $auth:expr) => {{
        let events: Events = $events;
        let auth: Value = $auth;
        let request_events = events.clone();
        let unknown_events = events.clone();
        let session_events = events.clone();
        let notification_events = events;
        Client
            .builder()
            .on_receive_request(
                async move |request: UntypedMessage,
                            responder: Responder<Value>,
                            _cx: ConnectionTo<Agent>| {
                    request_events.push(format!("request:{}", request.method));
                    responder.respond(request_result(&request.method, &request.params, &auth))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |message: UntypedMessage, cx: ConnectionTo<Agent>| {
                    if message.method == "session/update"
                        && serde_json::from_value::<SessionNotification>(message.params.clone())
                            .is_err()
                    {
                        unknown_events.push("notification:session/update:unknown-contained");
                        return Ok(Handled::Yes);
                    }
                    Ok(Handled::No {
                        message: (message, cx),
                        retry: false,
                    })
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification(
                async move |notification: SessionNotification, _cx: ConnectionTo<Agent>| {
                    session_events.push(format!(
                        "notification:session/update:{:?}",
                        notification.update
                    ));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification(
                async move |message: UntypedMessage, _cx: ConnectionTo<Agent>| {
                    notification_events.push(format!("notification:{}", message.method));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
    }};
}

fn direct_client(events: Events, auth: Value) -> impl ConnectTo<Agent> {
    build_client!(events, auth)
}

async fn run_live(name: &'static str, config: AcpAgentConfig, auth: Value) -> Result<Value> {
    let scratch = TempDir::new().context("create live Kiro scratch directory")?;
    std::fs::write(scratch.path().join("README.md"), "SDK2 parity probe\n")?;
    let events = Events::new();
    let events_after = events.clone();
    let events_for_run = events_after.clone();
    let client = build_client!(events, auth);
    let response = timeout(
        Duration::from_secs(180),
        client.connect_with(AcpAgent::new(config), async move |connection| {
            let initialize = connection
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_info(Implementation::new("cyril", "0").title("Cyril")),
                )
                .block_task()
                .await?;
            events_for_run.push("response:initialize");
            let session = connection
                .send_request(NewSessionRequest::new(scratch.path()))
                .block_task()
                .await?;
            events_for_run.push("response:session/new");
            let prompt = connection
                .send_request(PromptRequest::new(
                    session.session_id,
                    vec![ContentBlock::Text(TextContent::new(
                        "Reply exactly SDK2_PARITY. Do not use tools.",
                    ))],
                ))
                .block_task()
                .await?;
            events_for_run.push("response:session/prompt");
            Ok::<_, agent_client_protocol::Error>((
                initialize.protocol_version,
                prompt.stop_reason,
                true,
            ))
        }),
    )
    .await
    .with_context(|| format!("{name} live SDK path timed out"))?
    .map_err(|error| anyhow::anyhow!("{name} live SDK path failed: {error:?}"))?;
    let observed = events_after.snapshot();
    let prompt_response_last = observed
        .last()
        .is_some_and(|event| event == "response:session/prompt");
    if !prompt_response_last {
        bail!("{name} emitted traffic after the prompt response");
    }
    let agent_message_chunks = observed
        .iter()
        .filter(|event| event.contains("AgentMessageChunk"))
        .count();
    if agent_message_chunks == 0 {
        bail!("{name} completed without an agent message chunk");
    }
    let observed_methods = observed
        .iter()
        .filter_map(|event| {
            event
                .strip_prefix("request:")
                .or_else(|| event.strip_prefix("notification:"))
                .and_then(|rest| rest.split(':').next())
        })
        .collect::<BTreeSet<_>>();
    Ok(json!({
        "engine": name,
        "protocol_version": format!("{:?}", response.0),
        "stop_reason": format!("{:?}", response.1),
        "session_id_present": response.2,
        "event_count": observed.len(),
        "observed_methods": observed_methods,
        "agent_message_chunks": agent_message_chunks,
        "unknown_updates_contained": observed.iter().any(|event| event == "notification:session/update:unknown-contained"),
        "prompt_response_last": prompt_response_last,
    }))
}

async fn run_callback_matrix(auth: Value) -> Result<Value> {
    let methods = [
        "_kiro/auth/getAccessToken",
        "_kiro/terminal/shell_type",
        "fs/read_text_file",
        "fs/write_text_file",
        "_kiro/fs/read_file",
        "_kiro/fs/write_file",
        "_kiro/fs/stat",
        "_kiro/fs/read_directory",
        "_kiro/fs/delete",
        "terminal/create",
        "terminal/output",
        "terminal/wait_for_exit",
        "terminal/release",
        "terminal/kill",
        "session/request_permission",
        "_kiro/hooks/list",
        "_kiro/hooks/executeHook",
        "_kiro/hooks/sessionStart",
    ];
    let notification_methods = ["_kiro/hooks/cancel", "_kiro/hooks/didChange"];
    let events = Events::new();
    let client = direct_client(events.clone(), auth);
    let (transport, mut fake_agent) = Channel::duplex();
    let client_task = tokio::spawn(client.connect_to(transport));
    let mut response_ids = BTreeSet::new();
    for (index, method) in methods.iter().enumerate() {
        let id = RequestId::Number(i64::try_from(index + 1)?);
        let request = RawJsonRpcMessage::request(
            (*method).to_owned(),
            if *method == "session/request_permission" {
                json!({"options": [{"optionId": "allow-once"}]})
            } else {
                json!({})
            },
            id,
        )?;
        fake_agent
            .tx
            .unbounded_send(TransportFrame::Single(request))
            .context("send fake host callback")?;
        let response = fake_agent
            .rx
            .next()
            .await
            .context("receive fake callback response")?;
        let serialized = match response {
            TransportFrame::Single(message) => serde_json::to_value(message)?,
            other => bail!("callback response changed frame shape: {other:?}"),
        };
        if serialized.get("error").is_some() {
            bail!("callback {method} returned error: {serialized}");
        }
        response_ids.insert(serde_json::to_string(
            serialized
                .get("id")
                .context("callback response has no id")?,
        )?);
    }
    for method in notification_methods {
        fake_agent
            .tx
            .unbounded_send(TransportFrame::Single(RawJsonRpcMessage::notification(
                method.to_owned(),
                json!({}),
            )?))
            .context("send fake host notification")?;
    }
    drop(fake_agent);
    client_task.await.context("join callback matrix client")??;
    let observed = events.snapshot();
    Ok(json!({
        "request_methods": methods,
        "notification_methods": notification_methods,
        "request_response_count": response_ids.len(),
        "all_requests_answered": response_ids.len() == methods.len(),
        "observed_events": observed,
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = env::args().nth(1).unwrap_or_else(|| "all".to_owned());
    let auth = if mode == "matrix" {
        json!({
            "accessToken": "<in-memory-probe>",
            "expiresAt": "2099-01-01T00:00:00Z",
            "profileArn": "<in-memory-probe>",
        })
    } else {
        load_auth_response()?
    };
    let callbacks = run_callback_matrix(auth.clone()).await?;
    let live = match mode.as_str() {
        "matrix" => Vec::new(),
        "v2" => vec![
            run_live(
                "kiro-v2",
                AcpAgentConfig::new("kiro-cli").arg("acp"),
                auth.clone(),
            )
            .await?,
        ],
        "kas" => vec![
            run_live(
                "kiro-kas",
                AcpAgentConfig::new("kiro-cli").args(["acp", "--agent-engine", "v3"]),
                auth.clone(),
            )
            .await?,
        ],
        "all" => vec![
            run_live(
                "kiro-v2",
                AcpAgentConfig::new("kiro-cli").arg("acp"),
                auth.clone(),
            )
            .await?,
            run_live(
                "kiro-kas",
                AcpAgentConfig::new("kiro-cli").args(["acp", "--agent-engine", "v3"]),
                auth,
            )
            .await?,
        ],
        other => bail!("unknown mode `{other}`; expected matrix, v2, kas, or all"),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C2", "C7"],
            "sdk_version": "2.0.0",
            "wire_version": "V1",
            "mode": mode,
            "callback_matrix": callbacks,
            "live": live,
            "credential_logged": false,
            "production_state_modified": false,
        }))?
    );
    Ok(())
}

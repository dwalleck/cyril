use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use agent_client_protocol::schema::{
    InitializeProxyRequest, ProtocolVersion,
    v1::{
        ContentBlock, Implementation, InitializeRequest, NewSessionRequest, PromptRequest,
        TextContent,
    },
};
use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Agent, Client, Conductor, ConnectTo, ConnectionTo, Error, Proxy,
    Responder, UntypedMessage,
};
use agent_client_protocol_conductor::{ConductorImpl, ProxiesAndAgent};
use anyhow::{Context, Result, bail};
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
    let profile = if let Some(profile) = token.get("profile_arn").and_then(Value::as_str) {
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

fn callback_result(method: &str, params: &Value, auth: &Value) -> Value {
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
        "terminal/wait_for_exit" => json!({"exitStatus": {"exitCode": 0, "signal": null}}),
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

macro_rules! live_client {
    ($events:expr, $auth:expr) => {{
        let events: Events = $events;
        let auth: Value = $auth;
        let request_events = events.clone();
        Client
            .builder()
            .on_receive_request(
                async move |request: UntypedMessage,
                            responder: Responder<Value>,
                            _cx: ConnectionTo<Agent>| {
                    request_events.push(format!("request:{}", request.method));
                    responder.respond(callback_result(&request.method, &request.params, &auth))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification(
                async move |notification: UntypedMessage, _cx: ConnectionTo<Agent>| {
                    events.push(format!("notification:{}", notification.method));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
    }};
}
fn annotate_params(params: &mut Value) {
    let Some(params) = params.as_object_mut() else {
        return;
    };
    let meta = params.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta) = meta.as_object_mut() {
        meta.insert("cyrilProbeObserved".to_owned(), Value::Bool(true));
    }
}

#[derive(Clone)]
struct TransformingAuditProxy {
    transformed: Events,
}

impl ConnectTo<Conductor> for TransformingAuditProxy {
    async fn connect_to(self, client: impl ConnectTo<Proxy>) -> Result<(), Error> {
        let client_transformed = self.transformed.clone();
        let agent_transformed = self.transformed;
        Proxy
            .builder()
            .on_receive_request_from(
                Client,
                async |request: InitializeProxyRequest, responder, cx| {
                    cx.send_request_to(Agent, request.initialize)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Client,
                async move |mut request: UntypedMessage, responder: Responder<Value>, cx| {
                    if request.method == "session/prompt" {
                        annotate_params(&mut request.params);
                        client_transformed.push("transformed:client:session/prompt");
                    }
                    cx.send_request_to(Agent, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Agent,
                async |request: UntypedMessage, responder: Responder<Value>, cx| {
                    cx.send_request_to(Client, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification_from(
                Client,
                async |notification: UntypedMessage, cx| {
                    cx.send_notification_to(Agent, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification_from(
                Agent,
                async move |mut notification: UntypedMessage, cx| {
                    if matches!(
                        notification.method.as_str(),
                        "_kiro.dev/metadata" | "_kiro/mcp/status"
                    ) {
                        annotate_params(&mut notification.params);
                        agent_transformed
                            .push(format!("transformed:agent:{}", notification.method));
                    }
                    cx.send_notification_to(Client, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

async fn run_topology<C>(
    engine: &'static str,
    topology: &'static str,
    conductor: C,
    auth: Value,
    transformed: Events,
) -> Result<Value>
where
    C: ConnectTo<Client>,
{
    let scratch = TempDir::new().context("create live conductor scratch directory")?;
    let events = Events::new();
    let events_after = events.clone();
    let result = timeout(
        Duration::from_secs(180),
        live_client!(events, auth).connect_with(conductor, async move |cx| {
            let initialize = cx
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_info(Implementation::new("cyril", "0").title("Cyril")),
                )
                .block_task()
                .await?;
            let session = cx
                .send_request(NewSessionRequest::new(scratch.path()))
                .block_task()
                .await?;
            let prompt = cx
                .send_request(PromptRequest::new(
                    session.session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(
                        "Reply only SDK2-LIVE-OK. Do not use tools.",
                    ))],
                ))
                .block_task()
                .await?;
            Ok::<_, Error>((initialize, session, prompt))
        }),
    )
    .await
    .with_context(|| format!("{engine}/{topology} timed out"))?
    .map_err(|error| anyhow::anyhow!("{engine}/{topology} failed: {error:?}"))?;
    Ok(json!({
        "engine": engine,
        "topology": topology,
        "protocol_version": result.0.protocol_version,
        "session_id_present": !result.1.session_id.to_string().is_empty(),
        "stop_reason": result.2.stop_reason,
        "events": events_after.snapshot(),
        "proxy_transformations": transformed.snapshot(),
    }))
}

async fn run_engine(
    engine: &'static str,
    args: &'static [&'static str],
    auth: Value,
) -> Result<Vec<Value>> {
    let zero_transform = Events::new();
    let zero = ConductorImpl::new_agent(
        format!("{engine}-zero"),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        )),
    );
    let zero = run_topology(engine, "zero-proxy", zero, auth.clone(), zero_transform).await?;

    let noop_transform = Events::new();
    let noop = ConductorImpl::new_agent(
        format!("{engine}-noop"),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        ))
        .proxy(Proxy.builder()),
    );
    let noop = run_topology(engine, "no-op-proxy", noop, auth.clone(), noop_transform).await?;

    let transformed = Events::new();
    let transforming = ConductorImpl::new_agent(
        format!("{engine}-transforming"),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        ))
        .proxy(TransformingAuditProxy {
            transformed: transformed.clone(),
        }),
    );
    let transforming = run_topology(
        engine,
        "transforming-proxy",
        transforming,
        auth,
        transformed,
    )
    .await?;
    Ok(vec![zero, noop, transforming])
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = env::args().nth(1).unwrap_or_else(|| "all".to_owned());
    let auth = load_auth_response()?;
    let mut results = Vec::new();
    match mode.as_str() {
        "v2" => results.extend(run_engine("v2", &["acp"], auth).await?),
        "kas" => results.extend(run_engine("kas", &["acp", "--agent-engine", "v3"], auth).await?),
        "all" => {
            results.extend(run_engine("v2", &["acp"], auth.clone()).await?);
            results.extend(run_engine("kas", &["acp", "--agent-engine", "v3"], auth).await?);
        }
        other => bail!("unknown mode `{other}`; expected v2, kas, or all"),
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C2", "C7"],
            "sdk_version": "2.0.0",
            "wire_version": "V1",
            "mode": mode,
            "topologies": results,
            "credential_logged": false,
            "production_state_modified": false,
        }))?
    );
    Ok(())
}

use std::{
    collections::BTreeMap,
    env,
    sync::{Arc, Mutex, MutexGuard},
};

use agent_client_protocol::schema::{
    ProtocolVersion,
    v1::{
        ClientCapabilities, FileSystemCapabilities, Implementation, InitializeRequest,
        InitializeResponse, Meta, SessionNotification,
    },
};
use agent_client_protocol::{
    Agent, Channel, Client, ConnectTo, ConnectionTo, Error, ErrorCode, Handled, Responder,
    UntypedMessage,
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use rusqlite::Connection;
use serde_json::{Value, json};
use tokio::sync::oneshot;

pub const MAX_EVENTS: usize = 1_000;
const MAX_EVENT_ENTRY_BYTES: usize = 256 * 1024;
const MAX_EVENT_BYTES: usize = 1024 * 1024;
pub const MAX_SESSION_SECONDS: u64 = 60;
pub const MATRIX_BARRIER_METHOD: &str = "_kiro/probe/barrier";
pub const MATRIX_ERROR_METHOD: &str = "_kiro/probe/invalid-params";

pub const CALLBACK_REQUEST_METHODS: [&str; 18] = [
    "_kiro/auth/getAccessToken",
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
    "_kiro/terminal/shell_type",
    "session/request_permission",
    "_kiro/hooks/list",
    "_kiro/hooks/executeHook",
    "_kiro/hooks/sessionStart",
];

pub const CALLBACK_NOTIFICATION_METHODS: [&str; 2] =
    ["_kiro/hooks/cancel", "_kiro/hooks/didChange"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineKind {
    V2,
    Kas,
}

impl EngineKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::V2 => "v2",
            Self::Kas => "kas",
        }
    }

    pub const fn is_kas(self) -> bool {
        matches!(self, Self::Kas)
    }
}

#[must_use]
pub const fn live_prompt(engine: EngineKind) -> &'static str {
    if engine.is_kas() {
        "Before answering, you MUST use the filesystem integration to read README.md and the terminal integration to run `printf SDK2_HOST_CALLBACK`. After both tools finish, reply exactly SDK2_PARITY. Do not modify files."
    } else {
        "Reply exactly SDK2_PARITY. Do not use tools."
    }
}

#[derive(Default)]
struct EventBuffer {
    entries: Vec<String>,
    retained_bytes: usize,
}

#[derive(Clone, Default)]
pub struct Events(Arc<Mutex<EventBuffer>>);

impl Events {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, EventBuffer> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn push(&self, value: impl Into<String>) {
        let value = value.into();
        let mut events = self.lock();
        if events
            .entries
            .last()
            .is_some_and(|entry| entry == "error:event-limit")
        {
            return;
        }
        let exceeds_total_bytes = events
            .retained_bytes
            .checked_add(value.len())
            .is_none_or(|bytes| bytes > MAX_EVENT_BYTES);
        if events.entries.len() >= MAX_EVENTS
            || value.len() > MAX_EVENT_ENTRY_BYTES
            || exceeds_total_bytes
        {
            if events.entries.len() >= MAX_EVENTS
                && let Some(last) = events.entries.last_mut()
            {
                *last = "error:event-limit".to_owned();
            } else {
                events.entries.push("error:event-limit".to_owned());
            }
            return;
        }
        events.retained_bytes += value.len();
        events.entries.push(value);
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.lock().entries.clone()
    }
}

struct Observed<C> {
    base: C,
    wire: Events,
}

impl<C> ConnectTo<Client> for Observed<C>
where
    C: ConnectTo<Client>,
{
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let (client_channel, client_future) = client.into_channel_and_future();
        let (base_channel, base_future) = self.base.into_channel_and_future();
        let client_to_agent = self.wire.clone();
        let agent_to_client = self.wire;
        let bridge = Channel::bridge_with_inspection(
            client_channel,
            base_channel,
            move |message| {
                client_to_agent.push(format!("client->agent:{}", serde_json::to_string(message)?));
                Ok(())
            },
            move |message| {
                agent_to_client.push(format!("agent->client:{}", serde_json::to_string(message)?));
                Ok(())
            },
        );
        tokio::try_join!(client_future, base_future, bridge)?;
        Ok(())
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

pub fn load_auth_response() -> Result<Value> {
    let home = env::var_os("HOME").context("HOME is unset")?;
    let db_path = std::path::PathBuf::from(home).join(".local/share/kiro-cli/data.sqlite3");
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

pub fn capabilities(engine: EngineKind) -> Result<ClientCapabilities> {
    if !engine.is_kas() {
        return Ok(ClientCapabilities::new());
    }
    let fs_meta: Meta = serde_json::from_value(json!({
        "kiro": {
            "readFile": true,
            "writeFile": true,
            "stat": true,
            "readDirectory": true,
            "delete": true,
        }
    }))
    .context("deserialize hard-coded KAS filesystem capability metadata")?;
    let top_level_meta: Meta = serde_json::from_value(json!({
        "kiro": {"hooks": {"enabled": true}}
    }))
    .context("deserialize hard-coded KAS hook capability metadata")?;
    Ok(ClientCapabilities::new()
        .fs(FileSystemCapabilities::new()
            .read_text_file(true)
            .write_text_file(true)
            .meta(fs_meta))
        .terminal(true)
        .meta(top_level_meta))
}

#[must_use]
pub fn callback_params(method: &str) -> Value {
    if method == "session/request_permission" {
        json!({"options": [{"optionId": "allow-once"}]})
    } else if method == "terminal/create" {
        json!({"command": "printf SDK2_PARITY"})
    } else {
        json!({})
    }
}

#[must_use]
pub fn callback_result(method: &str, params: &Value, auth: &Value) -> Value {
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

pub async fn connect_client_with<R>(
    events: Events,
    auth: Value,
    agent: impl ConnectTo<Client>,
    main_fn: impl AsyncFnOnce(ConnectionTo<Agent>) -> std::result::Result<R, Error>,
) -> std::result::Result<R, Error> {
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
                request_events.push(format!(
                    "request:{}:id={:?}",
                    request.method,
                    responder.id()
                ));
                if request.method == MATRIX_ERROR_METHOD {
                    let error: Error = ErrorCode::InvalidParams.into();
                    responder.respond_with_error(error.data(json!({"probe": "invalid-params"})))
                } else {
                    responder.respond(callback_result(&request.method, &request.params, &auth))
                }
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
                let payload = serde_json::to_string(&notification).map_err(|error| {
                    Error::internal_error().data(format!(
                        "serialize session notification for live evidence: {error}"
                    ))
                })?;
                session_events.push(format!("notification:session/update:{payload}"));
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_notification(
            async move |notification: UntypedMessage, _cx: ConnectionTo<Agent>| {
                notification_events.push(format!("notification:{}", notification.method));
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(agent, main_fn)
        .await
}

#[must_use]
pub fn normalize_methods(events: &[String]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| {
            event
                .strip_prefix("request:")
                .or_else(|| event.strip_prefix("notification:"))
                .and_then(|event| event.split(':').next())
                .map(str::to_owned)
        })
        .collect()
}

#[must_use]
pub fn kas_turn_end_observed(events: &[String]) -> bool {
    events.iter().any(|event| {
        event.starts_with("notification:session/update:")
            && event.contains(r#""kind":"turn_end""#)
            && event.contains(r#""stopReason":"end_turn""#)
    })
}

#[must_use]
pub fn kas_host_families(events: &[String]) -> BTreeMap<&'static str, bool> {
    let methods = normalize_methods(events);
    BTreeMap::from([
        (
            "auth",
            methods
                .iter()
                .any(|method| method == "_kiro/auth/getAccessToken"),
        ),
        (
            "filesystem",
            methods
                .iter()
                .any(|method| method.starts_with("fs/") || method.starts_with("_kiro/fs/")),
        ),
        (
            "terminal",
            methods.iter().any(|method| {
                method.starts_with("terminal/") || method.starts_with("_kiro/terminal/")
            }),
        ),
        (
            "permission",
            methods
                .iter()
                .any(|method| method == "session/request_permission"),
        ),
        (
            "hooks",
            methods
                .iter()
                .any(|method| method.starts_with("_kiro/hooks/")),
        ),
    ])
}

#[must_use]
pub fn normalize_events(events: &[String]) -> Vec<String> {
    let mut normalized_events = Vec::with_capacity(events.len());
    for event in events {
        let normalized = if let Some((prefix, _)) = event.split_once(":id=") {
            prefix.to_owned()
        } else if event.starts_with("notification:session/update:") {
            "notification:session/update".to_owned()
        } else {
            event.to_owned()
        };
        let repeated_session_update = normalized == "notification:session/update"
            && normalized_events.last() == Some(&normalized);
        if !repeated_session_update {
            normalized_events.push(normalized);
        }
    }
    normalized_events
}

#[must_use]
pub fn method_counts(events: &[String]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for method in normalize_methods(events) {
        let count = counts.entry(method).or_insert(0);
        *count += 1;
    }
    counts
}

#[must_use]
pub fn callback_contract() -> Value {
    json!({
        "request_methods": CALLBACK_REQUEST_METHODS,
        "notification_methods": CALLBACK_NOTIFICATION_METHODS,
        "request_count": CALLBACK_REQUEST_METHODS.len(),
        "notification_count": CALLBACK_NOTIFICATION_METHODS.len(),
    })
}

pub async fn run_direct_matrix(auth: Value) -> Result<Value> {
    let (done_tx, done_rx) = oneshot::channel();
    run_conductor_matrix("direct", MatrixAgent::new(done_tx), auth, done_rx).await
}

pub struct MatrixAgent {
    done_tx: Option<oneshot::Sender<Result<Value, String>>>,
}

impl MatrixAgent {
    #[must_use]
    pub fn new(done_tx: oneshot::Sender<Result<Value, String>>) -> Self {
        Self {
            done_tx: Some(done_tx),
        }
    }
}

impl ConnectTo<Client> for MatrixAgent {
    async fn connect_to(self, client: impl ConnectTo<Agent>) -> Result<(), Error> {
        let done_tx = self.done_tx;
        Agent
            .builder()
            .on_receive_request(
                async |request: InitializeRequest, responder, _cx| {
                    responder.respond(InitializeResponse::new(request.protocol_version))
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(client, async move |cx| {
                let outcome = match run_matrix_requests(cx).await {
                    Ok(value) => Ok(value),
                    Err(error) => Err(error.to_string()),
                };
                let outcome_error = outcome.as_ref().err().cloned();
                if let Some(done_tx) = done_tx
                    && done_tx.send(outcome).is_err()
                {
                    return Err(Error::internal_error().data("matrix completion receiver closed"));
                }
                if let Some(error) = outcome_error {
                    return Err(Error::internal_error().data(error));
                }
                Ok::<(), Error>(())
            })
            .await
    }
}

async fn run_matrix_requests(cx: ConnectionTo<Client>) -> Result<Value> {
    let mut response_count = 0;
    for method in CALLBACK_REQUEST_METHODS {
        cx.send_request(UntypedMessage::new(method, callback_params(method))?)
            .block_task()
            .await?;
        response_count += 1;
    }
    for method in CALLBACK_NOTIFICATION_METHODS {
        cx.send_notification(UntypedMessage::new(method, json!({}))?)?;
    }
    let typed_error = match cx
        .send_request(UntypedMessage::new(MATRIX_ERROR_METHOD, json!({}))?)
        .block_task()
        .await
    {
        Ok(response) => bail!("callback matrix invalid-params control succeeded: {response}"),
        Err(error) => json!({"code": error.code, "data": error.data}),
    };
    let typed_error_contract = json!({"code": -32602, "data": {"probe": "invalid-params"}});
    ensure!(
        typed_error == typed_error_contract,
        "callback matrix typed-error control changed: {typed_error}"
    );
    cx.send_request(UntypedMessage::new(MATRIX_BARRIER_METHOD, json!({}))?)
        .block_task()
        .await?;
    Ok(json!({
        "contract": callback_contract(),
        "response_count": response_count,
        "all_requests_answered": response_count == CALLBACK_REQUEST_METHODS.len(),
        "typed_errors": [typed_error],
        "typed_error_contract": typed_error_contract,
        "cancellation_count": 0,
    }))
}
type ObservedCallbackIds = (Vec<Value>, Vec<Value>, Vec<Value>, usize);

fn observed_callback_ids(wire: &[String]) -> Result<ObservedCallbackIds> {
    let mut request_ids = Vec::new();
    let mut all_request_ids = Vec::new();
    let mut all_response_ids = Vec::new();
    let mut transformed_callback_requests = 0;
    for (index, entry) in wire.iter().enumerate() {
        let (direction, raw) = entry
            .split_once(':')
            .with_context(|| format!("matrix wire entry {index} has no direction"))?;
        let value: Value = serde_json::from_str(raw)
            .with_context(|| format!("parse matrix wire entry {index} ({direction})"))?;
        let method = value.get("method").and_then(Value::as_str);
        if direction == "agent->client" && method.is_some() && value.get("id").is_some() {
            let id = value
                .get("id")
                .cloned()
                .with_context(|| format!("matrix request {index} has no id"))?;
            all_request_ids.push(id.clone());
            if method.is_some_and(|method| CALLBACK_REQUEST_METHODS.contains(&method)) {
                request_ids.push(id);
                if value
                    .pointer("/params/_meta/cyrilProbeObserved")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    transformed_callback_requests += 1;
                }
            }
        } else if direction == "client->agent"
            && method.is_none()
            && (value.get("result").is_some() || value.get("error").is_some())
        {
            all_response_ids.push(
                value
                    .get("id")
                    .cloned()
                    .with_context(|| format!("matrix response {index} has no id"))?,
            );
        }
    }
    let expected_wire_responses = CALLBACK_REQUEST_METHODS.len() + 2;
    ensure!(
        all_request_ids.len() == expected_wire_responses
            && all_response_ids.len() == expected_wire_responses,
        "matrix wire response cardinality changed: requests={} responses={}",
        all_request_ids.len(),
        all_response_ids.len()
    );
    ensure!(
        all_request_ids.iter().all(|request_id| all_request_ids
            .iter()
            .filter(|id| *id == request_id)
            .count()
            == 1)
            && all_response_ids.iter().all(|response_id| all_response_ids
                .iter()
                .filter(|id| *id == response_id)
                .count()
                == 1)
            && all_request_ids
                .iter()
                .all(|request_id| all_response_ids.contains(request_id)),
        "matrix wire request/response IDs are not unique one-to-one pairs"
    );
    let response_ids = request_ids
        .iter()
        .filter_map(|request_id| {
            all_response_ids
                .iter()
                .find(|response_id| *response_id == request_id)
                .cloned()
        })
        .collect::<Vec<_>>();
    ensure!(
        request_ids.len() == CALLBACK_REQUEST_METHODS.len()
            && response_ids.len() == CALLBACK_REQUEST_METHODS.len(),
        "matrix callback response identity count changed: requests={} responses={}",
        request_ids.len(),
        response_ids.len()
    );
    let pairs = request_ids
        .iter()
        .zip(&response_ids)
        .map(|(request_id, response_id)| {
            json!({"request_id": request_id, "response_id": response_id})
        })
        .collect();
    Ok((
        request_ids,
        response_ids,
        pairs,
        transformed_callback_requests,
    ))
}
pub async fn run_conductor_matrix<C>(
    topology: &'static str,
    conductor: C,
    auth: Value,
    done_rx: oneshot::Receiver<Result<Value, String>>,
) -> Result<Value>
where
    C: ConnectTo<Client>,
{
    let events = Events::new();
    let wire = Events::new();
    let observed_conductor = Observed {
        base: conductor,
        wire: wire.clone(),
    };
    let matrix = tokio::time::timeout(
        std::time::Duration::from_secs(MAX_SESSION_SECONDS),
        connect_client_with(events.clone(), auth, observed_conductor, async move |cx| {
            cx.send_request(
                InitializeRequest::new(ProtocolVersion::V1)
                    .client_info(Implementation::new("cyril", "0")),
            )
            .block_task()
            .await?;
            done_rx
                .await
                .map_err(|_| Error::internal_error().data("matrix completion channel closed"))?
                .map_err(|error| Error::internal_error().data(error))
        }),
    )
    .await
    .with_context(|| format!("{topology} callback matrix timed out"))?
    .map_err(|error| anyhow!("{topology} callback matrix failed: {error:?}"))?;
    let mut observed = events.snapshot();
    if observed
        .last()
        .is_some_and(|event| event == "error:event-limit")
    {
        bail!("{topology} callback matrix exceeded the 1000-event bound");
    }
    ensure!(
        normalize_events(&observed).last().map(String::as_str)
            == Some("request:_kiro/probe/barrier"),
        "{topology} callback matrix completion barrier was not observed"
    );
    let barrier = observed.pop();
    ensure!(
        barrier
            .as_deref()
            .is_some_and(|event| event.starts_with("request:_kiro/probe/barrier:id=")),
        "{topology} callback matrix completion barrier changed shape"
    );
    let error_control = observed.pop();
    ensure!(
        error_control
            .as_deref()
            .is_some_and(|event| event.starts_with("request:_kiro/probe/invalid-params:id=")),
        "{topology} callback matrix typed-error control changed shape"
    );
    let (request_ids, response_ids, response_id_pairs, transformed_callback_requests) =
        observed_callback_ids(&wire.snapshot())?;
    let mut result = matrix;
    if let Some(object) = result.as_object_mut() {
        object.insert("topology".to_owned(), Value::String(topology.to_owned()));
        object.insert("request_ids".to_owned(), json!(request_ids));
        object.insert("response_ids".to_owned(), json!(response_ids));
        object.insert("response_id_pairs".to_owned(), json!(response_id_pairs));
        object.insert(
            "transformed_callback_requests".to_owned(),
            json!(transformed_callback_requests),
        );
        object.insert("observed_events".to_owned(), json!(observed));
        object.insert(
            "normalized_events".to_owned(),
            json!(normalize_events(&observed)),
        );
        object.insert("method_counts".to_owned(), json!(method_counts(&observed)));
        object.insert("event_count".to_owned(), json!(observed.len()));
        object.insert(
            "within_event_bound".to_owned(),
            Value::Bool(observed.len() <= MAX_EVENTS),
        );
    }
    Ok(result)
}

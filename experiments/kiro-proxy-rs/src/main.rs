//! Logging proxy between Kiro's v2 TUI (bun + tui.js) and kiro-cli-chat.
//!
//! Drop-in replacement for `kiro-cli-chat acp` — bun spawns this binary via
//! KIRO_AGENT_PATH, we spawn the real backend as a subprocess (via
//! [`sacp_tokio::AcpAgent`]), and forward bytes bidirectionally while logging
//! every newline-delimited JSON-RPC message to a JSONL file.
//!
//! ## What's sacp-integrated
//!
//! - **Phase B**: Messages are parsed into [`sacp::UntypedMessage`] and
//!   categorized by JSON-RPC envelope fields (request/notification/response).
//!   For a handful of well-known methods we also attempt typed-param
//!   deserialization into the specific [`sacp::schema`] struct.
//! - **Phase C**: The real backend is spawned via [`sacp_tokio::AcpAgent`]
//!   instead of a raw `tokio::process::Command`. This routes subprocess
//!   spawning through sacp-tokio's `McpServer::Stdio` config type.
//!
//! ## Optional: mid-turn `/context add` injection (for testing)
//!
//! Set `KIRO_PROXY_INJECT_AFTER_CHUNKS=N` and `KIRO_PROXY_INJECT_CONTEXT_PATH=/path`
//! to make the proxy fire a synthetic `_kiro.dev/commands/execute` for
//! `context add <path>` after the Nth `session/update` notification flows
//! through. Used to test whether the backend accepts mid-turn context
//! additions and whether they affect the in-flight turn's output.
//!
//! ## Usage
//!
//! Bun overwrites `KIRO_AGENT_PATH` when spawned via `kiro-cli chat --tui`,
//! so you must bypass kiro-cli entirely:
//!
//! ```text
//! KIRO_AGENT_PATH=/path/to/kiro-proxy-rs \
//!     ~/.local/share/kiro-cli/bun \
//!     ~/.local/share/kiro-cli/tui.js chat --tui
//! ```
//!
//! Configuration via env vars:
//! - `KIRO_PROXY_REAL_BACKEND` — real kiro-cli-chat path. Default:
//!   `$HOME/.local/bin/kiro-cli-chat`.
//! - `KIRO_PROXY_LOG` — JSONL log destination. Default:
//!   `/tmp/kiro-proxy-poc/messages-rs.jsonl`.
//! - `KIRO_PROXY_INJECT_AFTER_CHUNKS` — (optional) number of agent→client
//!   `session/update` notifications to count before firing an injection.
//! - `KIRO_PROXY_INJECT_CONTEXT_PATH` — (optional) path to add via `/context add`
//!   when the chunk counter reaches the trigger.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use sacp::UntypedMessage;
use sacp::schema::{InitializeRequest, NewSessionRequest, PromptRequest};
use sacp_tokio::AcpAgent;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;

/// Default log destination if `KIRO_PROXY_LOG` isn't set.
const DEFAULT_LOG_PATH: &str = "/tmp/kiro-proxy-poc/messages-rs.jsonl";

/// Default real-backend path if `KIRO_PROXY_REAL_BACKEND` isn't set.
///
/// Returns an error if `HOME` is unset rather than silently falling back to
/// a root-relative path that would mask the misconfiguration downstream.
fn default_real_backend() -> Result<PathBuf> {
    let home = std::env::var("HOME").context(
        "HOME is not set; set KIRO_PROXY_REAL_BACKEND explicitly to the kiro-cli-chat path",
    )?;
    Ok(PathBuf::from(home).join(".local/bin/kiro-cli-chat"))
}

/// Read ppid by parsing /proc/self/status. Linux-only; returns None elsewhere.
fn read_ppid() -> Option<u32> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("PPid:\t").and_then(|v| v.trim().parse().ok()))
}

fn now_secs() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64 / 1000.0
}

/// The JSON-RPC envelope category, derived from which fields are present.
#[derive(Debug, Clone, Copy)]
enum Envelope {
    Request,
    Notification,
    Response,
    Unknown,
}

impl Envelope {
    fn as_str(self) -> &'static str {
        match self {
            Envelope::Request => "request",
            Envelope::Notification => "notification",
            Envelope::Response => "response",
            Envelope::Unknown => "unknown",
        }
    }
}

fn classify(value: &serde_json::Value) -> Envelope {
    let has_method = value.get("method").is_some();
    let has_id = value.get("id").is_some();
    let has_result_or_error = value.get("result").is_some() || value.get("error").is_some();
    match (has_method, has_id, has_result_or_error) {
        (true, true, _) => Envelope::Request,
        (true, false, _) => Envelope::Notification,
        (false, true, true) => Envelope::Response,
        _ => Envelope::Unknown,
    }
}

fn try_untyped(value: &serde_json::Value) -> Option<UntypedMessage> {
    let method = value.get("method")?.as_str()?;
    let params = value.get("params").cloned().unwrap_or(serde_json::Value::Null);
    UntypedMessage::new(method, params).ok()
}

fn typed_variant_hint(method: &str, params: &serde_json::Value) -> Option<&'static str> {
    match method {
        "initialize" => serde_json::from_value::<InitializeRequest>(params.clone())
            .ok()
            .map(|_| "InitializeRequest"),
        "session/new" => serde_json::from_value::<NewSessionRequest>(params.clone())
            .ok()
            .map(|_| "NewSessionRequest"),
        "session/prompt" => serde_json::from_value::<PromptRequest>(params.clone())
            .ok()
            .map(|_| "PromptRequest"),
        _ => None,
    }
}

/// Proxy-side injection state. Enabled by setting both
/// `KIRO_PROXY_INJECT_AFTER_CHUNKS` and `KIRO_PROXY_INJECT_CONTEXT_PATH`.
/// Drives the mid-turn `/context add` test: counts `session/update`
/// notifications flowing agent→client, fires a synthetic
/// `_kiro.dev/commands/execute` when the counter reaches the trigger,
/// and drops the resulting response so tui.js doesn't see an unsolicited id.
struct InjectState {
    /// Trigger after this many `session/update` notifications. 0 = disabled.
    after_chunks: usize,
    /// Trigger on the first `session/update` with `sessionUpdate == "tool_call"`.
    /// When set, fires BEFORE LLM call 2 in a multi-inference turn.
    on_tool_call: bool,
    context_add_path: String,
    chunks_seen: AtomicUsize,
    session_id: Mutex<Option<String>>,
    injected_ids: Mutex<HashSet<u64>>,
    triggered: AtomicUsize,
    /// Set by `observe()` when a trigger condition is met. Polled by
    /// `run_injector()`.
    ready_to_fire: AtomicBool,
    /// Event-driven wake-up for the injector task. `notify_one()` when a
    /// trigger fires so the injector wakes immediately instead of polling.
    notify: Arc<tokio::sync::Notify>,
}

impl InjectState {
    fn from_env() -> Option<Arc<Self>> {
        let context_add_path = std::env::var("KIRO_PROXY_INJECT_CONTEXT_PATH").ok()?;
        let after_chunks: usize = std::env::var("KIRO_PROXY_INJECT_AFTER_CHUNKS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let on_tool_call = matches!(
            std::env::var("KIRO_PROXY_INJECT_ON_TOOL_CALL").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        );
        // At least one trigger must be configured.
        if after_chunks == 0 && !on_tool_call {
            return None;
        }
        Some(Arc::new(Self {
            after_chunks,
            on_tool_call,
            context_add_path,
            chunks_seen: AtomicUsize::new(0),
            session_id: Mutex::new(None),
            injected_ids: Mutex::new(HashSet::new()),
            triggered: AtomicUsize::new(0),
            ready_to_fire: AtomicBool::new(false),
            notify: Arc::new(tokio::sync::Notify::new()),
        }))
    }

    /// Called on every agent→client message. Captures the sessionId if this
    /// is a `session/new` response, and increments the chunk counter if it's
    /// a `session/update` notification.
    fn observe(&self, parsed: &serde_json::Value) {
        // Capture sessionId from the first session/new response.
        if let Some(sid) = parsed
            .get("result")
            .and_then(|r| r.get("sessionId"))
            .and_then(|v| v.as_str())
        {
            let mut slot = self.session_id.lock().unwrap();
            if slot.is_none() {
                *slot = Some(sid.to_string());
            }
        }

        // Only track session/update notifications for trigger purposes.
        if parsed.get("method").and_then(|m| m.as_str()) != Some("session/update") {
            return;
        }

        let update = parsed
            .get("params")
            .and_then(|p| p.get("update"));
        let variant = update
            .and_then(|u| u.get("sessionUpdate"))
            .and_then(|v| v.as_str());

        // Chunk-count trigger: every session/update counts, regardless of variant.
        let count = self.chunks_seen.fetch_add(1, Ordering::SeqCst) + 1;
        if self.after_chunks > 0 && count >= self.after_chunks {
            self.ready_to_fire.store(true, Ordering::SeqCst);
            self.notify.notify_one();
        }

        // Tool-call trigger: fire on the first session/update with
        // sessionUpdate == "tool_call". This lands in the window between
        // the agent's LLM call 1 (which emitted the tool_call) and LLM
        // call 2 (which processes the tool result). If Kiro rebuilds
        // context per inference, the new file will be in LLM 2's context.
        if self.on_tool_call && variant == Some("tool_call") {
            self.ready_to_fire.store(true, Ordering::SeqCst);
            self.notify.notify_one();
        }
    }

    /// Should we drop this agent→client message from the forwarded stream?
    /// True for responses whose id matches one we injected — those responses
    /// don't correspond to any tui.js request, so forwarding them would
    /// confuse the client.
    fn should_drop(&self, parsed: &serde_json::Value) -> bool {
        if let Some(id) = parsed.get("id").and_then(|v| v.as_u64()) {
            self.injected_ids.lock().unwrap().contains(&id)
        } else {
            false
        }
    }
}

fn build_context_add_request(session_id: &str, path: &str, id: u64) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "_kiro.dev/commands/execute",
        "params": {
            "command": {
                "command": "context",
                "args": { "value": format!("add {path}") }
            },
            "sessionId": session_id
        }
    })
}

struct Logger {
    file: Mutex<std::fs::File>,
}

impl Logger {
    fn new(path: &str) -> Result<Self> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening log {path}"))?;
        Ok(Self { file: Mutex::new(file) })
    }

    fn log_line(&self, direction: &str, line: &str) {
        let entry = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(parsed) => {
                let envelope = classify(&parsed);
                let untyped = try_untyped(&parsed);
                let method = untyped.as_ref().map(|m| m.method.clone());
                let typed = method.as_deref().and_then(|m| {
                    parsed.get("params").and_then(|p| typed_variant_hint(m, p))
                });
                let id = parsed.get("id").cloned();
                serde_json::json!({
                    "time": now_secs(),
                    "direction": direction,
                    "envelope": envelope.as_str(),
                    "method": method,
                    "id": id,
                    "typed": typed,
                    "len": line.len(),
                    "parsed": parsed,
                })
            }
            Err(_) => serde_json::json!({
                "time": now_secs(),
                "direction": direction,
                "envelope": "unknown",
                "len": line.len(),
                "raw": line,
            }),
        };
        self.write_entry(entry);
    }

    fn log_event(&self, event: &str, payload: serde_json::Value) {
        let entry = serde_json::json!({
            "time": now_secs(),
            "event": event,
            "payload": payload,
        });
        self.write_entry(entry);
    }

    fn write_entry(&self, entry: serde_json::Value) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{entry}");
        }
    }
}

/// Generic line-oriented forwarder with no state tracking. Used for the
/// client→agent direction, where we don't need to observe payloads beyond
/// logging.
async fn forward<R, W>(
    logger: Arc<Logger>,
    direction: &'static str,
    src: R,
    mut dst: W,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(src);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            logger.log_event(&format!("{direction}_eof"), serde_json::json!({}));
            return Ok(());
        }
        let trimmed = line.trim_end_matches('\n');
        logger.log_line(direction, trimmed);
        dst.write_all(line.as_bytes()).await?;
        dst.flush().await?;
    }
}

/// Agent→client forwarder with InjectState hooks. Observes payloads to
/// maintain session_id + chunk-count state, and drops responses that
/// correspond to injected request ids.
async fn forward_agent_to_client<R, W>(
    logger: Arc<Logger>,
    inject: Option<Arc<InjectState>>,
    src: R,
    mut dst: W,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(src);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            logger.log_event("agent_to_client_eof", serde_json::json!({}));
            return Ok(());
        }
        let trimmed = line.trim_end_matches('\n');

        let parsed = serde_json::from_str::<serde_json::Value>(trimmed).ok();
        let mut drop_this = false;
        if let (Some(parsed), Some(inject)) = (parsed.as_ref(), inject.as_ref()) {
            inject.observe(parsed);
            if inject.should_drop(parsed) {
                drop_this = true;
                logger.log_event(
                    "dropped_injected_response",
                    serde_json::json!({ "id": parsed.get("id") }),
                );
            }
        }

        logger.log_line("agent_to_client", trimmed);

        if !drop_this {
            if let Err(e) = dst.write_all(line.as_bytes()).await {
                logger.log_event(
                    "agent_to_client_write_err",
                    serde_json::json!({ "error": e.to_string() }),
                );
                return Err(e.into());
            }
            if let Err(e) = dst.flush().await {
                logger.log_event(
                    "agent_to_client_flush_err",
                    serde_json::json!({ "error": e.to_string() }),
                );
                return Err(e.into());
            }
        }
    }
}

/// Client→agent forwarder that writes through a shared Arc<Mutex<ChildStdin>>
/// so the injector task can share the same child stdin handle.
async fn forward_client_to_agent(
    logger: Arc<Logger>,
    dst: Arc<tokio::sync::Mutex<ChildStdin>>,
) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            logger.log_event("client_to_agent_eof", serde_json::json!({}));
            return Ok(());
        }
        let trimmed = line.trim_end_matches('\n');
        logger.log_line("client_to_agent", trimmed);

        let mut w = dst.lock().await;
        if let Err(e) = w.write_all(line.as_bytes()).await {
            logger.log_event(
                "client_to_agent_write_err",
                serde_json::json!({ "error": e.to_string() }),
            );
            return Err(e.into());
        }
        if let Err(e) = w.flush().await {
            logger.log_event(
                "client_to_agent_flush_err",
                serde_json::json!({ "error": e.to_string() }),
            );
            return Err(e.into());
        }
    }
}

async fn forward_stderr<R>(
    logger: Arc<Logger>,
    src: R,
    mut dst: tokio::io::Stderr,
) -> Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(src);
    let mut buf = vec![0u8; 4096];
    loop {
        let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await?;
        if n == 0 {
            logger.log_event("agent_stderr_eof", serde_json::json!({}));
            return Ok(());
        }
        logger.log_event(
            "agent_stderr",
            serde_json::json!({
                "bytes": n,
                "preview": String::from_utf8_lossy(&buf[..n.min(200)]).to_string(),
            }),
        );
        dst.write_all(&buf[..n]).await?;
        dst.flush().await?;
    }
}

/// Polls the chunk counter; fires a synthetic `/context add` once it hits
/// the trigger. Exits after firing (one-shot). Uses 100ms poll interval —
/// much coarser than chunk arrival rate, so precise timing isn't guaranteed,
/// but "after N chunks" is close enough for this test.
async fn run_injector(
    logger: Arc<Logger>,
    inject: Arc<InjectState>,
    child_stdin: Arc<tokio::sync::Mutex<ChildStdin>>,
) {
    // Wait event-driven for a trigger, not by polling. Observed latency from
    // trigger-to-fire was ~25ms with 50ms polling, which lost the race
    // against Kiro's tool-call → LLM2-invocation window (< 10ms for fast
    // tools like fs_read). notify_one() makes the wake-up essentially
    // instantaneous — the injector fires on the same tokio tick as the
    // forwarder's observe() call.
    inject.notify.notified().await;

    // Defensive: the trigger flag should already be set, but re-check.
    if !inject.ready_to_fire.load(Ordering::SeqCst) {
        return;
    }
    // Prevent multiple firings with a CAS-style swap
    if inject.triggered.swap(1, Ordering::SeqCst) != 0 {
        return;
    }
    let count = inject.chunks_seen.load(Ordering::SeqCst);

        let sid = { inject.session_id.lock().unwrap().clone() };
        let Some(sid) = sid else {
            logger.log_event(
                "inject_aborted_no_session_id",
                serde_json::json!({ "chunks_seen": count }),
            );
            return;
        };

        // Pick an id far above any the client would use.
        let id: u64 = 99101;
        inject.injected_ids.lock().unwrap().insert(id);

        let req = build_context_add_request(&sid, &inject.context_add_path, id);
        let mut bytes = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => {
                logger.log_event(
                    "inject_serialize_err",
                    serde_json::json!({ "error": e.to_string() }),
                );
                return;
            }
        };
        bytes.push(b'\n');

        logger.log_event(
            "inject_context_add_fired",
            serde_json::json!({
                "id": id,
                "path": inject.context_add_path,
                "session_id": sid,
                "chunks_seen_at_fire": count,
            }),
        );

        let mut w = child_stdin.lock().await;
        if let Err(e) = w.write_all(&bytes).await {
            logger.log_event(
                "inject_write_err",
                serde_json::json!({ "error": e.to_string() }),
            );
            return;
        }
        if let Err(e) = w.flush().await {
            logger.log_event(
                "inject_flush_err",
                serde_json::json!({ "error": e.to_string() }),
            );
        }
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_path = std::env::var("KIRO_PROXY_LOG").unwrap_or_else(|_| DEFAULT_LOG_PATH.into());
    let logger = Arc::new(Logger::new(&log_path)?);

    let argv: Vec<String> = std::env::args().collect();

    logger.log_event(
        "invocation",
        serde_json::json!({
            "argv": argv,
            "pid": std::process::id(),
            "ppid": read_ppid(),
            "log_path": log_path,
            "sacp_version": sacp_version(),
        }),
    );

    let inject = InjectState::from_env();
    if let Some(ref inject) = inject {
        logger.log_event(
            "inject_config",
            serde_json::json!({
                "after_chunks": inject.after_chunks,
                "on_tool_call": inject.on_tool_call,
                "context_add_path": inject.context_add_path,
            }),
        );
    }

    let real_backend_path = match std::env::var("KIRO_PROXY_REAL_BACKEND") {
        Ok(p) => PathBuf::from(p),
        Err(_) => default_real_backend()?,
    };

    let mut spawn_args: Vec<String> = vec![real_backend_path.display().to_string()];
    spawn_args.extend(argv[1..].iter().cloned());

    let agent = AcpAgent::from_args(spawn_args.clone())
        .map_err(|e| anyhow::anyhow!("AcpAgent::from_args failed: {e}"))?;

    logger.log_event(
        "sacp_spawn_config",
        serde_json::json!({
            "server_debug": format!("{:?}", agent.server()),
            "args": spawn_args,
        }),
    );

    let (child_stdin, child_stdout, child_stderr, mut child) = agent
        .spawn_process()
        .map_err(|e| anyhow::anyhow!("sacp_tokio spawn_process failed: {e}"))?;

    logger.log_event(
        "spawned_real_backend",
        serde_json::json!({
            "path": real_backend_path.display().to_string(),
            "child_pid": child.id(),
        }),
    );

    // Share child_stdin across the client→agent forwarder AND (optionally)
    // the injector task. Arc<Mutex> is fine here — contention is near zero
    // (the forwarders write discrete JSON-RPC lines, the injector fires once).
    let child_stdin = Arc::new(tokio::sync::Mutex::new(child_stdin));

    // Three forwarding tasks (daemons).
    let l1 = logger.clone();
    let stdin_for_c2a = child_stdin.clone();
    tokio::spawn(async move {
        let _ = forward_client_to_agent(l1, stdin_for_c2a).await;
    });
    let l2 = logger.clone();
    let inject_for_a2c = inject.clone();
    tokio::spawn(async move {
        let _ = forward_agent_to_client(l2, inject_for_a2c, child_stdout, tokio::io::stdout()).await;
    });
    let l3 = logger.clone();
    tokio::spawn(async move {
        let _ = forward_stderr(l3, child_stderr, tokio::io::stderr()).await;
    });

    // Injector task, if configured.
    if let Some(inject) = inject {
        let l4 = logger.clone();
        let stdin_for_inject = child_stdin.clone();
        tokio::spawn(run_injector(l4, inject, stdin_for_inject));
    }

    // Wait for the real backend to exit and propagate its code.
    let status = child.wait().await?;
    logger.log_event(
        "proxy_exit",
        serde_json::json!({
            "child_code": status.code(),
            "child_success": status.success(),
        }),
    );

    std::process::exit(status.code().unwrap_or(1));
}

fn sacp_version() -> &'static str {
    "sacp 11.0.0 + sacp-tokio 11.0.0 + agent-client-protocol-schema 0.11"
}

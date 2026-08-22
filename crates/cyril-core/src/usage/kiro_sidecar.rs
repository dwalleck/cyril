use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use thiserror::Error;

use super::{KiroSidecarKind, UsageRecordId, UsageTool};
use crate::types::{SessionId, SessionOrigin, ToolCallId, ToolKind};

const MAX_SIDECAR_BYTES: u64 = 64 * 1024 * 1024;
const ENRICHMENT_DEADLINE: Duration = Duration::from_secs(1);
const RETRY_DELAYS: [Duration; 2] = [Duration::from_millis(250), Duration::from_millis(500)];

#[derive(Debug)]
pub struct UsageEnrichment {
    pub record_id: UsageRecordId,
    pub billed_model_id: Option<String>,
    pub tools: Vec<UsageTool>,
}

#[derive(Debug)]
pub enum UsageEnrichmentResult {
    Enriched(UsageEnrichment),
    Failed {
        record_id: UsageRecordId,
        message: String,
    },
}

#[derive(Clone)]
pub struct UsageEnrichmentHandle {
    sender: std::sync::mpsc::Sender<WorkerCommand>,
}

impl UsageEnrichmentHandle {
    pub fn session_started(
        &self,
        session_id: SessionId,
        kind: KiroSidecarKind,
        origin: SessionOrigin,
    ) {
        if let Err(error) = self.sender.send(WorkerCommand::SessionStarted {
            session_id,
            kind,
            origin,
        }) {
            tracing::warn!(error = %error, "usage enrichment worker is unavailable");
        }
    }

    pub fn enrich(&self, record_id: UsageRecordId, session_id: SessionId, kind: KiroSidecarKind) {
        if let Err(error) = self.sender.send(WorkerCommand::Enrich {
            record_id,
            session_id,
            kind,
        }) {
            tracing::warn!(error = %error, "usage enrichment worker is unavailable");
        }
    }
}

pub fn spawn_usage_enrichment_worker() -> (
    UsageEnrichmentHandle,
    tokio::sync::mpsc::UnboundedReceiver<UsageEnrichmentResult>,
) {
    spawn_usage_enrichment_worker_at(crate::kiro_agent_config::kiro_home_dir())
}

fn spawn_usage_enrichment_worker_at(
    kiro_home: Option<PathBuf>,
) -> (
    UsageEnrichmentHandle,
    tokio::sync::mpsc::UnboundedReceiver<UsageEnrichmentResult>,
) {
    let (command_tx, command_rx) = std::sync::mpsc::channel();
    let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
    if let Err(error) = std::thread::Builder::new()
        .name("cyril-usage-enrichment".to_owned())
        .spawn(move || Worker::new(kiro_home, command_rx, result_tx).run())
    {
        tracing::error!(error = %error, "usage enrichment worker thread failed to spawn");
    }
    (UsageEnrichmentHandle { sender: command_tx }, result_rx)
}

#[derive(Debug)]
enum WorkerCommand {
    SessionStarted {
        session_id: SessionId,
        kind: KiroSidecarKind,
        origin: SessionOrigin,
    },
    Enrich {
        record_id: UsageRecordId,
        session_id: SessionId,
        kind: KiroSidecarKind,
    },
}

#[derive(Debug, Clone)]
enum Cursor {
    Ready { jsonl_offset: u64 },
    FreshMissing,
    Unavailable { reason: String },
}

struct Worker {
    kiro_home: Option<PathBuf>,
    commands: std::sync::mpsc::Receiver<WorkerCommand>,
    results: tokio::sync::mpsc::UnboundedSender<UsageEnrichmentResult>,
    cursors: HashMap<(SessionId, KiroSidecarKind), Cursor>,
}

impl Worker {
    fn new(
        kiro_home: Option<PathBuf>,
        commands: std::sync::mpsc::Receiver<WorkerCommand>,
        results: tokio::sync::mpsc::UnboundedSender<UsageEnrichmentResult>,
    ) -> Self {
        Self {
            kiro_home,
            commands,
            results,
            cursors: HashMap::new(),
        }
    }

    fn run(mut self) {
        while let Ok(command) = self.commands.recv() {
            match command {
                WorkerCommand::SessionStarted {
                    session_id,
                    kind,
                    origin,
                } => self.initialize_cursor(session_id, kind, origin),
                WorkerCommand::Enrich {
                    record_id,
                    session_id,
                    kind,
                } => {
                    let result = self.enrich_with_retries(record_id, &session_id, kind);
                    let message = match result {
                        Ok(enrichment) => UsageEnrichmentResult::Enriched(enrichment),
                        Err(error) => UsageEnrichmentResult::Failed {
                            record_id,
                            message: error.to_string(),
                        },
                    };
                    if self.results.send(message).is_err() {
                        return;
                    }
                }
            }
        }
    }

    fn initialize_cursor(
        &mut self,
        session_id: SessionId,
        kind: KiroSidecarKind,
        origin: SessionOrigin,
    ) {
        let key = (session_id.clone(), kind);
        let cursor = match validate_session_id(&session_id)
            .and_then(|()| self.locate_jsonl(&session_id, kind))
        {
            Ok(path) => match std::fs::metadata(&path) {
                Ok(metadata) => Cursor::Ready {
                    jsonl_offset: metadata.len(),
                },
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && origin == SessionOrigin::Fresh =>
                {
                    Cursor::FreshMissing
                }
                Err(error) => Cursor::Unavailable {
                    reason: format!("usage sidecar baseline {}: {error}", path.display()),
                },
            },
            Err(error) if error.is_missing() && origin == SessionOrigin::Fresh => {
                Cursor::FreshMissing
            }
            Err(error) => Cursor::Unavailable {
                reason: error.to_string(),
            },
        };
        if let Cursor::Unavailable { reason } = &cursor {
            tracing::warn!(
                session_id = %session_id,
                sidecar_kind = ?kind,
                reason,
                "usage sidecar baseline is unavailable"
            );
        }
        self.cursors.insert(key, cursor);
    }

    fn enrich_with_retries(
        &mut self,
        record_id: UsageRecordId,
        session_id: &SessionId,
        kind: KiroSidecarKind,
    ) -> Result<UsageEnrichment, SidecarError> {
        let deadline = Instant::now() + ENRICHMENT_DEADLINE;
        let mut last_error = None;
        for attempt in 0..=RETRY_DELAYS.len() {
            if Instant::now() >= deadline {
                return Err(SidecarError::DeadlineExceeded);
            }
            match self.enrich_once(record_id, session_id, kind, deadline) {
                Ok(enrichment) => return Ok(enrichment),
                Err(_) if Instant::now() >= deadline => {
                    return Err(SidecarError::DeadlineExceeded);
                }
                Err(error) if error.is_retryable() => last_error = Some(error),
                Err(error) => return Err(error),
            }
            if let Some(delay) = RETRY_DELAYS.get(attempt) {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(SidecarError::DeadlineExceeded);
                }
                std::thread::sleep((*delay).min(remaining));
            }
        }
        Err(last_error.unwrap_or(SidecarError::IncompleteTurn))
    }

    fn enrich_once(
        &mut self,
        record_id: UsageRecordId,
        session_id: &SessionId,
        kind: KiroSidecarKind,
        deadline: Instant,
    ) -> Result<UsageEnrichment, SidecarError> {
        validate_session_id(session_id)?;
        let key = (session_id.clone(), kind);
        let offset = match self.cursors.get(&key) {
            Some(Cursor::Ready { jsonl_offset }) => *jsonl_offset,
            Some(Cursor::FreshMissing) => 0,
            Some(Cursor::Unavailable { reason }) => {
                return Err(SidecarError::UnsafeBaseline {
                    session_id: session_id.as_str().to_owned(),
                    reason: reason.clone(),
                });
            }
            None => {
                return Err(SidecarError::MissingCursor(session_id.as_str().to_owned()));
            }
        };
        let path = self.locate_jsonl(session_id, kind)?;
        let parsed = read_sidecar_turn(&path, offset, kind, deadline)?;
        let billed_model_id = match kind {
            KiroSidecarKind::V2 => match self.read_v2_billed_model(session_id) {
                Ok(model) => model,
                Err(error) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %error,
                        "usage billed model enrichment unavailable"
                    );
                    None
                }
            },
            KiroSidecarKind::Kas => None,
        };
        self.cursors.insert(
            key,
            Cursor::Ready {
                jsonl_offset: offset + parsed.consumed_bytes,
            },
        );
        Ok(UsageEnrichment {
            record_id,
            billed_model_id,
            tools: parsed.tools,
        })
    }

    fn locate_jsonl(
        &self,
        session_id: &SessionId,
        kind: KiroSidecarKind,
    ) -> Result<PathBuf, SidecarError> {
        let home = self.kiro_home.as_ref().ok_or(SidecarError::NoKiroHome)?;
        match kind {
            KiroSidecarKind::V2 => Ok(home
                .join("sessions/cli")
                .join(format!("{}.jsonl", session_id.as_str()))),
            KiroSidecarKind::Kas => locate_kas_session(home, session_id)
                .map(|directory| directory.join("messages.jsonl")),
        }
    }

    fn read_v2_billed_model(&self, session_id: &SessionId) -> Result<Option<String>, SidecarError> {
        let home = self.kiro_home.as_ref().ok_or(SidecarError::NoKiroHome)?;
        let path = home
            .join("sessions/cli")
            .join(format!("{}.json", session_id.as_str()));
        let raw = read_whole(&path)?;
        let value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|source| SidecarError::Json {
                path: path.clone(),
                source,
            })?;
        Ok(value
            .pointer("/session_state/rts_model_state/model_info/model_id")
            .and_then(serde_json::Value::as_str)
            .filter(|model| !model.is_empty())
            .map(str::to_owned))
    }
}

#[derive(Debug, Error)]
enum SidecarError {
    #[error("Kiro home is unavailable")]
    NoKiroHome,
    #[error("invalid Kiro session id {0:?}")]
    InvalidSessionId(String),
    #[error("locate KAS session {session_id}: {source}")]
    Locate {
        session_id: String,
        #[source]
        source: std::io::Error,
    },
    #[error("KAS session {0} is not present")]
    MissingKasSession(String),
    #[error("multiple KAS session directories match {0}")]
    AmbiguousKasSession(String),
    #[error("read usage sidecar {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("usage sidecar {path} is {size} bytes; maximum is {MAX_SIDECAR_BYTES}")]
    Oversized { path: PathBuf, size: u64 },
    #[error("usage sidecar {path} was truncated below cursor {offset} to {size} bytes")]
    Truncated {
        path: PathBuf,
        offset: u64,
        size: u64,
    },
    #[error("parse usage sidecar {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("usage sidecar {path} has malformed {field}")]
    Malformed { path: PathBuf, field: &'static str },
    #[error("usage sidecar has no complete current turn")]
    IncompleteTurn,
    #[error("usage sidecar enrichment exceeded one second")]
    DeadlineExceeded,
    #[error("usage sidecar baseline for loaded session {session_id} is unsafe: {reason}")]
    UnsafeBaseline { session_id: String, reason: String },
    #[error("usage sidecar cursor was not initialized for session {0}")]
    MissingCursor(String),
}

impl SidecarError {
    fn is_missing(&self) -> bool {
        matches!(self, Self::MissingKasSession(_))
            || matches!(
                self,
                Self::Locate { source, .. } | Self::Read { source, .. }
                    if source.kind() == std::io::ErrorKind::NotFound
            )
    }

    fn is_retryable(&self) -> bool {
        matches!(self, Self::IncompleteTurn) || self.is_missing()
    }
}

struct ParsedTurn {
    tools: Vec<UsageTool>,
    consumed_bytes: u64,
}

fn validate_session_id(session_id: &SessionId) -> Result<(), SidecarError> {
    let value = session_id.as_str();
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(SidecarError::InvalidSessionId(value.to_owned()));
    }
    Ok(())
}

fn locate_kas_session(home: &Path, session_id: &SessionId) -> Result<PathBuf, SidecarError> {
    let root = home.join("sessions");
    let entries = std::fs::read_dir(&root).map_err(|source| SidecarError::Locate {
        session_id: session_id.as_str().to_owned(),
        source,
    })?;
    let mut match_path = None;
    for entry in entries {
        let entry = entry.map_err(|source| SidecarError::Locate {
            session_id: session_id.as_str().to_owned(),
            source,
        })?;
        if entry.file_name() == "cli" {
            continue;
        }
        let candidate = entry.path().join(session_id.as_str());
        match std::fs::metadata(&candidate) {
            Ok(metadata) if metadata.is_dir() => {
                if match_path.replace(candidate).is_some() {
                    return Err(SidecarError::AmbiguousKasSession(
                        session_id.as_str().to_owned(),
                    ));
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(SidecarError::Locate {
                    session_id: session_id.as_str().to_owned(),
                    source,
                });
            }
        }
    }
    match_path.ok_or_else(|| SidecarError::MissingKasSession(session_id.as_str().to_owned()))
}

fn read_sidecar_turn(
    path: &Path,
    offset: u64,
    kind: KiroSidecarKind,
    deadline: Instant,
) -> Result<ParsedTurn, SidecarError> {
    let mut file = File::open(path).map_err(|source| SidecarError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let size = file
        .metadata()
        .map_err(|source| SidecarError::Read {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    if size > MAX_SIDECAR_BYTES {
        return Err(SidecarError::Oversized {
            path: path.to_path_buf(),
            size,
        });
    }
    if offset > size {
        return Err(SidecarError::Truncated {
            path: path.to_path_buf(),
            offset,
            size,
        });
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| SidecarError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let reader = BufReader::new(file.take(MAX_SIDECAR_BYTES + 1));
    match kind {
        KiroSidecarKind::V2 => read_v2_turn(reader, path, deadline),
        KiroSidecarKind::Kas => read_kas_turn(reader, path, deadline),
    }
}

fn read_v2_turn(
    mut reader: impl BufRead,
    path: &Path,
    deadline: Instant,
) -> Result<ParsedTurn, SidecarError> {
    let mut tools = HashMap::new();
    let mut line = Vec::new();
    let mut consumed_bytes = 0_u64;
    let mut saw_prompt = false;
    let mut complete = false;
    loop {
        ensure_deadline(deadline)?;
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|source| SidecarError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        let read = u64::try_from(read).map_err(|_| SidecarError::Oversized {
            path: path.to_path_buf(),
            size: MAX_SIDECAR_BYTES + 1,
        })?;
        let next_consumed =
            consumed_bytes
                .checked_add(read)
                .ok_or_else(|| SidecarError::Oversized {
                    path: path.to_path_buf(),
                    size: MAX_SIDECAR_BYTES + 1,
                })?;
        if next_consumed > MAX_SIDECAR_BYTES {
            return Err(SidecarError::Oversized {
                path: path.to_path_buf(),
                size: next_consumed,
            });
        }
        if !line.ends_with(b"\n") {
            break;
        }
        let value = parse_json_line(path, &line)?;
        let is_prompt = value.get("kind").and_then(serde_json::Value::as_str) == Some("Prompt");
        if is_prompt && saw_prompt {
            return Ok(ParsedTurn {
                tools: sorted_tools(tools),
                consumed_bytes,
            });
        }
        saw_prompt |= is_prompt;
        complete |= apply_v2_value(path, &value, &mut tools)?;
        consumed_bytes = next_consumed;
    }
    if complete {
        Ok(ParsedTurn {
            tools: sorted_tools(tools),
            consumed_bytes,
        })
    } else {
        Err(SidecarError::IncompleteTurn)
    }
}

fn read_kas_turn(
    mut reader: impl BufRead,
    path: &Path,
    deadline: Instant,
) -> Result<ParsedTurn, SidecarError> {
    let mut tools = HashMap::new();
    let mut line = Vec::new();
    let mut consumed_bytes = 0_u64;
    loop {
        ensure_deadline(deadline)?;
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|source| SidecarError::Read {
                path: path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }
        let read = u64::try_from(read).map_err(|_| SidecarError::Oversized {
            path: path.to_path_buf(),
            size: MAX_SIDECAR_BYTES + 1,
        })?;
        consumed_bytes =
            consumed_bytes
                .checked_add(read)
                .ok_or_else(|| SidecarError::Oversized {
                    path: path.to_path_buf(),
                    size: MAX_SIDECAR_BYTES + 1,
                })?;
        if consumed_bytes > MAX_SIDECAR_BYTES {
            return Err(SidecarError::Oversized {
                path: path.to_path_buf(),
                size: consumed_bytes,
            });
        }
        if !line.ends_with(b"\n") {
            break;
        }
        let value = parse_json_line(path, &line)?;
        if apply_kas_value(path, &value, &mut tools)? {
            return Ok(ParsedTurn {
                tools: sorted_tools(tools),
                consumed_bytes,
            });
        }
    }
    Err(SidecarError::IncompleteTurn)
}

fn ensure_deadline(deadline: Instant) -> Result<(), SidecarError> {
    if Instant::now() >= deadline {
        Err(SidecarError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

fn parse_json_line(path: &Path, line: &[u8]) -> Result<serde_json::Value, SidecarError> {
    let mut trimmed = line;
    while let Some((last, rest)) = trimmed.split_last()
        && matches!(last, b'\n' | b'\r')
    {
        trimmed = rest;
    }
    serde_json::from_slice(trimmed).map_err(|source| SidecarError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_whole(path: &Path) -> Result<String, SidecarError> {
    let file = File::open(path).map_err(|source| SidecarError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut raw = String::new();
    file.take(MAX_SIDECAR_BYTES + 1)
        .read_to_string(&mut raw)
        .map_err(|source| SidecarError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let size = u64::try_from(raw.len()).map_err(|_| SidecarError::Oversized {
        path: path.to_path_buf(),
        size: MAX_SIDECAR_BYTES + 1,
    })?;
    if size > MAX_SIDECAR_BYTES {
        return Err(SidecarError::Oversized {
            path: path.to_path_buf(),
            size,
        });
    }
    Ok(raw)
}

#[derive(Debug)]
struct ParsedTool {
    call_id: ToolCallId,
    name: String,
    kind: ToolKind,
    failed: bool,
    argument_chars: Option<u64>,
    result_chars: Option<u64>,
}

impl ParsedTool {
    fn into_usage_tool(self) -> UsageTool {
        UsageTool::enriched(
            self.call_id,
            self.name,
            self.kind,
            self.failed,
            self.argument_chars,
            self.result_chars,
        )
    }
}

#[cfg(test)]
fn parse_v2_jsonl(raw: &str) -> Result<Vec<UsageTool>, SidecarError> {
    let path = Path::new("v2 current-session JSONL");
    let mut tools = HashMap::new();
    let mut complete = false;
    for line in raw.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|source| SidecarError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        complete |= apply_v2_value(path, &value, &mut tools)?;
    }
    if complete {
        Ok(sorted_tools(tools))
    } else {
        Err(SidecarError::IncompleteTurn)
    }
}

fn apply_v2_value(
    path: &Path,
    value: &serde_json::Value,
    tools: &mut HashMap<String, ParsedTool>,
) -> Result<bool, SidecarError> {
    match value.get("kind").and_then(serde_json::Value::as_str) {
        Some("AssistantMessage") => {
            if let Some(content) = value
                .pointer("/data/content")
                .and_then(serde_json::Value::as_array)
            {
                for item in content {
                    if item.get("kind").and_then(serde_json::Value::as_str) != Some("toolUse") {
                        continue;
                    }
                    let data = item.get("data").ok_or_else(|| SidecarError::Malformed {
                        path: path.to_path_buf(),
                        field: "v2 toolUse data",
                    })?;
                    let id = data
                        .get("toolUseId")
                        .and_then(serde_json::Value::as_str)
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| SidecarError::Malformed {
                            path: path.to_path_buf(),
                            field: "v2 toolUseId",
                        })?;
                    let name = data
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .filter(|name| !name.is_empty())
                        .ok_or_else(|| SidecarError::Malformed {
                            path: path.to_path_buf(),
                            field: "v2 tool name",
                        })?;
                    tools.insert(
                        id.to_owned(),
                        ParsedTool {
                            call_id: ToolCallId::new(id),
                            name: name.to_owned(),
                            kind: tool_kind(name),
                            failed: false,
                            argument_chars: data.get("input").and_then(value_chars),
                            result_chars: None,
                        },
                    );
                }
            }
            Ok(true)
        }
        Some("ToolResults") => {
            if let Some(content) = value
                .pointer("/data/content")
                .and_then(serde_json::Value::as_array)
            {
                for item in content {
                    if item.get("kind").and_then(serde_json::Value::as_str) != Some("toolResult") {
                        continue;
                    }
                    let data = item.get("data").ok_or_else(|| SidecarError::Malformed {
                        path: path.to_path_buf(),
                        field: "v2 toolResult data",
                    })?;
                    let id = data
                        .get("toolUseId")
                        .and_then(serde_json::Value::as_str)
                        .filter(|id| !id.is_empty())
                        .ok_or_else(|| SidecarError::Malformed {
                            path: path.to_path_buf(),
                            field: "v2 toolResult id",
                        })?;
                    if let Some(tool) = tools.get_mut(id) {
                        tool.failed |=
                            data.get("status").and_then(serde_json::Value::as_str) == Some("error");
                        if let Some(result_chars) = data.get("content").and_then(value_chars) {
                            tool.result_chars = Some(result_chars);
                        }
                    }
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
fn parse_kas_jsonl(raw: &str) -> Result<Vec<UsageTool>, SidecarError> {
    let path = Path::new("KAS current-session JSONL");
    let mut tools = HashMap::new();
    let mut complete = false;
    for line in raw.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|source| SidecarError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        complete |= apply_kas_value(path, &value, &mut tools)?;
    }
    if complete {
        Ok(sorted_tools(tools))
    } else {
        Err(SidecarError::IncompleteTurn)
    }
}

fn apply_kas_value(
    path: &Path,
    value: &serde_json::Value,
    tools: &mut HashMap<String, ParsedTool>,
) -> Result<bool, SidecarError> {
    let Some(payload) = value.get("payload") else {
        return Ok(false);
    };
    match payload.get("type").and_then(serde_json::Value::as_str) {
        Some("tool_call") => {
            let id = payload
                .get("toolCallId")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| SidecarError::Malformed {
                    path: path.to_path_buf(),
                    field: "KAS toolCallId",
                })?;
            let name = payload
                .get("toolName")
                .and_then(serde_json::Value::as_str)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| SidecarError::Malformed {
                    path: path.to_path_buf(),
                    field: "KAS toolName",
                })?;
            let kind = payload
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| tool_kind(name), tool_kind);
            tools.insert(
                id.to_owned(),
                ParsedTool {
                    call_id: ToolCallId::new(id),
                    name: name.to_owned(),
                    kind,
                    failed: matches!(
                        payload.get("status").and_then(serde_json::Value::as_str),
                        Some("failed" | "denied")
                    ),
                    argument_chars: payload.get("args").and_then(value_chars),
                    result_chars: None,
                },
            );
            Ok(false)
        }
        Some("tool_result") => {
            let id = payload
                .get("toolCallId")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| SidecarError::Malformed {
                    path: path.to_path_buf(),
                    field: "KAS tool result id",
                })?;
            if let Some(tool) = tools.get_mut(id) {
                tool.failed |=
                    payload.get("success").and_then(serde_json::Value::as_bool) == Some(false);
                if let Some(result_chars) = payload.get("content").and_then(value_chars) {
                    tool.result_chars = Some(result_chars);
                }
            }
            Ok(false)
        }
        Some("usage_summary") => Ok(true),
        _ => Ok(false),
    }
}

fn sorted_tools(tools: HashMap<String, ParsedTool>) -> Vec<UsageTool> {
    let mut tools = tools.into_values().collect::<Vec<_>>();
    tools.sort_by(|left, right| left.call_id.as_str().cmp(right.call_id.as_str()));
    tools.into_iter().map(ParsedTool::into_usage_tool).collect()
}

fn value_chars(value: &serde_json::Value) -> Option<u64> {
    let count = match value {
        serde_json::Value::String(value) => value.chars().count(),
        value => match serde_json::to_string(value) {
            Ok(encoded) => encoded.chars().count(),
            Err(error) => {
                tracing::warn!(error = %error, "usage sidecar value serialization failed");
                return None;
            }
        },
    };
    match u64::try_from(count) {
        Ok(count) => Some(count),
        Err(error) => {
            tracing::warn!(error = %error, "usage sidecar character count exceeds u64");
            None
        }
    }
}

fn tool_kind(value: &str) -> ToolKind {
    match value {
        "read" | "read_file" | "readFile" => ToolKind::Read,
        "write" | "edit" | "fs_write" | "write_file" => ToolKind::Write,
        "execute" | "execute_bash" | "run_command" => ToolKind::Execute,
        "search" | "file_search" | "grep_search" => ToolKind::Search,
        "fetch" | "web_search" => ToolKind::Fetch,
        "think" => ToolKind::Think,
        "switch_mode" => ToolKind::SwitchMode,
        _ => ToolKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parsers_preserve_exact_tools_and_unicode_character_counts() -> TestResult {
        let v2 = concat!(
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[{\"kind\":\"toolUse\",\"data\":{\"toolUseId\":\"b\",\"name\":\"read\",\"input\":{\"text\":\"é\"}}}]}}\n",
            "{\"kind\":\"ToolResults\",\"data\":{\"content\":[{\"kind\":\"toolResult\",\"data\":{\"toolUseId\":\"b\",\"content\":\"日本\",\"status\":\"error\"}}]}}\n"
        );
        let tools = parse_v2_jsonl(v2)?;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].call_id().map(ToolCallId::as_str), Some("b"));
        assert_eq!(tools[0].name(), Some("read"));
        assert_eq!(tools[0].kind(), ToolKind::Read);
        assert!(tools[0].failed());
        assert_eq!(tools[0].argument_chars(), Some(12));
        assert_eq!(tools[0].result_chars(), Some(2));

        let kas = concat!(
            "{\"payload\":{\"type\":\"tool_call\",\"toolCallId\":\"a\",\"toolName\":\"execute_bash\",\"kind\":\"execute\",\"status\":\"completed\",\"args\":{\"x\":1}}}\n",
            "{\"payload\":{\"type\":\"tool_result\",\"toolCallId\":\"a\",\"content\":\"ok\",\"success\":true}}\n",
            "{\"payload\":{\"type\":\"usage_summary\"}}\n"
        );
        let tools = parse_kas_jsonl(kas)?;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), Some("execute_bash"));
        assert_eq!(tools[0].kind(), ToolKind::Execute);
        assert_eq!(tools[0].argument_chars(), Some(7));
        assert_eq!(tools[0].result_chars(), Some(2));
        Ok(())
    }
    #[test]
    fn streamed_turn_reader_preserves_v2_and_kas_boundaries() -> TestResult {
        let directory = tempfile::tempdir()?;
        let v2_path = directory.path().join("v2.jsonl");
        let prompt_a = "{\"kind\":\"Prompt\",\"data\":{\"content\":\"a\"}}\n";
        let assistant_a = "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[{\"kind\":\"toolUse\",\"data\":{\"toolUseId\":\"a\",\"name\":\"read\",\"input\":{}}}]}}\n";
        let prompt_b = "{\"kind\":\"Prompt\",\"data\":{\"content\":\"b\"}}\n";
        let assistant_b = "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[{\"kind\":\"toolUse\",\"data\":{\"toolUseId\":\"b\",\"name\":\"write\",\"input\":{}}}]}}\n";
        let raw = format!("{prompt_a}{assistant_a}{prompt_b}{assistant_b}");
        std::fs::write(&v2_path, &raw)?;
        let deadline = Instant::now() + ENRICHMENT_DEADLINE;
        let first = read_sidecar_turn(&v2_path, 0, KiroSidecarKind::V2, deadline)?;
        assert_eq!(first.tools.len(), 1);
        assert_eq!(first.tools[0].call_id().map(ToolCallId::as_str), Some("a"));
        assert_eq!(
            first.consumed_bytes,
            u64::try_from(prompt_a.len() + assistant_a.len())?
        );
        let second = read_sidecar_turn(
            &v2_path,
            first.consumed_bytes,
            KiroSidecarKind::V2,
            deadline,
        )?;
        assert_eq!(second.tools.len(), 1);
        assert_eq!(second.tools[0].call_id().map(ToolCallId::as_str), Some("b"));

        let partial_prefix = "{\"kind\":\"AssistantMessage\",\"data\":";
        std::fs::write(&v2_path, format!("{prompt_a}{prompt_b}{partial_prefix}"))?;
        let first = read_sidecar_turn(&v2_path, 0, KiroSidecarKind::V2, deadline)?;
        assert!(first.tools.is_empty());
        assert_eq!(first.consumed_bytes, u64::try_from(prompt_a.len())?);
        assert!(matches!(
            read_sidecar_turn(
                &v2_path,
                first.consumed_bytes,
                KiroSidecarKind::V2,
                deadline,
            ),
            Err(SidecarError::IncompleteTurn)
        ));
        use std::io::Write as _;
        let mut partial = std::fs::OpenOptions::new().append(true).open(&v2_path)?;
        partial.write_all(b"{\"content\":[]}}\n")?;
        let completed = read_sidecar_turn(
            &v2_path,
            first.consumed_bytes,
            KiroSidecarKind::V2,
            deadline,
        )?;
        assert!(completed.tools.is_empty());

        let kas_path = directory.path().join("kas.jsonl");
        let summary_a = "{\"payload\":{\"type\":\"usage_summary\"}}\n";
        let call_b = "{\"payload\":{\"type\":\"tool_call\",\"toolCallId\":\"b\",\"toolName\":\"execute_bash\",\"args\":{}}}\n";
        let summary_b = "{\"payload\":{\"type\":\"usage_summary\"}}\n";
        std::fs::write(&kas_path, format!("{summary_a}{call_b}{summary_b}"))?;
        let first = read_sidecar_turn(&kas_path, 0, KiroSidecarKind::Kas, deadline)?;
        assert!(first.tools.is_empty());
        assert_eq!(first.consumed_bytes, u64::try_from(summary_a.len())?);
        let second = read_sidecar_turn(
            &kas_path,
            first.consumed_bytes,
            KiroSidecarKind::Kas,
            deadline,
        )?;
        assert_eq!(second.tools.len(), 1);
        assert_eq!(second.tools[0].call_id().map(ToolCallId::as_str), Some("b"));
        Ok(())
    }

    #[tokio::test]
    async fn sidecar_baseline_rejects_loaded_missing_and_invalid_sessions() -> TestResult {
        let directory = tempfile::tempdir()?;
        let home = directory.path().join(".kiro");
        let cli = home.join("sessions/cli");
        std::fs::create_dir_all(&cli)?;
        let (handle, mut receiver) = spawn_usage_enrichment_worker_at(Some(home.clone()));

        let loaded = SessionId::new("loaded-missing");
        handle.session_started(loaded.clone(), KiroSidecarKind::V2, SessionOrigin::Loaded);
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(
            cli.join("loaded-missing.jsonl"),
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[]}}\n",
        )?;
        handle.enrich(UsageRecordId::new(1), loaded, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        let UsageEnrichmentResult::Failed { message, .. } = result else {
            return Err("loaded missing sidecar must fail closed".into());
        };
        assert!(message.contains("baseline"));

        let fresh = SessionId::new("fresh-missing");
        handle.session_started(fresh.clone(), KiroSidecarKind::V2, SessionOrigin::Fresh);
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(
            cli.join("fresh-missing.jsonl"),
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[]}}\n",
        )?;
        handle.enrich(UsageRecordId::new(2), fresh, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        assert!(matches!(result, UsageEnrichmentResult::Enriched(_)));

        let invalid = SessionId::new("../escape");
        handle.session_started(invalid.clone(), KiroSidecarKind::V2, SessionOrigin::Fresh);
        handle.enrich(UsageRecordId::new(3), invalid, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        let UsageEnrichmentResult::Failed { message, .. } = result else {
            return Err("invalid session must fail".into());
        };
        assert!(message.contains("invalid Kiro session id"));
        Ok(())
    }

    #[tokio::test]
    async fn bounded_current_turn_enrichment_matrix() -> TestResult {
        let directory = tempfile::tempdir()?;
        let home = directory.path().join(".kiro");
        let cli = home.join("sessions/cli");
        std::fs::create_dir_all(&cli)?;
        let session = SessionId::new("session-1");
        let jsonl = cli.join("session-1.jsonl");
        std::fs::write(
            &jsonl,
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[{\"kind\":\"toolUse\",\"data\":{\"toolUseId\":\"old\",\"name\":\"read\",\"input\":{}}}]}}\n",
        )?;
        std::fs::write(
            cli.join("session-1.json"),
            r#"{"session_state":{"rts_model_state":{"model_info":{"model_id":"anthropic/claude-sonnet"}}}}"#,
        )?;
        let (handle, mut receiver) = spawn_usage_enrichment_worker_at(Some(home.clone()));
        handle.session_started(session.clone(), KiroSidecarKind::V2, SessionOrigin::Fresh);
        std::thread::sleep(Duration::from_millis(10));
        let mut file = std::fs::OpenOptions::new().append(true).open(&jsonl)?;
        use std::io::Write as _;
        file.write_all(
            concat!(
                r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"toolUse","data":{"toolUseId":"new","name":"write","input":{"text":"é"}}}]}}"#,
                "\n"
            )
            .as_bytes(),
        )?;
        file.write_all(
            concat!(
                r#"{"kind":"ToolResults","data":{"content":[{"kind":"toolResult","data":{"toolUseId":"new","content":"done","status":"success"}}]}}"#,
                "\n"
            )
            .as_bytes(),
        )?;
        handle.enrich(UsageRecordId::new(7), session, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        let UsageEnrichmentResult::Enriched(enrichment) = result else {
            return Err("expected successful enrichment".into());
        };
        assert_eq!(enrichment.record_id.get(), 7);
        assert_eq!(
            enrichment.billed_model_id.as_deref(),
            Some("anthropic/claude-sonnet")
        );
        assert_eq!(enrichment.tools.len(), 1);
        assert_eq!(
            enrichment.tools[0].call_id().map(ToolCallId::as_str),
            Some("new")
        );
        assert_eq!(enrichment.tools[0].name(), Some("write"));
        drop(file);

        let recovering = SessionId::new("session-2");
        let recovering_jsonl = cli.join("session-2.jsonl");
        std::fs::write(&recovering_jsonl, "")?;
        std::fs::write(
            cli.join("session-2.json"),
            r#"{"session_state":{"rts_model_state":{"model_info":{"model_id":"auto"}}}}"#,
        )?;
        handle.session_started(
            recovering.clone(),
            KiroSidecarKind::V2,
            SessionOrigin::Fresh,
        );
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(&recovering_jsonl, "{")?;
        handle.enrich(UsageRecordId::new(9), recovering, KiroSidecarKind::V2);
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(
            &recovering_jsonl,
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[]}}\n",
        )?;
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        assert!(matches!(result, UsageEnrichmentResult::Enriched(_)));

        let (handle, mut receiver) = spawn_usage_enrichment_worker_at(Some(home.clone()));
        let missing = SessionId::new("missing");
        handle.session_started(missing.clone(), KiroSidecarKind::V2, SessionOrigin::Fresh);
        std::thread::sleep(Duration::from_millis(10));
        let started = std::time::Instant::now();
        handle.enrich(UsageRecordId::new(8), missing, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        assert!(matches!(result, UsageEnrichmentResult::Failed { .. }));
        assert!(started.elapsed() <= Duration::from_secs(1));

        let invalid = validate_session_id(&SessionId::new("../escape"));
        assert!(matches!(invalid, Err(SidecarError::InvalidSessionId(_))));
        let malformed = cli.join("malformed.jsonl");
        std::fs::write(&malformed, "{not-json}\n")?;
        let Err(SidecarError::Json { path, .. }) = read_sidecar_turn(
            &malformed,
            0,
            KiroSidecarKind::V2,
            Instant::now() + ENRICHMENT_DEADLINE,
        ) else {
            return Err("malformed complete line must report JSON error".into());
        };
        assert_eq!(path, malformed);
        let oversized = cli.join("oversized.jsonl");
        File::create(&oversized)?.set_len(MAX_SIDECAR_BYTES + 1)?;
        assert!(matches!(
            read_sidecar_turn(
                &oversized,
                0,
                KiroSidecarKind::V2,
                Instant::now() + ENRICHMENT_DEADLINE,
            ),
            Err(SidecarError::Oversized { .. })
        ));
        Ok(())
    }
}

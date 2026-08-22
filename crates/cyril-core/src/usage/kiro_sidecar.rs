use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

use super::{KiroSidecarKind, UsageRecordId, UsageTool};
use crate::types::{SessionId, ToolCallId, ToolKind};

const MAX_SIDECAR_BYTES: u64 = 64 * 1024 * 1024;
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
    pub fn session_started(&self, session_id: SessionId, kind: KiroSidecarKind) {
        if let Err(error) = self
            .sender
            .send(WorkerCommand::SessionStarted { session_id, kind })
        {
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
    },
    Enrich {
        record_id: UsageRecordId,
        session_id: SessionId,
        kind: KiroSidecarKind,
    },
}

#[derive(Debug, Clone)]
struct Cursor {
    jsonl_offset: u64,
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
                WorkerCommand::SessionStarted { session_id, kind } => {
                    match self.locate_jsonl(&session_id, kind) {
                        Ok(path) => {
                            let offset = match std::fs::metadata(&path) {
                                Ok(metadata) => metadata.len(),
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
                                Err(error) => {
                                    tracing::warn!(
                                        path = %path.display(),
                                        error = %error,
                                        "usage sidecar metadata unavailable at session start"
                                    );
                                    0
                                }
                            };
                            self.cursors.insert(
                                (session_id, kind),
                                Cursor {
                                    jsonl_offset: offset,
                                },
                            );
                        }
                        Err(error) => {
                            tracing::debug!(error = %error, "usage sidecar absent at session start");
                            self.cursors
                                .insert((session_id, kind), Cursor { jsonl_offset: 0 });
                        }
                    }
                }
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

    fn enrich_with_retries(
        &mut self,
        record_id: UsageRecordId,
        session_id: &SessionId,
        kind: KiroSidecarKind,
    ) -> Result<UsageEnrichment, SidecarError> {
        let mut last_error = None;
        for attempt in 0..=RETRY_DELAYS.len() {
            match self.enrich_once(record_id, session_id, kind) {
                Ok(enrichment) => return Ok(enrichment),
                Err(error) => last_error = Some(error),
            }
            if let Some(delay) = RETRY_DELAYS.get(attempt) {
                std::thread::sleep(*delay);
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Err(SidecarError::IncompleteTurn),
        }
    }

    fn enrich_once(
        &mut self,
        record_id: UsageRecordId,
        session_id: &SessionId,
        kind: KiroSidecarKind,
    ) -> Result<UsageEnrichment, SidecarError> {
        validate_session_id(session_id)?;
        let path = self.locate_jsonl(session_id, kind)?;
        let key = (session_id.clone(), kind);
        let offset = self
            .cursors
            .get(&key)
            .map_or(0, |cursor| cursor.jsonl_offset);
        let (raw, end) = read_appended(&path, offset)?;
        let tools = match kind {
            KiroSidecarKind::V2 => parse_v2_jsonl(&raw)?,
            KiroSidecarKind::Kas => parse_kas_jsonl(&raw)?,
        };
        let billed_model_id = match kind {
            KiroSidecarKind::V2 => self.read_v2_billed_model(session_id)?,
            KiroSidecarKind::Kas => None,
        };
        self.cursors.insert(key, Cursor { jsonl_offset: end });
        Ok(UsageEnrichment {
            record_id,
            billed_model_id,
            tools,
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
    #[error("parse usage sidecar {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("usage sidecar has no complete current turn")]
    IncompleteTurn,
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

fn read_appended(path: &Path, offset: u64) -> Result<(String, u64), SidecarError> {
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
        return Err(SidecarError::IncompleteTurn);
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| SidecarError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .map_err(|source| SidecarError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    if raw.is_empty() {
        return Err(SidecarError::IncompleteTurn);
    }
    Ok((raw, size))
}

fn read_whole(path: &Path) -> Result<String, SidecarError> {
    let size = std::fs::metadata(path)
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
    std::fs::read_to_string(path).map_err(|source| SidecarError::Read {
        path: path.to_path_buf(),
        source,
    })
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

fn parse_v2_jsonl(raw: &str) -> Result<Vec<UsageTool>, SidecarError> {
    let mut tools: HashMap<String, ParsedTool> = HashMap::new();
    let mut complete = false;
    for line in raw.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|source| SidecarError::Json {
                path: PathBuf::from("v2 current-session JSONL"),
                source,
            })?;
        match value.get("kind").and_then(serde_json::Value::as_str) {
            Some("AssistantMessage") => {
                complete = true;
                if let Some(content) = value
                    .pointer("/data/content")
                    .and_then(serde_json::Value::as_array)
                {
                    for item in content {
                        if item.get("kind").and_then(serde_json::Value::as_str) != Some("toolUse") {
                            continue;
                        }
                        let Some(data) = item.get("data") else {
                            continue;
                        };
                        let Some(id) = data.get("toolUseId").and_then(serde_json::Value::as_str)
                        else {
                            continue;
                        };
                        let Some(name) = data.get("name").and_then(serde_json::Value::as_str)
                        else {
                            continue;
                        };
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
            }
            Some("ToolResults") => {
                complete = true;
                if let Some(content) = value
                    .pointer("/data/content")
                    .and_then(serde_json::Value::as_array)
                {
                    for item in content {
                        if item.get("kind").and_then(serde_json::Value::as_str)
                            != Some("toolResult")
                        {
                            continue;
                        }
                        let Some(data) = item.get("data") else {
                            continue;
                        };
                        let Some(id) = data.get("toolUseId").and_then(serde_json::Value::as_str)
                        else {
                            continue;
                        };
                        if let Some(tool) = tools.get_mut(id) {
                            tool.failed = data.get("status").and_then(serde_json::Value::as_str)
                                == Some("error");
                            tool.result_chars = data.get("content").and_then(value_chars);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if !complete {
        return Err(SidecarError::IncompleteTurn);
    }
    Ok(sorted_tools(tools))
}

fn parse_kas_jsonl(raw: &str) -> Result<Vec<UsageTool>, SidecarError> {
    let mut tools: HashMap<String, ParsedTool> = HashMap::new();
    let mut complete = false;
    for line in raw.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|source| SidecarError::Json {
                path: PathBuf::from("KAS current-session JSONL"),
                source,
            })?;
        let Some(payload) = value.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(serde_json::Value::as_str) {
            Some("tool_call") => {
                let Some(id) = payload
                    .get("toolCallId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let Some(name) = payload.get("toolName").and_then(serde_json::Value::as_str) else {
                    continue;
                };
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
            }
            Some("tool_result") => {
                let Some(id) = payload
                    .get("toolCallId")
                    .and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                if let Some(tool) = tools.get_mut(id) {
                    tool.failed |=
                        payload.get("success").and_then(serde_json::Value::as_bool) == Some(false);
                    tool.result_chars = payload.get("content").and_then(value_chars);
                }
            }
            Some("usage_summary") => complete = true,
            _ => {}
        }
    }
    if !complete {
        return Err(SidecarError::IncompleteTurn);
    }
    Ok(sorted_tools(tools))
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
        handle.session_started(session.clone(), KiroSidecarKind::V2);
        std::thread::sleep(Duration::from_millis(10));
        let mut file = std::fs::OpenOptions::new().append(true).open(&jsonl)?;
        use std::io::Write as _;
        writeln!(
            file,
            "{}",
            r#"{"kind":"AssistantMessage","data":{"content":[{"kind":"toolUse","data":{"toolUseId":"new","name":"write","input":{"text":"é"}}}]}}"#
        )?;
        writeln!(
            file,
            "{}",
            r#"{"kind":"ToolResults","data":{"content":[{"kind":"toolResult","data":{"toolUseId":"new","content":"done","status":"success"}}]}}"#
        )?;
        handle.enrich(UsageRecordId::new(7), session, KiroSidecarKind::V2);
        let result = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
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
        handle.session_started(recovering.clone(), KiroSidecarKind::V2);
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(&recovering_jsonl, "{")?;
        handle.enrich(UsageRecordId::new(9), recovering, KiroSidecarKind::V2);
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(
            &recovering_jsonl,
            "{\"kind\":\"AssistantMessage\",\"data\":{\"content\":[]}}\n",
        )?;
        let result = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await?
            .ok_or("worker closed")?;
        assert!(matches!(result, UsageEnrichmentResult::Enriched(_)));

        let (_command_tx, command_rx) = std::sync::mpsc::channel();
        let (result_tx, _result_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut missing_worker = Worker::new(Some(home.clone()), command_rx, result_tx);
        let started = std::time::Instant::now();
        let result = missing_worker.enrich_with_retries(
            UsageRecordId::new(8),
            &SessionId::new("missing"),
            KiroSidecarKind::V2,
        );
        assert!(result.is_err());
        assert!(started.elapsed() <= Duration::from_secs(1));

        let invalid = validate_session_id(&SessionId::new("../escape"));
        assert!(matches!(invalid, Err(SidecarError::InvalidSessionId(_))));
        let oversized = cli.join("oversized.jsonl");
        File::create(&oversized)?.set_len(MAX_SIDECAR_BYTES + 1)?;
        assert!(matches!(
            read_appended(&oversized, 0),
            Err(SidecarError::Oversized { .. })
        ));
        Ok(())
    }
}

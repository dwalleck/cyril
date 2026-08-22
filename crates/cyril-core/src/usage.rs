use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, Row, params};
use thiserror::Error;

use crate::types::{
    AgentMessage, AgentUsageGroup, ModelUsageGroup, Money, NamedUsageGroup, Notification,
    RecentUsage, RoutedNotification, SessionId, SessionOrigin, StopReason, TokenTotals, TokenUsage,
    ToolCall, ToolCallId, ToolCallStatus, ToolKind, ToolUsageGroup, TurnUsageContext,
    UsageAgentType, UsageOutcome, UsageRecord, UsageSnapshot, UsageSummary, UsageTiming, UsageTool,
};

const RECENT_LIMIT: usize = 20;
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
type ModelGroupKey = (Option<String>, Option<String>);

#[derive(Debug, Error)]
pub enum UsageError {
    #[error("usage database path has no file name: {path}")]
    InvalidPath { path: PathBuf },
    #[error("create usage database directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("canonicalize usage database directory {path}: {source}")]
    CanonicalizeDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("open usage database {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },
    #[error("configure usage database: {0}")]
    Configure(#[source] rusqlite::Error),
    #[error("write usage database: {0}")]
    Write(#[source] rusqlite::Error),
    #[error("query usage database: {0}")]
    Query(#[source] rusqlite::Error),
    #[error("usage field {field} exceeds SQLite's signed integer range: {value}")]
    IntegerRange { field: &'static str, value: u64 },
    #[error("usage database contains invalid {field}: {value}")]
    CorruptValue { field: &'static str, value: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UsageObserverError {
    #[error("usage turn already pending for session {0}")]
    TurnAlreadyPending(SessionId),
}

#[derive(Debug, Clone)]
enum CostBaseline {
    FreshZero,
    Known(Money),
    Unknown,
}

#[derive(Debug, Clone)]
struct ObservedTool {
    kind: ToolKind,
    failed: bool,
}

#[derive(Debug)]
struct PendingTurn {
    context: TurnUsageContext,
    started_at: Instant,
    timestamp_ms: u64,
    first_text_at: Option<Instant>,
    tokens: Option<TokenUsage>,
    cost_start: CostBaseline,
    tools: HashMap<ToolCallId, ObservedTool>,
    error: Option<String>,
}

/// Pure turn-correlator. The App supplies dispatch time and routed notifications;
/// the observer returns a record only at a matching turn boundary.
#[derive(Debug, Default)]
pub struct UsageObserver {
    pending: HashMap<SessionId, PendingTurn>,
    costs: HashMap<SessionId, CostBaseline>,
}

impl UsageObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_turn(
        &mut self,
        context: TurnUsageContext,
        started_at: Instant,
        timestamp_ms: u64,
    ) -> Result<(), UsageObserverError> {
        let session_id = context.session_id().clone();
        if self.pending.contains_key(&session_id) {
            return Err(UsageObserverError::TurnAlreadyPending(session_id));
        }
        let cost_start = self
            .costs
            .get(&session_id)
            .cloned()
            .unwrap_or(CostBaseline::Unknown);
        self.pending.insert(
            session_id,
            PendingTurn {
                context,
                started_at,
                timestamp_ms,
                first_text_at: None,
                tokens: None,
                cost_start,
                tools: HashMap::new(),
                error: None,
            },
        );
        Ok(())
    }

    pub fn abort_turn(&mut self, session_id: &SessionId) -> bool {
        self.pending.remove(session_id).is_some()
    }

    pub fn apply(&mut self, routed: &RoutedNotification, now: Instant) -> Option<UsageRecord> {
        if let Notification::UsageSessionStarted { session_id, origin } = &routed.notification {
            self.pending.remove(session_id);
            self.costs.insert(
                session_id.clone(),
                match origin {
                    SessionOrigin::Fresh => CostBaseline::FreshZero,
                    SessionOrigin::Loaded => CostBaseline::Unknown,
                },
            );
            return None;
        }

        let session_id = self.notification_session(routed);
        if let Notification::UsageUpdated {
            cost: Some(cost), ..
        } = &routed.notification
            && let Some(session_id) = session_id.as_ref()
        {
            self.costs
                .insert(session_id.clone(), CostBaseline::Known(cost.clone()));
        }

        let session_id = session_id?;

        match &routed.notification {
            Notification::AgentMessage(AgentMessage { .. }) => {
                if let Some(pending) = self.pending.get_mut(&session_id)
                    && pending.first_text_at.is_none()
                {
                    pending.first_text_at = Some(now);
                }
                None
            }
            Notification::ToolCallStarted(call) | Notification::ToolCallUpdated(call) => {
                if let Some(pending) = self.pending.get_mut(&session_id) {
                    observe_tool(&mut pending.tools, call);
                }
                None
            }
            Notification::BridgeError { operation, message } => {
                if let Some(pending) = self.pending.get_mut(&session_id) {
                    let next = format!("{operation}: {message}");
                    match pending.error.as_mut() {
                        Some(current) => {
                            current.push_str("; ");
                            current.push_str(&next);
                        }
                        None => pending.error = Some(next),
                    }
                }
                None
            }
            Notification::TurnUsageCaptured(tokens) => {
                if let Some(pending) = self.pending.get_mut(&session_id) {
                    pending.tokens = Some(tokens.clone());
                }
                None
            }
            Notification::TurnCompleted { stop_reason } => {
                let Some(pending) = self.pending.remove(&session_id) else {
                    tracing::warn!(
                        session_id = %session_id,
                        "turn completed without a usage observer pending turn"
                    );
                    return None;
                };
                Some(self.finish_turn(session_id, pending, *stop_reason, now))
            }
            _ => None,
        }
    }

    fn notification_session(&self, routed: &RoutedNotification) -> Option<SessionId> {
        if let Some(session_id) = routed.session_id.as_ref() {
            return Some(session_id.clone());
        }
        if self.pending.len() == 1 {
            return self.pending.keys().next().cloned();
        }
        None
    }

    fn finish_turn(
        &mut self,
        session_id: SessionId,
        pending: PendingTurn,
        stop_reason: StopReason,
        now: Instant,
    ) -> UsageRecord {
        let current_cost = self
            .costs
            .get(&session_id)
            .cloned()
            .unwrap_or(CostBaseline::Unknown);
        let cost = turn_cost(&pending.cost_start, &current_cost);
        if matches!(pending.cost_start, CostBaseline::FreshZero)
            && matches!(current_cost, CostBaseline::FreshZero)
        {
            self.costs.insert(session_id, CostBaseline::Unknown);
        }

        let elapsed_ms = duration_ms(now.saturating_duration_since(pending.started_at));
        let ttft_ms = pending
            .first_text_at
            .map(|first| duration_ms(first.saturating_duration_since(pending.started_at)));
        let mut tools: Vec<(String, UsageTool)> = pending
            .tools
            .into_iter()
            .map(|(id, tool)| {
                (
                    id.as_str().to_owned(),
                    UsageTool::new(tool.kind, tool.failed),
                )
            })
            .collect();
        tools.sort_by(|left, right| {
            tool_kind_name(left.1.kind())
                .cmp(tool_kind_name(right.1.kind()))
                .then_with(|| left.0.cmp(&right.0))
        });

        UsageRecord::new(
            pending.context,
            UsageTiming::new(pending.timestamp_ms, elapsed_ms, ttft_ms),
            UsageOutcome::new(stop_reason, pending.tokens, cost, pending.error),
            tools.into_iter().map(|(_, tool)| tool).collect(),
        )
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn observe_tool(tools: &mut HashMap<ToolCallId, ObservedTool>, call: &ToolCall) {
    let failed = call.status() == ToolCallStatus::Failed;
    tools
        .entry(call.id().clone())
        .and_modify(|current| {
            if call.kind() != ToolKind::Other || current.kind == ToolKind::Other {
                current.kind = call.kind();
            }
            current.failed |= failed;
        })
        .or_insert(ObservedTool {
            kind: call.kind(),
            failed,
        });
}

fn turn_cost(start: &CostBaseline, end: &CostBaseline) -> Option<Money> {
    match (start, end) {
        (CostBaseline::FreshZero, CostBaseline::Known(end)) => Some(end.clone()),
        (CostBaseline::Known(start), CostBaseline::Known(end))
            if start.currency() == end.currency() && end.amount() >= start.amount() =>
        {
            match Money::try_new(end.amount() - start.amount(), end.currency().to_owned()) {
                Ok(money) => Some(money),
                Err(error) => {
                    tracing::error!(error = %error, "validated usage cost delta became invalid");
                    None
                }
            }
        }
        (CostBaseline::Known(start), CostBaseline::Known(end)) => {
            tracing::warn!(
                start_amount = start.amount(),
                end_amount = end.amount(),
                start_currency = start.currency(),
                end_currency = end.currency(),
                "cumulative usage cost reset or changed currency; turn cost unavailable"
            );
            None
        }
        _ => None,
    }
}

/// Durable usage log. The path's parent is created and canonicalized before the
/// SQLite file is opened; callers may pass a not-yet-created database file.
pub struct UsageLog {
    connection: Connection,
}

impl UsageLog {
    pub fn open(path: &Path) -> Result<Self, UsageError> {
        let Some(file_name) = path.file_name() else {
            return Err(UsageError::InvalidPath {
                path: path.to_path_buf(),
            });
        };
        let Some(parent) = path.parent() else {
            return Err(UsageError::InvalidPath {
                path: path.to_path_buf(),
            });
        };
        std::fs::create_dir_all(parent).map_err(|source| UsageError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
        let canonical_parent =
            parent
                .canonicalize()
                .map_err(|source| UsageError::CanonicalizeDirectory {
                    path: parent.to_path_buf(),
                    source,
                })?;
        let canonical_path = canonical_parent.join(file_name);
        let connection = Connection::open(&canonical_path).map_err(|source| UsageError::Open {
            path: canonical_path,
            source,
        })?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self, UsageError> {
        let connection = Connection::open_in_memory().map_err(|source| UsageError::Open {
            path: PathBuf::from(":memory:"),
            source,
        })?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self, UsageError> {
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(UsageError::Configure)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS usage_turns (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    folder TEXT NOT NULL,
                    model TEXT,
                    provider TEXT,
                    agent_type TEXT NOT NULL,
                    timestamp_ms INTEGER NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    ttft_ms INTEGER,
                    stop_reason TEXT NOT NULL,
                    error TEXT,
                    total_tokens INTEGER,
                    input_tokens INTEGER,
                    output_tokens INTEGER,
                    thought_tokens INTEGER,
                    cached_read_tokens INTEGER,
                    cached_write_tokens INTEGER,
                    cost_amount REAL,
                    cost_currency TEXT,
                    CHECK ((cost_amount IS NULL) = (cost_currency IS NULL))
                 );
                 CREATE INDEX IF NOT EXISTS usage_turns_timestamp
                    ON usage_turns(timestamp_ms DESC);
                 CREATE INDEX IF NOT EXISTS usage_turns_provider_model
                    ON usage_turns(provider, model);
                 CREATE INDEX IF NOT EXISTS usage_turns_folder
                    ON usage_turns(folder);
                 CREATE INDEX IF NOT EXISTS usage_turns_errors
                    ON usage_turns(error, timestamp_ms DESC);
                 CREATE TABLE IF NOT EXISTS usage_tools (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    turn_id INTEGER NOT NULL REFERENCES usage_turns(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL,
                    failed INTEGER NOT NULL CHECK (failed IN (0, 1))
                 );
                 CREATE INDEX IF NOT EXISTS usage_tools_turn ON usage_tools(turn_id);
                 CREATE INDEX IF NOT EXISTS usage_tools_kind ON usage_tools(kind);",
            )
            .map_err(UsageError::Configure)?;
        Ok(Self { connection })
    }

    pub fn append(&mut self, record: &UsageRecord) -> Result<(), UsageError> {
        let timestamp = sqlite_integer("timestamp_ms", record.timestamp_ms())?;
        let duration = sqlite_integer("duration_ms", record.duration_ms())?;
        let ttft = record
            .ttft_ms()
            .map(|value| sqlite_integer("ttft_ms", value))
            .transpose()?;
        let tokens = record.tokens().map(sqlite_tokens).transpose()?;
        let cost_amount = record.cost().map(Money::amount);
        let cost_currency = record.cost().map(Money::currency);
        let transaction = self.connection.transaction().map_err(UsageError::Write)?;
        transaction
            .execute(
                "INSERT INTO usage_turns (
                    session_id, folder, model, provider, agent_type,
                    timestamp_ms, duration_ms, ttft_ms, stop_reason, error,
                    total_tokens, input_tokens, output_tokens, thought_tokens,
                    cached_read_tokens, cached_write_tokens, cost_amount, cost_currency
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    record.context().session_id().as_str(),
                    record.context().folder(),
                    record.context().model(),
                    record.context().provider(),
                    record.context().agent_type().as_str(),
                    timestamp,
                    duration,
                    ttft,
                    stop_reason_name(record.stop_reason()),
                    record.error(),
                    tokens.as_ref().map(|values| values.total),
                    tokens.as_ref().map(|values| values.input),
                    tokens.as_ref().map(|values| values.output),
                    tokens.as_ref().and_then(|values| values.thought),
                    tokens.as_ref().and_then(|values| values.cached_read),
                    tokens.as_ref().and_then(|values| values.cached_write),
                    cost_amount,
                    cost_currency,
                ],
            )
            .map_err(UsageError::Write)?;
        let turn_id = transaction.last_insert_rowid();
        for tool in record.tools() {
            transaction
                .execute(
                    "INSERT INTO usage_tools (turn_id, kind, failed) VALUES (?, ?, ?)",
                    params![turn_id, tool_kind_name(tool.kind()), tool.failed()],
                )
                .map_err(UsageError::Write)?;
        }
        transaction.commit().map_err(UsageError::Write)
    }

    pub fn snapshot(&self) -> Result<UsageSnapshot, UsageError> {
        Ok(UsageSnapshot {
            overview: self.overview()?,
            providers: self.named_groups("provider")?,
            models: self.model_groups()?,
            folders: self.named_groups("folder")?,
            agent_types: self.agent_groups()?,
            tools: self.tool_groups()?,
            recent: self.recent(false)?,
            errors: self.recent(true)?,
        })
    }

    fn overview(&self) -> Result<UsageSummary, UsageError> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM usage_turns"))
            .map_err(UsageError::Query)?;
        let costs = self.cost_totals(None)?;
        statement
            .query_row([], |row| summary_from_row(row, 0, costs))
            .map_err(UsageError::Query)
    }

    fn named_groups(&self, column: &'static str) -> Result<Vec<NamedUsageGroup>, UsageError> {
        let sql = format!(
            "SELECT {column}, {SUMMARY_COLUMNS} FROM usage_turns GROUP BY {column} ORDER BY COUNT(*) DESC, {column}"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let costs = self.named_cost_totals(column)?;
        let rows = statement
            .query_map([], |row| {
                let name: Option<String> = row.get(0)?;
                let summary =
                    summary_from_row(row, 1, costs.get(&name).cloned().unwrap_or_default())?;
                Ok(NamedUsageGroup { name, summary })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn model_groups(&self) -> Result<Vec<ModelUsageGroup>, UsageError> {
        let sql = format!(
            "SELECT provider, model, {SUMMARY_COLUMNS} FROM usage_turns \
             GROUP BY provider, model ORDER BY COUNT(*) DESC, provider, model"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let costs = self.model_cost_totals()?;
        let rows = statement
            .query_map([], |row| {
                let provider: Option<String> = row.get(0)?;
                let model: Option<String> = row.get(1)?;
                let key = (provider.clone(), model.clone());
                let summary =
                    summary_from_row(row, 2, costs.get(&key).cloned().unwrap_or_default())?;
                Ok(ModelUsageGroup {
                    provider,
                    model,
                    summary,
                })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn agent_groups(&self) -> Result<Vec<AgentUsageGroup>, UsageError> {
        let sql = format!(
            "SELECT agent_type, {SUMMARY_COLUMNS} FROM usage_turns \
             GROUP BY agent_type ORDER BY COUNT(*) DESC, agent_type"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let costs = self.named_cost_totals("agent_type")?;
        let rows = statement
            .query_map([], |row| {
                let raw: String = row.get(0)?;
                let agent_type = UsageAgentType::from_str(&raw).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(StoredValueError::new("agent_type", raw.clone())),
                    )
                })?;
                let summary =
                    summary_from_row(row, 1, costs.get(&Some(raw)).cloned().unwrap_or_default())?;
                Ok(AgentUsageGroup {
                    agent_type,
                    summary,
                })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn tool_groups(&self) -> Result<Vec<ToolUsageGroup>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH call_counts AS (
                    SELECT turn_id, COUNT(*) AS calls FROM usage_tools GROUP BY turn_id
                 )
                 SELECT tools.kind,
                        COUNT(*) AS calls,
                        SUM(tools.failed) AS errors,
                        SUM(turns.total_tokens * 1.0 / counts.calls) AS total_share,
                        SUM(turns.output_tokens * 1.0 / counts.calls) AS output_share
                 FROM usage_tools AS tools
                 JOIN call_counts AS counts ON counts.turn_id = tools.turn_id
                 JOIN usage_turns AS turns ON turns.id = tools.turn_id
                 GROUP BY tools.kind
                 ORDER BY calls DESC, tools.kind",
            )
            .map_err(UsageError::Query)?;
        let costs = self.tool_cost_totals()?;
        let rows = statement
            .query_map([], |row| {
                let raw: String = row.get(0)?;
                let kind = tool_kind_from_name(&raw).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(StoredValueError::new("tool kind", raw.clone())),
                    )
                })?;
                Ok(ToolUsageGroup {
                    kind,
                    calls: row_u64(row, 1, "tool calls")?,
                    errors: row_u64(row, 2, "tool errors")?,
                    total_tokens_share: row.get(3)?,
                    output_tokens_share: row.get(4)?,
                    costs: costs.get(&raw).cloned().unwrap_or_default(),
                })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn recent(&self, errors_only: bool) -> Result<Vec<RecentUsage>, UsageError> {
        let where_clause = if errors_only {
            "WHERE error IS NOT NULL"
        } else {
            ""
        };
        let sql = format!(
            "SELECT session_id, folder, model, provider, agent_type,
                    timestamp_ms, duration_ms, ttft_ms, stop_reason,
                    total_tokens, input_tokens, output_tokens, thought_tokens,
                    cached_read_tokens, cached_write_tokens,
                    cost_amount, cost_currency, error
             FROM usage_turns {where_clause}
             ORDER BY timestamp_ms DESC, id DESC LIMIT {RECENT_LIMIT}"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], recent_from_row)
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn cost_totals(&self, where_sql: Option<&str>) -> Result<Vec<Money>, UsageError> {
        let suffix = where_sql.unwrap_or("");
        let sql = format!(
            "SELECT cost_currency, SUM(cost_amount) FROM usage_turns \
             WHERE cost_amount IS NOT NULL {suffix} GROUP BY cost_currency ORDER BY cost_currency"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| money_from_row(row, 0, 1))
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn named_cost_totals(
        &self,
        column: &'static str,
    ) -> Result<HashMap<Option<String>, Vec<Money>>, UsageError> {
        let sql = format!(
            "SELECT {column}, cost_currency, SUM(cost_amount) FROM usage_turns \
             WHERE cost_amount IS NOT NULL GROUP BY {column}, cost_currency ORDER BY cost_currency"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key: Option<String> = row.get(0)?;
                Ok((key, money_from_row(row, 1, 2)?))
            })
            .map_err(UsageError::Query)?;
        let mut result: HashMap<Option<String>, Vec<Money>> = HashMap::new();
        for row in rows {
            let (key, money) = row.map_err(UsageError::Query)?;
            result.entry(key).or_default().push(money);
        }
        Ok(result)
    }

    fn model_cost_totals(&self) -> Result<HashMap<ModelGroupKey, Vec<Money>>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT provider, model, cost_currency, SUM(cost_amount) FROM usage_turns
                 WHERE cost_amount IS NOT NULL
                 GROUP BY provider, model, cost_currency ORDER BY cost_currency",
            )
            .map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key = (row.get(0)?, row.get(1)?);
                Ok((key, money_from_row(row, 2, 3)?))
            })
            .map_err(UsageError::Query)?;
        let mut result = HashMap::new();
        for row in rows {
            let (key, money) = row.map_err(UsageError::Query)?;
            result.entry(key).or_insert_with(Vec::new).push(money);
        }
        Ok(result)
    }

    fn tool_cost_totals(&self) -> Result<HashMap<String, Vec<Money>>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH call_counts AS (
                    SELECT turn_id, COUNT(*) AS calls FROM usage_tools GROUP BY turn_id
                 )
                 SELECT tools.kind, turns.cost_currency,
                        SUM(turns.cost_amount * 1.0 / counts.calls)
                 FROM usage_tools AS tools
                 JOIN call_counts AS counts ON counts.turn_id = tools.turn_id
                 JOIN usage_turns AS turns ON turns.id = tools.turn_id
                 WHERE turns.cost_amount IS NOT NULL
                 GROUP BY tools.kind, turns.cost_currency
                 ORDER BY turns.cost_currency",
            )
            .map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key: String = row.get(0)?;
                Ok((key, money_from_row(row, 1, 2)?))
            })
            .map_err(UsageError::Query)?;
        let mut result = HashMap::new();
        for row in rows {
            let (key, money) = row.map_err(UsageError::Query)?;
            result.entry(key).or_insert_with(Vec::new).push(money);
        }
        Ok(result)
    }
}

#[derive(Debug)]
struct SqliteTokens {
    total: i64,
    input: i64,
    output: i64,
    thought: Option<i64>,
    cached_read: Option<i64>,
    cached_write: Option<i64>,
}

fn sqlite_tokens(tokens: &TokenUsage) -> Result<SqliteTokens, UsageError> {
    Ok(SqliteTokens {
        total: sqlite_integer("total_tokens", tokens.total())?,
        input: sqlite_integer("input_tokens", tokens.input())?,
        output: sqlite_integer("output_tokens", tokens.output())?,
        thought: tokens
            .thought()
            .map(|value| sqlite_integer("thought_tokens", value))
            .transpose()?,
        cached_read: tokens
            .cached_read()
            .map(|value| sqlite_integer("cached_read_tokens", value))
            .transpose()?,
        cached_write: tokens
            .cached_write()
            .map(|value| sqlite_integer("cached_write_tokens", value))
            .transpose()?,
    })
}

fn sqlite_integer(field: &'static str, value: u64) -> Result<i64, UsageError> {
    i64::try_from(value).map_err(|_| UsageError::IntegerRange { field, value })
}

const SUMMARY_COLUMNS: &str = "
    COUNT(*) AS requests,
    COALESCE(SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END), 0) AS errors,
    COUNT(total_tokens) AS token_rows,
    SUM(total_tokens) AS total_tokens,
    SUM(input_tokens) AS input_tokens,
    SUM(output_tokens) AS output_tokens,
    SUM(COALESCE(thought_tokens, 0)) AS thought_tokens,
    SUM(COALESCE(cached_read_tokens, 0)) AS cached_read_tokens,
    SUM(COALESCE(cached_write_tokens, 0)) AS cached_write_tokens,
    AVG(duration_ms) AS avg_duration,
    AVG(ttft_ms) AS avg_ttft,
    AVG(CASE WHEN duration_ms > 0 AND output_tokens IS NOT NULL
        THEN output_tokens * 1000.0 / duration_ms ELSE NULL END) AS avg_tps";

fn summary_from_row(
    row: &Row<'_>,
    base: usize,
    costs: Vec<Money>,
) -> rusqlite::Result<UsageSummary> {
    let requests = row_u64(row, base, "requests")?;
    let errors = row_u64(row, base + 1, "errors")?;
    let token_rows = row_u64(row, base + 2, "token rows")?;
    let tokens = if token_rows == 0 {
        None
    } else {
        Some(TokenTotals {
            total: row_u64(row, base + 3, "total tokens")?,
            input: row_u64(row, base + 4, "input tokens")?,
            output: row_u64(row, base + 5, "output tokens")?,
            thought: row_u64(row, base + 6, "thought tokens")?,
            cached_read: row_u64(row, base + 7, "cached read tokens")?,
            cached_write: row_u64(row, base + 8, "cached write tokens")?,
        })
    };
    let cache_rate = tokens.as_ref().and_then(|totals| {
        let denominator = totals.input.saturating_add(totals.cached_read);
        (denominator > 0).then(|| totals.cached_read as f64 / denominator as f64)
    });
    Ok(UsageSummary {
        requests,
        errors,
        tokens,
        costs,
        cache_rate,
        avg_duration_ms: row.get(base + 9)?,
        avg_ttft_ms: row.get(base + 10)?,
        avg_tokens_per_second: row.get(base + 11)?,
    })
}

fn row_u64(row: &Row<'_>, index: usize, field: &'static str) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(StoredValueError::new(field, value.to_string())),
        )
    })
}

fn money_from_row(
    row: &Row<'_>,
    currency_index: usize,
    amount_index: usize,
) -> rusqlite::Result<Money> {
    let currency: String = row.get(currency_index)?;
    let amount: f64 = row.get(amount_index)?;
    Money::try_new(amount, currency.clone()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            amount_index,
            rusqlite::types::Type::Real,
            Box::new(StoredValueError::new(
                "money",
                format!("{amount} {currency}: {error}"),
            )),
        )
    })
}

fn recent_from_row(row: &Row<'_>) -> rusqlite::Result<RecentUsage> {
    let raw_agent: String = row.get(4)?;
    let agent_type = UsageAgentType::from_str(&raw_agent).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("agent_type", raw_agent.clone())),
        )
    })?;
    let raw_stop: String = row.get(8)?;
    let stop_reason = stop_reason_from_name(&raw_stop).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("stop_reason", raw_stop.clone())),
        )
    })?;
    let total: Option<i64> = row.get(9)?;
    let tokens = total
        .map(|total| {
            Ok::<TokenUsage, rusqlite::Error>(TokenUsage::new(
                stored_u64(9, "total_tokens", total)?,
                stored_u64(10, "input_tokens", row.get(10)?)?,
                stored_u64(11, "output_tokens", row.get(11)?)?,
                stored_optional_u64(12, "thought_tokens", row.get(12)?)?,
                stored_optional_u64(13, "cached_read_tokens", row.get(13)?)?,
                stored_optional_u64(14, "cached_write_tokens", row.get(14)?)?,
            ))
        })
        .transpose()?;
    let cost_amount: Option<f64> = row.get(15)?;
    let cost_currency: Option<String> = row.get(16)?;
    let cost = match (cost_amount, cost_currency) {
        (Some(amount), Some(currency)) => {
            Some(Money::try_new(amount, currency.clone()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    15,
                    rusqlite::types::Type::Real,
                    Box::new(StoredValueError::new(
                        "money",
                        format!("{amount} {currency}: {error}"),
                    )),
                )
            })?)
        }
        (None, None) => None,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Null,
                Box::new(StoredValueError::new("money", "partial cost row")),
            ));
        }
    };
    Ok(RecentUsage {
        session_id: SessionId::new(row.get::<_, String>(0)?),
        folder: row.get(1)?,
        model: row.get(2)?,
        provider: row.get(3)?,
        agent_type,
        timestamp_ms: stored_u64(5, "timestamp_ms", row.get(5)?)?,
        duration_ms: stored_u64(6, "duration_ms", row.get(6)?)?,
        ttft_ms: stored_optional_u64(7, "ttft_ms", row.get(7)?)?,
        stop_reason,
        tokens,
        cost,
        error: row.get(17)?,
    })
}

fn stored_u64(index: usize, field: &'static str, value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(StoredValueError::new(field, value.to_string())),
        )
    })
}

fn stored_optional_u64(
    index: usize,
    field: &'static str,
    value: Option<i64>,
) -> rusqlite::Result<Option<u64>> {
    value
        .map(|value| stored_u64(index, field, value))
        .transpose()
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, UsageError> {
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(UsageError::Query)
}

#[derive(Debug)]
struct StoredValueError {
    message: String,
}

impl StoredValueError {
    fn new(field: &str, value: impl fmt::Display) -> Self {
        Self {
            message: format!("invalid stored {field}: {value}"),
        }
    }
}

impl fmt::Display for StoredValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StoredValueError {}

fn stop_reason_name(reason: StopReason) -> &'static str {
    match reason {
        StopReason::EndTurn => "end_turn",
        StopReason::MaxTokens => "max_tokens",
        StopReason::MaxTurnRequests => "max_turn_requests",
        StopReason::Refusal => "refusal",
        StopReason::Cancelled => "cancelled",
    }
}

fn stop_reason_from_name(value: &str) -> Option<StopReason> {
    match value {
        "end_turn" => Some(StopReason::EndTurn),
        "max_tokens" => Some(StopReason::MaxTokens),
        "max_turn_requests" => Some(StopReason::MaxTurnRequests),
        "refusal" => Some(StopReason::Refusal),
        "cancelled" => Some(StopReason::Cancelled),
        _ => None,
    }
}

fn tool_kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Write => "write",
        ToolKind::Execute => "execute",
        ToolKind::Search => "search",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::SwitchMode => "switch_mode",
        ToolKind::Other => "other",
    }
}

fn tool_kind_from_name(value: &str) -> Option<ToolKind> {
    match value {
        "read" => Some(ToolKind::Read),
        "write" => Some(ToolKind::Write),
        "execute" => Some(ToolKind::Execute),
        "search" => Some(ToolKind::Search),
        "think" => Some(ToolKind::Think),
        "fetch" => Some(ToolKind::Fetch),
        "switch_mode" => Some(ToolKind::SwitchMode),
        "other" => Some(ToolKind::Other),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentMessage, ToolCallStatus};

    fn context(session: &str, model: Option<&str>) -> TurnUsageContext {
        TurnUsageContext::new(
            SessionId::new(session),
            "/tmp/space and 日本語",
            model,
            UsageAgentType::Main,
        )
    }

    fn scoped(session: &str, notification: Notification) -> RoutedNotification {
        RoutedNotification::scoped(SessionId::new(session), notification)
    }

    fn start_session(
        observer: &mut UsageObserver,
        session: &str,
        origin: SessionOrigin,
        now: Instant,
    ) {
        observer.apply(
            &RoutedNotification::global(Notification::UsageSessionStarted {
                session_id: SessionId::new(session),
                origin,
            }),
            now,
        );
    }

    fn complete(observer: &mut UsageObserver, session: &str, now: Instant) -> UsageRecord {
        observer
            .apply(
                &scoped(
                    session,
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    },
                ),
                now,
            )
            .expect("pending turn completes")
    }

    #[test]
    fn timing_uses_first_agent_text_only() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        observer
            .begin_turn(context("s", None), start, 100)
            .expect("begin turn");
        observer.apply(
            &scoped(
                "s",
                Notification::AgentThought(crate::types::AgentThought {
                    text: "thinking".into(),
                }),
            ),
            start + Duration::from_millis(10),
        );
        observer.apply(
            &scoped(
                "s",
                Notification::AgentMessage(AgentMessage {
                    text: "a".into(),
                    is_streaming: true,
                }),
            ),
            start + Duration::from_millis(25),
        );
        observer.apply(
            &scoped(
                "s",
                Notification::AgentMessage(AgentMessage {
                    text: "b".into(),
                    is_streaming: true,
                }),
            ),
            start + Duration::from_millis(40),
        );
        let record = complete(&mut observer, "s", start + Duration::from_millis(100));
        assert_eq!(record.duration_ms(), 100);
        assert_eq!(record.ttft_ms(), Some(25));
    }

    #[test]
    fn cumulative_cost_delta_matrix() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "fresh", SessionOrigin::Fresh, start);
        observer
            .begin_turn(context("fresh", None), start, 1)
            .expect("begin first turn");
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(Money::try_new(0.003_907_2, "USD").expect("valid cost")),
                },
            ),
            start,
        );
        let first = complete(&mut observer, "fresh", start);
        assert!((first.cost().expect("first cost").amount() - 0.003_907_2).abs() < 1e-12);

        observer
            .begin_turn(context("fresh", None), start, 2)
            .expect("begin second turn");
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 2,
                    size: 10,
                    cost: Some(Money::try_new(0.004_349, "USD").expect("valid cost")),
                },
            ),
            start,
        );
        let second = complete(&mut observer, "fresh", start);
        assert!((second.cost().expect("second cost").amount() - 0.000_441_8).abs() < 1e-12);

        observer
            .begin_turn(context("fresh", None), start, 3)
            .expect("begin reset turn");
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 3,
                    size: 10,
                    cost: Some(Money::try_new(0.001, "EUR").expect("valid cost")),
                },
            ),
            start,
        );
        assert!(complete(&mut observer, "fresh", start).cost().is_none());
    }

    #[test]
    fn loaded_session_requires_cost_baseline() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "loaded", SessionOrigin::Loaded, start);
        observer
            .begin_turn(context("loaded", None), start, 1)
            .expect("begin resumed turn");
        observer.apply(
            &scoped(
                "loaded",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(Money::try_new(9.0, "USD").expect("valid cost")),
                },
            ),
            start,
        );
        assert!(complete(&mut observer, "loaded", start).cost().is_none());

        observer
            .begin_turn(context("loaded", None), start, 2)
            .expect("begin next turn");
        observer.apply(
            &scoped(
                "loaded",
                Notification::UsageUpdated {
                    used: 2,
                    size: 10,
                    cost: Some(Money::try_new(9.25, "USD").expect("valid cost")),
                },
            ),
            start,
        );
        assert_eq!(
            complete(&mut observer, "loaded", start)
                .cost()
                .map(Money::amount),
            Some(0.25)
        );
    }

    #[test]
    fn fresh_session_attributes_first_cost() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        observer
            .begin_turn(context("s", None), start, 1)
            .expect("begin turn");
        observer.apply(
            &scoped(
                "s",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(Money::try_new(0.5, "USD").expect("valid cost")),
                },
            ),
            start,
        );
        assert_eq!(
            complete(&mut observer, "s", start)
                .cost()
                .map(Money::amount),
            Some(0.5)
        );
    }

    #[test]
    fn tool_calls_dedupe_and_shares_add_up() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        observer
            .begin_turn(context("s", None), start, 1)
            .expect("begin turn");
        let read = ToolCall::new(
            ToolCallId::new("a"),
            "Read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            None,
        );
        observer.apply(
            &scoped("s", Notification::ToolCallStarted(read.clone())),
            start,
        );
        observer.apply(&scoped("s", Notification::ToolCallUpdated(read)), start);
        observer.apply(
            &scoped(
                "s",
                Notification::ToolCallUpdated(ToolCall::new(
                    ToolCallId::new("b"),
                    "Run".into(),
                    ToolKind::Execute,
                    ToolCallStatus::Failed,
                    None,
                )),
            ),
            start,
        );
        let record = complete(&mut observer, "s", start);
        assert_eq!(record.tools().len(), 2);
        assert!(record.tools().iter().any(|tool| tool.failed()));
    }

    fn record(
        session: &str,
        model: Option<&str>,
        tokens: Option<TokenUsage>,
        cost: Option<Money>,
        duration_ms: u64,
        ttft_ms: Option<u64>,
        error: Option<&str>,
        tools: Vec<UsageTool>,
    ) -> UsageRecord {
        UsageRecord::new(
            context(session, model),
            UsageTiming::new(1, duration_ms, ttft_ms),
            UsageOutcome::new(StopReason::EndTurn, tokens, cost, error.map(str::to_owned)),
            tools,
        )
    }

    #[test]
    fn overview_matches_independent_omp_formula_oracle() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        log.append(&record(
            "s1",
            Some("p/m"),
            Some(TokenUsage::new(150, 100, 20, None, Some(30), None)),
            Some(Money::try_new(1.0, "USD").expect("valid cost")),
            100,
            Some(25),
            None,
            Vec::new(),
        ))
        .expect("append first");
        log.append(&record(
            "s2",
            Some("q/n"),
            Some(TokenUsage::new(100, 50, 40, None, Some(10), None)),
            Some(Money::try_new(2.0, "EUR").expect("valid cost")),
            200,
            None,
            Some("boom"),
            Vec::new(),
        ))
        .expect("append second");
        let overview = log.snapshot().expect("snapshot").overview;
        assert_eq!(overview.requests, 2);
        assert_eq!(overview.errors, 1);
        let totals = overview.tokens.expect("token totals");
        assert_eq!(totals.total, 250);
        assert_eq!(totals.input, 150);
        assert_eq!(totals.output, 60);
        assert_eq!(totals.cached_read, 40);
        assert!((overview.cache_rate.expect("cache rate") - 40.0 / 190.0).abs() < 1e-12);
        assert!((overview.avg_duration_ms.expect("duration") - 150.0).abs() < 1e-12);
        assert_eq!(overview.avg_ttft_ms, Some(25.0));
        assert!((overview.avg_tokens_per_second.expect("tps") - 200.0).abs() < 1e-12);
        assert_eq!(overview.costs.len(), 2);
    }

    #[test]
    fn append_is_atomic_across_record_and_tools() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        log.connection
            .execute_batch(
                "CREATE TRIGGER reject_usage_tool BEFORE INSERT ON usage_tools
                 BEGIN SELECT RAISE(ABORT, 'forced child failure'); END;",
            )
            .expect("install failure trigger");
        let result = log.append(&record(
            "s",
            None,
            None,
            None,
            1,
            None,
            None,
            vec![UsageTool::new(ToolKind::Read, false)],
        ));
        assert!(result.is_err());
        let count: i64 = log
            .connection
            .query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0))
            .expect("count parents");
        assert_eq!(count, 0);
    }

    #[test]
    #[ignore = "reference-workstation append budget"]
    fn append_100_tools_budget_reference() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        let tools = (0..100)
            .map(|_| UsageTool::new(ToolKind::Read, false))
            .collect();
        let record = record(
            "s",
            Some("provider/model"),
            Some(TokenUsage::new(10, 4, 6, None, None, None)),
            Some(Money::try_new(0.1, "USD").expect("valid cost")),
            10,
            Some(2),
            None,
            tools,
        );
        let started = Instant::now();
        log.append(&record).expect("append 100-tool turn");
        assert!(
            started.elapsed() <= Duration::from_millis(10),
            "100-tool usage append exceeded 10ms: {:?}",
            started.elapsed()
        );
    }
    #[test]
    fn invalid_values_fail_without_defaulting() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        let invalid = record(
            "s",
            None,
            Some(TokenUsage::new(u64::MAX, 0, 0, None, None, None)),
            None,
            1,
            None,
            None,
            Vec::new(),
        );
        assert!(matches!(
            log.append(&invalid),
            Err(UsageError::IntegerRange {
                field: "total_tokens",
                value: u64::MAX
            })
        ));
        assert_eq!(log.snapshot().expect("snapshot").overview.requests, 0);
    }

    #[test]
    fn usage_less_error_turn_is_persisted() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        observer
            .begin_turn(context("s", None), start, 1)
            .expect("begin error turn");
        observer.apply(
            &scoped(
                "s",
                Notification::BridgeError {
                    operation: "prompt".into(),
                    message: "failed".into(),
                },
            ),
            start + Duration::from_millis(2),
        );
        let record = complete(&mut observer, "s", start + Duration::from_millis(4));
        assert!(record.tokens().is_none());
        assert_eq!(record.error(), Some("prompt: failed"));
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        log.append(&record).expect("append error");
        let snapshot = log.snapshot().expect("snapshot");
        assert_eq!(snapshot.overview.requests, 1);
        assert_eq!(snapshot.overview.errors, 1);
        assert!(snapshot.overview.tokens.is_none());
        assert_eq!(snapshot.recent.len(), 1);
        assert_eq!(snapshot.errors.len(), 1);
    }

    #[test]
    fn breakdowns_are_identity_agnostic() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        for (session, model) in [("a", "openai-codex/m"), ("b", "beta/n")] {
            log.append(&record(
                session,
                Some(model),
                Some(TokenUsage::new(10, 4, 6, None, None, None)),
                None,
                10,
                Some(2),
                None,
                vec![UsageTool::new(ToolKind::Read, false)],
            ))
            .expect("append identity");
        }
        let snapshot = log.snapshot().expect("snapshot");
        assert_eq!(snapshot.providers.len(), 2);
        assert_eq!(snapshot.models.len(), 2);
        for group in &snapshot.providers {
            assert_eq!(group.summary.requests, 1);
            assert_eq!(group.summary.tokens.as_ref().map(|t| t.total), Some(10));
        }
        assert_eq!(snapshot.tools.len(), 1);
        assert_eq!(snapshot.tools[0].calls, 2);
        assert_eq!(snapshot.tools[0].total_tokens_share, Some(20.0));
    }

    #[test]
    fn snapshot_is_bounded_for_large_history() {
        let mut log = UsageLog::open_in_memory().expect("in-memory log");
        let transaction = log.connection.transaction().expect("bulk transaction");
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO usage_turns (
                        session_id, folder, model, provider, agent_type,
                        timestamp_ms, duration_ms, stop_reason,
                        total_tokens, input_tokens, output_tokens
                     ) VALUES (?, '/tmp', 'm', 'p', 'main', ?, 10, 'end_turn', 3, 1, 2)",
                )
                .expect("prepare seed");
            for index in 0..100_000_i64 {
                let error: Option<&str> = (index < 30).then_some("error");
                statement
                    .execute(params![format!("s{index}"), index])
                    .expect("seed row");
                if let Some(error) = error {
                    transaction
                        .execute(
                            "UPDATE usage_turns SET error = ? WHERE id = last_insert_rowid()",
                            [error],
                        )
                        .expect("mark error");
                }
            }
        }
        transaction.commit().expect("commit seed");
        let started = Instant::now();
        let snapshot = log.snapshot().expect("bounded snapshot");
        assert!(started.elapsed() <= Duration::from_secs(2));
        assert_eq!(snapshot.overview.requests, 100_000);
        assert_eq!(snapshot.recent.len(), 20);
        assert_eq!(snapshot.errors.len(), 20);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.folders.len(), 1);
    }
}

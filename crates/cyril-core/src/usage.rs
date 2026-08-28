//! Per-turn usage observation and the local usage log.
//!
//! The observer records one [`UsageRecord`] per **turn**: timing
//! ([`UsageTiming::duration_ms`], [`UsageTiming::ttft_ms`]), outcome, metered
//! charges, and the tools the turn used. Records persist to a local SQLite log
//! and are rolled up by [`UsageLog::snapshot`].
//!
//! # Non-goal: per-provider-request granularity
//!
//! Cyril records turns, **not individual provider requests, and cannot record
//! them.** This is an architectural boundary, not a missing feature.
//!
//! A single turn fans out into several backend requests — one captured
//! `turn_completion` carried four `requestIds`. kiro-cli's own hidden `/stats`
//! panel shows one row per request with its own duration and time-to-first-chunk
//! because *it* issues those calls and instruments them
//! (`chat_cli_v2::agent::acp::request_stats`, a process-local ring buffer).
//!
//! Cyril sits one layer above, on ACP. It observes the turn boundary and, at
//! turn end, an aggregate `requestIds[]` — never the individual requests, their
//! start times, or their first bytes. No amount of observer work recovers that;
//! the information is not on the wire cyril reads. `provider_requests` is
//! therefore a *count*, and turn-level `duration_ms` / `ttft_ms` cannot be
//! decomposed per request.
//!
//! Persisting the request IDs themselves is separately tracked (cyril-uefh) and
//! is worth doing — it enables correlating a turn with backend-side records —
//! but it does not and cannot yield per-request timings.
//!
//! See `docs/kiro-2.20.1-wire-audit.md` § 8 for the wire evidence.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use thiserror::Error;

use crate::types::{
    AgentMessage, AgentUsageGroup, CompactionPhase, ContextBreakdown, ContextBucket, MeteredAmount,
    MetricCoverage, ModelUsageGroup, Money, NamedUsageGroup, Notification, ObservedMetric,
    RecentUsage, RoutedNotification, SessionId, SessionOrigin, StopReason, TokenTotals, TokenUsage,
    ToolCall, ToolCallId, ToolCallStatus, ToolKind, ToolModelUsageGroup, ToolUsageGroup,
    TurnUsageContext, TurnUsageMetrics, UnavailableReason, UsageAgentType, UsageCompaction,
    UsageContextSample, UsageContextSummary, UsageOutcome, UsageRecord, UsageRecordId,
    UsageSnapshot, UsageSummary, UsageTiming, UsageTool, UsageTurnOutcome, UsageTurnStatus,
};

const RECENT_LIMIT: usize = 20;
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const STARTUP_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
type ToolGroupKey = (Option<String>, String);
type ModelGroupKey = (Option<String>, Option<String>);

mod kiro_sidecar;
pub use kiro_sidecar::{
    UsageEnrichment, UsageEnrichmentHandle, UsageEnrichmentResult, spawn_usage_enrichment_worker,
};

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
    #[error("unsupported usage database schema version {0}")]
    UnsupportedSchema(i64),
    #[error("usage context {field} percentage must be finite and within 0..=100: {value}")]
    InvalidContextPercentage { field: &'static str, value: String },
    #[error("usage database contains invalid {field}: {value}")]
    CorruptValue { field: &'static str, value: String },
    #[error("usage record {0} does not exist")]
    RecordNotFound(i64),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UsageObserverError {
    #[error("usage turn already pending for session {0}")]
    TurnAlreadyPending(SessionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KiroSidecarKind {
    V2,
    Kas,
}

#[derive(Debug)]
pub enum UsageWrite {
    Turn {
        record: UsageRecord,
        sidecar_kind: Option<KiroSidecarKind>,
    },
    Context {
        sample: UsageContextSample,
        compaction: Option<UsageCompaction>,
    },
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
    argument_chars: Option<u64>,
    result_chars: Option<u64>,
}

#[derive(Debug)]
struct PendingTurn {
    context: TurnUsageContext,
    started_at: Instant,
    timestamp_ms: u64,
    first_text_at: Option<Instant>,
    tokens: Option<TokenUsage>,
    cost_start: CostBaseline,
    charges: Vec<MeteredAmount>,
    metering_status: Option<UsageTurnStatus>,
    provider_requests: Option<u64>,
    backend_gated: bool,
    tools: HashMap<ToolCallId, ObservedTool>,
    error: Option<String>,
    sidecar_kind: Option<KiroSidecarKind>,
}

#[derive(Debug, Clone)]
struct ObservedContext {
    context: TurnUsageContext,
    timestamp_ms: u64,
}

#[derive(Debug, Clone)]
struct PendingCompaction {
    before_percentage: Option<f64>,
    completed: bool,
}

/// Pure usage correlator. The App supplies dispatch time and typed
/// notifications; the observer emits storage writes without knowing a store.
#[derive(Debug, Default)]
pub struct UsageObserver {
    pending: HashMap<SessionId, PendingTurn>,
    costs: HashMap<SessionId, CostBaseline>,
    contexts: HashMap<SessionId, ObservedContext>,
    context_percentages: HashMap<SessionId, f64>,
    context_breakdowns: HashMap<SessionId, ContextBreakdown>,
    compactions: HashMap<SessionId, PendingCompaction>,
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
        sidecar_kind: Option<KiroSidecarKind>,
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
        self.contexts.insert(
            session_id.clone(),
            ObservedContext {
                context: context.clone(),
                timestamp_ms,
            },
        );
        self.pending.insert(
            session_id,
            PendingTurn {
                context,
                started_at,
                timestamp_ms,
                first_text_at: None,
                tokens: None,
                cost_start,
                charges: Vec::new(),
                metering_status: None,
                provider_requests: None,
                backend_gated: false,
                sidecar_kind,
                tools: HashMap::new(),
                error: None,
            },
        );
        Ok(())
    }

    pub fn abort_turn(&mut self, session_id: &SessionId) -> bool {
        let removed = self.pending.remove(session_id).is_some();
        self.contexts.remove(session_id);
        removed
    }

    pub fn apply(&mut self, routed: &RoutedNotification, now: Instant) -> Option<UsageWrite> {
        if let Notification::UsageSessionStarted { session_id, origin } = &routed.notification {
            self.pending.remove(session_id);
            self.contexts.remove(session_id);
            self.context_percentages.remove(session_id);
            self.context_breakdowns.remove(session_id);
            self.compactions.remove(session_id);
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
            Notification::MetadataUpdated {
                metering,
                context_usage,
                ..
            } => {
                if let Some(pending) = self.pending.get_mut(&session_id) {
                    pending.backend_gated = true;
                    if let Some(metering) = metering
                        && !metering.charges().is_empty()
                    {
                        pending.charges = metering.charges().to_vec();
                    }
                    pending.sidecar_kind = Some(KiroSidecarKind::V2);
                }
                context_usage
                    .as_ref()
                    .and_then(|usage| self.context_write(&session_id, usage.percentage(), None))
            }
            Notification::TurnMeteringUpdated(update) => {
                if let Some(pending) = self.pending.get_mut(&session_id) {
                    pending.backend_gated = true;
                    if !update.charges().is_empty() {
                        pending.charges = update.charges().to_vec();
                    }
                    if let Some(status) = update.status() {
                        pending.metering_status = Some(status.clone());
                    }
                    pending.sidecar_kind = Some(KiroSidecarKind::Kas);
                    if let Some(request_ids) = update.request_ids() {
                        pending.provider_requests = match u64::try_from(request_ids.len()) {
                            Ok(count) => Some(count),
                            Err(error) => {
                                tracing::warn!(
                                    error = %error,
                                    "provider request count exceeds u64, ignoring"
                                );
                                None
                            }
                        };
                    }
                }
                None
            }
            Notification::ContextBreakdownUpdated {
                usage_percentage,
                breakdown,
            } => self.context_write(&session_id, *usage_percentage, breakdown.clone()),
            Notification::CompactionStatus { phase, .. } => {
                match phase {
                    CompactionPhase::Started => {
                        self.compactions.insert(
                            session_id.clone(),
                            PendingCompaction {
                                before_percentage: self
                                    .context_percentages
                                    .get(&session_id)
                                    .copied(),
                                completed: false,
                            },
                        );
                    }
                    CompactionPhase::Completed => {
                        if let Some(pending) = self.compactions.get_mut(&session_id) {
                            pending.completed = true;
                        } else {
                            tracing::warn!(
                                session_id = %session_id,
                                "compaction completed without a started compaction; ignoring"
                            );
                        }
                    }
                    CompactionPhase::Failed { .. } => {
                        self.compactions.remove(&session_id);
                    }
                }
                None
            }
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
                let sidecar_kind = pending.sidecar_kind;
                Some(UsageWrite::Turn {
                    record: self.finish_turn(session_id, pending, *stop_reason, now),
                    sidecar_kind,
                })
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
        if self.contexts.len() == 1 {
            return self.contexts.keys().next().cloned();
        }
        None
    }

    fn context_write(
        &mut self,
        session_id: &SessionId,
        percentage: f64,
        breakdown: Option<ContextBreakdown>,
    ) -> Option<UsageWrite> {
        let percentage = match valid_context_percentage("sample", percentage) {
            Ok(percentage) => percentage,
            Err(error) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "usage context percentage is invalid, ignoring"
                );
                return None;
            }
        };
        if let Some(breakdown) = breakdown.as_ref()
            && let Err(error) = validate_context_breakdown(breakdown)
        {
            tracing::warn!(
                session_id = %session_id,
                error = %error,
                "usage context breakdown contains an invalid percentage, ignoring"
            );
            return None;
        }
        let Some(observed) = self.contexts.get(session_id).cloned() else {
            tracing::warn!(
                session_id = %session_id,
                "usage context update has no observed turn identity, ignoring"
            );
            return None;
        };
        self.context_percentages
            .insert(session_id.clone(), percentage);
        if let Some(breakdown) = breakdown {
            self.context_breakdowns
                .insert(session_id.clone(), breakdown);
        }
        let sample = UsageContextSample {
            context: observed.context.clone(),
            timestamp_ms: observed.timestamp_ms,
            percentage,
            breakdown: self.context_breakdowns.get(session_id).cloned(),
        };
        let compaction = if self
            .compactions
            .get(session_id)
            .is_some_and(|pending| pending.completed)
        {
            self.compactions
                .remove(session_id)
                .map(|pending| UsageCompaction {
                    context: observed.context,
                    timestamp_ms: observed.timestamp_ms,
                    before_percentage: pending.before_percentage,
                    after_percentage: percentage,
                    reduction_percentage_points: pending
                        .before_percentage
                        .filter(|before| *before >= percentage)
                        .map(|before| before - percentage),
                })
        } else {
            None
        };
        Some(UsageWrite::Context { sample, compaction })
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
                    UsageTool::observed(
                        id,
                        tool.kind,
                        tool.failed,
                        tool.argument_chars,
                        tool.result_chars,
                    ),
                )
            })
            .collect();
        tools.sort_by(|left, right| {
            tool_kind_name(left.1.kind())
                .cmp(tool_kind_name(right.1.kind()))
                .then_with(|| left.0.cmp(&right.0))
        });

        let mut error = pending.error;
        if error.is_none()
            && let Some(UsageTurnStatus::Other(status)) = pending.metering_status.as_ref()
        {
            error = Some(format!("turn status: {status}"));
        }
        let outcome = classify_outcome(stop_reason, pending.metering_status.as_ref(), &error);
        let metrics = TurnUsageMetrics::new(
            observed_metric(pending.tokens, pending.backend_gated),
            observed_metric(cost, pending.backend_gated),
            pending.charges,
            pending.provider_requests,
            pending.metering_status,
        );

        UsageRecord::new(
            pending.context,
            UsageTiming::new(pending.timestamp_ms, elapsed_ms, ttft_ms),
            UsageOutcome::new(stop_reason, outcome, metrics, error),
            tools.into_iter().map(|(_, tool)| tool).collect(),
        )
    }
}

fn valid_context_percentage(field: &'static str, value: f64) -> Result<f64, UsageError> {
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(UsageError::InvalidContextPercentage {
            field,
            value: value.to_string(),
        });
    }
    Ok(value)
}

fn validate_context_breakdown(breakdown: &ContextBreakdown) -> Result<(), UsageError> {
    for (field, bucket) in [
        ("context_files", breakdown.context_files()),
        ("session_files", breakdown.session_files()),
        ("tools", breakdown.tools()),
        ("your_prompts", breakdown.your_prompts()),
        ("kiro_responses", breakdown.kiro_responses()),
    ] {
        valid_context_percentage(field, bucket.percent())?;
    }
    Ok(())
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn observed_metric<T>(value: Option<T>, backend_gated: bool) -> ObservedMetric<T> {
    match value {
        Some(value) => ObservedMetric::Value(value),
        None if backend_gated => ObservedMetric::Unavailable(UnavailableReason::BackendGated),
        None => ObservedMetric::Unreported,
    }
}

fn classify_outcome(
    stop_reason: StopReason,
    status: Option<&UsageTurnStatus>,
    error: &Option<String>,
) -> UsageTurnOutcome {
    if error.is_some() {
        return UsageTurnOutcome::Error;
    }
    if stop_reason == StopReason::Cancelled || matches!(status, Some(UsageTurnStatus::Aborted)) {
        return UsageTurnOutcome::Cancelled;
    }
    if stop_reason != StopReason::EndTurn || matches!(status, Some(UsageTurnStatus::Other(_))) {
        return UsageTurnOutcome::Error;
    }
    UsageTurnOutcome::Success
}

fn observe_tool(tools: &mut HashMap<ToolCallId, ObservedTool>, call: &ToolCall) {
    let failed = call.status() == ToolCallStatus::Failed;
    let argument_chars = json_value_chars(call.raw_input(), "tool raw_input");
    let result_chars = json_value_chars(call.raw_output(), "tool raw_output");
    tools
        .entry(call.id().clone())
        .and_modify(|current| {
            if call.kind() != ToolKind::Other || current.kind == ToolKind::Other {
                current.kind = call.kind();
            }
            current.failed |= failed;
            if argument_chars.is_some() {
                current.argument_chars = argument_chars;
            }
            if result_chars.is_some() {
                current.result_chars = result_chars;
            }
        })
        .or_insert(ObservedTool {
            kind: call.kind(),
            failed,
            argument_chars,
            result_chars,
        });
}

fn json_value_chars(value: Option<&serde_json::Value>, field: &'static str) -> Option<u64> {
    let value = value?;
    let encoded = match serde_json::to_string(value) {
        Ok(encoded) => encoded,
        Err(error) => {
            tracing::warn!(error = %error, field, "usage tool JSON serialization failed");
            return None;
        }
    };
    match u64::try_from(encoded.chars().count()) {
        Ok(count) => Some(count),
        Err(error) => {
            tracing::warn!(error = %error, field, "usage tool character count exceeds u64");
            None
        }
    }
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

    fn from_connection(mut connection: Connection) -> Result<Self, UsageError> {
        connection
            .busy_timeout(STARTUP_BUSY_TIMEOUT)
            .map_err(UsageError::Configure)?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(UsageError::Configure)?;
        Self::migrate_schema(&mut connection)?;
        connection
            .execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(UsageError::Configure)?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(UsageError::Configure)?;
        Ok(Self { connection })
    }

    fn migrate_schema(connection: &mut Connection) -> Result<(), UsageError> {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(UsageError::Configure)?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS usage_turns (
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
        let mut version = transaction
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .map_err(UsageError::Configure)?;
        if version == 0 {
            transaction
                .execute_batch(
                    "ALTER TABLE usage_turns ADD COLUMN outcome TEXT NOT NULL DEFAULT 'success';
                     ALTER TABLE usage_turns ADD COLUMN provider_requests INTEGER;
                     ALTER TABLE usage_turns ADD COLUMN token_availability TEXT NOT NULL DEFAULT 'unreported';
                     ALTER TABLE usage_turns ADD COLUMN cost_availability TEXT NOT NULL DEFAULT 'unreported';
                     UPDATE usage_turns
                        SET outcome = CASE
                            WHEN error IS NOT NULL THEN 'error'
                            WHEN stop_reason = 'cancelled' THEN 'cancelled'
                            WHEN stop_reason <> 'end_turn' THEN 'error'
                            ELSE 'success'
                        END,
                            token_availability = CASE
                                WHEN total_tokens IS NULL THEN 'unreported' ELSE 'observed' END,
                            cost_availability = CASE
                                WHEN cost_amount IS NULL THEN 'unreported' ELSE 'observed' END;
                     CREATE TABLE usage_charges (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        turn_id INTEGER NOT NULL REFERENCES usage_turns(id) ON DELETE CASCADE,
                        amount REAL NOT NULL CHECK (amount >= 0.0),
                        unit TEXT NOT NULL CHECK (length(unit) > 0),
                        unit_plural TEXT NOT NULL CHECK (length(unit_plural) > 0)
                     );
                     CREATE INDEX usage_charges_turn ON usage_charges(turn_id);
                     CREATE INDEX usage_charges_unit ON usage_charges(unit, unit_plural);
                     CREATE INDEX usage_turns_outcome
                        ON usage_turns(outcome, timestamp_ms DESC);
                     PRAGMA user_version = 2;",
                )
                .map_err(UsageError::Configure)?;
            version = 2;
        }
        if version == 2 {
            transaction
                .execute_batch(
                    "ALTER TABLE usage_tools ADD COLUMN call_id TEXT;
                     ALTER TABLE usage_tools ADD COLUMN name TEXT;
                     ALTER TABLE usage_tools ADD COLUMN argument_chars INTEGER;
                     ALTER TABLE usage_tools ADD COLUMN result_chars INTEGER;
                     CREATE UNIQUE INDEX usage_tools_turn_call
                        ON usage_tools(turn_id, call_id) WHERE call_id IS NOT NULL;
                     CREATE TABLE usage_context_latest (
                        session_id TEXT PRIMARY KEY,
                        folder TEXT NOT NULL,
                        provider TEXT,
                        model TEXT,
                        agent_type TEXT NOT NULL,
                        timestamp_ms INTEGER NOT NULL,
                        percentage REAL NOT NULL CHECK (percentage >= 0.0 AND percentage <= 100.0),
                        context_files_tokens INTEGER,
                        context_files_percent REAL,
                        session_files_tokens INTEGER,
                        session_files_percent REAL,
                        tools_tokens INTEGER,
                        tools_percent REAL,
                        your_prompts_tokens INTEGER,
                        your_prompts_percent REAL,
                        kiro_responses_tokens INTEGER,
                        kiro_responses_percent REAL
                     );
                     CREATE INDEX usage_context_latest_timestamp
                        ON usage_context_latest(timestamp_ms DESC);
                     CREATE TABLE usage_compactions (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        session_id TEXT NOT NULL,
                        folder TEXT NOT NULL,
                        provider TEXT,
                        model TEXT,
                        agent_type TEXT NOT NULL,
                        timestamp_ms INTEGER NOT NULL,
                        before_percentage REAL,
                        after_percentage REAL NOT NULL,
                        reduction_percentage_points REAL
                     );
                     CREATE INDEX usage_compactions_timestamp
                        ON usage_compactions(timestamp_ms DESC);
                     PRAGMA user_version = 3;",
                )
                .map_err(UsageError::Configure)?;
            version = 3;
        }
        if version == 3 {
            transaction
                .execute_batch(
                    "ALTER TABLE usage_turns ADD COLUMN billed_provider TEXT;
                     ALTER TABLE usage_turns ADD COLUMN billed_model TEXT;
                     CREATE INDEX usage_turns_billed_model
                        ON usage_turns(billed_provider, billed_model);
                     PRAGMA user_version = 4;",
                )
                .map_err(UsageError::Configure)?;
            version = 4;
        }
        if version != 4 {
            return Err(UsageError::UnsupportedSchema(version));
        }
        transaction.commit().map_err(UsageError::Configure)
    }

    pub fn append(&mut self, record: &UsageRecord) -> Result<UsageRecordId, UsageError> {
        let timestamp = sqlite_integer("timestamp_ms", record.timestamp_ms())?;
        let duration = sqlite_integer("duration_ms", record.duration_ms())?;
        let ttft = record
            .ttft_ms()
            .map(|value| sqlite_integer("ttft_ms", value))
            .transpose()?;
        let tokens = record.tokens().map(sqlite_tokens).transpose()?;
        let cost_amount = record.cost().map(Money::amount);
        let cost_currency = record.cost().map(Money::currency);
        let provider_requests = record
            .provider_requests()
            .map(|value| sqlite_integer("provider_requests", value))
            .transpose()?;
        let transaction = self.connection.transaction().map_err(UsageError::Write)?;
        transaction
            .execute(
                "INSERT INTO usage_turns (
                    session_id, folder, model, provider, agent_type,
                    timestamp_ms, duration_ms, ttft_ms, stop_reason, error,
                    outcome, provider_requests, token_availability, cost_availability,
                    total_tokens, input_tokens, output_tokens, thought_tokens,
                    cached_read_tokens, cached_write_tokens, cost_amount, cost_currency
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    outcome_name(record.outcome()),
                    provider_requests,
                    metric_state_name(record.token_metric()),
                    metric_state_name(record.cost_metric()),
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
            insert_usage_tool(&transaction, turn_id, tool)?;
        }
        for charge in record.charges() {
            transaction
                .execute(
                    "INSERT INTO usage_charges (turn_id, amount, unit, unit_plural)
                     VALUES (?, ?, ?, ?)",
                    params![
                        turn_id,
                        charge.amount(),
                        charge.unit(),
                        charge.unit_plural()
                    ],
                )
                .map_err(UsageError::Write)?;
        }
        transaction.commit().map_err(UsageError::Write)?;
        Ok(UsageRecordId::new(turn_id))
    }
    pub fn enrich_record(
        &mut self,
        record_id: UsageRecordId,
        billed_model_id: Option<&str>,
        tools: &[UsageTool],
    ) -> Result<(), UsageError> {
        let (billed_provider, billed_model) =
            crate::types::usage::split_model_identity(billed_model_id);
        let transaction = self.connection.transaction().map_err(UsageError::Write)?;
        let updated = transaction
            .execute(
                "UPDATE usage_turns
                 SET billed_provider = CASE WHEN ? THEN ? ELSE billed_provider END,
                     billed_model = CASE WHEN ? THEN ? ELSE billed_model END
                 WHERE id = ?",
                params![
                    billed_model_id.is_some(),
                    billed_provider,
                    billed_model_id.is_some(),
                    billed_model,
                    record_id.get(),
                ],
            )
            .map_err(UsageError::Write)?;
        if updated != 1 {
            return Err(UsageError::RecordNotFound(record_id.get()));
        }
        for tool in tools {
            let Some(call_id) = tool.call_id() else {
                insert_usage_tool(&transaction, record_id.get(), tool)?;
                continue;
            };
            let argument_chars = tool
                .argument_chars()
                .map(|value| sqlite_integer("argument_chars", value))
                .transpose()?;
            let result_chars = tool
                .result_chars()
                .map(|value| sqlite_integer("result_chars", value))
                .transpose()?;
            let updated = transaction
                .execute(
                    "UPDATE usage_tools
                     SET name = ?, kind = ?, failed = failed OR ?,
                         argument_chars = COALESCE(?, argument_chars),
                         result_chars = COALESCE(?, result_chars)
                     WHERE turn_id = ? AND call_id = ?",
                    params![
                        tool.name(),
                        tool_kind_name(tool.kind()),
                        tool.failed(),
                        argument_chars,
                        result_chars,
                        record_id.get(),
                        call_id.as_str(),
                    ],
                )
                .map_err(UsageError::Write)?;
            if updated == 0 {
                insert_usage_tool(&transaction, record_id.get(), tool)?;
            }
        }
        transaction.commit().map_err(UsageError::Write)
    }

    pub fn record_context(
        &mut self,
        sample: &UsageContextSample,
        compaction: Option<&UsageCompaction>,
    ) -> Result<(), UsageError> {
        valid_context_percentage("sample", sample.percentage)?;
        if let Some(breakdown) = sample.breakdown.as_ref() {
            validate_context_breakdown(breakdown)?;
        }
        if let Some(compaction) = compaction {
            if let Some(before) = compaction.before_percentage {
                valid_context_percentage("compaction before", before)?;
            }
            valid_context_percentage("compaction after", compaction.after_percentage)?;
            if let Some(reduction) = compaction.reduction_percentage_points {
                valid_context_percentage("compaction reduction", reduction)?;
            }
        }
        let timestamp = sqlite_integer("context timestamp_ms", sample.timestamp_ms)?;
        let breakdown = sample.breakdown.as_ref();
        let context_files_tokens = breakdown
            .map(ContextBreakdown::context_files)
            .map(ContextBucket::tokens)
            .map(|value| sqlite_integer("context_files_tokens", value))
            .transpose()?;
        let session_files_tokens = breakdown
            .map(ContextBreakdown::session_files)
            .map(ContextBucket::tokens)
            .map(|value| sqlite_integer("session_files_tokens", value))
            .transpose()?;
        let tools_tokens = breakdown
            .map(ContextBreakdown::tools)
            .map(ContextBucket::tokens)
            .map(|value| sqlite_integer("tools_tokens", value))
            .transpose()?;
        let prompts_tokens = breakdown
            .map(ContextBreakdown::your_prompts)
            .map(ContextBucket::tokens)
            .map(|value| sqlite_integer("your_prompts_tokens", value))
            .transpose()?;
        let responses_tokens = breakdown
            .map(ContextBreakdown::kiro_responses)
            .map(ContextBucket::tokens)
            .map(|value| sqlite_integer("kiro_responses_tokens", value))
            .transpose()?;
        let transaction = self.connection.transaction().map_err(UsageError::Write)?;
        transaction
            .execute(
                "INSERT INTO usage_context_latest (
                    session_id, folder, provider, model, agent_type, timestamp_ms, percentage,
                    context_files_tokens, context_files_percent,
                    session_files_tokens, session_files_percent,
                    tools_tokens, tools_percent, your_prompts_tokens, your_prompts_percent,
                    kiro_responses_tokens, kiro_responses_percent
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(session_id) DO UPDATE SET
                    folder = excluded.folder,
                    provider = excluded.provider,
                    model = excluded.model,
                    agent_type = excluded.agent_type,
                    timestamp_ms = excluded.timestamp_ms,
                    percentage = excluded.percentage,
                    context_files_tokens = excluded.context_files_tokens,
                    context_files_percent = excluded.context_files_percent,
                    session_files_tokens = excluded.session_files_tokens,
                    session_files_percent = excluded.session_files_percent,
                    tools_tokens = excluded.tools_tokens,
                    tools_percent = excluded.tools_percent,
                    your_prompts_tokens = excluded.your_prompts_tokens,
                    your_prompts_percent = excluded.your_prompts_percent,
                    kiro_responses_tokens = excluded.kiro_responses_tokens,
                    kiro_responses_percent = excluded.kiro_responses_percent",
                params![
                    sample.context.session_id().as_str(),
                    sample.context.folder(),
                    sample.context.provider(),
                    sample.context.model(),
                    sample.context.agent_type().as_str(),
                    timestamp,
                    sample.percentage,
                    context_files_tokens,
                    breakdown
                        .map(ContextBreakdown::context_files)
                        .map(ContextBucket::percent),
                    session_files_tokens,
                    breakdown
                        .map(ContextBreakdown::session_files)
                        .map(ContextBucket::percent),
                    tools_tokens,
                    breakdown
                        .map(ContextBreakdown::tools)
                        .map(ContextBucket::percent),
                    prompts_tokens,
                    breakdown
                        .map(ContextBreakdown::your_prompts)
                        .map(ContextBucket::percent),
                    responses_tokens,
                    breakdown
                        .map(ContextBreakdown::kiro_responses)
                        .map(ContextBucket::percent),
                ],
            )
            .map_err(UsageError::Write)?;
        if let Some(compaction) = compaction {
            transaction
                .execute(
                    "INSERT INTO usage_compactions (
                        session_id, folder, provider, model, agent_type, timestamp_ms,
                        before_percentage, after_percentage, reduction_percentage_points
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        compaction.context.session_id().as_str(),
                        compaction.context.folder(),
                        compaction.context.provider(),
                        compaction.context.model(),
                        compaction.context.agent_type().as_str(),
                        sqlite_integer("compaction timestamp_ms", compaction.timestamp_ms)?,
                        compaction.before_percentage,
                        compaction.after_percentage,
                        compaction.reduction_percentage_points,
                    ],
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
            context: self.context_summary()?,
            recent: self.recent(false)?,
            errors: self.recent(true)?,
        })
    }

    fn context_summary(&self) -> Result<UsageContextSummary, UsageError> {
        let latest = self
            .connection
            .query_row(
                "SELECT session_id, folder, provider, model, agent_type, timestamp_ms, percentage,
                        context_files_tokens, context_files_percent,
                        session_files_tokens, session_files_percent,
                        tools_tokens, tools_percent, your_prompts_tokens, your_prompts_percent,
                        kiro_responses_tokens, kiro_responses_percent
                 FROM usage_context_latest
                 ORDER BY timestamp_ms DESC, session_id
                 LIMIT 1",
                [],
                context_sample_from_row,
            )
            .optional()
            .map_err(UsageError::Query)?;
        let invalid_compactions: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM usage_compactions
                 WHERE (before_percentage IS NOT NULL
                        AND before_percentage NOT BETWEEN 0.0 AND 100.0)
                    OR after_percentage NOT BETWEEN 0.0 AND 100.0
                    OR (reduction_percentage_points IS NOT NULL
                        AND reduction_percentage_points NOT BETWEEN 0.0 AND 100.0)",
                [],
                |row| row.get(0),
            )
            .map_err(UsageError::Query)?;
        if invalid_compactions != 0 {
            return Err(UsageError::CorruptValue {
                field: "usage_compactions percentage",
                value: format!("{invalid_compactions} invalid rows"),
            });
        }
        self.connection
            .query_row(
                "SELECT COUNT(*), COUNT(reduction_percentage_points),
                        SUM(reduction_percentage_points), AVG(reduction_percentage_points)
                 FROM usage_compactions",
                [],
                |row| {
                    Ok(UsageContextSummary {
                        latest,
                        compactions: row_u64(row, 0, "compactions")?,
                        sampled_compactions: row_u64(row, 1, "sampled compactions")?,
                        total_reduction_percentage_points: row.get(2)?,
                        average_reduction_percentage_points: row.get(3)?,
                    })
                },
            )
            .map_err(UsageError::Query)
    }

    /// Nearest-rank p90 latency for one grouping, wrapped so the caller can
    /// tell "computed" from "unavailable".
    ///
    /// A failure here is deliberately NOT fatal to `snapshot()`. These are the
    /// only queries in this file that use window functions over an aliased
    /// subquery, so they are the most likely to break first; degrading two
    /// fields to `None` is strictly better than losing a panel whose requests,
    /// tokens, costs and charges were all computable (cyril-9kyk review).
    fn latency_p90(&self, keys: &[String], grouping: &'static str) -> LatencyLookup {
        let map = match self.latency_p90_map(keys) {
            Ok(map) => Some(map),
            Err(error) => {
                tracing::warn!(
                    grouping,
                    error = %error,
                    "latency p90 query failed; this rollup reports p90 as unavailable"
                );
                None
            }
        };
        LatencyLookup { grouping, map }
    }

    /// Runs the nearest-rank queries for one grouping and folds them into a
    /// map keyed by the grouping columns.
    ///
    /// One implementation covers every arity — the ungrouped overview passes
    /// `&[]` and lands under the empty key — and every metric, so a change to
    /// the fold cannot be applied to two of three call sites and missed on the
    /// third.
    ///
    /// `duration_ms` is `NOT NULL`, so its pass visits every row and therefore
    /// creates an entry for every group the matching rollup can produce. That
    /// is what makes a later lookup miss an invariant violation rather than
    /// absence (see `LatencyLookup::get`).
    ///
    /// Statements are prepared per call rather than through `prepare_cached`.
    /// That cache needs rusqlite's `cache` feature (and `hashlink`) against a
    /// workspace policy of explicit minimal features, and it would buy nothing
    /// measurable: preparing and planning all ten of these statements costs
    /// 0.028 ms against ~570 ms of query time — 0.005% (measured 2026-08-28).
    /// The cost here is the sort, not the parse.
    fn latency_p90_map(&self, keys: &[String]) -> Result<LatencyMap, UsageError> {
        let arity = keys.len();
        let mut result = LatencyMap::new();
        for metric in LatencyMetric::ALL {
            let sql = latency_p90_sql(keys, metric);
            let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
            let rows = statement
                .query_map([], |row: &Row<'_>| {
                    let key = (0..arity)
                        .map(|index| row.get::<_, Option<String>>(index))
                        .collect::<rusqlite::Result<Vec<Option<String>>>>()?;
                    Ok((key, row.get::<_, Option<f64>>(arity)?))
                })
                .map_err(UsageError::Query)?;
            for row in rows {
                let (key, p90) = row.map_err(UsageError::Query)?;
                metric.store(result.entry(key).or_default(), p90);
            }
        }
        Ok(result)
    }

    fn overview(&self) -> Result<UsageSummary, UsageError> {
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM usage_turns"))
            .map_err(UsageError::Query)?;
        let costs = self.cost_totals(None)?;
        let charges = self.charge_totals(None)?;
        let latency = self.latency_p90(&[], "overview");
        statement
            .query_row([], |row| {
                summary_from_row(row, 0, costs, charges, latency.get(&[]))
            })
            .map_err(UsageError::Query)
    }

    fn named_groups(&self, column: &'static str) -> Result<Vec<NamedUsageGroup>, UsageError> {
        let sql = format!(
            "SELECT {column}, {SUMMARY_COLUMNS} FROM usage_turns GROUP BY {column} ORDER BY COUNT(*) DESC, {column}"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let costs = self.named_cost_totals(column)?;
        let charges = self.named_charge_totals(column)?;
        let latency_keys = [column.to_owned()];
        let latency = self.latency_p90(&latency_keys, column);
        let rows = statement
            .query_map([], |row| {
                let name: Option<String> = row.get(0)?;
                let summary = summary_from_row(
                    row,
                    1,
                    costs.get(&name).cloned().unwrap_or_default(),
                    charges.get(&name).cloned().unwrap_or_default(),
                    latency.get(std::slice::from_ref(&name)),
                )?;
                Ok(NamedUsageGroup { name, summary })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn model_groups(&self) -> Result<Vec<ModelUsageGroup>, UsageError> {
        let keys = model_group_keys("").join(", ");
        let sql = format!(
            "SELECT {keys}, {SUMMARY_COLUMNS}
             FROM usage_turns
             GROUP BY {keys}
             ORDER BY COUNT(*) DESC, {keys}"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let costs = self.model_cost_totals()?;
        let charges = self.model_charge_totals()?;
        let latency = self.latency_p90(&model_group_keys(""), "model");
        let rows = statement
            .query_map([], |row| {
                let provider: Option<String> = row.get(0)?;
                let model: Option<String> = row.get(1)?;
                let key = (provider.clone(), model.clone());
                let summary = summary_from_row(
                    row,
                    2,
                    costs.get(&key).cloned().unwrap_or_default(),
                    charges.get(&key).cloned().unwrap_or_default(),
                    latency.get(&[provider.clone(), model.clone()]),
                )?;
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
        let charges = self.named_charge_totals("agent_type")?;
        let latency = self.latency_p90(&["agent_type".to_owned()], "agent_type");
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
                let latency_key = Some(raw.clone());
                let summary = summary_from_row(
                    row,
                    1,
                    costs.get(&latency_key).cloned().unwrap_or_default(),
                    charges.get(&Some(raw)).cloned().unwrap_or_default(),
                    latency.get(std::slice::from_ref(&latency_key)),
                )?;
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
                 SELECT tools.name, tools.kind,
                        COUNT(*) AS calls,
                        SUM(tools.failed) AS errors,
                        COUNT(tools.argument_chars) AS argument_rows,
                        SUM(tools.argument_chars) AS argument_chars,
                        COUNT(tools.result_chars) AS result_rows,
                        SUM(tools.result_chars) AS result_chars,
                        MAX(turns.timestamp_ms) AS last_used,
                        SUM(turns.total_tokens * 1.0 / counts.calls) AS total_share,
                        SUM(turns.output_tokens * 1.0 / counts.calls) AS output_share
                 FROM usage_tools AS tools
                 JOIN call_counts AS counts ON counts.turn_id = tools.turn_id
                 JOIN usage_turns AS turns ON turns.id = tools.turn_id
                 GROUP BY tools.name, tools.kind
                 ORDER BY calls DESC, tools.name, tools.kind",
            )
            .map_err(UsageError::Query)?;
        let costs = self.tool_cost_totals()?;
        let charges = self.tool_charge_totals()?;
        let models = self.tool_model_groups()?;
        let rows = statement
            .query_map([], |row| {
                let name: Option<String> = row.get(0)?;
                let raw: String = row.get(1)?;
                let kind = tool_kind_from_name(&raw).ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(StoredValueError::new("tool kind", raw.clone())),
                    )
                })?;
                let key = (name.clone(), raw);
                let argument_rows = row_u64(row, 4, "tool argument rows")?;
                let result_rows = row_u64(row, 6, "tool result rows")?;
                Ok(ToolUsageGroup {
                    name,
                    kind,
                    calls: row_u64(row, 2, "tool calls")?,
                    errors: row_u64(row, 3, "tool errors")?,
                    argument_chars: (argument_rows > 0)
                        .then(|| row_u64(row, 5, "tool argument chars"))
                        .transpose()?,
                    result_chars: (result_rows > 0)
                        .then(|| row_u64(row, 7, "tool result chars"))
                        .transpose()?,
                    last_used_ms: row_u64(row, 8, "tool last used")?,
                    total_tokens_share: row.get(9)?,
                    output_tokens_share: row.get(10)?,
                    costs: costs.get(&key).cloned().unwrap_or_default(),
                    charges: charges.get(&key).cloned().unwrap_or_default(),
                    models: models.get(&key).cloned().unwrap_or_default(),
                })
            })
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn recent(&self, errors_only: bool) -> Result<Vec<RecentUsage>, UsageError> {
        let where_clause = if errors_only {
            "WHERE outcome = 'error'"
        } else {
            ""
        };
        let [provider, model] = model_group_keys("");
        let sql = format!(
            "SELECT id, session_id, folder, {model},
                    {provider}, agent_type,
                    timestamp_ms, duration_ms, ttft_ms, stop_reason, outcome,
                    provider_requests, token_availability, cost_availability,
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
        let mut records = Vec::new();
        for row in rows {
            let (turn_id, mut record) = row.map_err(UsageError::Query)?;
            record.charges = self.charges_for_turn(turn_id)?;
            records.push(record);
        }
        Ok(records)
    }

    fn charges_for_turn(&self, turn_id: i64) -> Result<Vec<MeteredAmount>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT unit, unit_plural, amount
                 FROM usage_charges WHERE turn_id = ? ORDER BY unit, unit_plural",
            )
            .map_err(UsageError::Query)?;
        let rows = statement
            .query_map([turn_id], |row| metered_amount_from_row(row, 0, 1, 2))
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

    fn charge_totals(&self, where_sql: Option<&str>) -> Result<Vec<MeteredAmount>, UsageError> {
        let suffix = where_sql.unwrap_or("");
        let sql = format!(
            "SELECT charges.unit, charges.unit_plural, SUM(charges.amount)
             FROM usage_charges AS charges
             JOIN usage_turns AS turns ON turns.id = charges.turn_id
             WHERE 1 = 1 {suffix}
             GROUP BY charges.unit, charges.unit_plural
             ORDER BY charges.unit, charges.unit_plural"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| metered_amount_from_row(row, 0, 1, 2))
            .map_err(UsageError::Query)?;
        collect_rows(rows)
    }

    fn named_charge_totals(
        &self,
        column: &'static str,
    ) -> Result<HashMap<Option<String>, Vec<MeteredAmount>>, UsageError> {
        let sql = format!(
            "SELECT turns.{column}, charges.unit, charges.unit_plural, SUM(charges.amount)
             FROM usage_charges AS charges
             JOIN usage_turns AS turns ON turns.id = charges.turn_id
             GROUP BY turns.{column}, charges.unit, charges.unit_plural
             ORDER BY charges.unit, charges.unit_plural"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key: Option<String> = row.get(0)?;
                Ok((key, metered_amount_from_row(row, 1, 2, 3)?))
            })
            .map_err(UsageError::Query)?;
        let mut result: HashMap<Option<String>, Vec<MeteredAmount>> = HashMap::new();
        for row in rows {
            let (key, amount) = row.map_err(UsageError::Query)?;
            result.entry(key).or_default().push(amount);
        }
        Ok(result)
    }

    fn model_charge_totals(
        &self,
    ) -> Result<HashMap<ModelGroupKey, Vec<MeteredAmount>>, UsageError> {
        let keys = model_group_keys("turns.").join(", ");
        let sql = format!(
            "SELECT {keys}, charges.unit, charges.unit_plural, SUM(charges.amount)
             FROM usage_charges AS charges
             JOIN usage_turns AS turns ON turns.id = charges.turn_id
             GROUP BY {keys}, charges.unit, charges.unit_plural
             ORDER BY charges.unit, charges.unit_plural"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key = (row.get(0)?, row.get(1)?);
                Ok((key, metered_amount_from_row(row, 2, 3, 4)?))
            })
            .map_err(UsageError::Query)?;
        let mut result = HashMap::new();
        for row in rows {
            let (key, amount) = row.map_err(UsageError::Query)?;
            result.entry(key).or_insert_with(Vec::new).push(amount);
        }
        Ok(result)
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
        let keys = model_group_keys("").join(", ");
        let sql = format!(
            "SELECT {keys}, cost_currency, SUM(cost_amount)
             FROM usage_turns
             WHERE cost_amount IS NOT NULL
             GROUP BY {keys}, cost_currency
             ORDER BY cost_currency"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
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

    fn tool_cost_totals(&self) -> Result<HashMap<ToolGroupKey, Vec<Money>>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH call_counts AS (
                    SELECT turn_id, COUNT(*) AS calls FROM usage_tools GROUP BY turn_id
                 )
                 SELECT tools.name, tools.kind, turns.cost_currency,
                        SUM(turns.cost_amount * 1.0 / counts.calls)
                 FROM usage_tools AS tools
                 JOIN call_counts AS counts ON counts.turn_id = tools.turn_id
                 JOIN usage_turns AS turns ON turns.id = tools.turn_id
                 WHERE turns.cost_amount IS NOT NULL
                 GROUP BY tools.name, tools.kind, turns.cost_currency
                 ORDER BY turns.cost_currency",
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

    fn tool_charge_totals(&self) -> Result<HashMap<ToolGroupKey, Vec<MeteredAmount>>, UsageError> {
        let mut statement = self
            .connection
            .prepare(
                "WITH call_counts AS (
                    SELECT turn_id, COUNT(*) AS calls FROM usage_tools GROUP BY turn_id
                 )
                 SELECT tools.name, tools.kind, charges.unit, charges.unit_plural,
                        SUM(charges.amount * 1.0 / counts.calls)
                 FROM usage_tools AS tools
                 JOIN call_counts AS counts ON counts.turn_id = tools.turn_id
                 JOIN usage_charges AS charges ON charges.turn_id = tools.turn_id
                 GROUP BY tools.name, tools.kind, charges.unit, charges.unit_plural
                 ORDER BY charges.unit, charges.unit_plural",
            )
            .map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key = (row.get(0)?, row.get(1)?);
                Ok((key, metered_amount_from_row(row, 2, 3, 4)?))
            })
            .map_err(UsageError::Query)?;
        let mut result = HashMap::new();
        for row in rows {
            let (key, amount) = row.map_err(UsageError::Query)?;
            result.entry(key).or_insert_with(Vec::new).push(amount);
        }
        Ok(result)
    }

    fn tool_model_groups(
        &self,
    ) -> Result<HashMap<ToolGroupKey, Vec<ToolModelUsageGroup>>, UsageError> {
        let keys = model_group_keys("turns.").join(", ");
        let sql = format!(
            "SELECT tools.name, tools.kind, {keys}, COUNT(*), SUM(tools.failed)
             FROM usage_tools AS tools
             JOIN usage_turns AS turns ON turns.id = tools.turn_id
             GROUP BY tools.name, tools.kind, {keys}
             ORDER BY COUNT(*) DESC, {keys}"
        );
        let mut statement = self.connection.prepare(&sql).map_err(UsageError::Query)?;
        let rows = statement
            .query_map([], |row| {
                let key = (row.get(0)?, row.get(1)?);
                Ok((
                    key,
                    ToolModelUsageGroup {
                        provider: row.get(2)?,
                        model: row.get(3)?,
                        calls: row_u64(row, 4, "tool model calls")?,
                        errors: row_u64(row, 5, "tool model errors")?,
                    },
                ))
            })
            .map_err(UsageError::Query)?;
        let mut result = HashMap::new();
        for row in rows {
            let (key, group) = row.map_err(UsageError::Query)?;
            result.entry(key).or_insert_with(Vec::new).push(group);
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

fn insert_usage_tool(
    transaction: &Transaction<'_>,
    turn_id: i64,
    tool: &UsageTool,
) -> Result<(), UsageError> {
    transaction
        .execute(
            "INSERT INTO usage_tools (
                turn_id, call_id, name, kind, failed, argument_chars, result_chars
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                turn_id,
                tool.call_id().map(ToolCallId::as_str),
                tool.name(),
                tool_kind_name(tool.kind()),
                tool.failed(),
                tool.argument_chars()
                    .map(|value| sqlite_integer("argument_chars", value))
                    .transpose()?,
                tool.result_chars()
                    .map(|value| sqlite_integer("result_chars", value))
                    .transpose()?,
            ],
        )
        .map_err(UsageError::Write)?;
    Ok(())
}

const SUMMARY_COLUMNS: &str = "
    COUNT(*) AS requests,
    COALESCE(SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END), 0) AS successes,
    COALESCE(SUM(CASE WHEN outcome = 'cancelled' THEN 1 ELSE 0 END), 0) AS cancelled,
    COALESCE(SUM(CASE WHEN outcome = 'error' THEN 1 ELSE 0 END), 0) AS errors,
    COUNT(provider_requests) AS provider_request_rows,
    COALESCE(SUM(provider_requests), 0) AS provider_requests,
    COALESCE(SUM(CASE WHEN provider_requests > 0 THEN provider_requests - 1 ELSE 0 END), 0)
        AS retries,
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
        THEN output_tokens * 1000.0 / duration_ms ELSE NULL END) AS avg_tps,
    COALESCE(SUM(CASE WHEN token_availability = 'observed' THEN 1 ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN token_availability = 'unreported' THEN 1 ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN token_availability = 'backend_gated' THEN 1 ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN cost_availability = 'observed' THEN 1 ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN cost_availability = 'unreported' THEN 1 ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN cost_availability = 'backend_gated' THEN 1 ELSE 0 END), 0),
    CAST(MAX(duration_ms) AS REAL) AS max_duration,
    CAST(MAX(ttft_ms) AS REAL) AS max_ttft";

/// Nearest-rank p90 latency for one rollup group.
///
/// `max` is deliberately absent. `MAX(duration_ms)` and `MAX(ttft_ms)` need no
/// window function: they are plain aggregates over exactly the rows and groups
/// `SUMMARY_COLUMNS` already scans, so they ride the main rollup query, cost
/// nothing extra, and cannot skew against it (cyril-9kyk review).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct LatencyP90 {
    duration_ms: Option<f64>,
    ttft_ms: Option<f64>,
}

/// p90 by grouping-column tuple. The ungrouped overview uses the empty key.
type LatencyMap = HashMap<Vec<Option<String>>, LatencyP90>;

/// One grouping's p90 map, carrying whether it is authoritative.
struct LatencyLookup {
    grouping: &'static str,
    map: Option<LatencyMap>,
}

impl LatencyLookup {
    /// p90 for one rollup group.
    ///
    /// A miss on a `Some` map is an invariant violation, not absence: the p90
    /// query reads the same unfiltered `usage_turns` the rollup does, so every
    /// group present in the rollup MUST have a row here. Missing one means the
    /// two disagree about the grouping key, which would otherwise render as a
    /// blank column and log nothing (cyril-9kyk review). Contrast `costs`,
    /// where an empty entry genuinely means "no priced turns in this group".
    ///
    /// `None` means the query already failed and warned; staying quiet here
    /// avoids one duplicate warning per group.
    fn get(&self, key: &[Option<String>]) -> LatencyP90 {
        let Some(map) = self.map.as_ref() else {
            return LatencyP90::default();
        };
        match map.get(key) {
            Some(stats) => *stats,
            None => {
                tracing::warn!(
                    grouping = self.grouping,
                    key = ?key,
                    "no latency p90 row for a rollup group; the p90 query and the \
                     rollup disagree about the grouping key"
                );
                LatencyP90::default()
            }
        }
    }
}

/// The billed-fallback `(provider, model)` grouping key.
///
/// Every query that groups by model MUST build its key here. Four sites used
/// to spell the pair out independently, and because the sibling maps are
/// matched back in Rust, a divergence between them surfaces as blank columns
/// rather than as a failure (cyril-9kyk review). `prefix` qualifies the
/// columns for queries that join `usage_turns AS turns`.
fn model_group_keys(prefix: &str) -> [String; 2] {
    [
        format!("COALESCE({prefix}billed_provider, {prefix}provider)"),
        format!("COALESCE({prefix}billed_model, {prefix}model)"),
    ]
}

/// Which latency column a nearest-rank query ranks over.
///
/// An enum, not a `&str` matched with a catch-all `else`: a third metric added
/// to `ALL` must be handled explicitly here instead of silently landing in the
/// ttft fields with no compile error and no runtime error (cyril-9kyk review).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LatencyMetric {
    Duration,
    Ttft,
}

impl LatencyMetric {
    const ALL: [Self; 2] = [Self::Duration, Self::Ttft];

    fn column(self) -> &'static str {
        match self {
            Self::Duration => "duration_ms",
            Self::Ttft => "ttft_ms",
        }
    }

    fn store(self, stats: &mut LatencyP90, p90: Option<f64>) {
        match self {
            Self::Duration => stats.duration_ms = p90,
            Self::Ttft => stats.ttft_ms = p90,
        }
    }
}

/// Builds the nearest-rank p90 query for one latency column.
///
/// `keys` are the grouping expressions — empty for the ungrouped overview.
/// They are aliased to `k0`, `k1`, … in the inner select because a grouping
/// key can be an expression (`COALESCE(billed_provider, provider)`), and an
/// unnamed expression column is not resolvable from the outer query.
///
/// Nearest rank at 1-based position `ceil(0.9 × N)` is the smallest value `v`
/// with `count(<= v) >= 0.9N`, which is exactly `MIN(v)` over the rows whose
/// `CUME_DIST` has reached `0.9`. Ties need no special handling: `CUME_DIST`
/// counts peers, so duplicates share one value.
///
/// `WHERE {column} IS NOT NULL` is what keeps the ttft rank honest: `ttft_ms`
/// is nullable, and letting NULL rows into the partition inflates the rank
/// denominator and shifts the percentile. It is a no-op for `duration_ms`,
/// which the schema declares NOT NULL, so both metrics share one code path.
///
/// **One query per (grouping, metric) is the measured optimum.** Merging both
/// metrics into a single scan — rescaling the ttft distribution from a running
/// `COUNT(ttft_ms)` so the `IS NOT NULL` filter could be dropped — was built
/// and benchmarked on 2026-08-28 at 100k rows: it produced identical values
/// and ran **70% slower** (756 ms vs 441 ms), because the third window has to
/// re-sort by the partition key and the ttft window loses the filter that had
/// been keeping 40% of the rows out of its sort. A `ROW_NUMBER` + `COUNT`
/// formulation measured 58% slower again (696 ms). Do not "optimize" this back
/// into one query without re-running that measurement.
///
/// `MAX` is deliberately absent: it needs no window function and rides
/// `SUMMARY_COLUMNS` instead, over exactly the rows the rollup already scans.
/// Removing it from this subquery is where the real saving came from.
fn latency_p90_sql(keys: &[String], metric: LatencyMetric) -> String {
    let column = metric.column();
    let stats = format!("CAST(MIN(CASE WHEN cd >= 0.9 THEN {column} END) AS REAL)");
    if keys.is_empty() {
        return format!(
            "SELECT {stats} \
             FROM (SELECT {column}, CUME_DIST() OVER (ORDER BY {column}) AS cd \
                   FROM usage_turns WHERE {column} IS NOT NULL)"
        );
    }
    let aliased: Vec<String> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| format!("{key} AS k{index}"))
        .collect();
    let aliases: Vec<String> = (0..keys.len()).map(|index| format!("k{index}")).collect();
    let aliases = aliases.join(", ");
    format!(
        "SELECT {aliases}, {stats} \
         FROM (SELECT {inner}, {column}, \
                      CUME_DIST() OVER (PARTITION BY {partition} ORDER BY {column}) AS cd \
               FROM usage_turns WHERE {column} IS NOT NULL) \
         GROUP BY {aliases}",
        inner = aliased.join(", "),
        partition = keys.join(", "),
    )
}

fn summary_from_row(
    row: &Row<'_>,
    base: usize,
    costs: Vec<Money>,
    charges: Vec<MeteredAmount>,
    latency: LatencyP90,
) -> rusqlite::Result<UsageSummary> {
    let requests = row_u64(row, base, "requests")?;
    let successes = row_u64(row, base + 1, "successes")?;
    let cancelled = row_u64(row, base + 2, "cancelled")?;
    let errors = row_u64(row, base + 3, "errors")?;
    let provider_rows = row_u64(row, base + 4, "provider request rows")?;
    let token_rows = row_u64(row, base + 7, "token rows")?;
    let tokens = if token_rows == 0 {
        None
    } else {
        Some(TokenTotals {
            total: row_u64(row, base + 8, "total tokens")?,
            input: row_u64(row, base + 9, "input tokens")?,
            output: row_u64(row, base + 10, "output tokens")?,
            thought: row_u64(row, base + 11, "thought tokens")?,
            cached_read: row_u64(row, base + 12, "cached read tokens")?,
            cached_write: row_u64(row, base + 13, "cached write tokens")?,
        })
    };
    let cache_rate = tokens.as_ref().and_then(|totals| {
        let denominator = totals.input.saturating_add(totals.cached_read);
        (denominator > 0).then(|| totals.cached_read as f64 / denominator as f64)
    });
    Ok(UsageSummary {
        requests,
        successes,
        cancelled,
        errors,
        provider_requests: (provider_rows > 0)
            .then(|| row_u64(row, base + 5, "provider requests"))
            .transpose()?,
        retries: (provider_rows > 0)
            .then(|| row_u64(row, base + 6, "provider retries"))
            .transpose()?,
        tokens,
        token_coverage: MetricCoverage {
            observed: row_u64(row, base + 17, "observed token rows")?,
            unreported: row_u64(row, base + 18, "unreported token rows")?,
            backend_gated: row_u64(row, base + 19, "backend-gated token rows")?,
        },
        costs,
        cost_coverage: MetricCoverage {
            observed: row_u64(row, base + 20, "observed cost rows")?,
            unreported: row_u64(row, base + 21, "unreported cost rows")?,
            backend_gated: row_u64(row, base + 22, "backend-gated cost rows")?,
        },
        charges,
        cache_rate,
        avg_duration_ms: row.get(base + 14)?,
        avg_ttft_ms: row.get(base + 15)?,
        avg_tokens_per_second: row.get(base + 16)?,
        p90_duration_ms: latency.duration_ms,
        max_duration_ms: row.get(base + 23)?,
        p90_ttft_ms: latency.ttft_ms,
        max_ttft_ms: row.get(base + 24)?,
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

fn metered_amount_from_row(
    row: &Row<'_>,
    unit_index: usize,
    plural_index: usize,
    amount_index: usize,
) -> rusqlite::Result<MeteredAmount> {
    let unit: String = row.get(unit_index)?;
    let unit_plural: String = row.get(plural_index)?;
    let amount: f64 = row.get(amount_index)?;
    MeteredAmount::try_new(amount, unit.clone(), unit_plural.clone()).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            amount_index,
            rusqlite::types::Type::Real,
            Box::new(StoredValueError::new(
                "metered amount",
                format!("{amount} {unit}/{unit_plural}: {error}"),
            )),
        )
    })
}

fn context_sample_from_row(row: &Row<'_>) -> rusqlite::Result<UsageContextSample> {
    let raw_agent: String = row.get(4)?;
    let agent_type = UsageAgentType::from_str(&raw_agent).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("agent_type", raw_agent.clone())),
        )
    })?;
    let provider: Option<String> = row.get(2)?;
    let model: Option<String> = row.get(3)?;
    let model_id = match (&provider, &model) {
        (Some(provider), Some(model)) => Some(format!("{provider}/{model}")),
        (None, Some(model)) => Some(model.clone()),
        (Some(provider), None) => Some(provider.clone()),
        (None, None) => None,
    };
    let buckets = [
        context_bucket_from_row(row, 7, 8, "context_files")?,
        context_bucket_from_row(row, 9, 10, "session_files")?,
        context_bucket_from_row(row, 11, 12, "tools")?,
        context_bucket_from_row(row, 13, 14, "your_prompts")?,
        context_bucket_from_row(row, 15, 16, "kiro_responses")?,
    ];
    let breakdown = match buckets {
        [
            Some(context_files),
            Some(session_files),
            Some(tools),
            Some(prompts),
            Some(responses),
        ] => Some(ContextBreakdown::new(
            context_files,
            session_files,
            tools,
            prompts,
            responses,
        )),
        [None, None, None, None, None] => None,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Null,
                Box::new(StoredValueError::new(
                    "context breakdown",
                    "partially populated bucket set",
                )),
            ));
        }
    };
    let percentage: f64 = row.get(6)?;
    if !percentage.is_finite() || !(0.0..=100.0).contains(&percentage) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Real,
            Box::new(StoredValueError::new(
                "context percentage",
                percentage.to_string(),
            )),
        ));
    }
    Ok(UsageContextSample {
        context: TurnUsageContext::new(
            SessionId::new(row.get::<_, String>(0)?),
            row.get::<_, String>(1)?,
            model_id.as_deref(),
            agent_type,
        ),
        timestamp_ms: stored_u64(5, "context timestamp_ms", row.get(5)?)?,
        percentage,
        breakdown,
    })
}

fn context_bucket_from_row(
    row: &Row<'_>,
    tokens_index: usize,
    percent_index: usize,
    field: &'static str,
) -> rusqlite::Result<Option<ContextBucket>> {
    let tokens: Option<i64> = row.get(tokens_index)?;
    let percent: Option<f64> = row.get(percent_index)?;
    match (tokens, percent) {
        (None, None) => Ok(None),
        (Some(tokens), Some(percent)) => {
            ContextBucket::try_new(stored_u64(tokens_index, field, tokens)?, percent)
                .map(Some)
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        percent_index,
                        rusqlite::types::Type::Real,
                        Box::new(StoredValueError::new(field, error.to_string())),
                    )
                })
        }
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            tokens_index,
            rusqlite::types::Type::Null,
            Box::new(StoredValueError::new(field, "partial bucket")),
        )),
    }
}

fn recent_from_row(row: &Row<'_>) -> rusqlite::Result<(i64, RecentUsage)> {
    let turn_id: i64 = row.get(0)?;
    let raw_agent: String = row.get(5)?;
    let agent_type = UsageAgentType::from_str(&raw_agent).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("agent_type", raw_agent.clone())),
        )
    })?;
    let raw_stop: String = row.get(9)?;
    let stop_reason = stop_reason_from_name(&raw_stop).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            9,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("stop_reason", raw_stop.clone())),
        )
    })?;
    let raw_outcome: String = row.get(10)?;
    let outcome = outcome_from_name(&raw_outcome).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            10,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new("outcome", raw_outcome.clone())),
        )
    })?;
    let total: Option<i64> = row.get(14)?;
    let tokens = total
        .map(|total| {
            Ok::<TokenUsage, rusqlite::Error>(TokenUsage::new(
                stored_u64(14, "total_tokens", total)?,
                stored_u64(15, "input_tokens", row.get(15)?)?,
                stored_u64(16, "output_tokens", row.get(16)?)?,
                stored_optional_u64(17, "thought_tokens", row.get(17)?)?,
                stored_optional_u64(18, "cached_read_tokens", row.get(18)?)?,
                stored_optional_u64(19, "cached_write_tokens", row.get(19)?)?,
            ))
        })
        .transpose()?;
    let cost_amount: Option<f64> = row.get(20)?;
    let cost_currency: Option<String> = row.get(21)?;
    let cost = match (cost_amount, cost_currency) {
        (Some(amount), Some(currency)) => {
            Some(Money::try_new(amount, currency.clone()).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    20,
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
                20,
                rusqlite::types::Type::Null,
                Box::new(StoredValueError::new("money", "partial cost row")),
            ));
        }
    };
    let token_state: String = row.get(12)?;
    let cost_state: String = row.get(13)?;
    let record = RecentUsage {
        session_id: SessionId::new(row.get::<_, String>(1)?),
        folder: row.get(2)?,
        model: row.get(3)?,
        provider: row.get(4)?,
        agent_type,
        timestamp_ms: stored_u64(6, "timestamp_ms", row.get(6)?)?,
        duration_ms: stored_u64(7, "duration_ms", row.get(7)?)?,
        ttft_ms: stored_optional_u64(8, "ttft_ms", row.get(8)?)?,
        stop_reason,
        outcome,
        provider_requests: stored_optional_u64(11, "provider_requests", row.get(11)?)?,
        token_unavailable_reason: metric_unavailable_reason(12, &token_state, tokens.is_some())?,
        tokens,
        cost_unavailable_reason: metric_unavailable_reason(13, &cost_state, cost.is_some())?,
        cost,
        charges: Vec::new(),
        error: row.get(22)?,
    };
    Ok((turn_id, record))
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

fn outcome_name(outcome: UsageTurnOutcome) -> &'static str {
    match outcome {
        UsageTurnOutcome::Success => "success",
        UsageTurnOutcome::Cancelled => "cancelled",
        UsageTurnOutcome::Error => "error",
    }
}

fn outcome_from_name(value: &str) -> Option<UsageTurnOutcome> {
    match value {
        "success" => Some(UsageTurnOutcome::Success),
        "cancelled" => Some(UsageTurnOutcome::Cancelled),
        "error" => Some(UsageTurnOutcome::Error),
        _ => None,
    }
}

fn metric_state_name<T>(metric: &ObservedMetric<T>) -> &'static str {
    match metric {
        ObservedMetric::Value(_) => "observed",
        ObservedMetric::Unreported => "unreported",
        ObservedMetric::Unavailable(UnavailableReason::BackendGated) => "backend_gated",
    }
}

fn metric_unavailable_reason(
    index: usize,
    state: &str,
    has_value: bool,
) -> rusqlite::Result<Option<UnavailableReason>> {
    match (state, has_value) {
        ("observed", true) | ("unreported", false) => Ok(None),
        ("backend_gated", false) => Ok(Some(UnavailableReason::BackendGated)),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(StoredValueError::new(
                "metric availability",
                format!("{state} with has_value={has_value}"),
            )),
        )),
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
    use crate::test_support::must_succeed;
    use crate::types::{AgentMessage, ToolCallStatus, TurnMeteringUpdate};

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

    fn turn(write: UsageWrite) -> UsageRecord {
        match write {
            UsageWrite::Turn { record, .. } => record,
            UsageWrite::Context { .. } => panic!("expected turn write"),
        }
    }

    fn persist_context(log: &mut UsageLog, write: UsageWrite) {
        match write {
            UsageWrite::Context { sample, compaction } => must_succeed(
                log.record_context(&sample, compaction.as_ref()),
                "persist context",
            ),
            UsageWrite::Turn { .. } => panic!("expected context write"),
        }
    }

    fn complete(observer: &mut UsageObserver, session: &str, now: Instant) -> UsageRecord {
        let Some(write) = observer.apply(
            &scoped(
                session,
                Notification::TurnCompleted {
                    stop_reason: StopReason::EndTurn,
                },
            ),
            now,
        ) else {
            panic!("pending turn completes");
        };
        turn(write)
    }

    #[test]
    fn timing_uses_first_agent_text_only() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("s", None), start, 100, None),
            "begin turn",
        );
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
    fn context_and_compaction_state_matrix() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        start_session(&mut observer, "context", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("context", Some("provider/model")), start, 100, None),
            "begin turn",
        );
        let breakdown = ContextBreakdown::new(
            ContextBucket::new(1, 1.0),
            ContextBucket::new(2, 2.0),
            ContextBucket::new(3, 3.0),
            ContextBucket::new(4, 4.0),
            ContextBucket::new(5, 5.0),
        );
        let Some(first) = observer.apply(
            &scoped(
                "context",
                Notification::ContextBreakdownUpdated {
                    usage_percentage: 80.0,
                    breakdown: Some(breakdown.clone()),
                },
            ),
            start,
        ) else {
            panic!("context write");
        };
        persist_context(&mut log, first);

        for phase in [CompactionPhase::Started, CompactionPhase::Completed] {
            assert!(
                observer
                    .apply(
                        &scoped(
                            "context",
                            Notification::CompactionStatus {
                                phase,
                                summary: None,
                            },
                        ),
                        start,
                    )
                    .is_none()
            );
        }
        let Some(reduced) = observer.apply(
            &scoped(
                "context",
                Notification::ContextBreakdownUpdated {
                    usage_percentage: 50.0,
                    breakdown: None,
                },
            ),
            start,
        ) else {
            panic!("post-compaction context");
        };
        persist_context(&mut log, reduced);

        for phase in [CompactionPhase::Started, CompactionPhase::Completed] {
            observer.apply(
                &scoped(
                    "context",
                    Notification::CompactionStatus {
                        phase,
                        summary: None,
                    },
                ),
                start,
            );
        }
        let Some(increased) = observer.apply(
            &scoped(
                "context",
                Notification::ContextBreakdownUpdated {
                    usage_percentage: 55.0,
                    breakdown: None,
                },
            ),
            start,
        ) else {
            panic!("increased post-compaction context");
        };
        persist_context(&mut log, increased);

        observer.apply(
            &scoped(
                "context",
                Notification::CompactionStatus {
                    phase: CompactionPhase::Started,
                    summary: None,
                },
            ),
            start,
        );
        observer.apply(
            &scoped(
                "context",
                Notification::CompactionStatus {
                    phase: CompactionPhase::Failed { error: None },
                    summary: None,
                },
            ),
            start,
        );
        let Some(after_failure) = observer.apply(
            &scoped(
                "context",
                Notification::ContextBreakdownUpdated {
                    usage_percentage: 40.0,
                    breakdown: None,
                },
            ),
            start,
        ) else {
            panic!("context after failed compaction");
        };
        persist_context(&mut log, after_failure);

        let summary = must_succeed(log.snapshot(), "snapshot").context;
        let Some(latest) = summary.latest else {
            panic!("latest context");
        };
        assert_eq!(latest.percentage, 40.0);
        assert_eq!(latest.breakdown, Some(breakdown));
        assert_eq!(summary.compactions, 2);
        assert_eq!(summary.sampled_compactions, 1);
        assert_eq!(summary.total_reduction_percentage_points, Some(30.0));
        assert_eq!(summary.average_reduction_percentage_points, Some(30.0));
    }

    #[test]
    fn kiro_turn_timeline_matrix_records_once() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);

        let cases = [
            (
                UsageTurnStatus::Success,
                Some(vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()]),
                StopReason::EndTurn,
                UsageTurnOutcome::Success,
            ),
            (
                UsageTurnStatus::Aborted,
                Some(Vec::new()),
                StopReason::Cancelled,
                UsageTurnOutcome::Cancelled,
            ),
            (
                UsageTurnStatus::Other("failed".to_owned()),
                None,
                StopReason::EndTurn,
                UsageTurnOutcome::Error,
            ),
        ];
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        for (index, (status, request_ids, stop_reason, expected_outcome)) in
            cases.into_iter().enumerate()
        {
            must_succeed(
                observer.begin_turn(context("s", Some("auto")), start, index as u64 + 1, None),
                "begin turn",
            );
            let metering = Notification::TurnMeteringUpdated(TurnMeteringUpdate::new(
                vec![
                    must_succeed(
                        MeteredAmount::try_new(0.25, "credit", "credits"),
                        "valid credit",
                    ),
                    must_succeed(
                        MeteredAmount::try_new(2.0, "request", "requests"),
                        "valid requests",
                    ),
                ],
                Some(9_999),
                Some(status.clone()),
                vec!["read_file".to_owned()],
                request_ids,
            ));
            assert!(
                observer.apply(&scoped("s", metering), start).is_none(),
                "turn metering is not a lifecycle terminal"
            );
            let Some(write) = observer.apply(
                &scoped("s", Notification::TurnCompleted { stop_reason }),
                start + Duration::from_millis(10),
            ) else {
                panic!("lifecycle completes exactly one record");
            };
            let record = turn(write);
            assert_eq!(record.outcome(), expected_outcome);
            assert_eq!(
                record.token_metric().unavailable_reason(),
                Some(UnavailableReason::BackendGated)
            );
            assert_eq!(
                record.cost_metric().unavailable_reason(),
                Some(UnavailableReason::BackendGated)
            );
            assert_eq!(
                record
                    .charges()
                    .iter()
                    .map(|charge| (charge.unit(), charge.amount()))
                    .collect::<Vec<_>>(),
                vec![("credit", 0.25), ("request", 2.0)]
            );
            assert_eq!(
                record.provider_requests(),
                match index {
                    0 => Some(3),
                    1 => Some(0),
                    _ => None,
                }
            );
            if index == 2 {
                assert_eq!(record.error(), Some("turn status: failed"));
            }
            must_succeed(log.append(&record), "append Kiro record");
        }

        let summary = must_succeed(log.snapshot(), "snapshot").overview;
        assert_eq!(summary.requests, 3);
        assert_eq!(summary.successes, 1);
        assert_eq!(summary.cancelled, 1);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.provider_requests, Some(3));
        assert_eq!(summary.retries, Some(2));
        assert_eq!(summary.token_coverage.backend_gated, 3);
        assert_eq!(summary.cost_coverage.backend_gated, 3);
        assert_eq!(
            summary
                .charges
                .iter()
                .map(|charge| (charge.unit(), charge.amount()))
                .collect::<Vec<_>>(),
            vec![("credit", 0.75), ("request", 6.0)]
        );
    }

    #[cfg(feature = "kas")]
    #[test]
    fn captured_kas_usage_rollup_matches_oracle() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas/turn_completion_2_16_0_four.jsonl");
        let raw = must_succeed(std::fs::read_to_string(fixture), "read fixture");
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        start_session(&mut observer, "captured", SessionOrigin::Fresh, start);
        for (index, line) in raw.lines().enumerate() {
            must_succeed(
                observer.begin_turn(context("captured", Some("auto")), start, index as u64, None),
                "begin captured turn",
            );
            let notification: agent_client_protocol::SessionNotification =
                must_succeed(serde_json::from_str(line), "fixture deserializes");
            let update = match &notification.update {
                agent_client_protocol::SessionUpdate::SessionInfoUpdate(update) => update,
                other => panic!("expected session_info_update, got {other:?}"),
            };
            let Some(metering) =
                crate::protocol::convert::kas::session_info_to_notification(update)
            else {
                panic!("captured metering converts");
            };
            assert!(
                observer
                    .apply(&scoped("captured", metering), start)
                    .is_none()
            );
            let record = complete(&mut observer, "captured", start);
            must_succeed(log.append(&record), "append captured turn");
        }
        let summary = must_succeed(log.snapshot(), "snapshot").overview;
        assert_eq!(summary.requests, 4);
        assert_eq!(summary.successes, 4);
        assert_eq!(summary.errors, 0);
        assert_eq!(summary.provider_requests, Some(9));
        assert_eq!(summary.retries, Some(5));
        assert_eq!(summary.token_coverage.backend_gated, 4);
        assert_eq!(summary.cost_coverage.backend_gated, 4);
        let Some(credits) = summary
            .charges
            .iter()
            .find(|charge| charge.unit() == "credit")
        else {
            panic!("credit total");
        };
        assert!((credits.amount() - 0.385_573_410_447_761_2).abs() < 1e-12);
    }

    #[test]
    #[ignore = "reference-workstation observer budget"]
    fn observer_10k_tools_budget_reference() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "budget", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("budget", None), start, 1, None),
            "begin turn",
        );
        for index in 0..10_000 {
            observer.apply(
                &scoped(
                    "budget",
                    Notification::ToolCallStarted(ToolCall::new(
                        ToolCallId::new(format!("tool-{index}")),
                        format!("Tool {index}"),
                        ToolKind::Read,
                        ToolCallStatus::Completed,
                        None,
                    )),
                ),
                start,
            );
        }
        let started = Instant::now();
        let record = complete(&mut observer, "budget", start + Duration::from_millis(1));
        let elapsed = started.elapsed();
        assert_eq!(record.tools().len(), 10_000);
        assert!(
            elapsed <= Duration::from_millis(25),
            "10,000-tool observer completion exceeded 25ms: {elapsed:?}"
        );
    }

    #[test]
    fn cumulative_cost_delta_matrix() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "fresh", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("fresh", None), start, 1, None),
            "begin first turn",
        );
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(must_succeed(
                        Money::try_new(0.003_907_2, "USD"),
                        "valid cost",
                    )),
                },
            ),
            start,
        );
        let first = complete(&mut observer, "fresh", start);
        let Some(first_cost) = first.cost() else {
            panic!("first cost");
        };
        assert!((first_cost.amount() - 0.003_907_2).abs() < 1e-12);

        must_succeed(
            observer.begin_turn(context("fresh", None), start, 2, None),
            "begin second turn",
        );
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 2,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(0.004_349, "USD"), "valid cost")),
                },
            ),
            start,
        );
        let second = complete(&mut observer, "fresh", start);
        let Some(second_cost) = second.cost() else {
            panic!("second cost");
        };
        assert!((second_cost.amount() - 0.000_441_8).abs() < 1e-12);

        must_succeed(
            observer.begin_turn(context("fresh", None), start, 3, None),
            "begin reset turn",
        );
        observer.apply(
            &scoped(
                "fresh",
                Notification::UsageUpdated {
                    used: 3,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(0.001, "EUR"), "valid cost")),
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
        must_succeed(
            observer.begin_turn(context("loaded", None), start, 1, None),
            "begin resumed turn",
        );
        observer.apply(
            &scoped(
                "loaded",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(9.0, "USD"), "valid cost")),
                },
            ),
            start,
        );
        assert!(complete(&mut observer, "loaded", start).cost().is_none());

        must_succeed(
            observer.begin_turn(context("loaded", None), start, 2, None),
            "begin next turn",
        );
        observer.apply(
            &scoped(
                "loaded",
                Notification::UsageUpdated {
                    used: 2,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(9.25, "USD"), "valid cost")),
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
        must_succeed(
            observer.begin_turn(context("s", None), start, 1, None),
            "begin turn",
        );
        observer.apply(
            &scoped(
                "s",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(0.5, "USD"), "valid cost")),
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
        must_succeed(
            observer.begin_turn(context("s", None), start, 1, None),
            "begin turn",
        );
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

    #[test]
    fn tool_call_instance_attribution_matches_oracle() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "tools", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("tools", Some("provider/model")), start, 100, None),
            "begin turn",
        );
        let first = ToolCall::new(
            ToolCallId::new("a"),
            "Read".into(),
            ToolKind::Read,
            ToolCallStatus::InProgress,
            Some(serde_json::json!({"text": "é"})),
        );
        observer.apply(
            &scoped("tools", Notification::ToolCallStarted(first.clone())),
            start,
        );
        observer.apply(
            &scoped(
                "tools",
                Notification::ToolCallUpdated(
                    first.with_raw_output(Some(serde_json::json!({"ok": "日本"}))),
                ),
            ),
            start,
        );
        observer.apply(
            &scoped(
                "tools",
                Notification::ToolCallUpdated(ToolCall::new(
                    ToolCallId::new("b"),
                    "Read again".into(),
                    ToolKind::Read,
                    ToolCallStatus::Failed,
                    Some(serde_json::json!({"x": 1})),
                )),
            ),
            start,
        );
        observer.apply(
            &scoped(
                "tools",
                Notification::TurnUsageCaptured(TokenUsage::new(90, 60, 30, None, None, None)),
            ),
            start,
        );
        observer.apply(
            &scoped(
                "tools",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(must_succeed(Money::try_new(0.9, "USD"), "valid cost")),
                },
            ),
            start,
        );
        observer.apply(
            &scoped(
                "tools",
                Notification::TurnMeteringUpdated(TurnMeteringUpdate::new(
                    vec![must_succeed(
                        MeteredAmount::try_new(0.6, "credit", "credits"),
                        "valid credit",
                    )],
                    None,
                    Some(UsageTurnStatus::Success),
                    Vec::new(),
                    None,
                )),
            ),
            start,
        );
        let record = complete(&mut observer, "tools", start + Duration::from_millis(10));
        assert_eq!(record.tools().len(), 2);
        assert_eq!(
            record
                .tools()
                .iter()
                .map(|tool| {
                    let Some(id) = tool.call_id() else {
                        panic!("observed id");
                    };
                    id.as_str()
                })
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        must_succeed(log.append(&record), "append tool turn");
        let tools = must_succeed(log.snapshot(), "snapshot").tools;
        assert_eq!(tools.len(), 1);
        let group = &tools[0];
        assert_eq!(group.name, None);
        assert_eq!(group.kind, ToolKind::Read);
        assert_eq!(group.calls, 2);
        assert_eq!(group.errors, 1);
        assert_eq!(group.argument_chars, Some(19));
        assert_eq!(group.result_chars, Some(11));
        assert_eq!(group.last_used_ms, 100);
        assert_eq!(group.total_tokens_share, Some(90.0));
        assert_eq!(group.output_tokens_share, Some(30.0));
        assert_eq!(group.costs[0].amount(), 0.9);
        assert_eq!(group.charges[0].amount(), 0.6);
        assert_eq!(group.models.len(), 1);
        assert_eq!(group.models[0].provider.as_deref(), Some("provider"));
        assert_eq!(group.models[0].model.as_deref(), Some("model"));
        assert_eq!(group.models[0].calls, 2);
        assert_eq!(group.models[0].errors, 1);
    }

    #[test]
    fn billed_model_wins_grouping_matrix() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let record_id = must_succeed(
            log.append(&record(
                "model",
                Some("auto"),
                (None, None, None),
                (10, None),
                vec![UsageTool::new(ToolKind::Read, false)],
            )),
            "append requested model",
        );
        let exact_tools = vec![UsageTool::enriched(
            ToolCallId::new("call"),
            "read_file",
            ToolKind::Read,
            false,
            Some(7),
            Some(2),
        )];
        must_succeed(
            log.enrich_record(record_id, Some("anthropic/claude-sonnet"), &exact_tools),
            "enrich billed model",
        );
        must_succeed(
            log.enrich_record(record_id, Some("anthropic/claude-sonnet"), &exact_tools),
            "repeat enrichment is idempotent",
        );
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.models[0].provider.as_deref(), Some("anthropic"));
        assert_eq!(snapshot.models[0].model.as_deref(), Some("claude-sonnet"));
        assert_eq!(snapshot.tools.len(), 2);
        let Some(enriched_tool) = snapshot
            .tools
            .iter()
            .find(|tool| tool.name.as_deref() == Some("read_file"))
        else {
            panic!("enriched tool");
        };
        assert_eq!(enriched_tool.calls, 1);
        assert_eq!(snapshot.recent[0].provider.as_deref(), Some("anthropic"));
        assert_eq!(snapshot.recent[0].model.as_deref(), Some("claude-sonnet"));

        must_succeed(
            log.connection.execute_batch(
                "CREATE TRIGGER reject_enriched_tool BEFORE UPDATE ON usage_tools
                 BEGIN SELECT RAISE(ABORT, 'forced enrichment failure'); END;",
            ),
            "failure trigger",
        );
        assert!(
            log.enrich_record(record_id, Some("wrong/model"), &exact_tools)
                .is_err()
        );
        let after_failure = must_succeed(log.snapshot(), "snapshot after rollback");
        assert_eq!(
            after_failure.models[0].model.as_deref(),
            Some("claude-sonnet")
        );
        let Some(enriched_tool) = after_failure
            .tools
            .iter()
            .find(|tool| tool.name.as_deref() == Some("read_file"))
        else {
            panic!("enriched tool after rollback");
        };
        assert_eq!(enriched_tool.calls, 1);
        assert!(matches!(
            log.enrich_record(UsageRecordId::new(i64::MAX), None, &[]),
            Err(UsageError::RecordNotFound(i64::MAX))
        ));
    }

    fn record(
        session: &str,
        model: Option<&str>,
        usage: (Option<TokenUsage>, Option<Money>, Option<&str>),
        timing: (u64, Option<u64>),
        tools: Vec<UsageTool>,
    ) -> UsageRecord {
        let (tokens, cost, error) = usage;
        let (duration_ms, ttft_ms) = timing;
        UsageRecord::new(
            context(session, model),
            UsageTiming::new(1, duration_ms, ttft_ms),
            UsageOutcome::new(
                StopReason::EndTurn,
                if error.is_some() {
                    UsageTurnOutcome::Error
                } else {
                    UsageTurnOutcome::Success
                },
                TurnUsageMetrics::new(
                    tokens.map_or(ObservedMetric::Unreported, ObservedMetric::Value),
                    cost.map_or(ObservedMetric::Unreported, ObservedMetric::Value),
                    Vec::new(),
                    None,
                    None,
                ),
                error.map(str::to_owned),
            ),
            tools,
        )
    }

    #[test]
    fn overview_matches_independent_omp_formula_oracle() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        must_succeed(
            log.append(&record(
                "s1",
                Some("p/m"),
                (
                    Some(TokenUsage::new(150, 100, 20, None, Some(30), None)),
                    Some(must_succeed(Money::try_new(1.0, "USD"), "valid cost")),
                    None,
                ),
                (100, Some(25)),
                Vec::new(),
            )),
            "append first",
        );
        must_succeed(
            log.append(&record(
                "s2",
                Some("q/n"),
                (
                    Some(TokenUsage::new(100, 50, 40, None, Some(10), None)),
                    Some(must_succeed(Money::try_new(2.0, "EUR"), "valid cost")),
                    Some("boom"),
                ),
                (200, None),
                Vec::new(),
            )),
            "append second",
        );
        let overview = must_succeed(log.snapshot(), "snapshot").overview;
        assert_eq!(overview.requests, 2);
        assert_eq!(overview.errors, 1);
        let Some(totals) = overview.tokens else {
            panic!("token totals");
        };
        assert_eq!(totals.total, 250);
        assert_eq!(totals.input, 150);
        assert_eq!(totals.output, 60);
        assert_eq!(totals.cached_read, 40);
        let Some(cache_rate) = overview.cache_rate else {
            panic!("cache rate");
        };
        assert!((cache_rate - 40.0 / 190.0).abs() < 1e-12);
        let Some(avg_duration_ms) = overview.avg_duration_ms else {
            panic!("duration");
        };
        assert!((avg_duration_ms - 150.0).abs() < 1e-12);
        assert_eq!(overview.avg_ttft_ms, Some(25.0));
        let Some(avg_tokens_per_second) = overview.avg_tokens_per_second else {
            panic!("tps");
        };
        assert!((avg_tokens_per_second - 200.0).abs() < 1e-12);
        assert_eq!(overview.costs.len(), 2);
    }

    #[test]
    fn phase1_snapshot_is_unchanged_and_observed_wins_coverage() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let standard = record(
            "omp",
            Some("openai-codex/gpt-5.6"),
            (
                Some(TokenUsage::new(150, 100, 20, None, Some(30), None)),
                Some(must_succeed(Money::try_new(1.25, "USD"), "valid cost")),
                None,
            ),
            (100, Some(25)),
            Vec::new(),
        );
        must_succeed(log.append(&standard), "append standard turn");
        let phase1 = must_succeed(log.snapshot(), "phase1 snapshot").overview;
        assert_eq!(phase1.requests, 1);
        assert_eq!(phase1.successes, 1);
        assert_eq!(phase1.cancelled, 0);
        assert_eq!(phase1.errors, 0);
        assert_eq!(phase1.tokens.as_ref().map(|value| value.total), Some(150));
        assert_eq!(phase1.token_coverage.observed, 1);
        assert_eq!(phase1.cost_coverage.observed, 1);
        assert_eq!(
            phase1.costs,
            vec![must_succeed(Money::try_new(1.25, "USD"), "valid cost",)]
        );
        assert_eq!(phase1.cache_rate, Some(30.0 / 130.0));
        assert_eq!(phase1.avg_duration_ms, Some(100.0));
        assert_eq!(phase1.avg_ttft_ms, Some(25.0));
        assert_eq!(phase1.avg_tokens_per_second, Some(200.0));

        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "kiro", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("kiro", Some("anthropic/claude")), start, 2, None),
            "begin future Kiro turn",
        );
        observer.apply(
            &scoped(
                "kiro",
                Notification::TurnMeteringUpdated(TurnMeteringUpdate::new(
                    vec![must_succeed(
                        MeteredAmount::try_new(0.25, "credit", "credits"),
                        "valid credit",
                    )],
                    Some(50),
                    Some(UsageTurnStatus::Success),
                    Vec::new(),
                    Some(vec!["request".to_owned()]),
                )),
            ),
            start,
        );
        observer.apply(
            &scoped(
                "kiro",
                Notification::TurnUsageCaptured(TokenUsage::new(10, 4, 6, None, None, None)),
            ),
            start,
        );
        observer.apply(
            &scoped(
                "kiro",
                Notification::UsageUpdated {
                    used: 1,
                    size: 10,
                    cost: Some(must_succeed(
                        Money::try_new(0.5, "USD"),
                        "valid future wire cost",
                    )),
                },
            ),
            start,
        );
        let future_kiro = complete(&mut observer, "kiro", start + Duration::from_millis(50));
        assert!(matches!(
            future_kiro.token_metric(),
            ObservedMetric::Value(_)
        ));
        assert!(matches!(
            future_kiro.cost_metric(),
            ObservedMetric::Value(_)
        ));
        must_succeed(log.append(&future_kiro), "append future Kiro turn");
        let mixed = must_succeed(log.snapshot(), "mixed snapshot").overview;
        assert_eq!(mixed.token_coverage.observed, 2);
        assert_eq!(mixed.token_coverage.backend_gated, 0);
        assert_eq!(mixed.cost_coverage.observed, 2);
        assert_eq!(mixed.cost_coverage.backend_gated, 0);
        assert_eq!(mixed.tokens.as_ref().map(|value| value.total), Some(160));
        assert_eq!(mixed.charges[0].unit(), "credit");
        assert_eq!(mixed.charges[0].amount(), 0.25);
    }

    #[test]
    fn v1_migration_is_lossless_idempotent_and_enrichment_atomic() {
        let connection = must_succeed(Connection::open_in_memory(), "legacy connection");
        must_succeed(
            connection.execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE usage_turns (
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
                 CREATE TABLE usage_tools (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    turn_id INTEGER NOT NULL REFERENCES usage_turns(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL,
                    failed INTEGER NOT NULL CHECK (failed IN (0, 1))
                 );
                 INSERT INTO usage_turns (
                    session_id, folder, model, provider, agent_type, timestamp_ms,
                    duration_ms, ttft_ms, stop_reason, total_tokens, input_tokens,
                    output_tokens, cached_read_tokens, cost_amount, cost_currency
                 ) VALUES (
                    'legacy-ok', '/old', 'm', 'p', 'main', 1, 100, 25,
                    'end_turn', 150, 100, 20, 30, 1.25, 'USD'
                 );
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason, error
                 ) VALUES ('legacy-error', '/old', 'main', 2, 200, 'end_turn', 'boom');
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason, error
                 ) VALUES ('legacy-error-cancelled', '/old', 'main', 3, 200,
                           'cancelled', 'boom-cancelled');
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason
                 ) VALUES ('legacy-cancelled', '/old', 'main', 4, 200, 'cancelled');
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason
                 ) VALUES ('legacy-max-tokens', '/old', 'main', 5, 200, 'max_tokens');
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason
                 ) VALUES ('legacy-max-requests', '/old', 'main', 6, 200,
                           'max_turn_requests');
                 INSERT INTO usage_turns (
                    session_id, folder, agent_type, timestamp_ms, duration_ms,
                    stop_reason
                 ) VALUES ('legacy-refusal', '/old', 'main', 7, 200, 'refusal');",
            ),
            "seed legacy schema",
        );

        let mut log = must_succeed(UsageLog::from_connection(connection), "migrate v1 schema");
        let version: i64 = must_succeed(
            log.connection
                .query_row("PRAGMA user_version", [], |row| row.get(0)),
            "schema version",
        );
        assert_eq!(version, 4);
        let snapshot = must_succeed(log.snapshot(), "migrated snapshot");
        assert_eq!(snapshot.overview.requests, 7);
        assert_eq!(snapshot.overview.successes, 1);
        assert_eq!(snapshot.overview.cancelled, 1);
        assert_eq!(snapshot.overview.errors, 5);
        assert_eq!(
            snapshot.overview.tokens.as_ref().map(|v| v.total),
            Some(150)
        );
        assert_eq!(snapshot.overview.token_coverage.observed, 1);
        assert_eq!(snapshot.overview.token_coverage.unreported, 6);
        assert_eq!(snapshot.overview.cost_coverage.observed, 1);
        assert_eq!(snapshot.overview.cost_coverage.unreported, 6);
        assert_eq!(
            snapshot.overview.costs,
            vec![must_succeed(Money::try_new(1.25, "USD"), "valid cost",)]
        );
        let migrated_outcomes = {
            let mut statement = must_succeed(
                log.connection
                    .prepare("SELECT session_id, outcome FROM usage_turns ORDER BY id"),
                "prepare migrated outcome query",
            );
            let rows = must_succeed(
                statement.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }),
                "query migrated outcomes",
            );
            must_succeed(
                rows.collect::<Result<Vec<_>, _>>(),
                "collect migrated outcomes",
            )
        };
        assert_eq!(
            migrated_outcomes,
            vec![
                ("legacy-ok".to_owned(), "success".to_owned()),
                ("legacy-error".to_owned(), "error".to_owned()),
                ("legacy-error-cancelled".to_owned(), "error".to_owned()),
                ("legacy-cancelled".to_owned(), "cancelled".to_owned()),
                ("legacy-max-tokens".to_owned(), "error".to_owned()),
                ("legacy-max-requests".to_owned(), "error".to_owned()),
                ("legacy-refusal".to_owned(), "error".to_owned()),
            ]
        );

        must_succeed(
            log.connection.execute_batch(
                "CREATE TRIGGER reject_usage_charge BEFORE INSERT ON usage_charges
                     BEGIN SELECT RAISE(ABORT, 'forced charge failure'); END;",
            ),
            "install failure trigger",
        );
        let charged = UsageRecord::new(
            context("new", Some("auto")),
            UsageTiming::new(3, 10, None),
            UsageOutcome::new(
                StopReason::EndTurn,
                UsageTurnOutcome::Success,
                TurnUsageMetrics::new(
                    ObservedMetric::Unavailable(UnavailableReason::BackendGated),
                    ObservedMetric::Unavailable(UnavailableReason::BackendGated),
                    vec![must_succeed(
                        MeteredAmount::try_new(0.25, "credit", "credits"),
                        "valid credit",
                    )],
                    Some(2),
                    Some(UsageTurnStatus::Success),
                ),
                None,
            ),
            Vec::new(),
        );
        assert!(log.append(&charged).is_err());
        let count: i64 = must_succeed(
            log.connection
                .query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0)),
            "count after failed charge",
        );
        assert_eq!(count, 7, "failed child insert rolls back parent");

        must_succeed(
            log.connection
                .execute_batch("DROP TRIGGER reject_usage_charge;"),
            "remove failure trigger",
        );
        let UsageLog { connection } = log;
        let reopened = must_succeed(UsageLog::from_connection(connection), "version 2 reopens");
        assert_eq!(
            must_succeed(reopened.snapshot(), "reopened snapshot"),
            snapshot,
            "migration is idempotent"
        );
    }

    #[test]
    fn concurrent_schema_open_is_idempotent() {
        let directory = must_succeed(tempfile::tempdir(), "tempdir");
        let path = std::sync::Arc::new(directory.path().join("usage.sqlite3"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let handles = (0..2)
            .map(|_| {
                let path = std::sync::Arc::clone(&path);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    UsageLog::open(path.as_ref())
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();
        for handle in handles {
            let log = match handle.join() {
                Ok(result) => must_succeed(result, "concurrent open"),
                Err(_) => panic!("open thread"),
            };
            let version: i64 = must_succeed(
                log.connection
                    .query_row("PRAGMA user_version", [], |row| row.get(0)),
                "schema version",
            );
            assert_eq!(version, 4);
        }
    }

    #[test]
    fn schema_open_waits_for_a_concurrent_startup_lock() {
        let directory = must_succeed(tempfile::tempdir(), "tempdir");
        let path = directory.path().join("usage.sqlite3");
        let blocker = must_succeed(Connection::open(&path), "open blocking connection");
        must_succeed(
            blocker.execute_batch("BEGIN IMMEDIATE;"),
            "acquire startup write lock",
        );
        let open_path = path.clone();
        let handle = std::thread::spawn(move || UsageLog::open(&open_path));
        std::thread::sleep(Duration::from_millis(400));
        must_succeed(
            blocker.execute_batch("COMMIT;"),
            "release startup write lock",
        );
        let log = match handle.join() {
            Ok(result) => must_succeed(result, "open after startup lock"),
            Err(_) => panic!("open thread"),
        };
        let version: i64 = must_succeed(
            log.connection
                .query_row("PRAGMA user_version", [], |row| row.get(0)),
            "schema version",
        );
        assert_eq!(version, 4);
    }

    #[test]
    fn v2_schema_migrates_to_detail_tables() {
        let log = must_succeed(UsageLog::open_in_memory(), "version 4 log");
        let UsageLog { connection } = log;
        must_succeed(
            connection.execute_batch(
                "DROP INDEX usage_tools_turn_call;
                 DROP INDEX usage_turns_billed_model;
                 DROP TABLE usage_context_latest;
                 DROP TABLE usage_compactions;
                 ALTER TABLE usage_tools DROP COLUMN call_id;
                 ALTER TABLE usage_tools DROP COLUMN name;
                 ALTER TABLE usage_tools DROP COLUMN argument_chars;
                 ALTER TABLE usage_tools DROP COLUMN result_chars;
                 ALTER TABLE usage_turns DROP COLUMN billed_provider;
                 ALTER TABLE usage_turns DROP COLUMN billed_model;
                 PRAGMA user_version = 2;",
            ),
            "downgrade fixture to v2 shape",
        );
        let migrated = must_succeed(UsageLog::from_connection(connection), "migrate v2 schema");
        let version: i64 = must_succeed(
            migrated
                .connection
                .query_row("PRAGMA user_version", [], |row| row.get(0)),
            "schema version",
        );
        assert_eq!(version, 4);
        let mut statement = must_succeed(
            migrated
                .connection
                .prepare("PRAGMA table_info(usage_tools)"),
            "table info",
        );
        let rows = must_succeed(
            statement.query_map([], |row| row.get::<_, String>(1)),
            "column query",
        );
        let columns = must_succeed(rows.collect::<Result<Vec<_>, _>>(), "columns");
        for expected in ["call_id", "name", "argument_chars", "result_chars"] {
            assert!(columns.iter().any(|column| column == expected));
        }
        let snapshot = must_succeed(migrated.snapshot(), "snapshot");
        assert!(snapshot.context.latest.is_none());
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let connection = must_succeed(Connection::open_in_memory(), "connection");
        must_succeed(
            connection.execute_batch("PRAGMA user_version = 99;"),
            "set unsupported version",
        );
        assert!(matches!(
            UsageLog::from_connection(connection),
            Err(UsageError::UnsupportedSchema(99))
        ));
    }

    #[test]
    fn append_is_atomic_across_record_and_tools() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        must_succeed(
            log.connection.execute_batch(
                "CREATE TRIGGER reject_usage_tool BEFORE INSERT ON usage_tools
                     BEGIN SELECT RAISE(ABORT, 'forced child failure'); END;",
            ),
            "install failure trigger",
        );
        let result = log.append(&record(
            "s",
            None,
            (None, None, None),
            (1, None),
            vec![UsageTool::new(ToolKind::Read, false)],
        ));
        assert!(result.is_err());
        let count: i64 = must_succeed(
            log.connection
                .query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0)),
            "count parents",
        );
        assert_eq!(count, 0);
    }

    #[test]
    #[ignore = "reference-workstation append budget"]
    fn append_10k_tools_budget_reference() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let tools = (0..10_000)
            .map(|_| UsageTool::new(ToolKind::Read, false))
            .collect();
        let record = record(
            "s",
            Some("provider/model"),
            (
                Some(TokenUsage::new(10, 4, 6, None, None, None)),
                Some(must_succeed(Money::try_new(0.1, "USD"), "valid cost")),
                None,
            ),
            (10, Some(2)),
            tools,
        );
        let started = Instant::now();
        must_succeed(log.append(&record), "append 10,000-tool turn");
        assert!(
            started.elapsed() <= Duration::from_millis(250),
            "10,000-tool usage append exceeded 250ms: {:?}",
            started.elapsed()
        );
    }
    #[test]
    fn invalid_values_fail_without_defaulting() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let invalid = record(
            "s",
            None,
            (
                Some(TokenUsage::new(u64::MAX, 0, 0, None, None, None)),
                None,
                None,
            ),
            (1, None),
            Vec::new(),
        );
        assert!(matches!(
            log.append(&invalid),
            Err(UsageError::IntegerRange {
                field: "total_tokens",
                value: u64::MAX
            })
        ));
        assert_eq!(
            must_succeed(log.snapshot(), "snapshot").overview.requests,
            0
        );
    }

    #[test]
    fn usage_less_error_turn_is_persisted() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "s", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("s", None), start, 1, None),
            "begin error turn",
        );
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
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        must_succeed(log.append(&record), "append error");
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        assert_eq!(snapshot.overview.requests, 1);
        assert_eq!(snapshot.overview.errors, 1);
        assert!(snapshot.overview.tokens.is_none());
        assert_eq!(snapshot.recent.len(), 1);
        assert_eq!(snapshot.errors.len(), 1);
    }

    #[test]
    fn breakdowns_are_identity_agnostic() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        for (session, model) in [("a", "openai-codex/m"), ("b", "beta/n")] {
            must_succeed(
                log.append(&record(
                    session,
                    Some(model),
                    (
                        Some(TokenUsage::new(10, 4, 6, None, None, None)),
                        None,
                        None,
                    ),
                    (10, Some(2)),
                    vec![UsageTool::new(ToolKind::Read, false)],
                )),
                "append identity",
            );
        }
        let snapshot = must_succeed(log.snapshot(), "snapshot");
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
    fn abort_turn_removes_context_identity_from_global_frames() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "aborted", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("aborted", None), start, 1, None),
            "begin turn",
        );
        assert!(observer.abort_turn(&SessionId::new("aborted")));
        assert!(
            observer
                .apply(
                    &RoutedNotification::global(Notification::ContextBreakdownUpdated {
                        usage_percentage: 50.0,
                        breakdown: None,
                    }),
                    start,
                )
                .is_none(),
            "a global context frame must not persist an aborted turn"
        );
    }

    #[test]
    fn compaction_completed_without_started_is_ignored() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "compaction", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(context("compaction", None), start, 1, None),
            "begin turn",
        );
        observer.apply(
            &scoped(
                "compaction",
                Notification::CompactionStatus {
                    phase: CompactionPhase::Completed,
                    summary: None,
                },
            ),
            start,
        );
        let Some(UsageWrite::Context { compaction, .. }) = observer.apply(
            &scoped(
                "compaction",
                Notification::ContextBreakdownUpdated {
                    usage_percentage: 20.0,
                    breakdown: None,
                },
            ),
            start,
        ) else {
            panic!("expected context write");
        };
        assert!(compaction.is_none());
    }

    #[test]
    fn no_metering_turn_retains_configured_sidecar_kind() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(&mut observer, "sidecar", SessionOrigin::Fresh, start);
        must_succeed(
            observer.begin_turn(
                context("sidecar", None),
                start,
                1,
                Some(KiroSidecarKind::Kas),
            ),
            "begin turn",
        );
        let Some(UsageWrite::Turn { sidecar_kind, .. }) = observer.apply(
            &scoped(
                "sidecar",
                Notification::TurnCompleted {
                    stop_reason: StopReason::EndTurn,
                },
            ),
            start,
        ) else {
            panic!("expected turn write");
        };
        assert_eq!(sidecar_kind, Some(KiroSidecarKind::Kas));
    }

    #[test]
    fn record_context_rejects_invalid_direct_percentages() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let invalid_sample = UsageContextSample {
            context: context("invalid", None),
            timestamp_ms: 1,
            percentage: 101.0,
            breakdown: None,
        };
        assert!(log.record_context(&invalid_sample, None).is_err());
        let invalid_breakdown = ContextBreakdown::new(
            ContextBucket::new(1, f64::NAN),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
        );
        let sample = UsageContextSample {
            context: context("invalid", None),
            timestamp_ms: 2,
            percentage: 1.0,
            breakdown: Some(invalid_breakdown),
        };
        assert!(log.record_context(&sample, None).is_err());
        assert!(
            must_succeed(log.snapshot(), "snapshot")
                .context
                .latest
                .is_none()
        );
    }
    #[test]
    fn context_snapshot_rejects_corrupt_percentages() {
        let mut bucket_log = must_succeed(UsageLog::open_in_memory(), "bucket log");
        must_succeed(
            bucket_log.record_context(
                &UsageContextSample {
                    context: context("corrupt-bucket", None),
                    timestamp_ms: 1,
                    percentage: 1.0,
                    breakdown: None,
                },
                None,
            ),
            "seed valid context",
        );
        must_succeed(
            bucket_log.connection.execute(
                "UPDATE usage_context_latest
                 SET context_files_tokens = 1, context_files_percent = 101.0",
                [],
            ),
            "corrupt bucket percentage",
        );
        assert!(matches!(bucket_log.snapshot(), Err(UsageError::Query(_))));

        let compaction_log = must_succeed(UsageLog::open_in_memory(), "compaction log");
        must_succeed(
            compaction_log.connection.execute(
                "INSERT INTO usage_compactions (
                    session_id, folder, agent_type, timestamp_ms,
                    before_percentage, after_percentage, reduction_percentage_points
                 ) VALUES ('corrupt', '/tmp', 'main', 1, 80.0, 40.0, 101.0)",
                [],
            ),
            "corrupt compaction percentage",
        );
        assert!(matches!(
            compaction_log.snapshot(),
            Err(UsageError::CorruptValue {
                field: "usage_compactions percentage",
                ..
            })
        ));
    }

    #[test]
    fn enrichment_merges_partial_and_empty_sidecar_tools() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let record_id = must_succeed(
            log.append(&record(
                "enrichment",
                None,
                (None, None, None),
                (1, None),
                vec![
                    UsageTool::observed(
                        ToolCallId::new("portable"),
                        ToolKind::Read,
                        false,
                        Some(3),
                        None,
                    ),
                    UsageTool::observed(
                        ToolCallId::new("shared"),
                        ToolKind::Read,
                        true,
                        Some(4),
                        Some(5),
                    ),
                ],
            )),
            "append record",
        );
        must_succeed(
            log.enrich_record(record_id, None, &[]),
            "empty enrichment is a no-op",
        );
        let sidecar = [UsageTool::enriched(
            ToolCallId::new("shared"),
            "write_file",
            ToolKind::Write,
            false,
            None,
            Some(9),
        )];
        must_succeed(
            log.enrich_record(record_id, Some("provider/model"), &sidecar),
            "partial enrichment",
        );
        must_succeed(
            log.enrich_record(record_id, Some("provider/model"), &sidecar),
            "repeat enrichment is idempotent",
        );
        let mut statement = must_succeed(
            log.connection.prepare(
                "SELECT call_id, name, kind, failed, argument_chars, result_chars
                 FROM usage_tools WHERE turn_id = ? ORDER BY call_id",
            ),
            "prepare tools query",
        );
        type EnrichedToolRow = (
            String,
            Option<String>,
            String,
            i64,
            Option<i64>,
            Option<i64>,
        );
        let mapped_rows = must_succeed(
            statement.query_map(
                [record_id.get()],
                |row| -> rusqlite::Result<EnrichedToolRow> {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            ),
            "query tools",
        );
        let rows: Vec<EnrichedToolRow> = must_succeed(
            mapped_rows.collect::<rusqlite::Result<_>>(),
            "collect tools",
        );
        assert_eq!(rows.len(), 2, "portable rows absent from sidecar remain");
        assert_eq!(
            rows[0],
            ("portable".into(), None, "read".into(), 0, Some(3), None)
        );
        assert_eq!(
            rows[1],
            (
                "shared".into(),
                Some("write_file".into()),
                "write".into(),
                1,
                Some(4),
                Some(9)
            )
        );
    }

    #[test]
    fn context_write_rejects_invalid_scalar_and_bucket_percentages() {
        let start = Instant::now();
        let mut observer = UsageObserver::new();
        start_session(
            &mut observer,
            "invalid-context",
            SessionOrigin::Fresh,
            start,
        );
        must_succeed(
            observer.begin_turn(context("invalid-context", None), start, 1, None),
            "begin turn",
        );
        assert!(
            observer
                .apply(
                    &scoped(
                        "invalid-context",
                        Notification::ContextBreakdownUpdated {
                            usage_percentage: -1.0,
                            breakdown: None,
                        },
                    ),
                    start,
                )
                .is_none()
        );
        let breakdown = ContextBreakdown::new(
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 0.0),
            ContextBucket::new(1, 101.0),
        );
        assert!(
            observer
                .apply(
                    &scoped(
                        "invalid-context",
                        Notification::ContextBreakdownUpdated {
                            usage_percentage: 1.0,
                            breakdown: Some(breakdown),
                        },
                    ),
                    start,
                )
                .is_none()
        );
    }

    /// Seeds 100,000 turns across one provider/model/folder/agent group, with
    /// 1,000 tool rows and one context sample. Shared by the shape fence below
    /// and the wall-clock fence that follows it.
    fn seed_hundred_thousand_turns() -> UsageLog {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let transaction = must_succeed(log.connection.transaction(), "bulk transaction");
        {
            let mut statement = must_succeed(
                transaction.prepare(
                    "INSERT INTO usage_turns (
                        session_id, folder, model, provider, agent_type,
                        timestamp_ms, duration_ms, stop_reason, outcome, error,
                        token_availability, total_tokens, input_tokens, output_tokens
                     ) VALUES (
                        ?, '/tmp', 'm', 'p', 'main', ?, 10, 'end_turn', ?, ?,
                        'observed', 3, 1, 2
                     )",
                ),
                "prepare seed",
            );
            for index in 0..100_000_i64 {
                let error: Option<&str> = (index < 30).then_some("error");
                let outcome = if error.is_some() { "error" } else { "success" };
                must_succeed(
                    statement.execute(params![format!("s{index}"), index, outcome, error]),
                    "seed row",
                );
                if index < 1_000 {
                    must_succeed(
                        transaction.execute(
                            "INSERT INTO usage_tools (
                                turn_id, call_id, name, kind, failed,
                                argument_chars, result_chars
                             ) VALUES (last_insert_rowid(), ?, ?, 'read', 0, 10, 20)",
                            params![format!("call-{index}"), format!("tool-{index}")],
                        ),
                        "seed tool",
                    );
                }
            }
        }
        must_succeed(transaction.commit(), "commit seed");
        must_succeed(
            log.record_context(
                &UsageContextSample {
                    context: context("latest", Some("p/m")),
                    timestamp_ms: 100_000,
                    percentage: 50.0,
                    breakdown: None,
                },
                None,
            ),
            "seed context",
        );
        log
    }

    /// The snapshot stays bounded in SIZE at 100,000 rows: `recent` and
    /// `errors` clamp to `RECENT_LIMIT`, the rollups collapse to their one
    /// real group, and the tool rollup carries exactly what was seeded.
    ///
    /// The wall-clock half of this test moved to the `#[ignore]`d fence below
    /// (cyril-9kyk review). It asserted `snapshot()` completes in under two
    /// seconds, in the ordinary suite, on shared CI runners — and it failed on
    /// the ubuntu leg for machine load. Because nextest cancels on first
    /// failure, that one stopwatch aborted the job and every alphabetically
    /// later test never ran. The size assertions below are deterministic and
    /// belong in CI; the stopwatch does not.
    #[test]
    fn kiro_snapshot_remains_bounded_at_100k() {
        let log = seed_hundred_thousand_turns();
        let snapshot = must_succeed(log.snapshot(), "bounded snapshot");
        assert_eq!(snapshot.overview.requests, 100_000);
        assert_eq!(snapshot.recent.len(), 20);
        assert_eq!(snapshot.errors.len(), 20);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.folders.len(), 1);
        assert_eq!(snapshot.tools.len(), 1_000);
        assert_eq!(
            snapshot
                .context
                .latest
                .as_ref()
                .map(|value| value.percentage),
            Some(50.0)
        );
    }

    /// The same 100k fixture, timed. `#[ignore]` for the reason above: run it
    /// deliberately, on a machine that is not also building three other jobs.
    ///
    /// ```sh
    /// cargo test -p cyril-core --lib -- --ignored kiro_snapshot_100k_budget
    /// ```
    ///
    /// The two-second bound is unchanged from when this assertion lived in the
    /// test above. What changed is the headroom: adding p90/max took a local
    /// run from ~342 ms to ~700-760 ms (cyril-9kyk), which is why the CI leg
    /// started tipping over. Reducing it is cyril-nanu, not a wider bound.
    #[test]
    #[ignore = "reference-workstation 100k snapshot budget"]
    fn kiro_snapshot_100k_budget_reference() {
        let log = seed_hundred_thousand_turns();
        let started = Instant::now();
        let snapshot = must_succeed(log.snapshot(), "bounded snapshot");
        let elapsed = started.elapsed();
        println!("snapshot() at 100k rows: {elapsed:?} (bound 2s)");
        assert!(
            elapsed <= Duration::from_secs(2),
            "snapshot() at 100k rows took {elapsed:?}, over the 2s bound"
        );
        assert_eq!(
            snapshot.overview.requests, 100_000,
            "positive control: the measured call really did read the fixture"
        );
    }

    /// C7 — every rollup site maps every `UsageSummary` field at its declared
    /// offset. `summary_from_row` reads by positional index and rusqlite
    /// coerces between INTEGER and REAL, so a misaligned column returns a
    /// plausible wrong number rather than an error. The destructure is
    /// exhaustive on purpose: a new field added without a mapping fails to
    /// compile here rather than silently reading `None`.
    #[test]
    fn all_rollup_sites_map_every_summary_field() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        for (session, model, duration, ttft) in [
            ("s1", Some("p1/m1"), 100_u64, Some(10_u64)),
            ("s2", Some("p1/m1"), 300, Some(30)),
            ("s3", Some("p2/m2"), 500, None),
        ] {
            must_succeed(
                log.append(&record(
                    session,
                    model,
                    (None, None, None),
                    (duration, ttft),
                    Vec::new(),
                )),
                "append record",
            );
        }
        let snapshot = must_succeed(log.snapshot(), "snapshot");

        let assert_summary = |label: &str,
                              summary: &UsageSummary,
                              requests: u64,
                              durations: Vec<u64>,
                              ttfts: Vec<u64>| {
            let UsageSummary {
                requests: got_requests,
                successes,
                cancelled,
                errors,
                provider_requests,
                retries,
                tokens,
                // Destructured through `MetricCoverage` rather than bound to
                // `_`: a field added inside it must fail to compile here too,
                // which is the guarantee this test claims (cyril-9kyk review).
                token_coverage:
                    MetricCoverage {
                        observed: token_observed,
                        unreported: token_unreported,
                        backend_gated: token_gated,
                    },
                costs,
                cost_coverage:
                    MetricCoverage {
                        observed: cost_observed,
                        unreported: cost_unreported,
                        backend_gated: cost_gated,
                    },
                charges,
                cache_rate,
                avg_duration_ms,
                avg_ttft_ms,
                avg_tokens_per_second,
                p90_duration_ms,
                max_duration_ms,
                p90_ttft_ms,
                max_ttft_ms,
            } = summary;
            assert_eq!(*got_requests, requests, "{label}: requests");
            assert_eq!(*successes, requests, "{label}: successes");
            assert_eq!(*cancelled, 0, "{label}: cancelled");
            assert_eq!(*errors, 0, "{label}: errors");
            assert_eq!(*provider_requests, None, "{label}: provider_requests");
            assert_eq!(*retries, None, "{label}: retries");
            assert_eq!(*tokens, None, "{label}: tokens");
            assert!(costs.is_empty(), "{label}: costs");
            assert!(charges.is_empty(), "{label}: charges");
            assert_eq!(*cache_rate, None, "{label}: cache_rate");
            // Every row lands in exactly one coverage bucket, so the three
            // sum to the group's request count. A bucket that stopped being
            // mapped, or a row counted twice, breaks this.
            assert_eq!(
                token_observed + token_unreported + token_gated,
                requests,
                "{label}: token coverage buckets partition the group"
            );
            assert_eq!(
                cost_observed + cost_unreported + cost_gated,
                requests,
                "{label}: cost coverage buckets partition the group"
            );
            let mean = |values: &[u64]| -> Option<f64> {
                (!values.is_empty())
                    .then(|| values.iter().sum::<u64>() as f64 / values.len() as f64)
            };
            // Asserted against the oracle mean, not `is_some()`. The previous
            // `is_some() || requests == 1` was trivially true for the one group
            // whose ttft is NULL — precisely the interesting case, asserted
            // away (cyril-9kyk review).
            assert_eq!(
                *avg_duration_ms,
                mean(&durations),
                "{label}: avg_duration_ms"
            );
            assert_eq!(*avg_ttft_ms, mean(&ttfts), "{label}: avg_ttft_ms");
            assert_eq!(
                *avg_tokens_per_second, None,
                "{label}: avg_tokens_per_second"
            );
            assert_eq!(
                *p90_duration_ms,
                nearest_rank_p90(durations.clone()),
                "{label}: p90_duration_ms"
            );
            assert_eq!(
                *max_duration_ms,
                durations.iter().max().map(|value| *value as f64),
                "{label}: max_duration_ms"
            );
            assert_eq!(
                *p90_ttft_ms,
                nearest_rank_p90(ttfts.clone()),
                "{label}: p90_ttft_ms"
            );
            assert_eq!(
                *max_ttft_ms,
                ttfts.iter().max().map(|value| *value as f64),
                "{label}: max_ttft_ms"
            );
        };

        let all_durations = vec![100, 300, 500];
        let all_ttfts = vec![10, 30];
        let split = |first: bool| -> (u64, Vec<u64>, Vec<u64>) {
            if first {
                (2, vec![100, 300], vec![10, 30])
            } else {
                (1, vec![500], Vec::new())
            }
        };

        assert_summary(
            "overview",
            &snapshot.overview,
            3,
            all_durations.clone(),
            all_ttfts.clone(),
        );
        assert_eq!(snapshot.providers.len(), 2, "provider group count");
        for group in &snapshot.providers {
            let (requests, durations, ttfts) = split(group.name.as_deref() == Some("p1"));
            assert_summary("providers", &group.summary, requests, durations, ttfts);
        }
        for group in &snapshot.models {
            let (requests, durations, ttfts) = split(group.model.as_deref() == Some("m1"));
            assert_summary("models", &group.summary, requests, durations, ttfts);
        }
        for group in &snapshot.agent_types {
            assert_summary(
                "agent_types",
                &group.summary,
                3,
                all_durations.clone(),
                all_ttfts.clone(),
            );
        }
        for group in &snapshot.folders {
            assert_summary(
                "folders",
                &group.summary,
                3,
                all_durations.clone(),
                all_ttfts.clone(),
            );
        }
    }

    /// C10 — the tools rollup carries no `UsageSummary` and gains no latency
    /// statistics. The destructure is exhaustive: adding a field to
    /// `ToolUsageGroup` fails to compile here.
    #[test]
    fn tool_usage_group_has_no_latency_fields() {
        fn assert_shape(group: &ToolUsageGroup) {
            let ToolUsageGroup {
                name: _,
                kind: _,
                calls: _,
                errors: _,
                argument_chars: _,
                result_chars: _,
                last_used_ms: _,
                total_tokens_share: _,
                output_tokens_share: _,
                costs: _,
                charges: _,
                models: _,
            } = group;
        }
        let log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        for group in &snapshot.tools {
            assert_shape(group);
        }
    }

    /// Independent oracle for the approved nearest-rank definition: the value
    /// at 1-based ordered position `ceil(0.9 * N)`. Sorts a Vec in memory with
    /// no SQL involvement, so it shares no failure mechanism with the query.
    fn nearest_rank_p90(mut values: Vec<u64>) -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        values.sort_unstable();
        let position = (0.9_f64 * values.len() as f64).ceil() as usize;
        values.get(position - 1).map(|value| *value as f64)
    }

    /// Seeds one turn per (session, model, duration, ttft) tuple.
    fn seed_latency(log: &mut UsageLog, rows: &[(&str, Option<&str>, u64, Option<u64>)]) {
        for (session, model, duration, ttft) in rows {
            must_succeed(
                log.append(&record(
                    session,
                    *model,
                    (None, None, None),
                    (*duration, *ttft),
                    Vec::new(),
                )),
                "append latency record",
            );
        }
    }

    /// C1 — SQL nearest-rank p90 and max equal the independent sorted oracle,
    /// including at N=2 where `ceil(0.9*N)` and `floor((N-1)*0.9)` disagree
    /// (60 vs 50) and on a duplicate-heavy group.
    #[test]
    fn p90_matches_sorted_oracle_per_group() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        let group_a: Vec<u64> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 100];
        let group_b: Vec<u64> = vec![50, 60];
        let group_c: Vec<u64> = vec![5, 5, 5, 5, 9];
        let mut rows = Vec::new();
        for (index, duration) in group_a.iter().enumerate() {
            rows.push((format!("a{index}"), "pa/ma", *duration));
        }
        for (index, duration) in group_b.iter().enumerate() {
            rows.push((format!("b{index}"), "pb/mb", *duration));
        }
        for (index, duration) in group_c.iter().enumerate() {
            rows.push((format!("c{index}"), "pc/mc", *duration));
        }
        for (session, model, duration) in &rows {
            must_succeed(
                log.append(&record(
                    session,
                    Some(model),
                    (None, None, None),
                    (*duration, None),
                    Vec::new(),
                )),
                "append record",
            );
        }
        let snapshot = must_succeed(log.snapshot(), "snapshot");

        for (model, values) in [("ma", &group_a), ("mb", &group_b), ("mc", &group_c)] {
            let group = snapshot
                .models
                .iter()
                .find(|candidate| candidate.model.as_deref() == Some(model))
                .unwrap_or_else(|| panic!("model group {model} missing"));
            assert_eq!(
                group.summary.p90_duration_ms,
                nearest_rank_p90(values.to_vec()),
                "model {model}: p90 must equal the sorted oracle"
            );
            assert_eq!(
                group.summary.max_duration_ms,
                values.iter().max().map(|value| *value as f64),
                "model {model}: max"
            );
        }

        let mut all: Vec<u64> = group_a.clone();
        all.extend(group_b.iter().copied());
        all.extend(group_c.iter().copied());
        assert_eq!(
            snapshot.overview.p90_duration_ms,
            nearest_rank_p90(all.clone()),
            "overview p90 must equal the sorted oracle over every row"
        );
        assert_eq!(
            snapshot.overview.max_duration_ms,
            all.iter().max().map(|value| *value as f64),
            "overview max"
        );
    }

    /// C2 — `ttft_ms` is nullable; NULL rows must never enter the rank
    /// denominator. With four NULL and six reported ttft values the correct
    /// answer (1000) differs from the answer NULLs would produce (90).
    #[test]
    fn ttft_p90_excludes_nulls_from_denominator() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        seed_latency(
            &mut log,
            &[
                ("n1", Some("p/m"), 1, None),
                ("n2", Some("p/m"), 2, None),
                ("n3", Some("p/m"), 3, None),
                ("n4", Some("p/m"), 4, None),
                ("t1", Some("p/m"), 5, Some(50)),
                ("t2", Some("p/m"), 6, Some(60)),
                ("t3", Some("p/m"), 7, Some(70)),
                ("t4", Some("p/m"), 8, Some(80)),
                ("t5", Some("p/m"), 9, Some(90)),
                ("t6", Some("p/m"), 10, Some(1000)),
            ],
        );
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        let reported = vec![50_u64, 60, 70, 80, 90, 1000];
        assert_eq!(
            snapshot.overview.p90_ttft_ms,
            nearest_rank_p90(reported.clone()),
            "ttft p90 must rank over the six reported values only"
        );
        assert_eq!(
            snapshot.overview.max_ttft_ms,
            Some(1000.0),
            "ttft max over reported values"
        );
        assert_eq!(
            snapshot.overview.requests, 10,
            "every row still counts toward requests"
        );
    }

    /// C3 — each group ranks over its own rows; no group inherits the
    /// overview's value.
    #[test]
    fn grouped_p90_is_group_local() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        seed_latency(
            &mut log,
            &[
                ("a1", Some("pa/ma"), 1, None),
                ("a2", Some("pa/ma"), 2, None),
                ("a3", Some("pa/ma"), 3, None),
                ("b1", Some("pb/mb"), 500, None),
                ("b2", Some("pb/mb"), 600, None),
            ],
        );
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        let find = |model: &str| {
            snapshot
                .models
                .iter()
                .find(|candidate| candidate.model.as_deref() == Some(model))
                .unwrap_or_else(|| panic!("model {model} missing"))
                .summary
                .p90_duration_ms
        };
        let a = find("ma");
        let b = find("mb");
        assert_eq!(a, nearest_rank_p90(vec![1, 2, 3]), "group a is local");
        assert_eq!(b, nearest_rank_p90(vec![500, 600]), "group b is local");
        assert_ne!(a, b, "groups with different distributions must differ");
        assert_ne!(
            a, snapshot.overview.p90_duration_ms,
            "a group must not inherit the overview value"
        );
    }

    /// C4 — `provider` and `model` are nullable, so a NULL grouping key is
    /// reachable in production. It forms its own group and ranks normally.
    #[test]
    fn null_group_key_gets_its_own_p90() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        seed_latency(
            &mut log,
            &[
                ("k1", Some("p/m"), 10, None),
                ("k2", Some("p/m"), 20, None),
                ("u1", None, 700, None),
                ("u2", None, 800, None),
            ],
        );
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        let unnamed = snapshot
            .models
            .iter()
            .find(|candidate| candidate.model.is_none())
            .unwrap_or_else(|| panic!("NULL-key model group missing"));
        assert_eq!(
            unnamed.summary.p90_duration_ms,
            nearest_rank_p90(vec![700, 800]),
            "the NULL-key group ranks over its own rows"
        );
        assert_eq!(unnamed.summary.max_duration_ms, Some(800.0));
    }

    /// C5 — absence is reported as absence. Carries a positive control first,
    /// because an assertion that something is missing proves nothing unless
    /// the same code path can be shown to produce it.
    #[test]
    fn absent_latency_data_is_none_not_zero() {
        let mut populated = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        seed_latency(&mut populated, &[("p1", Some("p/m"), 42, Some(7))]);
        let control = must_succeed(populated.snapshot(), "populated snapshot");
        assert!(
            control.overview.p90_duration_ms.is_some() && control.overview.p90_ttft_ms.is_some(),
            "positive control: the fields can be populated at all"
        );

        let empty = must_succeed(UsageLog::open_in_memory(), "empty log");
        let snapshot = must_succeed(empty.snapshot(), "empty snapshot");
        assert_eq!(
            snapshot.overview.p90_duration_ms, None,
            "empty: p90 duration"
        );
        assert_eq!(
            snapshot.overview.max_duration_ms, None,
            "empty: max duration"
        );
        assert_eq!(snapshot.overview.p90_ttft_ms, None, "empty: p90 ttft");
        assert_eq!(snapshot.overview.max_ttft_ms, None, "empty: max ttft");

        let mut no_ttft = must_succeed(UsageLog::open_in_memory(), "no-ttft log");
        seed_latency(
            &mut no_ttft,
            &[("x1", Some("p/m"), 10, None), ("x2", Some("p/m"), 20, None)],
        );
        let snapshot = must_succeed(no_ttft.snapshot(), "no-ttft snapshot");
        assert!(
            snapshot.overview.p90_duration_ms.is_some(),
            "duration is still reported when ttft is absent"
        );
        assert_eq!(snapshot.overview.p90_ttft_ms, None, "all-NULL ttft: p90");
        assert_eq!(snapshot.overview.max_ttft_ms, None, "all-NULL ttft: max");
    }

    /// C6 — nearest rank is defined at N=1; no minimum-sample threshold
    /// suppresses it, matching how the existing averages behave.
    #[test]
    fn single_row_group_reports_its_own_value() {
        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        seed_latency(
            &mut log,
            &[
                ("solo", Some("ps/ms"), 77, Some(11)),
                ("big1", Some("pb/mb"), 500, Some(90)),
                ("big2", Some("pb/mb"), 600, Some(95)),
            ],
        );
        let snapshot = must_succeed(log.snapshot(), "snapshot");
        let solo = snapshot
            .models
            .iter()
            .find(|candidate| candidate.model.as_deref() == Some("ms"))
            .unwrap_or_else(|| panic!("solo group missing"));
        assert_eq!(solo.summary.p90_duration_ms, Some(77.0), "n=1 p90 duration");
        assert_eq!(solo.summary.max_duration_ms, Some(77.0), "n=1 max duration");
        assert_eq!(solo.summary.p90_ttft_ms, Some(11.0), "n=1 p90 ttft");
        assert_eq!(solo.summary.requests, 1, "the solo group holds one row");
    }

    /// C12 — a rollup group with no latency row is an invariant violation, and
    /// says so.
    ///
    /// `duration_ms` is `INTEGER NOT NULL` and the p90 query reads the same
    /// unfiltered `usage_turns` the rollup does, so every group the rollup can
    /// produce MUST have an entry. A miss means the two disagree about the
    /// grouping key. It used to land in `unwrap_or_default()`: blank columns,
    /// nothing logged, indistinguishable from `costs`, where an empty entry
    /// genuinely means "no priced turns here" (cyril-9kyk review).
    #[test]
    fn latency_lookup_warns_on_a_missing_group_but_not_when_degraded() {
        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        let mut map = LatencyMap::new();
        map.insert(
            vec![Some("known".to_owned())],
            LatencyP90 {
                duration_ms: Some(5.0),
                ttft_ms: None,
            },
        );
        let computed = LatencyLookup {
            grouping: "provider",
            map: Some(map),
        };
        let degraded = LatencyLookup {
            grouping: "provider",
            map: None,
        };

        tracing::dispatcher::with_default(&dispatch, || {
            assert_eq!(
                computed.get(&[Some("known".to_owned())]).duration_ms,
                Some(5.0),
                "a present group returns its own p90"
            );
            assert_eq!(
                computed.get(&[Some("missing".to_owned())]),
                LatencyP90::default(),
                "a miss still degrades to absence for the user"
            );
            // The degraded lookup already warned once when the query failed;
            // warning again per group would be pure noise.
            assert_eq!(
                degraded.get(&[Some("anything".to_owned())]),
                LatencyP90::default(),
                "an unavailable map reports absence"
            );
        });

        let events = capture.captured();
        assert_eq!(
            events.len(),
            1,
            "exactly one warning, for the missing group: {events:?}"
        );
        let rendered = format!("{events:?}");
        assert!(
            rendered.contains("provider"),
            "the warning must name the grouping it came from: {rendered}"
        );
    }

    /// C9 — the grouped nearest-rank computation stays inside its production
    /// budget. This is a wall-clock measurement with a deterministic assertion
    /// of the bound, not an eyeball.
    ///
    /// **`#[ignore]` on purpose** (cyril-9kyk review). CI runs `cargo nextest`,
    /// which cancels on first failure: a stopwatch assertion on a shared runner
    /// does not merely flake, it aborts the whole job, and every
    /// alphabetically-later test then never runs — real regressions hidden
    /// behind machine load. Run it deliberately instead:
    ///
    /// ```sh
    /// cargo test -p cyril-core --lib -- --ignored grouped_percentile
    /// ```
    ///
    /// The bound stays at the approved 700 ms (`.cyril-9kyk/spec.md`) — an
    /// approved acceptance criterion is not something to quietly re-cut. What
    /// changed on 2026-08-28 is the headroom under it: moving `MAX` out of the
    /// ranked subquery onto `SUMMARY_COLUMNS` took four runs to 565 / 573 /
    /// 568 / 601 ms, against 600 ms for the shape the budget was raised for.
    /// Two attempts to do better than that were built and then rejected by
    /// measurement rather than by argument — see `latency_p90_sql`. The
    /// always-on recompute shape that puts any of this on the event loop is
    /// cyril-nanu; the unbounded row growth that makes 100k rows reachable at
    /// all is cyril-b163.
    ///
    /// The latency queries are pure additions to `snapshot()` — they touch no
    /// other query — so their total IS the delta the budget governs. Measuring
    /// them directly is both tighter and less noisy than differencing two
    /// whole-snapshot runs.
    #[test]
    #[ignore = "reference-workstation grouped-percentile budget"]
    fn grouped_percentile_stays_within_budget() {
        const ROWS: i64 = 100_000;
        const GROUPS: i64 = 20;
        const BUDGET: Duration = Duration::from_millis(700);

        let mut log = must_succeed(UsageLog::open_in_memory(), "in-memory log");
        {
            let transaction = must_succeed(log.connection.transaction(), "bulk transaction");
            {
                let mut statement = must_succeed(
                    transaction.prepare(
                        "INSERT INTO usage_turns (
                            session_id, folder, model, provider, agent_type,
                            timestamp_ms, duration_ms, ttft_ms, stop_reason, outcome,
                            token_availability
                         ) VALUES (?, '/tmp', ?, ?, 'main', ?, ?, ?, 'end_turn', 'success', 'unreported')",
                    ),
                    "prepare seed",
                );
                for index in 0..ROWS {
                    // Skewed on purpose: one group holds ~40% of the rows, so a
                    // per-partition sort cannot be flattered by uniform groups.
                    let group = if index % 10 < 4 {
                        0
                    } else {
                        (index / 10) % GROUPS
                    };
                    // ~40% of rows report no ttft, so the filtered subquery does
                    // real work and its rank denominator differs from duration's.
                    let ttft: Option<i64> = if index % 10 < 4 {
                        None
                    } else {
                        Some(index % 900)
                    };
                    must_succeed(
                        statement.execute(params![
                            format!("s{index}"),
                            format!("m{group}"),
                            format!("p{}", group % 4),
                            index,
                            index % 5_000,
                            ttft
                        ]),
                        "seed row",
                    );
                }
            }
            must_succeed(transaction.commit(), "commit seed");
        }

        // Identical warm-up before the measured run, so a cold page cache
        // cannot be mistaken for percentile cost.
        let warm = must_succeed(log.snapshot(), "warm snapshot");
        // `must_succeed`, not `unwrap_or_default`: a failed conversion used to
        // degrade the first assertion to `requests == 0` and the second to
        // `len() >= 0`, so the perf number could have been measured against a
        // fixture that never spanned 20 groups (cyril-9kyk review).
        assert_eq!(
            warm.overview.requests,
            must_succeed(u64::try_from(ROWS), "ROWS fits u64"),
            "fixture seeded"
        );
        assert!(
            warm.models.len() >= must_succeed(usize::try_from(GROUPS), "GROUPS fits usize"),
            "fixture must span at least {GROUPS} (provider, model) groups, saw {}",
            warm.models.len()
        );
        assert!(
            warm.overview.p90_duration_ms.is_some(),
            "positive control: the measured work actually produces a percentile"
        );

        let started = Instant::now();
        must_succeed(log.latency_p90_map(&[]), "overview latency");
        must_succeed(
            log.latency_p90_map(&["provider".to_owned()]),
            "provider latency",
        );
        must_succeed(
            log.latency_p90_map(&["folder".to_owned()]),
            "folder latency",
        );
        must_succeed(
            log.latency_p90_map(&["agent_type".to_owned()]),
            "agent latency",
        );
        must_succeed(log.latency_p90_map(&model_group_keys("")), "model latency");
        let added = started.elapsed();
        // Printed unconditionally: this fence is run by hand, and the number
        // is the point — a pass 1 ms under the bound is worth seeing.
        println!("grouped percentile cost: {added:?} (budget {BUDGET:?})");

        assert!(
            added <= BUDGET,
            "grouped percentile cost {added:?} exceeds the {BUDGET:?} budget at \
             {ROWS} rows across {GROUPS} groups"
        );
    }
}

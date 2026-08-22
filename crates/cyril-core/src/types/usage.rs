use std::fmt;

use thiserror::Error;

use super::{SessionId, StopReason, ToolKind};

/// Whether a session starts with a known zero cost or resumes existing history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionOrigin {
    Fresh,
    Loaded,
}

/// Portable agent role used by usage aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsageAgentType {
    Main,
    Subagent,
    Advisor,
}

impl UsageAgentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Subagent => "subagent",
            Self::Advisor => "advisor",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "main" => Some(Self::Main),
            "subagent" => Some(Self::Subagent),
            "advisor" => Some(Self::Advisor),
            _ => None,
        }
    }
}

impl fmt::Display for UsageAgentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Invalid value at the usage trust boundary.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UsageValueError {
    #[error("usage cost amount must be finite and non-negative")]
    InvalidAmount,
    #[error("usage cost currency must be a three-letter uppercase ISO 4217 code")]
    InvalidCurrency,
}

/// A validated monetary amount. Currencies are never combined implicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct Money {
    amount: f64,
    currency: String,
}

impl Money {
    pub fn try_new(amount: f64, currency: impl Into<String>) -> Result<Self, UsageValueError> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(UsageValueError::InvalidAmount);
        }
        let currency = currency.into();
        if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(UsageValueError::InvalidCurrency);
        }
        Ok(Self { amount, currency })
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }
}

/// Standard ACP token usage for one completed turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    total: u64,
    input: u64,
    output: u64,
    thought: Option<u64>,
    cached_read: Option<u64>,
    cached_write: Option<u64>,
}

impl TokenUsage {
    pub fn new(
        total: u64,
        input: u64,
        output: u64,
        thought: Option<u64>,
        cached_read: Option<u64>,
        cached_write: Option<u64>,
    ) -> Self {
        Self {
            total,
            input,
            output,
            thought,
            cached_read,
            cached_write,
        }
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn input(&self) -> u64 {
        self.input
    }

    pub fn output(&self) -> u64 {
        self.output
    }

    pub fn thought(&self) -> Option<u64> {
        self.thought
    }

    pub fn cached_read(&self) -> Option<u64> {
        self.cached_read
    }

    pub fn cached_write(&self) -> Option<u64> {
        self.cached_write
    }
}

/// One distinct tool call observed during a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageTool {
    kind: ToolKind,
    failed: bool,
}

impl UsageTool {
    pub fn new(kind: ToolKind, failed: bool) -> Self {
        Self { kind, failed }
    }

    pub fn kind(&self) -> ToolKind {
        self.kind
    }

    pub fn failed(&self) -> bool {
        self.failed
    }
}

/// Identity snapshotted when a prompt is dispatched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnUsageContext {
    session_id: SessionId,
    folder: String,
    model: Option<String>,
    provider: Option<String>,
    agent_type: UsageAgentType,
}

impl TurnUsageContext {
    pub fn new(
        session_id: SessionId,
        folder: impl Into<String>,
        model_id: Option<&str>,
        agent_type: UsageAgentType,
    ) -> Self {
        let (provider, model) = split_model_identity(model_id);
        Self {
            session_id,
            folder: folder.into(),
            model,
            provider,
            agent_type,
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn folder(&self) -> &str {
        &self.folder
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    pub fn agent_type(&self) -> UsageAgentType {
        self.agent_type
    }
}

fn split_model_identity(model_id: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(raw) = model_id.filter(|value| !value.is_empty()) else {
        return (None, None);
    };
    if let Some((provider, model)) = raw.split_once('/')
        && !provider.is_empty()
        && !model.is_empty()
    {
        return (Some(provider.to_owned()), Some(model.to_owned()));
    }
    (None, Some(raw.to_owned()))
}

/// Monotonic timing captured for one turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageTiming {
    timestamp_ms: u64,
    duration_ms: u64,
    ttft_ms: Option<u64>,
}

impl UsageTiming {
    pub fn new(timestamp_ms: u64, duration_ms: u64, ttft_ms: Option<u64>) -> Self {
        Self {
            timestamp_ms,
            duration_ms,
            ttft_ms,
        }
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn ttft_ms(&self) -> Option<u64> {
        self.ttft_ms
    }
}

/// Wire outcome captured at the turn boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageOutcome {
    stop_reason: StopReason,
    tokens: Option<TokenUsage>,
    cost: Option<Money>,
    error: Option<String>,
}

impl UsageOutcome {
    pub fn new(
        stop_reason: StopReason,
        tokens: Option<TokenUsage>,
        cost: Option<Money>,
        error: Option<String>,
    ) -> Self {
        Self {
            stop_reason,
            tokens,
            cost,
            error: error.filter(|value| !value.is_empty()),
        }
    }

    pub fn stop_reason(&self) -> StopReason {
        self.stop_reason
    }

    pub fn tokens(&self) -> Option<&TokenUsage> {
        self.tokens.as_ref()
    }

    pub fn cost(&self) -> Option<&Money> {
        self.cost.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

/// Complete observer output for one turn. Optional wire values stay optional.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRecord {
    context: TurnUsageContext,
    timing: UsageTiming,
    outcome: UsageOutcome,
    tools: Vec<UsageTool>,
}

impl UsageRecord {
    pub fn new(
        context: TurnUsageContext,
        timing: UsageTiming,
        outcome: UsageOutcome,
        tools: Vec<UsageTool>,
    ) -> Self {
        Self {
            context,
            timing,
            outcome,
            tools,
        }
    }

    pub fn context(&self) -> &TurnUsageContext {
        &self.context
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.timing.timestamp_ms()
    }

    pub fn duration_ms(&self) -> u64 {
        self.timing.duration_ms()
    }

    pub fn ttft_ms(&self) -> Option<u64> {
        self.timing.ttft_ms()
    }

    pub fn stop_reason(&self) -> StopReason {
        self.outcome.stop_reason()
    }

    pub fn tokens(&self) -> Option<&TokenUsage> {
        self.outcome.tokens()
    }

    pub fn cost(&self) -> Option<&Money> {
        self.outcome.cost()
    }

    pub fn tools(&self) -> &[UsageTool] {
        &self.tools
    }

    pub fn error(&self) -> Option<&str> {
        self.outcome.error()
    }
}

/// Token totals across records that actually carried token usage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenTotals {
    pub total: u64,
    pub input: u64,
    pub output: u64,
    pub thought: u64,
    pub cached_read: u64,
    pub cached_write: u64,
}

/// Aggregated usage metrics. `tokens` is absent when no record supplied usage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSummary {
    pub requests: u64,
    pub errors: u64,
    pub tokens: Option<TokenTotals>,
    pub costs: Vec<Money>,
    pub cache_rate: Option<f64>,
    pub avg_duration_ms: Option<f64>,
    pub avg_ttft_ms: Option<f64>,
    pub avg_tokens_per_second: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedUsageGroup {
    pub name: Option<String>,
    pub summary: UsageSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelUsageGroup {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub summary: UsageSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentUsageGroup {
    pub agent_type: UsageAgentType,
    pub summary: UsageSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolUsageGroup {
    pub kind: ToolKind,
    pub calls: u64,
    pub errors: u64,
    pub total_tokens_share: Option<f64>,
    pub output_tokens_share: Option<f64>,
    pub costs: Vec<Money>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentUsage {
    pub session_id: SessionId,
    pub folder: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub agent_type: UsageAgentType,
    pub timestamp_ms: u64,
    pub duration_ms: u64,
    pub ttft_ms: Option<u64>,
    pub stop_reason: StopReason,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<Money>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageSnapshot {
    pub overview: UsageSummary,
    pub providers: Vec<NamedUsageGroup>,
    pub models: Vec<ModelUsageGroup>,
    pub folders: Vec<NamedUsageGroup>,
    pub agent_types: Vec<AgentUsageGroup>,
    pub tools: Vec<ToolUsageGroup>,
    pub recent: Vec<RecentUsage>,
    pub errors: Vec<RecentUsage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_rejects_invalid_values_without_defaulting() {
        for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
            assert_eq!(
                Money::try_new(amount, "USD"),
                Err(UsageValueError::InvalidAmount)
            );
        }
        for currency in ["", "usd", "US", "USDD", "€€€"] {
            assert_eq!(
                Money::try_new(1.0, currency),
                Err(UsageValueError::InvalidCurrency)
            );
        }
        let zero = match Money::try_new(0.0, "USD") {
            Ok(zero) => zero,
            Err(_) => panic!("explicit zero is valid"),
        };
        assert_eq!(zero.amount(), 0.0);
    }

    #[test]
    fn identity_snapshot_matrix() {
        let sid = SessionId::new("s");
        let cases = [
            (None, None, None),
            (Some(""), None, None),
            (Some("model"), None, Some("model")),
            (
                Some("openai-codex/gpt-5.6-luna"),
                Some("openai-codex"),
                Some("gpt-5.6-luna"),
            ),
            (Some("/model"), None, Some("/model")),
            (Some("provider/"), None, Some("provider/")),
            (Some("p/family/model"), Some("p"), Some("family/model")),
        ];
        for (raw, provider, model) in cases {
            let context = TurnUsageContext::new(
                sid.clone(),
                "/tmp/space and 日本語",
                raw,
                UsageAgentType::Main,
            );
            assert_eq!(context.provider(), provider);
            assert_eq!(context.model(), model);
            assert_eq!(context.folder(), "/tmp/space and 日本語");
        }
    }
}

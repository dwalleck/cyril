use std::fmt;

/// Unique session identifier. Newtype wrapper preventing string mixups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Thinking-effort level reported under thinking models (Kiro 2.5.0+). NOT a
/// closed wire set: levels are schema-negotiated per model — Anthropic models
/// advertise `low`/`medium`/`high`/`xhigh`/`max` under output_config, GPT
/// models a different enum under reasoning (cyril-1gim). Known levels keep
/// typed variants; a backend-defined level is preserved verbatim in `Other`
/// so it renders in the toolbar instead of silently vanishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    XHigh,
    Max,
    /// Backend-defined level outside the known set; carries the raw wire
    /// string so it can be displayed as-is.
    Other(String),
}

impl EffortLevel {
    /// Parse a wire `effort` string. Returns `None` for an empty value (the
    /// wire's way of saying "not set"); a known level maps to its typed
    /// variant, anything else is preserved as `Other` — unknown levels are
    /// displayed, never dropped. The `debug!` log keeps backend additions
    /// visible.
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "" => None,
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            other => {
                tracing::debug!(
                    effort = other,
                    "unrecognized thinking-effort level on the wire; preserved as Other"
                );
                Some(Self::Other(other.to_string()))
            }
        }
    }

    /// The wire string for this level (also its display form).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
            Self::Other(s) => s,
        }
    }
}

impl fmt::Display for EffortLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Effort-level change carried by a `MetadataUpdated` frame. The wire is
/// tri-state (cyril-1gim, carved from 2.12.1 tui.js `handleMetadataUpdate`):
/// an *absent* `effort` field means "no update" (retain the current badge),
/// an explicit `effort: null` is the engine clearing the level, and a string
/// sets it. tui.js checks `"effort" in e`, so `null` is a real badge-CLEAR
/// signal a plain `Option` cannot represent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffortUpdate {
    /// `effort` absent — keep whatever level is showing (sticky).
    Unchanged,
    /// Explicit `effort: null` — the engine cleared the badge.
    Clear,
    /// A string level, known or backend-defined.
    Set(EffortLevel),
}

/// Session lifecycle state machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SessionStatus {
    #[default]
    Disconnected,
    Initializing,
    Active,
    Busy,
    Compacting,
    Error {
        message: String,
    },
}

/// Phase of a context-compaction operation reported by Kiro's
/// `kiro.dev/compaction/status` notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionPhase {
    /// Compaction has started; no summary yet.
    Started,
    /// Compaction finished successfully; a summary may be provided alongside.
    Completed,
    /// Compaction failed. `error` carries the agent's reason if supplied.
    Failed { error: Option<String> },
}

/// Mode identifier. Newtype over `String` so `SessionMode::new` catches
/// id/label positional swaps at compile time. Construct via `ModeId::new(..)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModeId(String);

impl ModeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// NOTE: intentionally no `From<&str>` / `From<String>` — see the matching
// comment on `ModelId` for the rationale.

/// An available agent mode (e.g., "code", "chat").
///
/// Kiro populates `_meta.welcomeMessage` on some modes (observed on
/// `kiro_planner`); we capture it here so the UI can greet the user when
/// that mode becomes active without a second roundtrip.
#[derive(Debug, Clone)]
pub struct SessionMode {
    id: ModeId,
    label: String,
    description: Option<String>,
    welcome_message: Option<String>,
}

impl SessionMode {
    pub fn new(
        id: ModeId,
        label: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            description: description.map(Into::into),
            welcome_message: None,
        }
    }

    #[must_use]
    pub fn with_welcome_message(mut self, welcome_message: Option<String>) -> Self {
        // Normalize empty (including whitespace-only) to None — a blank welcome
        // would render as a blank system message and is semantically identical
        // to "no welcome".
        self.welcome_message = welcome_message.filter(|s| !s.trim().is_empty());
        self
    }

    pub fn id(&self) -> &ModeId {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn welcome_message(&self) -> Option<&str> {
        self.welcome_message.as_deref()
    }
}

/// Model identifier. Newtype wrapper over `String` so `ModelInfo::new`
/// catches swaps between `id` and `name` (both stringly-typed on the wire)
/// at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// NOTE: intentionally no `From<&str>` or `From<String>` for ModelId.
// Those impls would combine with `impl Into<ModelId>` on call sites to
// silently wrap bare strings, re-admitting exactly the id/name positional
// swap bug this newtype exists to prevent. Construct via `ModelId::new(...)`.

/// A selectable model reported in `SessionModelState.availableModels`.
///
/// Mirrors `acp::ModelInfo` but lives in `cyril-core` so `cyril-ui` can read
/// it through the `TuiState` trait without depending on ACP types.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    id: ModelId,
    name: String,
    description: Option<String>,
}

impl ModelInfo {
    pub fn new(
        id: ModelId,
        name: impl Into<String>,
        description: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            description: description.map(Into::into),
        }
    }

    pub fn id(&self) -> &ModelId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Context window usage percentage, clamped to [0.0, 100.0].
#[derive(Debug, Clone)]
pub struct ContextUsage {
    percentage: f64,
}

impl ContextUsage {
    pub fn new(percentage: f64) -> Self {
        Self {
            percentage: percentage.clamp(0.0, 100.0),
        }
    }

    pub fn percentage(&self) -> f64 {
        self.percentage
    }
}

/// One category of the KAS context-usage breakdown (KAS-2b, cyril-5et2).
///
/// Aggregate-only — `tokens` + `percent`, with no per-item field. The KAS wire
/// itemizes only the file buckets (`items[]` on contextFiles/sessionFiles), and
/// cyril-5et2 renders the aggregate bar; per-file drill-in is a separate feature
/// (cyril-1116). Omitting an `items` field makes the no-drill-in invariant
/// unrepresentable rather than merely unenforced.
#[derive(Debug, Clone)]
pub struct ContextBucket {
    tokens: u64,
    percent: f64,
}

impl ContextBucket {
    pub fn new(tokens: u64, percent: f64) -> Self {
        Self { tokens, percent }
    }

    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    pub fn percent(&self) -> f64 {
        self.percent
    }
}

/// The KAS per-category context-usage breakdown (KAS-2b, cyril-5et2): the five
/// `session_info_update` `context_usage` buckets KAS pushes proactively each
/// turn. v2 has only the scalar [`ContextUsage`]; bucket names mirror the wire
/// `_meta.kiro.breakdown.*`.
#[derive(Debug, Clone)]
pub struct ContextBreakdown {
    context_files: ContextBucket,
    session_files: ContextBucket,
    tools: ContextBucket,
    your_prompts: ContextBucket,
    kiro_responses: ContextBucket,
}

impl ContextBreakdown {
    pub fn new(
        context_files: ContextBucket,
        session_files: ContextBucket,
        tools: ContextBucket,
        your_prompts: ContextBucket,
        kiro_responses: ContextBucket,
    ) -> Self {
        Self {
            context_files,
            session_files,
            tools,
            your_prompts,
            kiro_responses,
        }
    }

    pub fn context_files(&self) -> &ContextBucket {
        &self.context_files
    }

    pub fn session_files(&self) -> &ContextBucket {
        &self.session_files
    }

    pub fn tools(&self) -> &ContextBucket {
        &self.tools
    }

    pub fn your_prompts(&self) -> &ContextBucket {
        &self.your_prompts
    }

    pub fn kiro_responses(&self) -> &ContextBucket {
        &self.kiro_responses
    }
}

/// Credit usage tracking.
#[derive(Debug, Clone)]
pub struct CreditUsage {
    used: f64,
    limit: f64,
}

impl CreditUsage {
    pub fn new(used: f64, limit: f64) -> Self {
        Self { used, limit }
    }

    pub fn used(&self) -> f64 {
        self.used
    }

    pub fn limit(&self) -> f64 {
        self.limit
    }
}

/// Per-turn metering data from kiro.dev/metadata.
#[derive(Debug, Clone)]
pub struct TurnMetering {
    /// Credits for the turn. `None` when the wire carried no `meteringUsage`
    /// aggregate (a duration/effort-only frame) — deliberately distinct from
    /// an explicit `Some(0.0)`, which is a real zero-cost turn (cyril-1gim).
    credits: Option<f64>,
    duration_ms: Option<u64>,
}

impl TurnMetering {
    pub fn new(credits: Option<f64>, duration_ms: Option<u64>) -> Self {
        Self {
            credits,
            duration_ms,
        }
    }

    pub fn credits(&self) -> Option<f64> {
        self.credits
    }

    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// Replace the duration, preserving credits.
    pub fn with_duration_ms(mut self, duration_ms: Option<u64>) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Merge a metadata frame's parsed pieces into the pending turn state.
    /// Order-independent, last-writer-wins per field (cyril-1gim): credits
    /// come from `meteringUsage` frames, the duration from whichever frame
    /// last carried `turnDurationMs` — credits frames and duration/effort-
    /// only frames (real 2.4.1 shape) interleave, so neither must clobber
    /// the other. Never fabricates a credits figure: a lone duration yields
    /// `credits: None`, distinct from an explicit zero.
    pub fn merge_pending(
        pending: Option<TurnMetering>,
        metering: Option<TurnMetering>,
        duration_ms: Option<u64>,
    ) -> Option<TurnMetering> {
        let mut merged = pending;
        if let Some(m) = metering {
            merged = Some(match merged {
                // Incoming populated fields win; absent fields preserve the
                // pending value. The standalone `duration_ms` below is the
                // newest override when the frame carried `turnDurationMs`.
                Some(prev) => TurnMetering::new(
                    m.credits().or(prev.credits()),
                    m.duration_ms().or(prev.duration_ms()),
                ),
                None => m,
            });
        }
        if let Some(d) = duration_ms {
            merged = Some(match merged {
                Some(m) => m.with_duration_ms(Some(d)),
                None => TurnMetering::new(None, Some(d)),
            });
        }
        merged
    }

    pub fn duration_display(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms < 1000 {
                format!("{ms}ms")
            } else if ms < 60_000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                let mins = ms / 60_000;
                let secs = (ms % 60_000) / 1000;
                format!("{mins}m {secs}s")
            }
        })
    }
}

/// Running session cost accumulator.
#[derive(Debug, Clone, Default)]
pub struct SessionCost {
    total_credits: f64,
    turn_count: u32,
    last_turn_credits: Option<f64>,
    last_turn_duration_ms: Option<u64>,
}

impl SessionCost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_turn(&mut self, metering: &TurnMetering) {
        match metering.credits() {
            Some(credits) if credits.is_finite() => {
                self.total_credits += credits;
                self.last_turn_credits = Some(credits);
            }
            Some(credits) => {
                tracing::warn!(
                    credits,
                    "TurnMetering credits is non-finite, skipping accumulation"
                );
                self.last_turn_credits = Some(credits);
            }
            None => {
                // Duration-only turn: the wire carried no meteringUsage
                // aggregate, so there is nothing to sum (cyril-1gim). The
                // duration still lands in `last_turn_duration_ms` below.
                self.last_turn_credits = None;
            }
        }
        self.turn_count = self.turn_count.saturating_add(1);
        self.last_turn_duration_ms = metering.duration_ms();
    }

    pub fn total_credits(&self) -> f64 {
        self.total_credits
    }

    pub fn turn_count(&self) -> u32 {
        self.turn_count
    }

    pub fn last_turn_credits(&self) -> Option<f64> {
        self.last_turn_credits
    }

    pub fn last_turn_duration_ms(&self) -> Option<u64> {
        self.last_turn_duration_ms
    }
}

/// Token counts from a single turn.
#[derive(Debug, Clone)]
pub struct TokenCounts {
    input: u64,
    output: u64,
    cached: Option<u64>,
}

impl TokenCounts {
    pub fn new(input: u64, output: u64, cached: Option<u64>) -> Self {
        Self {
            input,
            output,
            cached,
        }
    }

    pub fn input(&self) -> u64 {
        self.input
    }

    pub fn output(&self) -> u64 {
        self.output
    }

    pub fn cached(&self) -> Option<u64> {
        self.cached
    }
}

/// Reason the agent stopped processing a prompt turn.
///
/// We define our own enum rather than reusing `acp::StopReason` so that
/// `cyril-ui` (which must not import ACP types) can read it through the
/// `TuiState` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StopReason {
    #[default]
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
}

/// Atomic summary of a completed turn.
///
/// Assembled by `SessionController` when `TurnCompleted` arrives: the
/// `stop_reason` comes from the `session/prompt` response; `token_counts`
/// and `metering` were buffered from the preceding `MetadataUpdated`
/// notification. Grouping them prevents the renderer from ever seeing
/// token counts from turn N paired with a stop reason from turn N-1.
///
/// NOTE: `stop_reason` is not yet extracted from the `session/prompt` response.
/// The bridge currently hardcodes `StopReason::EndTurn` for all outcomes.
/// Task #2 in the protocol parity backlog will wire the real value.
#[derive(Debug, Clone)]
pub struct TurnSummary {
    stop_reason: StopReason,
    token_counts: Option<TokenCounts>,
    metering: Option<TurnMetering>,
}

impl TurnSummary {
    pub fn new(
        stop_reason: StopReason,
        token_counts: Option<TokenCounts>,
        metering: Option<TurnMetering>,
    ) -> Self {
        Self {
            stop_reason,
            token_counts,
            metering,
        }
    }

    pub fn stop_reason(&self) -> StopReason {
        self.stop_reason
    }

    pub fn token_counts(&self) -> Option<&TokenCounts> {
        self.token_counts.as_ref()
    }

    pub fn metering(&self) -> Option<&TurnMetering> {
        self.metering.as_ref()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::collections::HashMap;

    #[test]
    fn session_id_roundtrip() {
        let id = SessionId::new("sess_123");
        assert_eq!(id.as_str(), "sess_123");
    }

    #[test]
    fn session_id_display() {
        let id = SessionId::new("sess_123");
        assert_eq!(format!("{id}"), "sess_123");
    }

    #[test]
    fn session_id_usable_as_hashmap_key() {
        let mut map = HashMap::new();
        let id = SessionId::new("sess_1");
        map.insert(id.clone(), 42);
        assert_eq!(map.get(&SessionId::new("sess_1")), Some(&42));
    }

    #[test]
    fn effort_level_wire_roundtrips_all_variants() {
        // Every known level must round-trip wire → variant → wire (== display).
        // `xhigh` is the historically-common thinking default, so pin it too.
        for s in ["low", "medium", "high", "xhigh", "max"] {
            let level = EffortLevel::from_wire(s).expect("known level parses");
            assert_eq!(level.as_str(), s, "as_str must invert from_wire");
            assert_eq!(format!("{level}"), s, "Display must match wire string");
        }
    }

    #[test]
    fn effort_level_from_wire_preserves_unknown_levels() {
        // Backend-defined levels (schema-negotiated per model, cyril-1gim):
        // a GPT-style level must survive as `Other` and render, not vanish.
        assert_eq!(EffortLevel::from_wire(""), None, "empty => None");
        let level = EffortLevel::from_wire("turbo").expect("unknown level preserved");
        assert_eq!(level, EffortLevel::Other("turbo".into()));
        assert_eq!(level.as_str(), "turbo", "as_str keeps the raw wire string");
        assert_eq!(
            format!("{level}"),
            "turbo",
            "Display shows the raw wire string"
        );
    }

    #[test]
    fn session_status_default_is_disconnected() {
        let status = SessionStatus::default();
        assert_eq!(status, SessionStatus::Disconnected);
    }

    #[test]
    fn session_status_error_carries_message() {
        let status = SessionStatus::Error {
            message: "oops".into(),
        };
        assert!(matches!(status, SessionStatus::Error { message } if message == "oops"));
    }

    #[test]
    fn context_usage_stores_value() {
        let usage = ContextUsage::new(50.0);
        assert!((usage.percentage() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn context_usage_clamps_high() {
        let usage = ContextUsage::new(150.0);
        assert!((usage.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn context_usage_clamps_low() {
        let usage = ContextUsage::new(-10.0);
        assert!((usage.percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn context_breakdown_round_trips_each_bucket() {
        // Slice 1 oracle: construct with five distinct (tokens, percent) pairs and
        // read each back through its named accessor — catches a constructor that
        // wires a field to the wrong bucket (e.g. tools <-> your_prompts swapped).
        let bd = ContextBreakdown::new(
            ContextBucket::new(10, 1.1),
            ContextBucket::new(20, 2.2),
            ContextBucket::new(30, 3.3),
            ContextBucket::new(40, 4.4),
            ContextBucket::new(50, 5.5),
        );
        for (bucket, tokens, percent) in [
            (bd.context_files(), 10u64, 1.1),
            (bd.session_files(), 20, 2.2),
            (bd.tools(), 30, 3.3),
            (bd.your_prompts(), 40, 4.4),
            (bd.kiro_responses(), 50, 5.5),
        ] {
            assert_eq!(bucket.tokens(), tokens);
            assert!((bucket.percent() - percent).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn session_mode_accessors() {
        let mode = SessionMode::new(
            ModeId::new("code"),
            "Code Mode",
            Some("Write and edit code"),
        );
        assert_eq!(mode.id().as_str(), "code");
        assert_eq!(mode.label(), "Code Mode");
        assert_eq!(mode.description(), Some("Write and edit code"));
    }

    #[test]
    fn session_mode_no_description() {
        let mode = SessionMode::new(ModeId::new("chat"), "Chat", None::<&str>);
        assert_eq!(mode.description(), None);
    }

    #[test]
    fn credit_usage_accessors() {
        let credits = CreditUsage::new(5.25, 10.0);
        assert!((credits.used() - 5.25).abs() < f64::EPSILON);
        assert!((credits.limit() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn session_cost_accumulates() {
        let mut cost = SessionCost::new();
        cost.record_turn(&TurnMetering::new(Some(0.018), Some(1948)));
        cost.record_turn(&TurnMetering::new(Some(0.042), Some(5200)));
        assert_eq!(cost.turn_count(), 2);
        assert!((cost.total_credits() - 0.060).abs() < 0.001);
        assert!((cost.last_turn_credits().unwrap() - 0.042).abs() < 0.001);
        assert_eq!(cost.last_turn_duration_ms(), Some(5200));
    }

    #[test]
    fn metering_merge_pending_is_order_independent() {
        // Credits frames and duration/effort-only frames interleave (real
        // 2.4.1 shape); the merge is last-writer-wins per field, never
        // clobbering (cyril-1gim).
        // Duration after credits:
        let m = TurnMetering::merge_pending(
            None,
            Some(TurnMetering::new(Some(0.018), Some(1000))),
            None,
        )
        .and_then(|m| TurnMetering::merge_pending(Some(m), None, Some(2281)))
        .expect("merged");
        assert_eq!(
            m.credits(),
            Some(0.018),
            "credits survive a duration-only frame"
        );
        assert_eq!(m.duration_ms(), Some(2281), "duration merged in");

        // Credits after duration (the reverse order):
        let m = TurnMetering::merge_pending(None, None, Some(2281))
            .and_then(|m| {
                TurnMetering::merge_pending(
                    Some(m),
                    Some(TurnMetering::new(Some(0.018), None)),
                    None,
                )
            })
            .expect("merged");
        assert_eq!(m.credits(), Some(0.018), "credits frame lands");
        assert_eq!(
            m.duration_ms(),
            Some(2281),
            "earlier duration not clobbered"
        );

        // Newest duration wins when the standalone param carries it:
        let m = TurnMetering::merge_pending(
            None,
            Some(TurnMetering::new(Some(0.018), Some(1000))),
            Some(2281),
        )
        .expect("merged");
        assert_eq!(m.duration_ms(), Some(2281));

        // Incoming metering's own duration survives when the standalone param
        // is absent:
        let m = TurnMetering::merge_pending(
            None,
            Some(TurnMetering::new(Some(0.018), Some(1000))),
            None,
        )
        .expect("merged");
        assert_eq!(
            m.duration_ms(),
            Some(1000),
            "metering's own duration preserved"
        );
    }

    #[test]
    fn metering_absence_merge_preserves_pending_credits() {
        let merged = TurnMetering::merge_pending(
            Some(TurnMetering::new(Some(0.018), None)),
            Some(TurnMetering::new(None, Some(2281))),
            None,
        )
        .expect("merged");
        assert_eq!(
            merged.credits(),
            Some(0.018),
            "an absent incoming credit field must preserve pending credits"
        );
        assert_eq!(merged.duration_ms(), Some(2281));
    }

    #[test]
    fn metering_merge_pending_never_fabricates_credits() {
        let m = TurnMetering::merge_pending(None, None, Some(2281))
            .expect("a lone duration still produces metering state");
        assert_eq!(m.credits(), None, "no credits figure fabricated");
        assert_eq!(m.duration_ms(), Some(2281));
        assert!(
            TurnMetering::merge_pending(None, None, None).is_none(),
            "nothing on the wire => no pending metering"
        );
    }

    #[test]
    fn session_cost_skips_duration_only_turns() {
        let mut cost = SessionCost::new();
        cost.record_turn(&TurnMetering::new(None, Some(2281)));
        assert_eq!(cost.turn_count(), 1);
        assert_eq!(
            cost.total_credits(),
            0.0,
            "no credits summed for duration-only"
        );
        assert!(cost.last_turn_credits().is_none());
        assert_eq!(cost.last_turn_duration_ms(), Some(2281));
    }

    #[test]
    fn duration_display_formatting() {
        assert_eq!(
            TurnMetering::new(Some(0.01), Some(500)).duration_display(),
            Some("500ms".into())
        );
        assert_eq!(
            TurnMetering::new(Some(0.01), Some(1948)).duration_display(),
            Some("1.9s".into())
        );
        assert_eq!(
            TurnMetering::new(Some(0.01), Some(135000)).duration_display(),
            Some("2m 15s".into())
        );
        assert!(
            TurnMetering::new(Some(0.01), None)
                .duration_display()
                .is_none()
        );
    }

    #[test]
    fn session_cost_default() {
        let cost = SessionCost::new();
        assert_eq!(cost.total_credits(), 0.0);
        assert_eq!(cost.turn_count(), 0);
        assert!(cost.last_turn_credits().is_none());
    }

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn session_types_are_send_sync() {
        assert_send::<SessionId>();
        assert_sync::<SessionId>();
        assert_send::<SessionStatus>();
        assert_sync::<SessionStatus>();
        assert_send::<SessionMode>();
        assert_sync::<SessionMode>();
        assert_send::<ContextUsage>();
        assert_sync::<ContextUsage>();
        assert_send::<CreditUsage>();
        assert_sync::<CreditUsage>();
        assert_send::<TurnMetering>();
        assert_sync::<TurnMetering>();
        assert_send::<SessionCost>();
        assert_sync::<SessionCost>();
        assert_send::<TokenCounts>();
        assert_sync::<TokenCounts>();
    }

    #[test]
    fn stop_reason_default_is_end_turn() {
        assert_eq!(StopReason::default(), StopReason::EndTurn);
    }

    #[test]
    fn turn_summary_accessors() {
        let summary = TurnSummary::new(
            StopReason::MaxTokens,
            Some(TokenCounts::new(1000, 500, Some(200))),
            Some(TurnMetering::new(Some(0.05), Some(3000))),
        );
        assert_eq!(summary.stop_reason(), StopReason::MaxTokens);
        assert!(summary.token_counts().is_some());
        assert!(summary.metering().is_some());
    }

    #[test]
    fn turn_summary_minimal() {
        let summary = TurnSummary::new(StopReason::Cancelled, None, None);
        assert_eq!(summary.stop_reason(), StopReason::Cancelled);
        assert!(summary.token_counts().is_none());
        assert!(summary.metering().is_none());
    }

    #[test]
    fn stop_reason_is_send_sync() {
        assert_send::<StopReason>();
        assert_sync::<StopReason>();
    }

    #[test]
    fn turn_summary_is_send_sync() {
        assert_send::<TurnSummary>();
        assert_sync::<TurnSummary>();
    }
}

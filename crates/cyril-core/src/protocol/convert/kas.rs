//! KAS (`_kiro/*`) `session/update` specifics — the KAS analogue of
//! [`super::kiro`] for the v2 dialect. KAS rides standard ACP `session/update`
//! frames whose KAS-specific payload lives entirely in `_meta.kiro`.
//!
//! KAS-2a (cyril-j16p) Slice 1: recognise the `turn_end` lifecycle frame — the
//! signal that drives turn completion under KAS, in place of v2's prompt
//! response — and map it to [`Notification::TurnCompleted`].

use agent_client_protocol as acp;

use super::kiro::{steering_message_id, steering_message_ids, steering_text};
use crate::types::{
    ContextBreakdown, ContextBucket, MeteredAmount, Notification, StopReason, TurnMeteringUpdate,
    UsageAccount, UsageAccountBreakdown, UsageAddOnCredit, UsageBonusCredit, UsageTurnStatus,
};

pub(crate) mod workflow;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AccountUsageParseError {
    #[error("account usage request failed: {0}")]
    Rejected(String),
    #[error("account usage response is missing {0}")]
    Missing(&'static str),
    #[error("account usage response has invalid {0}")]
    Invalid(&'static str),
}

pub(crate) fn account_usage_from_response(
    response: &serde_json::Value,
) -> Result<UsageAccount, AccountUsageParseError> {
    if response.get("success").and_then(serde_json::Value::as_bool) != Some(true) {
        let message = response
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("agent returned success=false");
        return Err(AccountUsageParseError::Rejected(message.to_owned()));
    }
    let Some(data) = response.get("data").filter(|data| !data.is_null()) else {
        let message = response
            .get("message")
            .and_then(serde_json::Value::as_str)
            .filter(|message| !message.is_empty())
            .ok_or(AccountUsageParseError::Missing("data and message"))?;
        return Err(AccountUsageParseError::Rejected(message.to_owned()));
    };
    let breakdowns = required_array(data, "usageBreakdowns")?
        .iter()
        .map(parse_account_breakdown)
        .collect::<Result<Vec<_>, _>>()?;
    let bonus_credits = required_array(data, "bonusCredits")?
        .iter()
        .map(parse_bonus_credit)
        .collect::<Result<Vec<_>, _>>()?;
    let add_on_credits = required_array(data, "addOnCredits")?
        .iter()
        .map(parse_add_on_credit)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(UsageAccount {
        plan_name: required_string(data, "planName")?.to_owned(),
        billing_cycle_reset: required_string(data, "billingCycleReset")?.to_owned(),
        overages_enabled: required_bool(data, "overagesEnabled")?,
        is_enterprise: required_bool(data, "isEnterprise")?,
        overage_capable: required_bool(data, "overageCapable")?,
        usage_breakdowns: breakdowns,
        bonus_credits,
        add_on_credits,
    })
}

fn parse_account_breakdown(
    value: &serde_json::Value,
) -> Result<UsageAccountBreakdown, AccountUsageParseError> {
    Ok(UsageAccountBreakdown {
        resource_type: required_string(value, "resourceType")?.to_owned(),
        display_name: required_string(value, "displayName")?.to_owned(),
        used: required_nonnegative(value, "used")?,
        has_limit: required_bool(value, "hasLimit")?,
        limit: required_nonnegative(value, "limit")?,
        percentage: required_percentage(value, "percentage")?,
        current_overages: required_nonnegative(value, "currentOverages")?,
        overage_rate: required_nonnegative(value, "overageRate")?,
        overage_charges: optional_nonnegative(value, "overageCharges")?,
        currency: required_string(value, "currency")?.to_owned(),
    })
}

fn parse_bonus_credit(
    value: &serde_json::Value,
) -> Result<UsageBonusCredit, AccountUsageParseError> {
    Ok(UsageBonusCredit {
        name: required_string(value, "name")?.to_owned(),
        used: required_nonnegative(value, "used")?,
        total: required_nonnegative(value, "total")?,
        days_until_expiry: value
            .get("daysUntilExpiry")
            .and_then(serde_json::Value::as_u64)
            .ok_or(AccountUsageParseError::Invalid(
                "bonusCredits[].daysUntilExpiry",
            ))?,
    })
}

fn parse_add_on_credit(
    value: &serde_json::Value,
) -> Result<UsageAddOnCredit, AccountUsageParseError> {
    Ok(UsageAddOnCredit {
        used: required_nonnegative(value, "used")?,
        total: required_nonnegative(value, "total")?,
        is_active: required_bool(value, "isActive")?,
        expires_at: optional_string(value, "expiresAt")?,
    })
}

fn optional_string(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<String>, AccountUsageParseError> {
    match value.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty())
            .map(|value| Some(value.to_owned()))
            .ok_or(AccountUsageParseError::Invalid(field)),
    }
}

fn required_string<'a>(
    value: &'a serde_json::Value,
    field: &'static str,
) -> Result<&'a str, AccountUsageParseError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(AccountUsageParseError::Invalid(field))
}

fn required_bool(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<bool, AccountUsageParseError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_bool)
        .ok_or(AccountUsageParseError::Invalid(field))
}

fn required_array<'a>(
    value: &'a serde_json::Value,
    field: &'static str,
) -> Result<&'a [serde_json::Value], AccountUsageParseError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .ok_or(AccountUsageParseError::Invalid(field))
}

fn required_nonnegative(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<f64, AccountUsageParseError> {
    let amount = value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or(AccountUsageParseError::Invalid(field))?;
    if !amount.is_finite() || amount < 0.0 {
        return Err(AccountUsageParseError::Invalid(field));
    }
    Ok(amount)
}

fn optional_nonnegative(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<f64>, AccountUsageParseError> {
    match value.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(_) => required_nonnegative(value, field).map(Some),
    }
}

fn required_percentage(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<f64, AccountUsageParseError> {
    required_nonnegative(value, field).map(|percentage| percentage.min(100.0))
}

/// The four command names `resolveWorkflows()` registers when the workflow
/// gate is on. Cyril never sets the gate (ADR-0011), but a backend feature
/// flip could still advertise them — and they dispatch to
/// `kiro.dev/commands/execute`, a v2-only method KAS lacks (cyril-oieu), so
/// offering them is offering a dead end. Exact names only: prefix-matching
/// would eat legitimate names like `workflow-creator`.
const GATE_WORKFLOW_COMMANDS: [&str; 4] = [
    "workflow-run",
    "workflow-status",
    "workflow-cancel",
    "workflow-resume",
];

/// Drops the gate-advertised workflow commands from a KAS commands update
/// (cyril-0qe6 C8); every other notification passes through untouched.
pub(crate) fn suppress_workflow_gate_commands(notification: Notification) -> Notification {
    let Notification::CommandsUpdated {
        mut commands,
        prompts,
    } = notification
    else {
        return notification;
    };
    commands.retain(|command| {
        let suppress = GATE_WORKFLOW_COMMANDS.contains(&command.name());
        if suppress {
            tracing::debug!(
                command = command.name(),
                "suppressing gate-advertised workflow command (ADR-0011: cyril owns the control plane)"
            );
        }
        !suppress
    });
    Notification::CommandsUpdated { commands, prompts }
}

/// Outcome of offering an extension method to the KAS workflow adapter.
#[derive(Debug)]
pub(crate) enum WorkflowFrameOutcome {
    /// Not a workflow lifecycle method — continue through the engine's
    /// remaining extension converters.
    NotWorkflow,
    /// An exact workflow lifecycle method whose payload was malformed;
    /// the adapter warned and dropped it.
    Dropped,
    /// One converted workflow lifecycle event, boxed for its ride inside
    /// [`Notification::Workflow`].
    Converted(Box<crate::types::WorkflowEvent>),
}

/// Convert a KAS `session_info_update` to an internal notification.
///
/// KAS multiplexes turn lifecycle, metering, context telemetry, and steering
/// echoes through one `session_info_update` envelope, discriminated by
/// `_meta.kiro.kind`. Sub-kinds surfaced today:
/// - **`turn_end`** — the terminal lifecycle signal → [`Notification::TurnCompleted`]
///   (KAS-2a), stop reason from `_meta.kiro.stopReason`.
/// - **`context_usage`** — the proactively-pushed per-category breakdown
///   (KAS-2b, cyril-5et2) → [`Notification::ContextBreakdownUpdated`].
/// - **`steering_queued` / `steering_injected` / `steering_cleared`** — the
///   steering-echo lifecycle (cyril-vgcm C5; fixtures captured live on
///   kiro-cli 2.12.1, KAS bundle byte-identical to 2.11.0/2.12.0). KAS keeps
///   the old-family kind names but carries ids like v2's new family; note
///   *injected*, not v2's `steering_consumed`, and Cleared fires BOTH on
///   explicit `_session/steer/clear` and routinely post-injection (findings
///   F4) — which is why [`Notification::SteeringCleared`] must stay id-scoped.
///
/// Every other sub-kind (`user_message_id_assigned`, `steering_inclusion`
/// fileMatch catalog, …) returns `None` — matching is exact on the `kind`
/// value, never prefix/substring. `turn_completion` maps metering only;
/// lifecycle completion keys on `kind == "turn_end"`, never frame ordering,
/// because a `context_usage` frame trails `turn_end` on the wire.
///
/// A `turn_end` whose `_meta.kiro.stopReason` is missing or unparseable still
/// completes the turn (defaults [`StopReason::EndTurn`]): silently returning
/// `None` would strand the UI in the busy state forever, so this is a runtime
/// fallback that survives release builds, not a `debug_assert!`.
pub(crate) fn session_info_to_notification(siu: &acp::SessionInfoUpdate) -> Option<Notification> {
    let kiro = siu.meta.as_ref()?.get("kiro")?;
    match kiro.get("kind").and_then(serde_json::Value::as_str) {
        // cyril-gk17: a KAS-executed hook changed state. Carved producer:
        // `ContextualHookInvoked` / `handleHookAction` build
        // `{hook: {hookId, operationId, name, status, actionType, output?}}`,
        // where `status` comes from `mapActionStateToHookStatus`:
        // completed | failed | canceled | running | awaiting_approval.
        //
        // EVERY state surfaces, progress included. `running` and
        // `awaiting_approval` were dropped as noise on the reasoning that
        // `awaiting_approval` is already represented by the permission request
        // — the live capture refutes it: `kas-v2hooks-2.16.0.jsonl` holds ZERO
        // `session/request_permission` frames, so a dropped `awaiting_approval`
        // is represented by nothing at all. Dropping `running` is worse: a hook
        // that hangs, or is killed before it reports, then leaves NO evidence it
        // ever started — precisely the case an audit trail exists for. Under
        // `kas_hooks = "kas"` this line is the only record that agent-run shell
        // touched this host, so a duplicate line costs less than a missing one.
        Some("hook_update") => {
            let hook = kiro.get("hook")?;
            let status = hook.get("status").and_then(serde_json::Value::as_str)?;
            // A hook with no name is unattributable — surfacing "hook  failed"
            // tells the user nothing actionable, so log and drop.
            let Some(name) = hook.get("name").and_then(serde_json::Value::as_str) else {
                tracing::warn!(status, "KAS hook_update carries no name; dropped");
                return None;
            };
            Some(Notification::HookExecuted {
                name: name.to_string(),
                status: status.to_string(),
                // Only `runCommand` hooks carry an exit code, and only once the
                // run produced one. Absent means "not reported", never 0.
                exit_code: hook
                    .pointer("/output/result/exitCode")
                    .and_then(serde_json::Value::as_i64),
            })
        }
        Some("turn_completion") => Some(Notification::TurnMeteringUpdated(turn_metering_update(
            kiro,
        ))),
        Some("turn_end") => Some(Notification::TurnCompleted {
            stop_reason: turn_end_stop_reason(kiro),
        }),
        Some("context_usage") => {
            // A context_usage frame missing its required `usagePercentage` carries
            // nothing to show → drop (unlike turn_end, which must complete). When
            // present, ALWAYS return Some even if the breakdown is absent/malformed
            // — the scalar `Context: N%` must still update (the bars retain-last in
            // UiState). No unwrap; a malformed breakdown degrades to `None`.
            let usage_percentage = kiro
                .get("usagePercentage")
                .and_then(serde_json::Value::as_f64)?;
            // Distinguish "missing" from "corrupt" (CLAUDE.md "Log before
            // returning None"): a frame with no `breakdown` key is the normal
            // scalar-only case (silent), but a `breakdown` that IS present yet
            // fails to parse (a bucket missing, or a `tokens`/`percent` field
            // absent or wrong-typed — e.g. a float-encoded `tokens` that
            // `as_u64` rejects) is wire drift that silently blanks the whole bar.
            // Log it so it's diagnosable — the same discipline turn_end_stop_reason
            // applies to `stopReason` below.
            let raw_breakdown = kiro.get("breakdown");
            let breakdown = parse_breakdown(raw_breakdown);
            if raw_breakdown.is_some() && breakdown.is_none() {
                tracing::warn!(
                    "KAS context_usage `breakdown` present but unparseable (bucket \
                     missing or a tokens/percent field absent/wrong-typed); \
                     degrading to scalar-only this frame"
                );
            }
            Some(Notification::ContextBreakdownUpdated {
                usage_percentage,
                breakdown,
            })
        }
        // Steering-echo lifecycle (cyril-vgcm C5). Old-family kind names,
        // new-family payloads: `{messageId, content}` beside `kind` in
        // `_meta.kiro`. Field reads and their never-drop degrade discipline
        // are shared with convert::kiro (`steering_text` /
        // `steering_message_id` / `steering_message_ids`) — only envelope
        // navigation differs (`_meta.kiro` here vs `update` there), so each
        // arm passes the already-navigated value. The "KAS " echo labels keep
        // the degrade warns distinguishable from the v2 old family's
        // identical kind names; a KAS `session_info_update` carries no
        // session id, hence `None`.
        Some("steering_queued") => Some(Notification::SteeringQueued {
            message: steering_text(Some(kiro), "content", "KAS steering_queued", None),
            message_id: steering_message_id(Some(kiro)),
        }),
        Some("steering_injected") => Some(Notification::SteeringConsumed {
            content: steering_text(Some(kiro), "content", "KAS steering_injected", None),
            message_id: steering_message_id(Some(kiro)),
        }),
        Some("steering_cleared") => Some(Notification::SteeringCleared {
            message_ids: steering_message_ids(Some(kiro), "KAS steering_cleared", None),
        }),
        _ => None,
    }
}

fn turn_metering_update(kiro: &serde_json::Value) -> TurnMeteringUpdate {
    let duration_ms = optional_u64(kiro, "elapsedTime", "KAS turn_completion");
    let status = match kiro.get("status") {
        None => None,
        Some(value) => match value.as_str().and_then(UsageTurnStatus::from_wire) {
            Some(status) => Some(status),
            None => {
                tracing::warn!(
                    value = ?value,
                    "KAS turn_completion `status` present but invalid, ignoring"
                );
                None
            }
        },
    };

    let mut charges = Vec::new();
    let mut used_tools = Vec::new();
    match kiro.get("promptTurnSummaries") {
        None => {}
        Some(value) => match value.as_array() {
            Some(summaries) => {
                for summary in summaries {
                    if let Some(charge) = parse_metered_amount(summary) {
                        charges.push(charge);
                    }
                    match summary.get("usedTools") {
                        None => {}
                        Some(tools) => match tools.as_array() {
                            Some(tools) => {
                                for tool in tools {
                                    if let Some(tool) =
                                        tool.as_str().filter(|tool| !tool.is_empty())
                                    {
                                        used_tools.push(tool.to_owned());
                                    } else {
                                        tracing::warn!(
                                            value = ?tool,
                                            "KAS turn_completion `usedTools` entry is invalid, ignoring"
                                        );
                                    }
                                }
                            }
                            None => tracing::warn!(
                                value = ?tools,
                                "KAS turn_completion `usedTools` is not an array, ignoring"
                            ),
                        },
                    }
                }
            }
            None => tracing::warn!(
                value = ?value,
                "KAS turn_completion `promptTurnSummaries` is not an array, ignoring"
            ),
        },
    }

    let request_ids = match kiro.get("requestIds") {
        None => None,
        Some(value) => match value.as_array() {
            Some(ids) => {
                let mut parsed = Vec::with_capacity(ids.len());
                for id in ids {
                    if let Some(id) = id.as_str().filter(|id| !id.is_empty()) {
                        parsed.push(id.to_owned());
                    } else {
                        tracing::warn!(
                            value = ?id,
                            "KAS turn_completion `requestIds` entry is invalid, ignoring"
                        );
                    }
                }
                Some(parsed)
            }
            None => {
                tracing::warn!(
                    value = ?value,
                    "KAS turn_completion `requestIds` is not an array, ignoring"
                );
                None
            }
        },
    };

    TurnMeteringUpdate::new(charges, duration_ms, status, used_tools, request_ids)
}

fn parse_metered_amount(summary: &serde_json::Value) -> Option<MeteredAmount> {
    let amount = summary.get("usage").and_then(serde_json::Value::as_f64);
    let unit = summary.get("unit").and_then(serde_json::Value::as_str);
    let unit_plural = summary
        .get("unitPlural")
        .and_then(serde_json::Value::as_str);
    let (Some(amount), Some(unit), Some(unit_plural)) = (amount, unit, unit_plural) else {
        tracing::warn!(
            value = ?summary,
            "KAS turn_completion summary lacks valid usage/unit/unitPlural, ignoring charge"
        );
        return None;
    };
    match MeteredAmount::try_new(amount, unit, unit_plural) {
        Ok(charge) => Some(charge),
        Err(error) => {
            tracing::warn!(
                value = ?summary,
                error = %error,
                "KAS turn_completion summary charge is invalid, ignoring"
            );
            None
        }
    }
}

fn optional_u64(
    value: &serde_json::Value,
    field: &'static str,
    source: &'static str,
) -> Option<u64> {
    match value.get(field) {
        None => None,
        Some(value) => match value.as_u64() {
            Some(parsed) => Some(parsed),
            None => {
                tracing::warn!(value = ?value, field, source, "numeric field is invalid, ignoring");
                None
            }
        },
    }
}

/// Stop reason for a `turn_end` frame, defaulting [`StopReason::EndTurn`] when
/// `_meta.kiro.stopReason` is missing or unparseable (a dropped turn_end would
/// strand the UI busy forever — a runtime fallback, not a `debug_assert!`).
fn turn_end_stop_reason(kiro: &serde_json::Value) -> StopReason {
    let raw_stop_reason = kiro.get("stopReason");
    raw_stop_reason
        .and_then(serde_json::Value::as_str)
        .and_then(|s| {
            serde_json::from_value::<acp::StopReason>(serde_json::Value::String(s.to_owned())).ok()
        })
        .map(super::to_stop_reason)
        .unwrap_or_else(|| {
            // Distinguish "missing" from "corrupt" (CLAUDE.md): log the offending
            // value (`None` = absent, `Some(..)` = present-but-unparseable) so a
            // wire drift is diagnosable, not hidden behind a generic message.
            tracing::warn!(
                stop_reason = ?raw_stop_reason,
                "KAS turn_end `_meta.kiro.stopReason` missing or unparseable; defaulting to EndTurn"
            );
            StopReason::EndTurn
        })
}

/// Parse the `_meta.kiro.breakdown` object into a [`ContextBreakdown`]. Returns
/// `None` (treated as "no breakdown this frame") if the object is absent or any
/// of the five named buckets is missing/malformed — never an error, never a
/// panic. O(1): five fixed buckets.
fn parse_breakdown(breakdown: Option<&serde_json::Value>) -> Option<ContextBreakdown> {
    let bd = breakdown?;
    Some(ContextBreakdown::new(
        parse_bucket(bd.get("contextFiles"))?,
        parse_bucket(bd.get("sessionFiles"))?,
        parse_bucket(bd.get("tools"))?,
        parse_bucket(bd.get("yourPrompts"))?,
        parse_bucket(bd.get("kiroResponses"))?,
    ))
}

/// Parse one breakdown bucket `{tokens, percent}`. `None` if absent or either
/// field is missing/the wrong type — so a malformed bucket degrades the whole
/// breakdown to absent rather than fabricating a sentinel zero.
fn parse_bucket(bucket: Option<&serde_json::Value>) -> Option<ContextBucket> {
    let b = bucket?;
    let tokens = b.get("tokens").and_then(serde_json::Value::as_u64)?;
    let percent = b.get("percent").and_then(serde_json::Value::as_f64)?;
    Some(ContextBucket::new(tokens, percent))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use serde_json::json;

    use super::*;
    use crate::protocol::engine::{Engine, KasEngine};

    /// Deserialize a captured fixture into a `SessionNotification` — the exact
    /// layer the acp Client parses a `session/update` at (mirrors the
    /// `schema_deserializes_captured_kas_session_updates` loader in `mod.rs`).
    fn load(name: &str) -> (serde_json::Value, acp::SessionNotification) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas")
            .join(name);
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("fixture is JSON");
        let parsed: acp::SessionNotification =
            serde_json::from_value(value.clone()).expect("fixture deserializes");
        (value, parsed)
    }

    fn load_jsonl(name: &str) -> Vec<acp::SessionNotification> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas")
            .join(name);
        std::fs::read_to_string(&path)
            .expect("read fixture")
            .lines()
            .map(|line| serde_json::from_str(line).expect("fixture line deserializes"))
            .collect()
    }

    #[test]
    fn account_usage_response_maps_exactly() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas/workflow/terminal-aborted-2.16.2.jsonl");
        let raw = std::fs::read_to_string(path).expect("read captured sweep");
        let response = raw
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSONL fixture"))
            .find_map(|line| {
                line.pointer("/parsed/result")
                    .filter(|result| result.pointer("/data/planName").is_some())
                    .cloned()
            })
            .expect("captured account response");
        let account = account_usage_from_response(&response).expect("captured response maps");
        assert_eq!(account.plan_name, "KIRO PRO MAX");
        assert_eq!(account.billing_cycle_reset, "2026-09-01");
        assert!(!account.overages_enabled);
        assert!(!account.is_enterprise);
        assert!(account.overage_capable);
        assert_eq!(account.usage_breakdowns.len(), 1);
        let credits = &account.usage_breakdowns[0];
        assert_eq!(credits.resource_type, "CREDIT");
        assert_eq!(credits.display_name, "Credits");
        assert_eq!(credits.used, 1075.01);
        assert!(credits.has_limit);
        assert_eq!(credits.limit, 5000.0);
        assert_eq!(credits.percentage, 21.0);
        assert_eq!(credits.current_overages, 0.0);
        assert_eq!(credits.overage_rate, 0.04);
        assert_eq!(credits.overage_charges, Some(0.0));
        assert_eq!(credits.currency, "USD");
        assert!(account.bonus_credits.is_empty());
        assert!(account.add_on_credits.is_empty());

        let with_bonus = json!({
            "success": true,
            "data": {
                "planName": "Plan",
                "billingCycleReset": "2026-09-01",
                "overagesEnabled": true,
                "isEnterprise": false,
                "overageCapable": true,
                "usageBreakdowns": [{
                    "resourceType": "CREDIT",
                    "displayName": "Credits",
                    "used": 104.0,
                    "limit": 100.0,
                    "percentage": 104,
                    "hasLimit": true,
                    "currentOverages": 4.0,
                    "overageRate": 0.04,
                    "overageCharges": 0.16,
                    "currency": "USD"
                }],
                "bonusCredits": [{
                    "name": "Welcome bonus",
                    "used": 81.96,
                    "total": 500.0,
                    "daysUntilExpiry": 12
                }],
                "addOnCredits": [{
                    "used": 2.0,
                    "total": 100.0,
                    "isActive": true,
                    "expiresAt": "2026-10-01"
                }]
            }
        });
        let account = account_usage_from_response(&with_bonus).expect("bonus maps");
        assert_eq!(account.bonus_credits[0].name, "Welcome bonus");
        assert_eq!(account.bonus_credits[0].days_until_expiry, 12);
        assert_eq!(account.usage_breakdowns[0].percentage, 100.0);
        assert_eq!(account.add_on_credits[0].total, 100.0);
        assert_eq!(
            account.add_on_credits[0].expires_at.as_deref(),
            Some("2026-10-01")
        );
        let admin_managed = json!({
            "success": true,
            "message": "Your plan is managed by admin"
        });
        assert_eq!(
            account_usage_from_response(&admin_managed)
                .expect_err("admin-managed response is unavailable")
                .to_string(),
            "account usage request failed: Your plan is managed by admin"
        );

        for invalid in [
            json!({"success": false, "message": "denied"}),
            json!({"success": true, "data": {}}),
            json!({
                "success": true,
                "data": {
                    "planName": "Plan", "billingCycleReset": "date",
                    "overagesEnabled": false, "isEnterprise": false,
                    "overageCapable": false,
                    "usageBreakdowns": [{
                        "resourceType": "CREDIT", "displayName": "Credits",
                        "used": -1, "limit": 1, "percentage": 101,
                        "hasLimit": true,
                        "currentOverages": 0, "overageRate": 0,
                        "currency": "USD"
                    }],
                    "bonusCredits": [],
                    "addOnCredits": []
                }
            }),
        ] {
            assert!(account_usage_from_response(&invalid).is_err());
        }
    }

    #[test]
    #[ignore = "reference-workstation account conversion budget"]
    fn account_usage_conversion_budget_reference() {
        let breakdowns = (0..5_000)
            .map(|index| {
                json!({
                    "resourceType": "CREDIT",
                    "displayName": format!("Credits {index}"),
                    "used": 1, "limit": 100, "percentage": 1,
                    "hasLimit": true,
                    "currentOverages": 0, "overageRate": 0.04,
                    "overageCharges": 0, "currency": "USD"
                })
            })
            .collect::<Vec<_>>();
        let bonuses = (0..5_000)
            .map(|index| {
                json!({
                    "name": format!("Bonus {index}"),
                    "used": 1, "total": 10, "daysUntilExpiry": 30
                })
            })
            .collect::<Vec<_>>();
        let response = json!({
            "success": true,
            "data": {
                "planName": "Plan", "billingCycleReset": "date",
                "overagesEnabled": false, "isEnterprise": false,
                "overageCapable": false,
                "usageBreakdowns": breakdowns, "bonusCredits": bonuses,
                "addOnCredits": []
            }
        });
        let started = std::time::Instant::now();
        let account = account_usage_from_response(&response).expect("stress response maps");
        let elapsed = started.elapsed();
        assert_eq!(account.usage_breakdowns.len(), 5_000);
        assert_eq!(account.bonus_credits.len(), 5_000);
        assert!(
            elapsed <= std::time::Duration::from_millis(25),
            "10,000-entry account conversion exceeded 25ms: {elapsed:?}"
        );
    }

    fn info_update(sn: &acp::SessionNotification) -> &acp::SessionInfoUpdate {
        match &sn.update {
            acp::SessionUpdate::SessionInfoUpdate(siu) => siu,
            other => panic!("fixture is not a session_info_update: {other:?}"),
        }
    }

    /// Build a `session_info_update` carrying `_meta.kiro = kiro`, the envelope
    /// every KAS lifecycle frame arrives in.
    fn siu(kiro: serde_json::Value) -> acp::SessionInfoUpdate {
        let sn: acp::SessionNotification = serde_json::from_value(json!({
            "sessionId": "sess_test",
            "update": { "sessionUpdate": "session_info_update", "_meta": { "kiro": kiro } }
        }))
        .expect("session_info_update envelope deserializes");
        match sn.update {
            acp::SessionUpdate::SessionInfoUpdate(siu) => siu,
            other => panic!("not a session_info_update: {other:?}"),
        }
    }

    // cyril-gk17: the hook_update frame, verbatim from the live capture
    // `kas-v2hooks-2.16.0.jsonl`. Under kas_hooks="kas" the AGENT runs these
    // commands on the host with no permission prompt, so dropping the frame
    // means shell execution with no user-visible record at all.
    #[test]
    fn hook_update_terminal_states_surface_progress_states_drop() {
        let frame = |status: &str, exit: Option<i64>| {
            let mut hook = serde_json::json!({
                "hookId": "/w/.kiro/hooks/audit.json#hook-0",
                "operationId": "7211cbc3-fca6-4c7f-a4c7-58435b8be937",
                "name": "cyril-audit-pre",
                "status": status,
                "actionType": "runCommand"
            });
            if let Some(code) = exit {
                hook["output"] = serde_json::json!({ "result": { "exitCode": code } });
            }
            siu(serde_json::json!({ "kind": "hook_update", "hook": hook }))
        };

        // Terminal states surface, carrying the exit code when reported.
        match session_info_to_notification(&frame("completed", Some(0))) {
            Some(Notification::HookExecuted {
                name,
                status,
                exit_code,
            }) => {
                assert_eq!(name, "cyril-audit-pre");
                assert_eq!(status, "completed");
                assert_eq!(exit_code, Some(0));
            }
            other => panic!("completed must surface, got {other:?}"),
        }
        for status in ["failed", "canceled"] {
            assert!(
                matches!(
                    session_info_to_notification(&frame(status, None)),
                    Some(Notification::HookExecuted {
                        exit_code: None,
                        ..
                    })
                ),
                "{status} must surface with no invented exit code"
            );
        }

        // Progress states surface too: a hook that hangs (or is killed before
        // it reports) emits `running` and nothing else, and dropping it would
        // leave the only audit trail of agent-run shell completely empty.
        // Regression fence for the cyril-gk17 review finding.
        for status in ["running", "awaiting_approval"] {
            match session_info_to_notification(&frame(status, None)) {
                Some(Notification::HookExecuted {
                    name,
                    status: got,
                    exit_code,
                }) => {
                    assert_eq!(name, "cyril-audit-pre");
                    assert_eq!(got, status, "status must surface verbatim");
                    assert_eq!(exit_code, None, "{status} reports no exit code");
                }
                other => panic!("{status} must reach the transcript, got {other:?}"),
            }
        }
    }

    #[test]
    fn hook_update_without_a_name_is_dropped() {
        // "Hook : failed" tells the user nothing actionable.
        let f = siu(serde_json::json!({
            "kind": "hook_update",
            "hook": { "status": "failed", "actionType": "runCommand" }
        }));
        assert!(session_info_to_notification(&f).is_none());
    }

    #[test]
    fn turn_end_maps_to_turn_completed_endturn() {
        let (value, sn) = load("session_info_update_turn_end.json");
        let result = session_info_to_notification(info_update(&sn));

        // Oracle: the converter reads the FLAT `_meta.kiro.stopReason`; this
        // independently reads the MIRRORED `_meta.kiro.turnEnd.stopReason` path
        // (the capture showed they agree) and maps it via the acp deserializer.
        let mirrored = value["update"]["_meta"]["kiro"]["turnEnd"]["stopReason"]
            .as_str()
            .expect("mirrored stopReason");
        let oracle = super::super::to_stop_reason(
            serde_json::from_value::<acp::StopReason>(json!(mirrored)).expect("oracle parses"),
        );
        assert_eq!(oracle, StopReason::EndTurn, "oracle precondition");
        assert!(
            matches!(result, Some(Notification::TurnCompleted { stop_reason }) if stop_reason == oracle),
            "turn_end must map to TurnCompleted with the mirrored stop reason, got {result:?}"
        );
    }

    #[test]
    fn captured_turn_completion_maps_exactly_and_is_not_terminal() {
        let notifications = load_jsonl("turn_completion_2_16_0_four.jsonl");
        let expected = [
            (0.104_566_227_363_184_09, 4442, ["read_file"].as_slice(), 2),
            (
                0.076_486_929_353_233_84,
                4223,
                ["execute_bash"].as_slice(),
                2,
            ),
            (0.081_022_536_815_920_39, 4690, ["fs_write"].as_slice(), 2),
            (
                0.123_497_716_915_422_9,
                8087,
                ["invoke_sub_agent", "subagent_response"].as_slice(),
                3,
            ),
        ];
        assert_eq!(notifications.len(), expected.len());
        for (notification, (credits, duration, tools, request_count)) in
            notifications.iter().zip(expected)
        {
            let result = session_info_to_notification(info_update(notification))
                .expect("captured turn completion converts");
            let Notification::TurnMeteringUpdated(update) = result else {
                panic!("turn_completion must map only to metering, got {result:?}");
            };
            assert_eq!(update.duration_ms(), Some(duration));
            assert_eq!(
                update.status(),
                Some(&crate::types::UsageTurnStatus::Success)
            );
            assert_eq!(
                update.request_ids().map(<[String]>::len),
                Some(request_count)
            );
            assert_eq!(
                update.used_tools(),
                tools,
                "captured usedTools must remain exact"
            );
            assert_eq!(update.charges().len(), 1);
            assert!((update.charges()[0].amount() - credits).abs() < f64::EPSILON);
            assert_eq!(update.charges()[0].unit(), "credit");
            assert_eq!(update.charges()[0].unit_plural(), "credits");
        }
    }

    #[test]
    fn other_sub_kind_is_ignored() {
        // user_message_id_assigned — guards "every session_info_update is a turn end".
        let (_v, sn) = load("session_info_update.json");
        assert!(session_info_to_notification(info_update(&sn)).is_none());
    }

    /// Synthetic `session_info_update` frame around an arbitrary `_meta.kiro`
    /// payload (renamed from `context_usage_frame` when steering tests started
    /// using it too — it was never context-specific).
    fn kiro_frame(kiro: serde_json::Value) -> acp::SessionNotification {
        serde_json::from_value(json!({
            "sessionId": "sess_x",
            "update": { "sessionUpdate": "session_info_update", "_meta": { "kiro": kiro } }
        }))
        .expect("frame deserializes")
    }

    #[test]
    fn turn_completion_presence_matrix_preserves_absence_and_units() {
        let notification = kiro_frame(json!({
            "kind": "turn_completion",
            "promptTurnSummaries": [
                {"unit": "credit", "unitPlural": "credits", "usage": 0.0},
                {"unit": "request", "unitPlural": "requests", "usage": 2.0},
                {"unit": "credit", "unitPlural": "credits", "usage": -1.0},
                {"unit": "", "unitPlural": "invalid", "usage": 4.0}
            ],
            "status": "future"
        }));
        let result = session_info_to_notification(info_update(&notification))
            .expect("turn completion converts despite optional gaps");
        let Notification::TurnMeteringUpdated(update) = result else {
            panic!("expected metering update, got {result:?}");
        };
        assert_eq!(update.duration_ms(), None);
        assert_eq!(
            update.status(),
            Some(&crate::types::UsageTurnStatus::Other("future".to_owned()))
        );
        assert_eq!(update.request_ids(), None);
        assert_eq!(update.charges().len(), 2);
        assert_eq!(
            update
                .charges()
                .iter()
                .map(|charge| (charge.unit(), charge.amount()))
                .collect::<Vec<_>>(),
            vec![("credit", 0.0), ("request", 2.0)]
        );

        let explicit_empty = kiro_frame(json!({
            "kind": "turn_completion",
            "requestIds": [],
            "status": "",
            "elapsedTime": "invalid"
        }));
        let result = session_info_to_notification(info_update(&explicit_empty))
            .expect("kind remains observable");
        let Notification::TurnMeteringUpdated(update) = result else {
            panic!("expected metering update, got {result:?}");
        };
        assert_eq!(update.request_ids(), Some([].as_slice()));
        assert_eq!(update.status(), None);
        assert_eq!(update.duration_ms(), None);
    }

    #[test]
    fn turn_completion_conversion_stays_within_budget() {
        let summaries = (0..2_500)
            .map(|index| {
                json!({
                    "unit": "credit",
                    "unitPlural": "credits",
                    "usage": 0.001,
                    "usedTools": [format!("tool_{index}")]
                })
            })
            .collect::<Vec<_>>();
        let request_ids = (0..5_000)
            .map(|index| format!("request_{index}"))
            .collect::<Vec<_>>();
        let notification = kiro_frame(json!({
            "kind": "turn_completion",
            "promptTurnSummaries": summaries,
            "elapsedTime": 1,
            "status": "success",
            "requestIds": request_ids
        }));

        let started = std::time::Instant::now();
        let result = session_info_to_notification(info_update(&notification))
            .expect("stress frame converts");
        let elapsed = started.elapsed();
        let Notification::TurnMeteringUpdated(update) = result else {
            panic!("expected metering update, got {result:?}");
        };
        assert_eq!(update.charges().len(), 2_500);
        assert_eq!(update.used_tools().len(), 2_500);
        assert_eq!(update.request_ids().map(<[String]>::len), Some(5_000));
        assert!(
            elapsed <= std::time::Duration::from_millis(25),
            "10,000-element conversion took {elapsed:?}, budget is 25ms"
        );
    }

    // cyril-vgcm C5: the three steering kinds map to the same notifications the
    // v2 families produce, off verbatim captured frames (2026-07-14, kiro-cli
    // 2.12.1 KAS — bundle byte-identical across 2.11.0–2.12.1). The expected
    // ids/text below are the capture's own values, not invented ones.
    #[test]
    fn steering_kind_queued_converts() {
        let (_v, sn) = load("session_info_update_steering_queued.json");
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                &result,
                Some(Notification::SteeringQueued { message, message_id })
                    if message.as_deref()
                        == Some("IMPORTANT: end your reply with the single word STEERMARK_KILO")
                        && message_id.as_deref()
                            == Some("steer-8307ad8f-6404-40c3-a730-f7a1bfff4f60")
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn steering_kind_injected_converts() {
        // KAS says `steering_injected` where v2's old family said consumed —
        // both mean "one queued steer drained into the turn".
        let (_v, sn) = load("session_info_update_steering_injected.json");
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                &result,
                Some(Notification::SteeringConsumed { content, message_id })
                    if content.as_deref()
                        == Some("IMPORTANT: end your reply with the single word STEERMARK_LIMA")
                        && message_id.as_deref()
                            == Some("steer-6c7728c1-8eb8-4f99-8bdf-71d3c5a3bc26")
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn steering_kind_cleared_converts() {
        let (_v, sn) = load("session_info_update_steering_cleared.json");
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                &result,
                Some(Notification::SteeringCleared { message_ids })
                    if message_ids
                        == &vec!["steer-8307ad8f-6404-40c3-a730-f7a1bfff4f60".to_string()]
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn steering_inclusion_is_not_a_queue_echo() {
        // The fileMatch steering *catalog* kind (captured same run) must stay
        // ignored — kind matching is exact, never prefix/substring: the probe
        // itself had to filter this noise, so the bug class is live.
        let (_v, sn) = load("session_info_update_steering_inclusion.json");
        assert!(session_info_to_notification(info_update(&sn)).is_none());
    }

    // cyril-vgcm C5 stress: degrade discipline (mirrors convert::kiro's).
    // Bug classes: drop-on-missing-field (chip desync), empty-string id
    // sentinel, absent-vs-empty messageIds divergence, corrupt id entries.
    #[test]
    fn steering_kinds_degrade_never_drop() {
        // Queued with id but no content -> emitted, text None.
        let sn = kiro_frame(json!({ "kind": "steering_queued", "messageId": "steer-x" }));
        assert!(matches!(
            session_info_to_notification(info_update(&sn)),
            Some(Notification::SteeringQueued { message: None, message_id })
                if message_id.as_deref() == Some("steer-x")
        ));
        // Empty-string id -> None; text still carried.
        let sn = kiro_frame(json!({
            "kind": "steering_injected", "messageId": "", "content": "still counted"
        }));
        assert!(matches!(
            session_info_to_notification(info_update(&sn)),
            Some(Notification::SteeringConsumed {
                content: Some(_),
                message_id: None
            })
        ));
        // Cleared: absent messageIds AND present-but-empty -> both empty
        // (UI drain-all semantics, C7).
        for kiro in [
            json!({ "kind": "steering_cleared" }),
            json!({ "kind": "steering_cleared", "messageIds": [] }),
        ] {
            let sn = kiro_frame(kiro);
            assert!(matches!(
                session_info_to_notification(info_update(&sn)),
                Some(Notification::SteeringCleared { message_ids }) if message_ids.is_empty()
            ));
        }
        // Corrupt entries dropped with the valid one kept.
        let sn = kiro_frame(json!({
            "kind": "steering_cleared", "messageIds": ["steer-ok", 7, ""]
        }));
        assert!(matches!(
            session_info_to_notification(info_update(&sn)),
            Some(Notification::SteeringCleared { message_ids })
                if message_ids == vec!["steer-ok".to_string()]
        ));
    }

    #[test]
    fn context_usage_maps_breakdown() {
        // Slice 3 / claim C1. The real 2.10.0 frame maps to ContextBreakdownUpdated
        // with the 5 buckets' exact tokens/percent. Expected values are the
        // independent jq oracle's (.cyril-5et2/oracle.sh on the same fixture).
        let (_v, sn) = load("session_info_update_context_usage.json");
        let result = session_info_to_notification(info_update(&sn));
        let Some(Notification::ContextBreakdownUpdated {
            usage_percentage,
            breakdown,
        }) = result
        else {
            panic!("expected ContextBreakdownUpdated, got {result:?}");
        };
        assert!((usage_percentage - 4.3).abs() < f64::EPSILON);
        let bd = breakdown.expect("breakdown present");
        for (bucket, tokens, percent) in [
            (bd.context_files(), 0u64, 0.0),
            (bd.tools(), 4662, 2.3),
            (bd.your_prompts(), 4096, 2.0),
            (bd.kiro_responses(), 0, 0.0),
            (bd.session_files(), 0, 0.0),
        ] {
            assert_eq!(bucket.tokens(), tokens);
            assert!((bucket.percent() - percent).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn context_usage_reads_flat_usage_not_nested() {
        // Slice 3 / claim C2. Flat `_meta.kiro.usagePercentage` (9.9) wins over the
        // nested `contextUsage.usagePercentage` (1.1). Fails if the converter reads
        // the nested wrapper.
        let sn = kiro_frame(json!({
            "kind": "context_usage",
            "usagePercentage": 9.9,
            "contextUsage": { "usagePercentage": 1.1 }
        }));
        let result = session_info_to_notification(info_update(&sn));
        let Some(Notification::ContextBreakdownUpdated {
            usage_percentage, ..
        }) = result
        else {
            panic!("expected ContextBreakdownUpdated, got {result:?}");
        };
        assert!(
            (usage_percentage - 9.9).abs() < f64::EPSILON,
            "got {usage_percentage}"
        );
    }

    #[test]
    fn context_usage_breakdown_absent_still_carries_scalar() {
        // Slice 3 / claim C3. No `breakdown` key → Some with breakdown None, scalar
        // intact. Fails under `breakdown.unwrap()` or returning None (which would
        // drop the % update and freeze the toolbar).
        let sn = kiro_frame(json!({ "kind": "context_usage", "usagePercentage": 12.5 }));
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                result,
                Some(Notification::ContextBreakdownUpdated { usage_percentage, breakdown: None })
                    if (usage_percentage - 12.5).abs() < f64::EPSILON
            ),
            "got {result:?}"
        );
    }

    #[test]
    fn context_usage_malformed_breakdown_degrades_to_none() {
        // Slice 3 / claim C3. A breakdown missing a bucket (here `tools`) degrades
        // the whole breakdown to None — never a fabricated sentinel-zero bucket —
        // while the scalar still updates.
        let sn = kiro_frame(json!({
            "kind": "context_usage", "usagePercentage": 3.0,
            "breakdown": {
                "contextFiles": { "tokens": 0, "percent": 0 },
                "sessionFiles": { "tokens": 0, "percent": 0 },
                "yourPrompts": { "tokens": 1, "percent": 1 },
                "kiroResponses": { "tokens": 0, "percent": 0 }
                // tools missing
            }
        }));
        let result = session_info_to_notification(info_update(&sn));
        assert!(
            matches!(
                result,
                Some(Notification::ContextBreakdownUpdated {
                    breakdown: None,
                    ..
                })
            ),
            "malformed breakdown must degrade to None, got {result:?}"
        );
    }

    #[test]
    fn turn_end_without_stop_reason_still_completes() {
        // Load-bearing fallback: a turn_end missing stopReason must NOT be
        // dropped (that strands the UI busy) — defaults EndTurn.
        let value = json!({
            "sessionId": "sess_x",
            "update": { "sessionUpdate": "session_info_update",
                        "_meta": { "kiro": { "kind": "turn_end" } } }
        });
        let sn: acp::SessionNotification = serde_json::from_value(value).unwrap();
        assert!(matches!(
            session_info_to_notification(info_update(&sn)),
            Some(Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn
            })
        ));
    }

    #[test]
    fn kas_engine_routes_turn_end_to_completion() {
        let (_v, sn) = load("session_info_update_turn_end.json");
        let n = KasEngine::default().convert_session_update(&sn);
        assert!(matches!(
            n,
            Some(Notification::TurnCompleted {
                stop_reason: StopReason::EndTurn
            })
        ));
    }

    #[test]
    fn kas_engine_still_renders_agent_text() {
        // Slice 1 must NOT break text rendering: non-turn_end updates delegate
        // to the generic converter (agent_message_chunk -> AgentMessage).
        let (_v, sn) = load("agent_message_chunk.json");
        let n = KasEngine::default().convert_session_update(&sn);
        assert!(
            matches!(n, Some(Notification::AgentMessage(_))),
            "agent_message_chunk must still render via the generic path, got {n:?}"
        );
    }

    /// ENGINE-LEVEL fence for the `Dropped` arm (2026-08-09 review, test
    /// finding 2): a recognized workflow method with a malformed payload must
    /// reach the client as `Ok(None)` — never `Err`, which would route into
    /// the client's malformed-extension error path and poison the stream.
    /// cyril-0qe6 C8: exactly the four gate-advertised workflow commands are
    /// suppressed — the buggy implementations are no filter at all, and a
    /// prefix match that eats `workflow-creator`-style names.
    #[test]
    fn gate_workflow_commands_suppressed_exactly() {
        let info = |name: &str| {
            crate::types::CommandInfo::new(name, name, None::<String>, false, false, false)
        };
        let update = Notification::CommandsUpdated {
            commands: vec![
                info("workflow-run"),
                info("workflow-status"),
                info("workflow-cancel"),
                info("workflow-resume"),
                info("workflow-creator"),
                info("steer"),
                info("wörkflöw"),
            ],
            prompts: Vec::new(),
        };
        let Notification::CommandsUpdated { commands, .. } =
            suppress_workflow_gate_commands(update)
        else {
            panic!("variant must be preserved");
        };
        let names: Vec<&str> = commands.iter().map(|command| command.name()).collect();
        assert_eq!(
            names,
            vec!["workflow-creator", "steer", "wörkflöw"],
            "exact-name suppression only — order and other entries preserved"
        );

        // Identity on an empty update and on non-command notifications.
        let empty = suppress_workflow_gate_commands(Notification::CommandsUpdated {
            commands: Vec::new(),
            prompts: Vec::new(),
        });
        assert!(
            matches!(empty, Notification::CommandsUpdated { ref commands, .. } if commands.is_empty())
        );
        let other = suppress_workflow_gate_commands(Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        });
        assert!(matches!(other, Notification::TurnCompleted { .. }));
    }

    /// The adapter suite fences the adapter; this fences the engine match.
    #[test]
    fn kas_engine_drops_malformed_workflow_frame_to_ok_none() {
        let r = KasEngine::default()
            .convert_ext_notification("kiro/workflow/run_start", &json!({"inputs": {}}));
        assert!(
            matches!(r, Ok(None)),
            "malformed workflow frame must warn-and-drop to Ok(None), got {r:?}"
        );
    }

    #[test]
    fn kas_engine_drops_unknown_ext_frame() {
        // KAS-2a (cyril-j16p) Slice 3 — unknown-variant tolerance: an
        // unrecognised `_kiro/*` frame (arriving as `kiro/*` once the acp crate
        // strips the leading underscore) drops to `Ok(None)` — no error, no hang.
        // Non-workflow names continue to the existing Kiro extension handler;
        // its unknown-variant arm owns this and fences the fallback path.
        let r = KasEngine::default().convert_ext_notification("kiro/does/not/exist", &json!({}));
        assert!(
            matches!(r, Ok(None)),
            "unknown _kiro/* frame must drop to Ok(None), got {r:?}"
        );
    }
}

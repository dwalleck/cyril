//! KAS powers adapter (cyril-v19o).
//!
//! KAS announces its installed power set with a single push,
//! `_kiro/powers/items_changed` (canonicalized by the mediator to
//! `kiro/powers/items_changed` before it reaches an engine):
//!
//! ```json
//! {
//!   "sessionId": "sess_…",
//!   "status": "success",
//!   "powers": [
//!     {
//!       "name": "datadog",
//!       "displayName": "Datadog Observability",
//!       "description": "Query logs, metrics, traces…",
//!       "keywords": ["datadog", "observability"],
//!       "mcpServerNames": ["datadog"],
//!       "hasSteeringFiles": true,
//!       "isAgentPlugin": false,
//!       "_meta": { "kiro": { "resource": { … } } }
//!     }
//!   ]
//! }
//! ```
//!
//! The push is **unprompted** — measured on 2.21.2 as the client issuing only
//! `initialize` and `session/new`, with the frame arriving 18 ms after the
//! `session/new` reply — so this adapter is the only way the catalog reaches
//! cyril. The sibling pull methods are deliberately unused: `_kiro/powers/list`
//! is not advertised in `extensionMethods`, so it cannot be feature-detected
//! and its contract is unverifiable, and `_kiro/powers/refresh` is
//! declared-but-unimplemented, answering `-32603 Unknown ext method`
//! (cyril-v19o evidence P2/P3). See [`crate::types::PowerInfo`] for why nothing
//! here is addressable.
//!
//! Only the fields cyril renders are decoded. `keywords`, `isAgentPlugin`, and
//! `_meta` are ignored — no consumer exists for them, and decoding them would
//! be weightless state; unknown keys are ignored on purpose so a future
//! additive wire field cannot break the frame.

use serde::Deserialize;
use serde::de::Error as _;

use crate::protocol::kas::discovery::nonempty;
use crate::types::{Notification, PowerInfo};

/// The canonicalized method name this adapter claims.
pub(crate) const METHOD: &str = "kiro/powers/items_changed";

/// The push frame's params. `sessionId` and `status` are present on the wire
/// and deliberately not decoded. The catalog is the user-level
/// `~/.kiro/powers` install — one fact per agent process, not per session — so
/// the frame's `sessionId` names no scope the payload has; KAS extension frames
/// route globally for the same reason (`domain_mediator/inbound.rs` routes
/// every converted extension notification as `RoutedNotification::global`). And
/// `status` was `"success"` on every observed frame — including the
/// empty-catalog one, which is the point of `powers: []` being a *loaded*
/// catalog rather than an error.
#[derive(Debug, Deserialize)]
struct WirePowersChanged {
    /// Required. A frame without it is drift, not an empty catalog.
    powers: Vec<WirePower>,
}

/// One power on the wire. Extra keys (`keywords`, `isAgentPlugin`, `_meta`)
/// are ignored, never rejected.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePower {
    /// The only required item field: without it a power cannot be rendered,
    /// matched, or sorted, so a frame containing such an item is dropped
    /// whole (see [`to_notification`]). A blank string is that same fact, not
    /// an identifier — an empty BOLD row sorted above every real power
    /// (cyril-v19o review finding 12).
    #[serde(deserialize_with = "identified_name")]
    name: String,
    display_name: Option<String>,
    description: Option<String>,
    /// Absent, or explicitly `null`, means the agent reported no MCP servers.
    ///
    /// `Option` rather than a plain `#[serde(default)]` `Vec`: `default`
    /// covers an ABSENT key only, so an explicit `null` reaches
    /// `Vec::deserialize`, errors, and discards every power in the frame over
    /// one optional token (cyril-v19o review finding 9).
    #[serde(default)]
    mcp_server_names: Option<Vec<String>>,
    /// Absent, or explicitly `null`, means the agent did not claim steering
    /// files: `false` and "not stated" are the same fact for a display-only
    /// marker, and rejecting the frame would discard a power the user has
    /// over a field cyril does not act on.
    #[serde(default)]
    has_steering_files: Option<bool>,
}

/// `name` is required *and* required to identify something: a blank or
/// whitespace-only name cannot title a row, be matched, or be sorted, which is
/// the state a missing `name` leaves the frame in — so both fail through the
/// same predicate the discovery path uses for env values ([`nonempty`]).
fn identified_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    nonempty(Some(raw)).ok_or_else(|| D::Error::custom("power name must not be blank"))
}

impl From<WirePower> for PowerInfo {
    fn from(wire: WirePower) -> Self {
        Self::new(
            wire.name,
            wire.display_name,
            wire.description,
            wire.mcp_server_names.unwrap_or_default(),
            wire.has_steering_files.unwrap_or_default(),
        )
    }
}

/// Convert a `kiro/powers/items_changed` frame into its notification.
///
/// Three outcomes, all mapped onto the engine's `Result<Option<_>>`:
///
/// - a different method → `Ok(None)` (the engine continues to its next
///   converter),
/// - a convertible frame, empty catalog included → `Ok(Some(PowersChanged))`,
/// - this method with a payload that does not decode → a `warn!` naming the
///   method and the failing field, then `Ok(None)`.
///
/// The last case is deliberately not destructive: dropping the frame leaves
/// whatever catalog the session already held untouched, so a malformed push
/// cannot blank a panel the user is reading (cyril-v19o B7). It is also not a
/// pull-trigger: cyril never asks for the catalog again.
pub(crate) fn to_notification(
    method: &str,
    params: &serde_json::Value,
) -> crate::Result<Option<Notification>> {
    if method != METHOD {
        // Near-miss drift: a re-spelled or versioned member of the powers
        // family would otherwise fall through to the generic unknown-extension
        // `debug!` and vanish below the default log level, leaving `/powers`
        // answering "not reported yet" on a live KAS session with nothing in
        // cyril.log naming the family — the same reason the workflow adapter
        // warns for an unrecognized `kiro/workflow/` member.
        if method.contains("powers") {
            tracing::warn!(
                method,
                expected = METHOD,
                "unrecognized powers method; not converted"
            );
        }
        return Ok(None);
    }
    match parse(params) {
        Ok(powers) => Ok(Some(Notification::PowersChanged { powers })),
        Err(error) => {
            tracing::warn!(
                method,
                field_path = %error.path(),
                error = %error.inner(),
                "malformed powers notification; not converted"
            );
            Ok(None)
        }
    }
}

/// Decode the params into the domain catalog, naming the failing field
/// (`params.powers[1].name`) through the same path-tracking deserializer the
/// workflow adapter uses.
fn parse(
    params: &serde_json::Value,
) -> Result<Vec<PowerInfo>, serde_path_to_error::Error<serde_json::Error>> {
    let wire: WirePowersChanged = serde_path_to_error::deserialize(params)?;
    Ok(wire.powers.into_iter().map(PowerInfo::from).collect())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    /// The live 2.21.2 frame, verbatim (session id and all), captured from
    /// kiro-cli with cyril's own spawn shape. Every expectation below is read
    /// off this file, never off the converter's output.
    const CAPTURE: &str =
        include_str!("../../../../tests/fixtures/kas/powers/items-changed-2.21.2.json");

    fn capture_params() -> serde_json::Value {
        serde_json::from_str(CAPTURE).expect("fixture is JSON")
    }

    fn converted(params: serde_json::Value) -> Vec<PowerInfo> {
        match to_notification(METHOD, &params).expect("conversion does not fail") {
            Some(Notification::PowersChanged { powers }) => powers,
            other => panic!("expected PowersChanged, got {other:?}"),
        }
    }

    /// REGRESSION FENCE (cyril-v19o C1). Field-for-field agreement with the
    /// captured wire values.
    #[test]
    fn powers_frame_maps_every_field_from_the_capture() {
        let powers = converted(capture_params());
        assert_eq!(powers.len(), 3, "capture carries three installed powers");

        let names: Vec<&str> = powers.iter().map(PowerInfo::name).collect();
        assert_eq!(
            names,
            ["aws-infrastructure-as-code", "datadog", "markdownlint"],
            "wire order is preserved; ordering for display is UiState's job"
        );

        let titles: Vec<&str> = powers.iter().map(PowerInfo::title).collect();
        assert_eq!(
            titles,
            [
                "Build AWS infrastructure with CDK and CloudFormation",
                "Datadog Observability",
                "Markdownlint"
            ]
        );

        let aws = &powers[0];
        assert_eq!(
            aws.description(),
            Some(
                "Build well-architected AWS infrastructure with CDK using latest documentation, \
                 best practices, and code samples. Validate CloudFormation templates, check \
                 resource configuration security compliance, and troubleshoot deployments."
            )
        );
        assert_eq!(aws.mcp_server_names(), ["awslabs.aws-iac-mcp-server"]);
        assert!(
            !aws.has_steering_files(),
            "aws-infrastructure-as-code ships an EMPTY steering/ directory: the wire's \
             hasSteeringFiles counts files, and an empty directory is not a steering file"
        );

        let datadog = &powers[1];
        assert_eq!(datadog.mcp_server_names(), ["datadog"]);
        assert!(datadog.has_steering_files());
        assert_eq!(
            datadog.description(),
            Some(
                "Query logs, metrics, traces, RUM events, incidents, and monitors from Datadog \
                 for production debugging and performance analysis"
            )
        );

        let markdownlint = &powers[2];
        assert_eq!(markdownlint.mcp_server_names(), ["markdownlint"]);
        assert!(markdownlint.has_steering_files());
        assert_eq!(
            markdownlint.description(),
            Some(
                "Lint, validate, and auto-fix Markdown files using markdownlint rules. Enforces \
                 writing best practices including language consistency, fenced code language \
                 tags, image alt text, and table of contents for long documents."
            )
        );
    }

    /// REGRESSION FENCE (cyril-v19o C2). An empty installed set is a loaded
    /// catalog, not a dropped frame — B6's placeholder depends on it.
    #[test]
    fn empty_catalog_is_loaded_not_dropped() {
        let powers = converted(json!({ "powers": [], "status": "success" }));
        assert!(powers.is_empty());

        // Control: the same call shape on a malformed payload must NOT yield
        // an empty catalog, or "empty" and "unreadable" would be
        // indistinguishable downstream.
        assert!(
            to_notification(METHOD, &json!({ "status": "success" }))
                .unwrap()
                .is_none()
        );
    }

    /// REGRESSION FENCE (cyril-v19o C3). Drift shapes are dropped with a
    /// warning, never converted into an empty catalog that would blank the
    /// panel.
    #[test]
    fn malformed_powers_frames_drop_and_never_clear() {
        let malformed = [
            json!({ "status": "success" }),                // powers absent
            json!({ "powers": null }),                     // explicit null
            json!({ "powers": {} }),                       // not an array
            json!({ "powers": "datadog" }),                // not an array
            json!({ "powers": ["datadog"] }),              // item is not an object
            json!({ "powers": [{ "displayName": "x" }] }), // item has no name
            json!({ "powers": [{ "name": 7 }] }),          // name is not a string
            json!({ "powers": [{ "name": "ok" }, { "displayName": "no name" }] }),
            // A blank name identifies nothing, which is what a missing name
            // leaves the frame in: an empty BOLD row sorted above every real
            // power (review finding 12).
            json!({ "powers": [{ "name": "" }] }),
            json!({ "powers": [{ "name": "   " }] }),
        ];
        for params in malformed {
            assert!(
                to_notification(METHOD, &params).unwrap().is_none(),
                "expected drop for {params}"
            );
        }
    }

    /// REGRESSION FENCE (cyril-v19o review finding 9). An explicit `null` on a
    /// defaulted ITEM field is the agent saying "not provided", not a reason to
    /// discard the catalog: `#[serde(default)]` covers an absent key only, so
    /// both of those fields are `Option`-shaped. Dropping the frame here would
    /// leave `/powers` answering "not reported yet" for the rest of the
    /// session — the push fires once per session and cyril issues no pull.
    #[test]
    fn explicit_null_on_a_defaulted_item_field_keeps_the_catalog() {
        let powers = converted(json!({
            "powers": [
                { "name": "before" },
                {
                    "name": "nulls",
                    "mcpServerNames": null,
                    "hasSteeringFiles": null
                },
                { "name": "after" }
            ]
        }));
        assert_eq!(
            powers.len(),
            3,
            "one null field must not discard the whole frame"
        );
        assert_eq!(powers[1].name(), "nulls");
        assert!(
            powers[1].mcp_server_names().is_empty(),
            "a null server list is an empty one"
        );
        assert!(
            !powers[1].has_steering_files(),
            "a null steering flag is `not stated`, which renders as no token"
        );
        // The neighbours are untouched: the frame converted whole, not part.
        assert_eq!(powers[0].name(), "before");
        assert_eq!(powers[2].name(), "after");
    }

    /// The frame family is claimed by exact method name; anything else falls
    /// through untouched so the engine's remaining converters still run.
    #[test]
    fn other_methods_are_not_claimed() {
        assert!(
            to_notification("kiro/workflow/run_start", &json!({}))
                .unwrap()
                .is_none()
        );
        assert!(
            to_notification("kiro.dev/commands/available", &json!({}))
                .unwrap()
                .is_none()
        );
        assert!(
            to_notification("", &json!({ "powers": [] }))
                .unwrap()
                .is_none()
        );
    }

    /// REGRESSION FENCE (cyril-v19o review finding 11). A re-spelled or
    /// versioned powers method is drift the log must NAME: the push is
    /// once-per-session, so a frame that falls through silently leaves
    /// `/powers` answering "not reported yet" on a live KAS session with
    /// nothing in cyril.log to explain it — while an unrelated family must
    /// stay silent, or the warning is noise.
    ///
    /// The lookalike is spelled with the version on the FAMILY
    /// (`kiro/powers_v2/items_changed`), not on the member: the C7 census
    /// forbids any `kiro/powers/<segment>` spelling other than the push
    /// anywhere under `crates/*/src`, test modules included, and a versioned
    /// member is exactly the unverifiable method that census exists to keep
    /// out. Respelling this back to the member form fails the census.
    #[test]
    fn unrecognized_powers_methods_warn_but_other_families_stay_silent() {
        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        tracing::dispatcher::with_default(&dispatch, || {
            assert!(
                to_notification("kiro/powers_v2/items_changed", &json!({ "powers": [] }))
                    .unwrap()
                    .is_none(),
                "a lookalike must not be claimed as the push"
            );
            assert!(
                to_notification("kiro/workflow/run_start", &json!({}))
                    .unwrap()
                    .is_none(),
                "another family's frame still falls through untouched"
            );
        });

        let events = capture.captured();
        assert_eq!(
            events.len(),
            1,
            "exactly one warning — for the powers lookalike: {events:?}"
        );
        assert_eq!(events[0]["level"], "WARN", "at warn level: {events:?}");
        assert_eq!(
            events[0]["fields"]["method"], "kiro/powers_v2/items_changed",
            "the warning must name the method that went unrecognized: {events:?}"
        );
        assert_eq!(
            events[0]["fields"]["expected"], METHOD,
            "…and the method it was mistaken for: {events:?}"
        );
    }

    /// Forward compatibility (S14): a future additive key must not break the
    /// frame, and a large catalog must convert whole — no truncation, no
    /// de-duplication by name.
    #[test]
    fn unknown_keys_are_ignored_and_large_catalogs_convert_whole() {
        let with_unknown = json!({
            "powers": [{
                "name": "datadog",
                "displayName": "Datadog Observability",
                "fromTheFuture": { "nested": [1, 2, 3] },
                "mcpServerNames": ["datadog"],
                "hasSteeringFiles": true
            }]
        });
        let powers = converted(with_unknown);
        assert_eq!(powers.len(), 1);
        assert_eq!(powers[0].title(), "Datadog Observability");

        let bulk: Vec<serde_json::Value> = (0..200)
            .map(|n| {
                json!({
                    "name": format!("power-{n:03}"),
                    "mcpServerNames": [],
                    "hasSteeringFiles": false
                })
            })
            .collect();
        let powers = converted(json!({ "powers": bulk }));
        assert_eq!(powers.len(), 200);
        assert_eq!(powers[199].name(), "power-199");
    }

    /// Optional item fields: absent and empty both mean "not provided", and
    /// both fall back to the identifier rather than rendering a blank row.
    #[test]
    fn optional_item_fields_fall_back_without_placeholders() {
        let powers = converted(json!({
            "powers": [
                { "name": "bare" },
                { "name": "blank", "displayName": "", "description": "" },
                { "name": "named", "displayName": "Named" }
            ]
        }));
        assert_eq!(powers[0].title(), "bare");
        assert_eq!(powers[0].description(), None);
        assert!(powers[0].mcp_server_names().is_empty());
        assert!(!powers[0].has_steering_files());
        assert_eq!(powers[1].title(), "blank");
        assert_eq!(powers[1].description(), None);
        assert_eq!(powers[2].title(), "Named");
    }
}

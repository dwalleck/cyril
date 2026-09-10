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
//! is not advertised in `extensionMethods`, and `_kiro/powers/refresh` answers
//! `-32603 Unknown ext method` (cyril-v19o evidence P2/P3). See
//! [`crate::types::PowerInfo`] for why nothing here is addressable.
//!
//! Only the fields cyril renders are decoded. `keywords`, `isAgentPlugin`, and
//! `_meta` are ignored — no consumer exists for them, and decoding them would
//! be weightless state; unknown keys are ignored on purpose so a future
//! additive wire field cannot break the frame.

use serde::Deserialize;

use crate::types::{Notification, PowerInfo};

/// The canonicalized method name this adapter claims.
pub(crate) const METHOD: &str = "kiro/powers/items_changed";

/// The push frame's params. `sessionId` and `status` are present on the wire
/// and deliberately not decoded: the session is already known from the routing
/// envelope, and `status` was `"success"` on every observed frame — including
/// the empty-catalog one, which is the point of `powers: []` being a *loaded*
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
    /// whole (see [`to_notification`]).
    name: String,
    display_name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    mcp_server_names: Vec<String>,
    /// Absent means the agent did not claim steering files. Decoded with
    /// `default` rather than as a required field: `false` and "not stated"
    /// are the same fact for a display-only marker, and rejecting the frame
    /// would discard a power the user has over a field cyril does not act on.
    #[serde(default)]
    has_steering_files: bool,
}

impl From<WirePower> for PowerInfo {
    fn from(wire: WirePower) -> Self {
        Self::new(
            wire.name,
            wire.display_name,
            wire.description,
            wire.mcp_server_names,
            wire.has_steering_files,
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
        ];
        for params in malformed {
            assert!(
                to_notification(METHOD, &params).unwrap().is_none(),
                "expected drop for {params}"
            );
        }
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

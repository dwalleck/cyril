//! Engine-scoped resolution of cyril's presented identity (cyril-0wyn ADR-0006,
//! default flipped by cyril-df5l ADR-0008).
//!
//! Keyed off the **bound engine at runtime** ([`AgentEngine`]), never
//! `cfg(feature = "kas")` — a feature-gated build still drives v2 engines, and
//! keying identity behavior off the compile flag is exactly the trap ADR-0002
//! warns about (cyril-dn91).

use crate::types::agent_engine::AgentEngine;
use crate::types::present_as::PresentAs;

/// The identity actually presented on the wire for this engine.
///
/// `present_as` is a KAS-only knob: the v2 engine ignores `clientInfo.name`
/// behaviorally, so presenting a Kiro name there changes nothing but telemetry
/// attribution — pure misrepresentation with zero function. On V2 the
/// configured value is therefore inert and the honest identity is presented,
/// **including under ADR-0008's `KiroCli` default** — the flip is scoped to
/// the engine that actually reads the name. The discard stays detectable as
/// `effective_present_as(..) != configured` for the spawn path's log.
#[must_use]
pub fn effective_present_as(engine: AgentEngine, configured: PresentAs) -> PresentAs {
    match engine {
        AgentEngine::V2 => PresentAs::Cyril,
        AgentEngine::Kas => configured,
    }
}

/// The one-line startup advisory for the resolved identity, or `None` when
/// there is nothing worth saying (v2: the name has no behavioral effect).
///
/// KAS classifies clients by `clientInfo.name` and does not surface the
/// resolved classification in the initialize response
/// (`.cyril-0wyn/findings.md` Q3), so cyril states its own standing in
/// `cyril.log` at `info` — the fail-loud half of ADR-0006, kept by ADR-0008.
/// Both KAS arms advise: under ADR-0008 the default is a Kiro name, and a
/// default that silently changes how the agent is prompted is exactly the
/// thing that must stay stated somewhere the user can read.
#[must_use]
pub fn identity_advisory(engine: AgentEngine, effective: PresentAs) -> Option<&'static str> {
    match (engine, effective) {
        (AgentEngine::V2, _) => None,
        (AgentEngine::Kas, PresentAs::Cyril) => Some(
            "opted out to clientInfo.name=cyril ([agent] present_as): KAS does not recognize \
             the name and falls back to kiro-ide — IDE persona, channel-gated remote tools, \
             IDE hooks briefing in the system prompt — ADR-0008",
        ),
        (AgentEngine::Kas, PresentAs::KiroCli) => Some(
            "presenting as kiro-cli (default, ADR-0008): CLI persona, memoryEnabled \
             remote-tools branch; title stays \"Cyril\", but Kiro telemetry will attribute \
             this session to kiro-cli — set [agent] present_as = \"cyril\" to opt out",
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    // cyril-0wyn claim 7 fence: the knob is inert on v2 — a configured
    // KiroCli resolves back to the honest identity, and the discard is
    // detectable (that inequality is the spawn path's warn condition).
    #[test]
    fn present_as_inert_on_v2() {
        assert_eq!(
            effective_present_as(AgentEngine::V2, PresentAs::KiroCli),
            PresentAs::Cyril
        );
        assert_ne!(
            effective_present_as(AgentEngine::V2, PresentAs::KiroCli),
            PresentAs::KiroCli,
            "the discard must be detectable for the spawn-path warn"
        );
        assert_eq!(
            effective_present_as(AgentEngine::V2, PresentAs::Cyril),
            PresentAs::Cyril
        );
        // KAS passes the configured value through untouched.
        assert_eq!(
            effective_present_as(AgentEngine::Kas, PresentAs::KiroCli),
            PresentAs::KiroCli
        );
        assert_eq!(
            effective_present_as(AgentEngine::Kas, PresentAs::Cyril),
            PresentAs::Cyril
        );
    }

    // cyril-0wyn claim 3 fence: all four (engine × identity) cells. These
    // tests key on AgentEngine VALUES while the suite also runs under
    // `--features kas` — an implementation keyed on cfg(feature) instead of
    // the bound engine (the cyril-dn91 trap) fails the V2 cells in that build.
    #[test]
    fn advisory_matrix() {
        assert_eq!(identity_advisory(AgentEngine::V2, PresentAs::Cyril), None);
        assert_eq!(identity_advisory(AgentEngine::V2, PresentAs::KiroCli), None);

        let fallback = identity_advisory(AgentEngine::Kas, PresentAs::Cyril)
            .expect("KAS+Cyril must advise about the kiro-ide fallback");
        assert!(fallback.contains("kiro-ide"));
        assert!(fallback.contains("ADR-0008"));

        // cyril-df5l: the DEFAULT arm must still advise. A flip that made the
        // default silent would hide the persona change and the telemetry
        // attribution from every user who never touched the knob — so this
        // arm additionally has to name the telemetry consequence and the way
        // out, not merely announce the name.
        let default_arm = identity_advisory(AgentEngine::Kas, PresentAs::KiroCli)
            .expect("KAS+KiroCli must advise about the presented persona");
        assert!(default_arm.contains("kiro-cli"));
        assert!(default_arm.contains("telemetry"));
        assert!(default_arm.contains("present_as"));
        assert!(default_arm.contains("ADR-0008"));

        assert_ne!(
            fallback, default_arm,
            "the two advisories must be distinct texts (swapped-message guard)"
        );
    }
}

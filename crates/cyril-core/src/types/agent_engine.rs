//! Which Kiro engine the bridge binds at agent-subprocess spawn (ADR-0001).

/// The Kiro engine to drive. Bound once at agent-subprocess spawn and immutable
/// for that subprocess's life (ADR-0001) — switching engines means restarting
/// the bridge. This is a **typed selection** (CONTEXT.md: "Engine" is a
/// first-class axis), not sniffed from the agent command string.
///
/// KAS support is compiled behind the `kas` cargo feature (ADR-0002). The
/// default follows that capability: KAS when enabled, V2 otherwise. Explicit
/// selections remain available; selecting KAS without support fails at spawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentEngine {
    /// The Rust engine (`kiro.dev/*` dialect), reached via `kiro-cli acp`.
    /// Default when KAS support is not compiled in.
    #[cfg_attr(not(feature = "kas"), default)]
    V2,
    /// The TypeScript/LangGraph engine (`_kiro/*` dialect), reached via direct
    /// Free spawn or `kiro-cli acp --agent-engine <v3|kas>` Wrapper spawn.
    /// Default when KAS support is compiled in. `v3` — kiro-cli's own name
    /// since 2.8.0 — is accepted as an input alias; serialization emits `"kas"`.
    #[cfg_attr(feature = "kas", default)]
    #[serde(alias = "v3")]
    Kas,
}

/// The error from parsing an [`AgentEngine`] selector. A real `Error` (not a
/// bare `String`) so clap can use [`AgentEngine`]'s `FromStr` directly as a
/// value parser.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown engine {0:?} (expected `v2`, `kas`, or `v3`)")]
pub struct ParseAgentEngineError(pub String);

impl std::str::FromStr for AgentEngine {
    type Err = ParseAgentEngineError;

    /// Parse cyril's own `--agent-engine <v2|kas|v3>` selector
    /// (case-insensitive). `v3` is kiro-cli's flag vocabulary for the same
    /// engine (its wrapper spawn takes `--agent-engine v3` since 2.8.0), so it
    /// is accepted as an alias for [`AgentEngine::Kas`] (cyril-6iek).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v2" => Ok(Self::V2),
            "kas" | "v3" => Ok(Self::Kas),
            other => Err(ParseAgentEngineError(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    // Slice 5 (D7 parse table): FromStr maps cyril's selector values, is
    // case/whitespace-tolerant, and REJECTS the unknown rather than defaulting.
    // cyril-6iek (design-pause decision) reversed D7's v3 rejection: v3 is
    // kiro-cli's own flag vocabulary since 2.8.0, so it now aliases Kas.
    #[test]
    fn from_str_parses_known_and_rejects_unknown() {
        assert_eq!("v2".parse::<AgentEngine>(), Ok(AgentEngine::V2));
        assert_eq!("kas".parse::<AgentEngine>(), Ok(AgentEngine::Kas));
        assert_eq!(" KAS ".parse::<AgentEngine>(), Ok(AgentEngine::Kas));
        assert_eq!(
            "v3".parse::<AgentEngine>(),
            Ok(AgentEngine::Kas),
            "v3 is kiro-cli's name for the KAS engine — accepted as an alias"
        );
        assert_eq!(
            " V3 ".parse::<AgentEngine>(),
            Ok(AgentEngine::Kas),
            "the alias goes through the same trim/lowercase normalization"
        );
        assert!(
            "v3x".parse::<AgentEngine>().is_err(),
            "the alias is an exact token, not a prefix"
        );
        assert!("".parse::<AgentEngine>().is_err());
        assert!("bogus".parse::<AgentEngine>().is_err());
    }

    #[test]
    fn config_roundtrips_lowercase() {
        // TOML `engine = "v2"` / `"kas"` (serde rename_all = lowercase);
        // `"v3"` deserializes as an alias for Kas (cyril-6iek) but
        // serialization always emits the canonical `"kas"`.
        assert_eq!(serde_json::to_string(&AgentEngine::Kas).unwrap(), "\"kas\"");
        assert_eq!(
            serde_json::from_str::<AgentEngine>("\"v2\"").unwrap(),
            AgentEngine::V2
        );
        assert_eq!(
            serde_json::from_str::<AgentEngine>("\"v3\"").unwrap(),
            AgentEngine::Kas
        );
    }
}

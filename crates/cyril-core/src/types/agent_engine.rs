//! Which Kiro engine the bridge binds at agent-subprocess spawn (ADR-0001).

/// The Kiro engine to drive. Bound once at agent-subprocess spawn and immutable
/// for that subprocess's life (ADR-0001) — switching engines means restarting
/// the bridge. This is a **typed selection** (CONTEXT.md: "Engine" is a
/// first-class axis), not sniffed from the agent command string.
///
/// KAS-0 wires only [`AgentEngine::V2`]; [`AgentEngine::Kas`] is selectable but
/// reports "not available yet" until KAS-1 (cyril-evwh) makes the spawn real,
/// behind the `kas` cargo feature (ADR-0002).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentEngine {
    /// The Rust engine (`kiro.dev/*` dialect), reached via `kiro-cli acp` —
    /// cyril's default.
    #[default]
    V2,
    /// The TypeScript/LangGraph engine (`_kiro/*` dialect), reached via
    /// `kiro-cli acp --agent-engine <v3|kas>` (version-dependent flag, resolved
    /// in KAS-1). Not wired in KAS-0.
    Kas,
}

/// The error from parsing an [`AgentEngine`] selector. A real `Error` (not a
/// bare `String`) so clap can use [`AgentEngine`]'s `FromStr` directly as a
/// value parser.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown engine {0:?} (expected `v2` or `kas`)")]
pub struct ParseAgentEngineError(pub String);

impl std::str::FromStr for AgentEngine {
    type Err = ParseAgentEngineError;

    /// Parse cyril's own `--agent-engine <v2|kas>` selector (case-insensitive).
    /// This is cyril's vocabulary; the version-dependent kiro-cli flag (`kas`
    /// vs `v3`) is resolved separately in KAS-1.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "v2" => Ok(Self::V2),
            "kas" => Ok(Self::Kas),
            other => Err(ParseAgentEngineError(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn default_is_v2() {
        // The default build drives v2 — selecting KAS is always explicit (ADR-0002).
        assert_eq!(AgentEngine::default(), AgentEngine::V2);
    }

    // Slice 5 (D7 parse table): FromStr maps cyril's selector values, is
    // case/whitespace-tolerant, and REJECTS the unknown rather than defaulting.
    #[test]
    fn from_str_parses_known_and_rejects_unknown() {
        assert_eq!("v2".parse::<AgentEngine>(), Ok(AgentEngine::V2));
        assert_eq!("kas".parse::<AgentEngine>(), Ok(AgentEngine::Kas));
        assert_eq!(" KAS ".parse::<AgentEngine>(), Ok(AgentEngine::Kas));
        assert!(
            "v3".parse::<AgentEngine>().is_err(),
            "v3 is the kiro-cli flag, not cyril's selector value"
        );
        assert!("".parse::<AgentEngine>().is_err());
        assert!("bogus".parse::<AgentEngine>().is_err());
    }

    #[test]
    fn config_roundtrips_lowercase() {
        // TOML `engine = "v2"` / `"kas"` (serde rename_all = lowercase).
        assert_eq!(serde_json::to_string(&AgentEngine::Kas).unwrap(), "\"kas\"");
        assert_eq!(
            serde_json::from_str::<AgentEngine>("\"v2\"").unwrap(),
            AgentEngine::V2
        );
    }
}

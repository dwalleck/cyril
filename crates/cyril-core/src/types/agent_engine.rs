//! Which Kiro engine the bridge binds at agent-subprocess spawn (ADR-0001).

/// The Kiro engine to drive. Bound once at agent-subprocess spawn and immutable
/// for that subprocess's life (ADR-0001) — switching engines means restarting
/// the bridge. This is a **typed selection** (CONTEXT.md: "Engine" is a
/// first-class axis), not sniffed from the agent command string.
///
/// KAS-0 wires only [`AgentEngine::V2`]; [`AgentEngine::Kas`] is selectable but
/// reports "not available yet" until KAS-1 (cyril-evwh) makes the spawn real,
/// behind the `kas` cargo feature (ADR-0002).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_v2() {
        // The default build drives v2 — selecting KAS is always explicit (ADR-0002).
        assert_eq!(AgentEngine::default(), AgentEngine::V2);
    }
}

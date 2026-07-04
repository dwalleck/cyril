//! Which KAS spawn shape the bridge uses when the engine is KAS (KAS-1, cyril-evwh).

/// How to launch the KAS engine when [`crate::types::AgentEngine::Kas`] is
/// selected: the zero-credential **free path** (a direct `acp-server.js` spawn,
/// where KAS uses its own file-auth) or the **wrapper** (`kiro-cli acp
/// --agent-engine <v3|kas>`, which delegates auth to cyril's
/// `_kiro/auth/getAccessToken` responder).
///
/// Configured via TOML `[agent] kas_spawn = "free" | "wrapper"`; defaults to
/// `free`. Irrelevant when the engine is v2.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KasSpawn {
    /// Direct `node acp-server.js --transport=stdio --auth=acp-callback`
    /// (cyril-dcc6); cyril answers `_kiro/auth/getAccessToken` from kiro-cli's
    /// sqlite credential store, same as wrapper mode.
    #[default]
    Free,
    /// `kiro-cli acp --agent-engine <flag>` (injects `--auth=acp-callback`);
    /// cyril answers `_kiro/auth/getAccessToken` as the credential custodian.
    Wrapper,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn default_is_free() {
        assert_eq!(KasSpawn::default(), KasSpawn::Free);
    }

    #[test]
    fn toml_lowercase_roundtrip() {
        assert_eq!(
            serde_json::from_str::<KasSpawn>("\"wrapper\"").unwrap(),
            KasSpawn::Wrapper
        );
        assert_eq!(serde_json::to_string(&KasSpawn::Free).unwrap(), "\"free\"");
    }
}

//! What identity cyril presents as ACP `clientInfo.name` (cyril-0wyn, ADR-0006).

/// The `clientInfo.name` cyril presents at `initialize`.
///
/// KAS derives persona, remote-tool allowlist, hooks briefing, and repository
/// honoring from this one string, silently falling back to `kiro-ide` for
/// unrecognized names (`.cyril-0wyn/findings.md`). The default is cyril's own
/// honest name; `kiro-cli` is an opt-in impersonation for users who need the
/// `memoryEnabled` remote-tools branch — it changes `name` only, never
/// `title`, and is inert on the v2 engine (ADR-0006).
///
/// Configured via TOML `[agent] present_as = "cyril" | "kiro-cli"`. Other KAS
/// names (`kiro-ide`, `kiro-web`) are deliberately unrepresentable: neither
/// has a defined purpose for cyril, and free strings would be arbitrary
/// impersonation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PresentAs {
    /// Honest identity: `clientInfo.name = "cyril"` (accepts the kiro-ide
    /// fallback on KAS, advertised by a startup advisory).
    #[default]
    #[serde(rename = "cyril")]
    Cyril,
    /// Opt-in impersonation: `clientInfo.name = "kiro-cli"` (CLI persona +
    /// `memoryEnabled` allowlist branch on KAS; Kiro telemetry attributes
    /// sessions to kiro-cli).
    #[serde(rename = "kiro-cli")]
    KiroCli,
}

impl PresentAs {
    /// The exact string placed in `clientInfo.name` on the wire.
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Cyril => "cyril",
            Self::KiroCli => "kiro-cli",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn default_is_cyril() {
        assert_eq!(PresentAs::default(), PresentAs::Cyril);
    }

    #[test]
    fn toml_hyphenated_roundtrip() {
        assert_eq!(
            serde_json::from_str::<PresentAs>("\"kiro-cli\"").unwrap(),
            PresentAs::KiroCli
        );
        assert_eq!(
            serde_json::to_string(&PresentAs::Cyril).unwrap(),
            "\"cyril\""
        );
    }

    #[test]
    fn wire_names_match_serde_names() {
        for v in [PresentAs::Cyril, PresentAs::KiroCli] {
            assert_eq!(
                serde_json::to_string(&v).unwrap(),
                format!("\"{}\"", v.wire_name()),
                "serde and wire_name must agree — one table, two projections"
            );
        }
    }

    #[test]
    fn unrepresentable_names_are_rejected() {
        // kiro-web and kiro-ide are REAL KAS names — the enum must not have
        // quietly grown them; case variants must not parse either.
        for bad in ["kiro-web", "kiro-ide", "KiroCli", "Cyril", ""] {
            assert!(
                serde_json::from_str::<PresentAs>(&format!("\"{bad}\"")).is_err(),
                "{bad:?} must not deserialize"
            );
        }
    }
}

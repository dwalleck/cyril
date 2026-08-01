//! What identity cyril presents as ACP `clientInfo.name` (cyril-0wyn ADR-0006,
//! superseded by cyril-df5l ADR-0008).

/// The `clientInfo.name` cyril presents at `initialize`.
///
/// KAS derives persona, remote-tool allowlist, hooks briefing, and repository
/// honoring from this one string, silently falling back to `kiro-ide` for
/// unrecognized names (`.cyril-0wyn/findings.md`) — so cyril's own honest name
/// does not buy neutrality on KAS, it buys the **IDE** persona. cyril-df5l's
/// 4-arm live A/B measured what actually differs: nothing in the advertised
/// surface, only the system prompt. The default is therefore the persona that
/// describes what cyril *is* — a terminal client — and `cyril` is the opt-out
/// for users who prefer the unrecognized-name fallback (ADR-0008).
///
/// Whichever is configured, the impersonation is never total: `title` stays
/// `"Cyril"`, and the knob is inert on the v2 engine (which ignores
/// `clientInfo.name` behaviorally, so a name there would be pure telemetry
/// misrepresentation with zero function).
///
/// Configured via TOML `[agent] present_as = "kiro-cli" | "cyril"`. The other
/// two KAS names are deliberately unrepresentable: `kiro-ide` is already what
/// the fallback yields, and `kiro-web` additionally provokes two
/// repository/learnings tool calls at session start that a local TUI has no
/// use for (cyril-df5l).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PresentAs {
    /// Opt-out: `clientInfo.name = "cyril"`, accepting KAS's `kiro-ide`
    /// unrecognized-name fallback (IDE persona + IDE hooks briefing),
    /// advertised by a startup advisory.
    #[serde(rename = "cyril")]
    Cyril,
    /// Default: `clientInfo.name = "kiro-cli"` (CLI persona + `memoryEnabled`
    /// allowlist branch on KAS; Kiro telemetry attributes sessions to
    /// kiro-cli, tempered by the unchanged `title`).
    #[default]
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

    // cyril-df5l / ADR-0008: the default is the CLI persona. The 4-arm live
    // A/B measured the ONLY behavioral difference as the system prompt
    // (kiro-ide 0.9% vs kiro-cli 0.8% context on an empty session); the
    // advertised surface is persona-invariant. Fails if ADR-0006's honest
    // default is restored without superseding ADR-0008.
    #[test]
    fn default_is_kiro_cli() {
        assert_eq!(PresentAs::default(), PresentAs::KiroCli);
        assert_eq!(PresentAs::default().wire_name(), "kiro-cli");
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

//! Which hook model cyril enables on the KAS engine (cyril-jiyn, KAS-7).

/// The `_meta.kiro.hooks` advertisement cyril sends to KAS.
///
/// The two hook models do NOT compose per session — KAS's `buildSessionHooks`
/// is winner-take-all (`.cyril-jiyn/findings.md` Q1): with `v2:true` the
/// standalone agent-side loader replaces the host callbacks wholesale. So the
/// knob is a true either/or:
///
/// - [`Host`](Self::Host) (default): cyril owns the hook registry and executes
///   hooks — every trigger round-trips through cyril, and a `preToolUse` hook
///   exiting 2 blocks the tool (the org write/exec-policy gate).
/// - [`Kas`](Self::Kas): KAS's own file-watched loader executes on-disk
///   `.kiro/hooks` agent-side (hook-authoring tool + confirm dialogs);
///   execution leaves cyril's gate entirely.
///
///   **Workspace trust (cyril-gk17).** In this mode a `.kiro/hooks/*.json` in
///   the workspace causes the AGENT to run arbitrary shell on this host, and
///   no `session/request_permission` precedes it — live-verified on 2.16.0.
///   Cyril's decision is deliberate and narrow: it does **not** implement a
///   trust prompt, because it has no trust store to consult and inventing one
///   would give a false assurance (KAS's own `workspaceTrusted` flag, which
///   feeds `disabledReason: "untrusted-workspace"`, is the mechanism that
///   belongs in that role and is not yet wired — cyril-mq15). Instead the
///   mode stays **opt-in** (`Host` is the default), and every hook state KAS
///   reports is surfaced in the transcript via `Notification::HookExecuted`.
///
///   **The compensating control is partial, and the gap is upstream.** The
///   claim this decision originally rested on — "execution is never silent" —
///   is false as stated. In `kas-v2hooks-2.16.0.jsonl` two hooks executed
///   (host-side evidence file: `cyril-audit-hook-fired-start |
///   cyril-audit-hook-fired-pre`) but KAS emitted exactly ONE `hook_update`,
///   for the `preToolUse` hook; the `sessionStart` hook ran and reported
///   nothing. Cyril surfaces every frame it receives, so this is not a cyril
///   drop — but it means opting in accepts hook execution that leaves no
///   record at all. Two further limits: the knob is **global**, so one opt-in
///   trusts every workspace cyril is ever launched in, including one cloned
///   later; and `Notification::HookExecuted` is a record, never a gate —
///   nothing here can refuse a hook. Wiring `workspaceTrusted` (cyril-mq15)
///   is what would make this a control rather than a disclosure.
/// - [`Off`](Self::Off): no hooks advertisement at all.
///
/// Configured via TOML `[agent] kas_hooks = "host" | "kas" | "off"`.
/// Irrelevant on the v2 engine (no `_meta.kiro` capabilities there).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KasHooksMode {
    /// cyril executes hooks: advertise `{enabled: true}`.
    #[default]
    Host,
    /// KAS's standalone loader executes hooks: advertise
    /// `{enabled: true, v2: true}`.
    Kas,
    /// No advertisement; hooks are off everywhere.
    Off,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn default_is_host() {
        assert_eq!(KasHooksMode::default(), KasHooksMode::Host);
    }

    #[test]
    fn toml_lowercase_roundtrip() {
        assert_eq!(
            serde_json::from_str::<KasHooksMode>("\"kas\"").unwrap(),
            KasHooksMode::Kas
        );
        assert_eq!(
            serde_json::to_string(&KasHooksMode::Off).unwrap(),
            "\"off\""
        );
    }

    #[test]
    fn unrepresentable_values_are_rejected() {
        // "both" is the plausible user guess for the composition that does
        // not exist upstream; case variants guard serde laxness.
        for bad in ["both", "Host", "v2", ""] {
            assert!(
                serde_json::from_str::<KasHooksMode>(&format!("\"{bad}\"")).is_err(),
                "{bad:?} must not deserialize"
            );
        }
    }
}

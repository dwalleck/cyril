use std::path::Path;

use super::agent_engine::AgentEngine;
use super::kas_hooks::KasHooksMode;
use super::kas_spawn::KasSpawn;
use super::present_as::PresentAs;

/// Application configuration, loaded from a TOML file.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub ui: UiConfig,
    pub agent: AgentConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Max messages retained in chat history.
    pub max_messages: usize,
    /// Enable mouse capture on startup. Mouse capture intercepts selection and
    /// scroll, so a user who prefers their terminal's own selection sets this
    /// `false`; `Ctrl+M` toggles it at runtime from whichever state this picks.
    pub mouse_capture: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            max_messages: 500,
            mouse_capture: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Agent binary name.
    pub agent_name: String,
    /// Extra args for agent subprocess.
    pub extra_args: Vec<String>,
    /// Which Kiro engine to drive (KAS-0, ADR-0001). TOML `engine = "v2"` or
    /// `"kas"` (`"v3"` is accepted as an alias for `"kas"`, cyril-6iek); the
    /// `--agent-engine` flag overrides this. Defaults to v2.
    pub engine: AgentEngine,
    /// For the KAS engine: which spawn shape (KAS-1, cyril-evwh). TOML `kas_spawn
    /// = "free"` (default, zero-credential direct spawn) or `"wrapper"`
    /// (`kiro-cli acp --agent-engine v3` + the auth responder). Ignored for v2.
    pub kas_spawn: KasSpawn,
    /// What identity cyril presents as `clientInfo.name` (cyril-df5l,
    /// ADR-0008, superseding ADR-0006). TOML `present_as = "kiro-cli"`
    /// (default: the CLI persona) or `"cyril"` (opt out to KAS's
    /// unrecognized-name `kiro-ide` fallback). KAS engine only — inert on v2,
    /// which ignores `clientInfo.name` behaviorally.
    ///
    /// `None` means the user named no persona, and is **not** interchangeable
    /// with `Some(PresentAs::default())` even though the two resolve alike: on
    /// v2 the knob is discarded, and an explicit choice being discarded earns a
    /// `warn!` where cyril's own default being discarded does not. Collapsing
    /// them under `#[serde(default)]` is what made that a false either/or —
    /// every v2 user warned, or no v2 user told. Resolve with
    /// [`AgentConfig::resolved_present_as`].
    #[serde(default)]
    pub present_as: Option<PresentAs>,
    /// Which hook model runs on the KAS engine (cyril-jiyn, KAS-7). TOML
    /// `kas_hooks = "host"` (default: cyril executes hooks and can block
    /// preToolUse), `"kas"` (KAS's standalone loader executes them
    /// agent-side), or `"off"`. The models do not compose.
    pub kas_hooks: KasHooksMode,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_name: "kiro-cli".to_string(),
            extra_args: Vec::new(),
            engine: AgentEngine::default(),
            kas_spawn: KasSpawn::default(),
            present_as: None,
            kas_hooks: KasHooksMode::default(),
        }
    }
}

impl AgentConfig {
    /// The persona to present, applying ADR-0008's default when the user named
    /// none. Use this wherever the *value* is needed; read the field directly
    /// only to ask whether the choice was explicit.
    pub fn resolved_present_as(&self) -> PresentAs {
        self.present_as.unwrap_or_default()
    }
}

impl Config {
    /// Load config from a specific path. Returns defaults if the file is
    /// missing, unreadable, or contains invalid TOML.
    pub fn load_from_path(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "could not read config file, using defaults");
                return Self::default();
            }
        };
        match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "invalid config file, using defaults");
                Self::default()
            }
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_ui_config() {
        let config = UiConfig::default();
        assert_eq!(config.max_messages, 500);
        assert!(config.mouse_capture);
    }

    // cyril-nd4h claim C7 fence, serialization layer: the schema is exactly the
    // fields that production actually consumes. `highlight_cache_size` and
    // `stream_buffer_timeout_ms` were serialized and documented here for months
    // with zero readers, so a *new* key appearing in this list is the signal
    // that the same rot has started again. Pairs with the exhaustive
    // destructure in `App::new`, which catches it at the consumption end.
    #[test]
    fn default_ui_config_schema_is_exactly_two_fields() -> anyhow::Result<()> {
        use anyhow::Context;

        let config: Config = toml::from_str(
            r#"
[ui]
max_messages = 1000
mouse_capture = false
"#,
        )?;
        let encoded = toml::to_string(&config.ui)?;
        let value: toml::Value = toml::from_str(&encoded)?;
        let table = value
            .as_table()
            .context("serialized UI config should be a table")?;
        let mut keys: Vec<_> = table.keys().map(String::as_str).collect();
        keys.sort_unstable();

        assert_eq!(config.ui.max_messages, 1000);
        assert!(!config.ui.mouse_capture);
        assert_eq!(keys, ["max_messages", "mouse_capture"]);
        Ok(())
    }

    #[test]
    fn default_agent_config() {
        let config = AgentConfig::default();
        assert_eq!(config.agent_name, "kiro-cli");
        assert!(config.extra_args.is_empty());
    }

    #[test]
    fn config_default() {
        let config = Config::default();
        assert_eq!(config.ui.max_messages, 500);
        assert_eq!(config.agent.agent_name, "kiro-cli");
    }

    #[test]
    fn config_from_valid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"
[ui]
max_messages = 1000
mouse_capture = false

[agent]
agent_name = "opencode"
"#
        )
        .unwrap();

        let config = Config::load_from_path(&path);
        assert_eq!(config.ui.max_messages, 1000);
        assert!(!config.ui.mouse_capture);
        assert_eq!(config.agent.agent_name, "opencode");
        // Unspecified fields get defaults
        assert_eq!(config.ui.max_messages, 1000);
        assert!(config.agent.extra_args.is_empty());
    }

    #[test]
    fn config_from_missing_file() {
        let path = std::path::PathBuf::from("/tmp/nonexistent_cyril_config.toml");
        let config = Config::load_from_path(&path);
        // Should return defaults, not error
        assert_eq!(config.ui.max_messages, 500);
    }

    #[test]
    fn config_from_invalid_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is not valid toml {{{}}}").unwrap();

        let config = Config::load_from_path(&path);
        // Should return defaults, not error
        assert_eq!(config.ui.max_messages, 500);
    }

    // cyril-nd4h claim C10: honoring mouse_capture must not change the failure
    // posture. A wrong-typed value follows the house rule set by
    // invalid_kas_hooks_falls_back_to_default_config -- the whole file is
    // rejected (warn + defaults), never field-skipped. The bug class: a
    // refactor that "tightens" load_from_path into `?` or `.expect()` and turns
    // a warn-and-continue into a startup crash, which no happy-path config test
    // would notice.
    #[test]
    fn wrong_typed_mouse_capture_falls_back_to_whole_file_defaults() {
        for bad in ["\"yes\"", "1", "[]"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                format!("[ui]\nmax_messages = 1000\nmouse_capture = {bad}\n"),
            )
            .unwrap();

            let config = Config::load_from_path(&path);
            assert!(
                config.ui.mouse_capture,
                "{bad}: an unparseable value must leave the default in place"
            );
            assert_eq!(
                config.ui.max_messages, 500,
                "{bad}: rejection must be whole-file, not field-skipping"
            );
        }
    }

    // cyril-df5l / ADR-0008: a config that never mentions `present_as` gets
    // the CLI persona, and the opt-out is expressible. Both halves matter —
    // a flip that made "cyril" unreachable would be impersonation without
    // consent, which ADR-0008 explicitly does not decide.
    #[test]
    fn present_as_absent_defaults_to_kiro_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\nengine = \"kas\"\n").unwrap();

        let config = Config::load_from_path(&path);
        // Absent stays ABSENT — the default is applied on resolve, not on
        // parse, so the v2 discard can tell "chose kiro-cli" from "said
        // nothing" and pick its log level accordingly.
        assert_eq!(config.agent.present_as, None);
        assert_eq!(config.agent.resolved_present_as(), PresentAs::KiroCli);
    }

    #[test]
    fn present_as_cyril_opt_out_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\npresent_as = \"cyril\"\n").unwrap();

        let config = Config::load_from_path(&path);
        assert_eq!(config.agent.present_as, Some(PresentAs::Cyril));
        assert_eq!(config.agent.resolved_present_as(), PresentAs::Cyril);
    }

    #[test]
    fn present_as_kiro_cli_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\npresent_as = \"kiro-cli\"\n").unwrap();

        let config = Config::load_from_path(&path);
        // An EXPLICIT kiro-cli is Some, not None, even though it matches the
        // default: that is the whole point of the Option.
        assert_eq!(config.agent.present_as, Some(PresentAs::KiroCli));
        assert_eq!(config.agent.resolved_present_as(), PresentAs::KiroCli);
    }

    // cyril-0wyn claim 6 fence: an invalid present_as value follows the
    // house config posture — the whole file is rejected (warn + defaults),
    // so the identity stays honest. "kiro-web" is a REAL KAS client name
    // that must never be expressible; the case variant guards serde
    // laxness; a config carrying other valid keys proves the rejection is
    // whole-file, not field-skipping.
    // cyril-jiyn claim 3 fence: same whole-file posture for kas_hooks.
    // "both" is the plausible guess for the composition KAS doesn't offer.
    #[test]
    fn invalid_kas_hooks_falls_back_to_default_config() {
        for bad in ["both", "Host"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                format!("[ui]\nmax_messages = 1000\n\n[agent]\nkas_hooks = \"{bad}\"\n"),
            )
            .unwrap();

            let config = Config::load_from_path(&path);
            assert_eq!(config.agent.kas_hooks, KasHooksMode::Host, "{bad}");
            assert_eq!(
                config.ui.max_messages, 500,
                "rejection must be whole-file, not field-skipping"
            );
        }
    }

    #[test]
    fn kas_hooks_valid_values_parse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\nkas_hooks = \"off\"\n").unwrap();
        assert_eq!(
            Config::load_from_path(&path).agent.kas_hooks,
            KasHooksMode::Off
        );
        std::fs::write(&path, "[agent]\nkas_hooks = \"kas\"\n").unwrap();
        assert_eq!(
            Config::load_from_path(&path).agent.kas_hooks,
            KasHooksMode::Kas
        );
        std::fs::write(&path, "[agent]\nengine = \"kas\"\n").unwrap();
        assert_eq!(
            Config::load_from_path(&path).agent.kas_hooks,
            KasHooksMode::Host,
            "absent defaults to Host"
        );
    }

    #[test]
    fn invalid_present_as_falls_back_to_default_config() {
        for bad in ["kiro-web", "KiroCli"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.toml");
            std::fs::write(
                &path,
                format!("[ui]\nmax_messages = 1000\n\n[agent]\npresent_as = \"{bad}\"\n"),
            )
            .unwrap();

            let config = Config::load_from_path(&path);
            // The rejected file yields the ADR-0008 default, spelled out
            // rather than written as `PresentAs::default()` — a tautological
            // assertion would still pass if the default moved to `kiro-web`
            // itself, which is the exact value this test exists to keep
            // unreachable.
            assert_eq!(config.agent.present_as, None, "{bad}");
            assert_eq!(
                config.agent.resolved_present_as(),
                PresentAs::KiroCli,
                "{bad}"
            );
            assert_eq!(
                config.ui.max_messages, 500,
                "rejection must be whole-file (house posture), not field-skipping"
            );
        }
    }
}

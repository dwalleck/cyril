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
    /// Max syntax highlight cache entries.
    pub highlight_cache_size: usize,
    /// Streaming buffer flush timeout in ms.
    pub stream_buffer_timeout_ms: u64,
    /// Enable mouse capture on startup.
    pub mouse_capture: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            max_messages: 500,
            highlight_cache_size: 20,
            stream_buffer_timeout_ms: 150,
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
    /// What identity cyril presents as `clientInfo.name` (cyril-0wyn,
    /// ADR-0006). TOML `present_as = "cyril"` (default, honest) or
    /// `"kiro-cli"` (opt-in impersonation, KAS engine only — inert with a
    /// warning on v2).
    pub present_as: PresentAs,
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
            present_as: PresentAs::default(),
            kas_hooks: KasHooksMode::default(),
        }
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
        assert_eq!(config.highlight_cache_size, 20);
        assert_eq!(config.stream_buffer_timeout_ms, 150);
        assert!(config.mouse_capture);
    }

    #[test]
    fn default_ui_config_schema_is_exactly_four_fields() -> anyhow::Result<()> {
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
        assert_eq!(
            keys,
            [
                "highlight_cache_size",
                "max_messages",
                "mouse_capture",
                "stream_buffer_timeout_ms",
            ]
        );
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
        assert_eq!(config.ui.highlight_cache_size, 20);
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

    #[test]
    fn present_as_absent_defaults_to_cyril() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\nengine = \"kas\"\n").unwrap();

        let config = Config::load_from_path(&path);
        assert_eq!(config.agent.present_as, PresentAs::Cyril);
    }

    #[test]
    fn present_as_kiro_cli_parses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[agent]\npresent_as = \"kiro-cli\"\n").unwrap();

        let config = Config::load_from_path(&path);
        assert_eq!(config.agent.present_as, PresentAs::KiroCli);
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
            assert_eq!(config.agent.present_as, PresentAs::Cyril, "{bad}");
            assert_eq!(
                config.ui.max_messages, 500,
                "rejection must be whole-file (house posture), not field-skipping"
            );
        }
    }
}

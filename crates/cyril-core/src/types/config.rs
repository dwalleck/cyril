use std::path::Path;

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
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent_name: "kiro-cli".to_string(),
            extra_args: Vec::new(),
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
}

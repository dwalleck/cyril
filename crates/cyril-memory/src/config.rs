use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use cyril_core::types::config::Config;

const DEFAULT_STARTUP_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 2_000;
const MAX_DIAGNOSTIC_MESSAGE_BYTES: usize = 512;
const MAX_DIAGNOSTIC_FIELD_BYTES: usize = 128;

const MEMORY_FIELDS: [&str; 4] = [
    "enabled",
    "data_root",
    "startup_timeout_ms",
    "request_timeout_ms",
];

/// The strict configuration for the local memory runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    enabled: bool,
    data_root: Option<PathBuf>,
    startup_timeout_ms: u64,
    request_timeout_ms: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_root: None,
            startup_timeout_ms: DEFAULT_STARTUP_TIMEOUT_MS,
            request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
        }
    }
}

impl MemoryConfig {
    /// Whether the memory runtime is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The optional absolute data-root override.
    pub fn data_root(&self) -> Option<&Path> {
        self.data_root.as_deref()
    }

    /// The bounded startup deadline in milliseconds.
    pub fn startup_timeout_ms(&self) -> u64 {
        self.startup_timeout_ms
    }

    /// The bounded request deadline in milliseconds.
    pub fn request_timeout_ms(&self) -> u64 {
        self.request_timeout_ms
    }
}

/// A bounded, field-aware configuration diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    field: Option<String>,
    message: String,
}

impl ConfigDiagnostic {
    /// The TOML field associated with this diagnostic, when one exists.
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    /// The human-readable diagnostic, bounded to 512 UTF-8 bytes.
    pub fn message(&self) -> &str {
        &self.message
    }

    fn new(field: Option<&str>, message: impl std::fmt::Display) -> Self {
        Self {
            field: field.map(|value| bound_text(value, MAX_DIAGNOSTIC_FIELD_BYTES)),
            message: bound_text(&message.to_string(), MAX_DIAGNOSTIC_MESSAGE_BYTES),
        }
    }
}

/// The lossless presence and validation outcome for the memory section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryConfigState {
    /// The configuration file did not contain a `[memory]` section.
    Absent,
    /// The memory section was present and valid, including when disabled.
    Valid(MemoryConfig),
    /// The memory section was present but had one or more invalid fields.
    Invalid(Vec<ConfigDiagnostic>),
    /// The configuration file could not be read after it was found.
    Unreadable(ConfigDiagnostic),
    /// The configuration file was not valid TOML.
    ConfigUnparseable(ConfigDiagnostic),
}

/// Ordinary Cyril configuration plus an independent memory-section outcome.
#[derive(Debug, Clone)]
pub struct ConfigLoadReport {
    ordinary: Config,
    memory: MemoryConfigState,
}

impl ConfigLoadReport {
    /// The ordinary configuration, using `Config`'s established whole-file fallback.
    pub fn ordinary(&self) -> &Config {
        &self.ordinary
    }

    /// The independent, presence-aware memory configuration outcome.
    pub fn memory(&self) -> &MemoryConfigState {
        &self.memory
    }

    fn new(ordinary: Config, memory: MemoryConfigState) -> Self {
        Self { ordinary, memory }
    }
}

/// Loads ordinary and memory configuration without allowing a memory error to
/// disappear into the ordinary configuration's default fallback.
pub fn load_config_report(path: &Path) -> ConfigLoadReport {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return ConfigLoadReport::new(Config::default(), MemoryConfigState::Absent);
        }
        Err(error) => {
            let diagnostic =
                ConfigDiagnostic::new(None, format!("could not read configuration: {error}"));
            return ConfigLoadReport::new(
                Config::default(),
                MemoryConfigState::Unreadable(diagnostic),
            );
        }
    };

    let document = match toml::from_str::<toml::Value>(&content) {
        Ok(document) => document,
        Err(error) => {
            let diagnostic =
                ConfigDiagnostic::new(None, format!("invalid TOML: {}", error.message()));
            return ConfigLoadReport::new(
                Config::default(),
                MemoryConfigState::ConfigUnparseable(diagnostic),
            );
        }
    };

    // This deliberately mirrors Config::load_from_path's semantics while
    // avoiding a second read (and the resulting read/parse race). Unknown
    // top-level keys remain accepted by cyril-core's serde model.
    let ordinary = match toml::from_str::<Config>(&content) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(error = %error, "invalid ordinary config, using defaults");
            Config::default()
        }
    };

    let memory = match document.get("memory") {
        None => MemoryConfigState::Absent,
        Some(value) => parse_memory_value(value),
    };

    ConfigLoadReport::new(ordinary, memory)
}

fn parse_memory_value(value: &toml::Value) -> MemoryConfigState {
    let Some(table) = value.as_table() else {
        return MemoryConfigState::Invalid(vec![ConfigDiagnostic::new(
            Some("memory"),
            "must be a table",
        )]);
    };

    let mut diagnostics = Vec::new();
    for field in table.keys() {
        if !MEMORY_FIELDS.contains(&field.as_str()) {
            diagnostics.push(ConfigDiagnostic::new(
                Some(&format!("memory.{field}")),
                "unknown field",
            ));
        }
    }

    let enabled = match table.get("enabled") {
        None => false,
        Some(value) => match value.as_bool() {
            Some(enabled) => enabled,
            None => {
                diagnostics.push(ConfigDiagnostic::new(
                    Some("memory.enabled"),
                    "must be a boolean",
                ));
                false
            }
        },
    };

    let data_root = match table.get("data_root") {
        None => None,
        Some(value) => match value.as_str() {
            None => {
                diagnostics.push(ConfigDiagnostic::new(
                    Some("memory.data_root"),
                    "must be a non-empty absolute path",
                ));
                None
            }
            Some("") => {
                diagnostics.push(ConfigDiagnostic::new(
                    Some("memory.data_root"),
                    "must be a non-empty absolute path",
                ));
                None
            }
            Some(path) if !Path::new(path).is_absolute() => {
                diagnostics.push(ConfigDiagnostic::new(
                    Some("memory.data_root"),
                    "must be a non-empty absolute path",
                ));
                None
            }
            Some(path) => Some(PathBuf::from(path)),
        },
    };

    let startup_timeout_ms = parse_timeout(
        table,
        "startup_timeout_ms",
        DEFAULT_STARTUP_TIMEOUT_MS,
        &mut diagnostics,
    );
    let request_timeout_ms = parse_timeout(
        table,
        "request_timeout_ms",
        DEFAULT_REQUEST_TIMEOUT_MS,
        &mut diagnostics,
    );

    if diagnostics.is_empty() {
        MemoryConfigState::Valid(MemoryConfig {
            enabled,
            data_root,
            startup_timeout_ms,
            request_timeout_ms,
        })
    } else {
        MemoryConfigState::Invalid(diagnostics)
    }
}

fn parse_timeout(
    table: &toml::map::Map<String, toml::Value>,
    field: &str,
    default: u64,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> u64 {
    let Some(value) = table.get(field) else {
        return default;
    };

    match value.as_integer() {
        Some(value) if value > 0 => value as u64,
        Some(0) => {
            diagnostics.push(ConfigDiagnostic::new(
                Some(&format!("memory.{field}")),
                "must be greater than zero",
            ));
            default
        }
        Some(_) => {
            diagnostics.push(ConfigDiagnostic::new(
                Some(&format!("memory.{field}")),
                "must be a positive integer",
            ));
            default
        }
        None => {
            diagnostics.push(ConfigDiagnostic::new(
                Some(&format!("memory.{field}")),
                "must be a positive integer",
            ));
            default
        }
    }
}

fn bound_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }

    if max_bytes <= 3 {
        let mut end = max_bytes;
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        return value[..end].to_owned();
    }

    let mut end = max_bytes - 3;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = value[..end].to_owned();
    bounded.push_str("...");
    bounded
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::tempdir;

    fn write_config(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, contents).unwrap();
        (directory, path)
    }

    fn absolute_root(directory: &tempfile::TempDir) -> String {
        directory
            .path()
            .join("memory data Ω")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn defaults_and_public_accessors_are_stable() {
        let config = MemoryConfig::default();
        assert!(!config.enabled());
        assert_eq!(config.data_root(), None);
        assert_eq!(config.startup_timeout_ms(), 10_000);
        assert_eq!(config.request_timeout_ms(), 2_000);
    }

    #[test]
    fn valid_memory_uses_defaults_for_absent_fields() {
        let (_directory, path) = write_config("[memory]\n");
        let report = load_config_report(&path);
        let MemoryConfigState::Valid(config) = report.memory() else {
            panic!("empty memory table should be valid");
        };
        assert_eq!(config, &MemoryConfig::default());
    }

    #[test]
    fn valid_memory_parses_all_fields_and_unicode_absolute_root() {
        let (directory, _config_path) = write_config("");
        let root = absolute_root(&directory);
        let body = format!(
            "[memory]\nenabled = true\ndata_root = {root:?}\nstartup_timeout_ms = 123\nrequest_timeout_ms = 456\n"
        );
        fs::write(&_config_path, body).unwrap();

        let report = load_config_report(&_config_path);
        let MemoryConfigState::Valid(config) = report.memory() else {
            panic!("configuration should be valid");
        };
        assert!(config.enabled());
        assert_eq!(config.data_root(), Some(Path::new(root.as_str())));
        assert_eq!(config.startup_timeout_ms(), 123);
        assert_eq!(config.request_timeout_ms(), 456);
    }

    #[test]
    fn missing_file_is_default_and_absent() {
        let directory = tempdir().unwrap();
        let report = load_config_report(&directory.path().join("missing.toml"));
        assert_eq!(report.ordinary().ui.max_messages, 500);
        assert!(matches!(report.memory(), MemoryConfigState::Absent));
    }

    #[test]
    fn unreadable_path_is_default_and_unreadable() {
        let directory = tempdir().unwrap();
        let report = load_config_report(directory.path());
        assert_eq!(report.ordinary().ui.max_messages, 500);
        let MemoryConfigState::Unreadable(diagnostic) = report.memory() else {
            panic!("a directory path is unreadable as a config file");
        };
        assert_eq!(diagnostic.field(), None);
        assert!(!diagnostic.message().is_empty());
    }

    #[test]
    fn malformed_toml_is_default_and_config_unparseable() {
        let (_directory, path) = write_config("[memory\n");
        let report = load_config_report(&path);
        assert_eq!(report.ordinary().ui.max_messages, 500);
        let MemoryConfigState::ConfigUnparseable(diagnostic) = report.memory() else {
            panic!("malformed TOML should be classified separately");
        };
        assert_eq!(diagnostic.field(), None);
        assert!(diagnostic.message().contains("invalid TOML"));
    }

    #[test]
    fn well_formed_without_memory_preserves_ordinary_config() {
        let (_directory, path) = write_config("[ui]\nmax_messages = 321\n");
        let report = load_config_report(&path);
        assert_eq!(report.ordinary().ui.max_messages, 321);
        assert!(matches!(report.memory(), MemoryConfigState::Absent));
    }

    #[test]
    fn unknown_top_level_fields_remain_legacy_compatible() {
        let (_directory, path) = write_config(
            "legacy_top_level = true\n[legacy_section]\nvalue = \"kept\"\n[ui]\nmax_messages = 321\n",
        );
        let report = load_config_report(&path);
        assert_eq!(report.ordinary().ui.max_messages, 321);
        assert!(matches!(report.memory(), MemoryConfigState::Absent));
    }

    #[test]
    fn invalid_ordinary_config_does_not_hide_valid_memory() {
        let (directory, config_path) = write_config("");
        let root = absolute_root(&directory);
        fs::write(
            &config_path,
            format!(
                "[ui]\nmax_messages = \"not a number\"\n[memory]\nenabled = true\ndata_root = {root:?}\n"
            ),
        )
        .unwrap();

        let report = load_config_report(&config_path);
        assert_eq!(report.ordinary().ui.max_messages, 500);
        assert!(matches!(report.memory(), MemoryConfigState::Valid(_)));
    }

    #[test]
    fn disabled_memory_is_still_strictly_validated() {
        let (_directory, path) =
            write_config("[memory]\nenabled = false\nrequest_timeout_ms = 0\nunknown = true\n");
        let report = load_config_report(&path);
        let MemoryConfigState::Invalid(diagnostics) = report.memory() else {
            panic!("disabled memory must not bypass validation");
        };
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].field(), Some("memory.unknown"));
        assert_eq!(diagnostics[1].field(), Some("memory.request_timeout_ms"));
    }

    #[test]
    fn invalid_memory_reports_every_field_with_a_bounded_diagnostic() {
        let (_directory, path) = write_config(
            "[memory]\nenabled = \"yes\"\ndata_root = \"relative\"\nstartup_timeout_ms = 0\nrequest_timeout_ms = -1\nextra = true\n",
        );
        let report = load_config_report(&path);
        let MemoryConfigState::Invalid(diagnostics) = report.memory() else {
            panic!("invalid fields should produce Invalid");
        };
        assert_eq!(diagnostics.len(), 5);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.message().len() <= MAX_DIAGNOSTIC_MESSAGE_BYTES })
        );
        assert_eq!(diagnostics[0].field(), Some("memory.extra"));
        assert_eq!(diagnostics[1].field(), Some("memory.enabled"));
        assert_eq!(diagnostics[2].field(), Some("memory.data_root"));
        assert_eq!(diagnostics[3].field(), Some("memory.startup_timeout_ms"));
        assert_eq!(diagnostics[4].field(), Some("memory.request_timeout_ms"));
    }

    #[test]
    fn wrong_memory_shape_is_invalid_and_field_aware() {
        let (_directory, path) = write_config("memory = true\n");
        let report = load_config_report(&path);
        let MemoryConfigState::Invalid(diagnostics) = report.memory() else {
            panic!("a non-table memory value should be invalid");
        };
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].field(), Some("memory"));
        assert_eq!(diagnostics[0].message(), "must be a table");
    }

    #[test]
    fn all_known_wrong_types_are_rejected() {
        let (_directory, path) = write_config(
            "[memory]\nenabled = 1\ndata_root = []\nstartup_timeout_ms = 1.0\nrequest_timeout_ms = true\n",
        );
        let report = load_config_report(&path);
        let MemoryConfigState::Invalid(diagnostics) = report.memory() else {
            panic!("wrong field types should be invalid");
        };
        assert_eq!(diagnostics.len(), 4);
    }

    #[test]
    fn empty_and_relative_data_roots_are_rejected() {
        let (_directory, empty_path) = write_config("[memory]\ndata_root = \"\"\n");
        let empty_report = load_config_report(&empty_path);
        assert!(matches!(
            empty_report.memory(),
            MemoryConfigState::Invalid(_)
        ));

        let (_directory, relative_path) = write_config("[memory]\ndata_root = \"relative\"\n");
        let relative_report = load_config_report(&relative_path);
        assert!(matches!(
            relative_report.memory(),
            MemoryConfigState::Invalid(_)
        ));
    }

    #[test]
    fn timeout_values_must_be_positive() {
        for field in ["startup_timeout_ms", "request_timeout_ms"] {
            let (_directory, path) = write_config(&format!("[memory]\n{field} = 0\n"));
            let report = load_config_report(&path);
            let MemoryConfigState::Invalid(diagnostics) = report.memory() else {
                panic!("zero timeout must be invalid");
            };
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(
                diagnostics[0].field(),
                Some(format!("memory.{field}").as_str())
            );
        }
    }

    #[test]
    fn diagnostic_messages_are_bounded_without_invalid_utf8() {
        let (_directory, path) = write_config("[memory]\n");
        let long = "é".repeat(1_000);
        let diagnostic = ConfigDiagnostic::new(Some(&long), &long);
        assert!(
            diagnostic
                .field()
                .is_some_and(|field| field.len() <= MAX_DIAGNOSTIC_FIELD_BYTES)
        );
        assert!(diagnostic.message().len() <= MAX_DIAGNOSTIC_MESSAGE_BYTES);
        assert!(
            diagnostic
                .message()
                .is_char_boundary(diagnostic.message().len())
        );
        let _ = path;
    }
}

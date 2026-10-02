//! The repository's `[review]` settings in `<workspace>/.cyril/config.toml`,
//! read fresh on every `/review`. Strict on purpose: an unknown key, a wrong
//! type or a path escaping the workspace refuses the review with a named
//! error instead of falling back to a plausible default.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Where the settings live, relative to the repository root.
pub const CONFIG_FILE: &str = ".cyril/config.toml";
/// How long the check may run when `check_timeout_s` is not set.
pub const DEFAULT_CHECK_TIMEOUT: Duration = Duration::from_secs(1800);

/// The check command run once before the workflow starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckCommand {
    pub command: String,
    pub timeout: Duration,
}

/// What the verifiers are told is authoritative for the change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextSource {
    Inline(String),
    /// Workspace-relative; read fresh at launch.
    File(PathBuf),
}

/// A validated `[review]` table. Every field is optional; an absent check
/// command means no check is run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReviewConfig {
    pub check: Option<CheckCommand>,
    pub context: Option<ContextSource>,
    pub scope: Option<Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {}: {source}", .path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{} is not valid TOML: {message}", .path.display())]
    Corrupt { path: PathBuf, message: String },
    #[error("[review] in {CONFIG_FILE}: {problem}")]
    Invalid { problem: String },
}

fn invalid(problem: impl Into<String>) -> ConfigError {
    ConfigError::Invalid {
        problem: problem.into(),
    }
}

impl ReviewConfig {
    /// Read and validate `<workspace>/.cyril/config.toml`. A missing file or a
    /// file without a `[review]` table is the default configuration.
    pub fn load(workspace: &Path) -> Result<Self, ConfigError> {
        let path = workspace.join(CONFIG_FILE);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(ConfigError::Unreadable { path, source }),
        };
        let document: toml::Table =
            toml::from_str(&text).map_err(|error| ConfigError::Corrupt {
                path: path.clone(),
                message: error.message().to_owned(),
            })?;
        match document.get("review") {
            None => Ok(Self::default()),
            Some(toml::Value::Table(review)) => Self::from_table(review),
            Some(_) => Err(invalid("`review` must be a table")),
        }
    }

    fn from_table(review: &toml::Table) -> Result<Self, ConfigError> {
        let mut command = None;
        let mut timeout = None;
        let mut context = None;
        let mut context_file = None;
        let mut scope = None;
        for (key, value) in review {
            match key.as_str() {
                "check_cmd" => command = Some(text(key, value)?),
                "check_timeout_s" => match value {
                    toml::Value::Integer(seconds) if *seconds > 0 => {
                        timeout = Some(Duration::from_secs(seconds.unsigned_abs()));
                    }
                    toml::Value::Integer(_) => {
                        return Err(invalid(
                            "`check_timeout_s` must be a positive number of seconds",
                        ));
                    }
                    _ => return Err(wrong_type(key, "an integer")),
                },
                "context" => context = Some(text(key, value)?),
                "context_file" => {
                    let file = text(key, value)?;
                    context_file = Some(inside_workspace(key, &file)?);
                }
                "scope" => {
                    let toml::Value::Array(items) = value else {
                        return Err(wrong_type(key, "an array of paths"));
                    };
                    let mut paths = Vec::with_capacity(items.len());
                    for item in items {
                        let toml::Value::String(path) = item else {
                            return Err(wrong_type(key, "an array of paths"));
                        };
                        inside_workspace(key, path)?;
                        paths.push(path.clone());
                    }
                    if paths.is_empty() {
                        return Err(invalid("`scope` must name at least one path"));
                    }
                    scope = Some(paths);
                }
                other => return Err(invalid(format!("unknown key `{other}`"))),
            }
        }
        let check = match (command, timeout) {
            (Some(command), timeout) => Some(CheckCommand {
                command,
                timeout: timeout.unwrap_or(DEFAULT_CHECK_TIMEOUT),
            }),
            (None, Some(_)) => {
                return Err(invalid("`check_timeout_s` is set but `check_cmd` is not"));
            }
            (None, None) => None,
        };
        let context = match (context, context_file) {
            (Some(_), Some(_)) => {
                return Err(invalid("set `context` or `context_file`, not both"));
            }
            (Some(inline), None) => Some(ContextSource::Inline(inline)),
            (None, Some(file)) => Some(ContextSource::File(file)),
            (None, None) => None,
        };
        Ok(Self {
            check,
            context,
            scope,
        })
    }

    /// The verifiers' context, with `context_file` read now. `None` means
    /// the recipe's fallback sentence applies.
    pub fn context_text(&self, workspace: &Path) -> Result<Option<String>, ConfigError> {
        match &self.context {
            None => Ok(None),
            Some(ContextSource::Inline(text)) => Ok(Some(text.clone())),
            Some(ContextSource::File(relative)) => {
                let path = workspace.join(relative);
                let unreadable = |source| ConfigError::Unreadable {
                    path: path.clone(),
                    source,
                };
                // A symlink may still lead out of the workspace.
                let resolved = path.canonicalize().map_err(unreadable)?;
                let root = workspace.canonicalize().map_err(unreadable)?;
                if !resolved.starts_with(&root) {
                    return Err(invalid(format!(
                        "`context_file` {} leads outside the repository",
                        relative.display()
                    )));
                }
                fs::read_to_string(&resolved).map(Some).map_err(unreadable)
            }
        }
    }
}

fn wrong_type(key: &str, expected: &str) -> ConfigError {
    invalid(format!("`{key}` must be {expected}"))
}

fn text(key: &str, value: &toml::Value) -> Result<String, ConfigError> {
    match value {
        toml::Value::String(text) if !text.trim().is_empty() => Ok(text.clone()),
        toml::Value::String(_) => Err(invalid(format!("`{key}` is empty"))),
        _ => Err(wrong_type(key, "a string")),
    }
}

/// A relative path that stays inside the repository, judged lexically.
fn inside_workspace(key: &str, path: &str) -> Result<PathBuf, ConfigError> {
    let escapes = || invalid(format!("`{key}` path {path:?} is outside the repository"));
    let mut depth = 0usize;
    for component in Path::new(path).components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => depth = depth.checked_sub(1).ok_or_else(escapes)?,
            Component::RootDir | Component::Prefix(_) => return Err(escapes()),
        }
    }
    if path.starts_with('-') {
        return Err(invalid(format!("`{key}` path {path:?} starts with '-'")));
    }
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str) -> Result<ReviewConfig, ConfigError> {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir(dir.path().join(".cyril")).expect(".cyril");
        fs::write(dir.path().join(CONFIG_FILE), text).expect("config");
        ReviewConfig::load(dir.path())
    }

    fn problem(text: &str) -> String {
        match load(text) {
            Err(error) => error.to_string(),
            Ok(config) => panic!("{text:?} must be refused, got {config:?}"),
        }
    }

    #[test]
    fn absent_file_or_table_is_the_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            ReviewConfig::load(dir.path()).expect("no file"),
            ReviewConfig::default()
        );
        assert_eq!(
            load("[other]\nx = 1\n").expect("no table"),
            ReviewConfig::default()
        );
    }

    #[test]
    fn every_field_parses() {
        let config = load(
            "[review]\ncheck_cmd = \"cargo check\"\ncheck_timeout_s = 90\n\
             context_file = \"docs/review.md\"\nscope = [\"crates\", \"docs\"]\n",
        )
        .expect("valid");
        assert_eq!(
            config.check,
            Some(CheckCommand {
                command: "cargo check".to_owned(),
                timeout: Duration::from_secs(90),
            })
        );
        assert_eq!(
            config.context,
            Some(ContextSource::File(PathBuf::from("docs/review.md")))
        );
        assert_eq!(
            config.scope,
            Some(vec!["crates".to_owned(), "docs".to_owned()])
        );
        let default_timeout = load("[review]\ncheck_cmd = \"make\"\n").expect("valid");
        assert_eq!(
            default_timeout.check.map(|check| check.timeout),
            Some(DEFAULT_CHECK_TIMEOUT)
        );
    }

    #[test]
    fn invalid_settings_are_named_errors() {
        let prefix = "[review] in .cyril/config.toml: ";
        for (text, expected) in [
            ("[review]\nchek_cmd = \"x\"\n", "unknown key `chek_cmd`"),
            ("[review]\ncheck_cmd = 3\n", "`check_cmd` must be a string"),
            ("[review]\ncheck_cmd = \" \"\n", "`check_cmd` is empty"),
            (
                "[review]\ncheck_cmd = \"x\"\ncheck_timeout_s = 0\n",
                "`check_timeout_s` must be a positive number of seconds",
            ),
            (
                "[review]\ncheck_timeout_s = 5\n",
                "`check_timeout_s` is set but `check_cmd` is not",
            ),
            (
                "[review]\ncontext = \"a\"\ncontext_file = \"b\"\n",
                "set `context` or `context_file`, not both",
            ),
            (
                "[review]\ncontext_file = \"../secrets\"\n",
                "`context_file` path \"../secrets\" is outside the repository",
            ),
            (
                "[review]\nscope = [\"/etc\"]\n",
                "`scope` path \"/etc\" is outside the repository",
            ),
            (
                "[review]\nscope = \"crates\"\n",
                "`scope` must be an array of paths",
            ),
            (
                "[review]\nscope = []\n",
                "`scope` must name at least one path",
            ),
            ("review = 1\n", "`review` must be a table"),
        ] {
            assert_eq!(problem(text), format!("{prefix}{expected}"), "{text:?}");
        }
        assert!(problem("[review\n").contains("is not valid TOML"));
    }

    #[test]
    fn context_file_is_read_fresh_and_contained() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(dir.path().join("docs")).expect("docs");
        let config = ReviewConfig {
            context: Some(ContextSource::File(PathBuf::from("docs/review.md"))),
            ..ReviewConfig::default()
        };
        assert!(matches!(
            config.context_text(dir.path()),
            Err(ConfigError::Unreadable { .. })
        ));
        fs::write(dir.path().join("docs/review.md"), "first").expect("write");
        assert_eq!(
            config.context_text(dir.path()).expect("read").as_deref(),
            Some("first")
        );
        fs::write(dir.path().join("docs/review.md"), "second").expect("rewrite");
        assert_eq!(
            config
                .context_text(dir.path())
                .expect("read again")
                .as_deref(),
            Some("second")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_context_file_symlinked_outside_is_refused() {
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret"), "s").expect("secret");
        let dir = tempfile::tempdir().expect("tempdir");
        std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("ctx"))
            .expect("symlink");
        let config = ReviewConfig {
            context: Some(ContextSource::File(PathBuf::from("ctx"))),
            ..ReviewConfig::default()
        };
        assert_eq!(
            config
                .context_text(dir.path())
                .expect_err("escapes")
                .to_string(),
            "[review] in .cyril/config.toml: `context_file` ctx leads outside the repository"
        );
    }
}

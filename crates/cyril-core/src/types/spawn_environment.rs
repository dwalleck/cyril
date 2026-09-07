//! Per-launch environment selection; never mutates the hosting process.

use std::collections::BTreeMap;
#[cfg(feature = "kas")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;

/// The complete environment policy for an agent and its version probe.
///
/// `Replace` forwards only the supplied entries, including when the map is
/// empty. Native OS strings preserve non-Unicode environment values. Values
/// (and names) are omitted from diagnostics because they may carry secrets.
///
/// Executable lookup follows the platform's `std::process::Command` rules.
/// For predictable lookup with `Replace`, use an absolute executable path or
/// supply `PATH` explicitly. An omitted `PATH` is not restored from the parent
/// by Cyril; platform default search paths may still apply.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum SpawnEnvironment {
    #[default]
    Inherit,
    Replace(BTreeMap<OsString, OsString>),
}

impl fmt::Debug for SpawnEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inherit => formatter.write_str("Inherit"),
            Self::Replace(values) => formatter
                .debug_struct("Replace")
                .field("entries", &values.len())
                .finish_non_exhaustive(),
        }
    }
}

impl SpawnEnvironment {
    pub(crate) fn apply(&self, command: &mut std::process::Command) {
        if let Self::Replace(values) = self {
            command.env_clear().envs(values);
        }
    }

    #[cfg(feature = "kas")]
    pub(crate) fn effective_var(&self, name: &OsStr) -> Option<OsString> {
        match self {
            Self::Inherit => std::env::var_os(name),
            Self::Replace(values) => {
                #[cfg(windows)]
                // Match ASCII configuration names case-insensitively as the
                // Windows process environment does. Last entry wins, matching
                // Command::envs when callers supply differently cased keys.
                let value = values
                    .iter()
                    .rev()
                    .find(|(key, _)| {
                        key.as_encoded_bytes()
                            .eq_ignore_ascii_case(name.as_encoded_bytes())
                    })
                    .map(|(_, value)| value);
                #[cfg(not(windows))]
                let value = values.get(name);
                value.cloned()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_disclose_environment_secrets() {
        let environment = SpawnEnvironment::Replace(BTreeMap::from([(
            OsString::from("secret-key"),
            OsString::from("secret-value"),
        )]));
        let debug = format!("{environment:?}");
        assert!(!debug.contains("secret-key"));
        assert!(!debug.contains("secret-value"));
    }

    #[cfg(feature = "kas")]
    #[test]
    fn absent_selected_variable_does_not_fall_back_to_parent() {
        let selected = SpawnEnvironment::Replace(BTreeMap::new());
        assert_eq!(selected.effective_var(OsStr::new("HOME")), None);
        assert_eq!(selected.effective_var(OsStr::new("KIRO_HOME")), None);
    }
}

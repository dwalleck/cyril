//! `/review`: the crtool prefix, the permission policy of an armed run, its
//! inputs, `run.json` and completion summary.
//!
//! Safe shell spelling for hidden `cyril crtool`.
//! HostShell owns terminal rendering; this module keeps the pinned,
//! double-quoted, forward-slash prefix and rejects unsafe fallback forms.
pub mod authorization;
pub mod config;
pub mod consent;
pub mod inputs;
pub mod launch;
pub mod policy;
pub mod run_record;
pub mod summary;
pub mod target;

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
pub type Result<T, E = PrefixError> = std::result::Result<T, E>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellDialect {
    Posix,
    Fish,
    Pwsh,
    WindowsPowerShell,
}

#[derive(Debug, Error)]
pub enum PrefixError {
    #[error("could not determine the current executable")]
    CurrentExecutable(#[source] io::Error),
    #[error("could not canonicalize executable path {path:?}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("executable path is not absolute: {path:?}")]
    RelativePath { path: PathBuf },
    #[error("executable path is not valid UTF-8: {path:?}")]
    NonUtf8Path { path: PathBuf },
    #[error("executable path contains unsafe shell character {character:?} in {path:?}")]
    UnsafeCharacter { path: PathBuf, character: char },
    #[error("executable path contains a surviving backslash: {path:?}")]
    SurvivingBackslash { path: PathBuf },
    #[error("unsupported Windows verbatim executable path: {path:?}")]
    UnsupportedVerbatimPrefix { path: PathBuf },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrtoolPrefix {
    value: String,
}

impl CrtoolPrefix {
    pub fn current(dialect: ShellDialect) -> Result<Self> {
        let executable = env::current_exe().map_err(PrefixError::CurrentExecutable)?;
        let canonical = executable
            .canonicalize()
            .map_err(|source| PrefixError::Canonicalize {
                path: executable,
                source,
            })?;
        Self::from_executable(&canonical, dialect)
    }

    /// `executable` is expected to be canonical and absolute; this method
    /// validates/spells it but deliberately does not resolve it.
    pub fn from_executable(executable: &Path, dialect: ShellDialect) -> Result<Self> {
        let text = executable
            .to_str()
            .ok_or_else(|| PrefixError::NonUtf8Path {
                path: executable.to_path_buf(),
            })?;
        if !executable.is_absolute() {
            return Err(PrefixError::RelativePath {
                path: executable.to_path_buf(),
            });
        }
        validate_shell_characters(executable, text, dialect)?;
        #[cfg(windows)]
        let (leading, body) = windows_spelling(executable, text)?;
        #[cfg(not(windows))]
        let (leading, body) = ("", text);
        let opening = match dialect {
            ShellDialect::Posix | ShellDialect::Fish => "\"",
            ShellDialect::Pwsh | ShellDialect::WindowsPowerShell => "& \"",
        };
        let mut value = String::with_capacity(opening.len() + leading.len() + body.len() + 8);
        value.push_str(opening);
        value.push_str(leading);
        #[cfg(windows)]
        for (index, part) in body.split('\\').enumerate() {
            if index != 0 {
                value.push('/');
            }
            value.push_str(part);
        }
        #[cfg(not(windows))]
        value.push_str(body);
        value.push_str("\" crtool");
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

fn validate_shell_characters(path: &Path, text: &str, dialect: ShellDialect) -> Result<()> {
    let powershell = matches!(
        dialect,
        ShellDialect::Pwsh | ShellDialect::WindowsPowerShell
    );
    for character in text.chars() {
        // The native PowerShell parser treats these typographic double
        // quotes as delimiters; other Unicode quote punctuation is literal.
        let powershell_quote =
            powershell && matches!(character, '\u{201c}' | '\u{201d}' | '\u{201e}');
        if matches!(character, '"' | '$' | '`') || character.is_control() || powershell_quote {
            return Err(PrefixError::UnsafeCharacter {
                path: path.to_path_buf(),
                character,
            });
        }
        #[cfg(not(windows))]
        if character == '\\' {
            return Err(PrefixError::SurvivingBackslash {
                path: path.to_path_buf(),
            });
        }
    }
    Ok(())
}

#[cfg(windows)]
fn windows_spelling<'a>(path: &Path, text: &'a str) -> Result<(&'static str, &'a str)> {
    use std::path::{Component, Prefix};

    // Canonical native paths expose typed namespaces; do not hand-parse drive
    // letters or turn a device path into an executable command spelling.
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err(PrefixError::RelativePath {
            path: path.to_path_buf(),
        });
    };
    match prefix.kind() {
        Prefix::VerbatimDisk(_) => Ok(("", &text[4..])),
        Prefix::VerbatimUNC(_, _) => Ok(("//", &text[8..])),
        Prefix::Verbatim(_) | Prefix::DeviceNS(_) => Err(PrefixError::UnsupportedVerbatimPrefix {
            path: path.to_path_buf(),
        }),
        Prefix::Disk(_) | Prefix::UNC(_, _) => Ok(("", text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix(path: &Path, dialect: ShellDialect) -> CrtoolPrefix {
        crate::test_support::must_succeed(
            CrtoolPrefix::from_executable(path, dialect),
            "valid prefix fixture failed",
        )
    }
    #[cfg(windows)]
    const DIALECT_PATH: &str = r"C:\Program Files\Cyril\cyril.exe";
    #[cfg(not(windows))]
    const DIALECT_PATH: &str = "/opt/Cyril tools/cyril";
    #[cfg(windows)]
    const DIALECT_TEXT: &str = "C:/Program Files/Cyril/cyril.exe";
    #[cfg(not(windows))]
    const DIALECT_TEXT: &str = "/opt/Cyril tools/cyril";
    #[cfg(windows)]
    const UNICODE_PATH: &str = r"C:\opt\雪\Cyril tools\cyril.exe";
    #[cfg(not(windows))]
    const UNICODE_PATH: &str = "/opt/雪/Cyril tools/cyril";
    #[cfg(windows)]
    const UNICODE_TEXT: &str = "C:/opt/雪/Cyril tools/cyril.exe";
    #[cfg(not(windows))]
    const UNICODE_TEXT: &str = "/opt/雪/Cyril tools/cyril";

    fn hazard_path(character: char) -> String {
        #[cfg(windows)]
        {
            format!(r"C:\tmp\cyril{character}bin")
        }
        #[cfg(not(windows))]
        {
            format!("/tmp/cyril{character}bin")
        }
    }

    fn display_path(path: &str) -> String {
        #[cfg(windows)]
        {
            path.replace('\\', "/")
        }
        #[cfg(not(windows))]
        {
            path.to_owned()
        }
    }

    #[test]
    fn dialects_select_only_the_required_prefix_form() {
        let executable = Path::new(DIALECT_PATH);
        let posix = format!("\"{DIALECT_TEXT}\" crtool");
        let powershell = format!("& \"{DIALECT_TEXT}\" crtool");
        let cases = [
            (ShellDialect::Posix, posix.as_str()),
            (ShellDialect::Fish, posix.as_str()),
            (ShellDialect::Pwsh, powershell.as_str()),
            (ShellDialect::WindowsPowerShell, powershell.as_str()),
        ];
        for (dialect, expected) in cases {
            assert_eq!(prefix(executable, dialect).as_str(), expected);
        }
    }
    #[test]
    fn unicode_and_spaces_are_preserved_in_each_quote_form() {
        let executable = Path::new(UNICODE_PATH);
        assert_eq!(
            prefix(executable, ShellDialect::Posix).as_str(),
            format!("\"{UNICODE_TEXT}\" crtool")
        );
        assert_eq!(
            prefix(executable, ShellDialect::Pwsh).as_str(),
            format!("& \"{UNICODE_TEXT}\" crtool")
        );
    }

    #[test]
    fn apostrophes_and_safe_smart_quotes_are_literal() {
        let safe = [
            '\'', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}', '\u{201f}',
        ];
        for dialect in [
            ShellDialect::Posix,
            ShellDialect::Fish,
            ShellDialect::Pwsh,
            ShellDialect::WindowsPowerShell,
        ] {
            for character in safe {
                let path = hazard_path(character);
                let display = display_path(&path);
                let expected = if matches!(
                    dialect,
                    ShellDialect::Pwsh | ShellDialect::WindowsPowerShell
                ) {
                    format!("& \"{display}\" crtool")
                } else {
                    format!("\"{display}\" crtool")
                };
                assert_eq!(prefix(Path::new(&path), dialect).as_str(), expected);
            }
        }
    }

    #[test]
    fn power_shell_double_smart_quotes_are_refused_only_for_power_shell() {
        for character in ['\u{201c}', '\u{201d}', '\u{201e}'] {
            let path = hazard_path(character);
            for dialect in [ShellDialect::Pwsh, ShellDialect::WindowsPowerShell] {
                assert!(matches!(
                    CrtoolPrefix::from_executable(Path::new(&path), dialect),
                    Err(PrefixError::UnsafeCharacter { character: actual, .. })
                        if actual == character
                ));
            }
            for dialect in [ShellDialect::Posix, ShellDialect::Fish] {
                assert_eq!(
                    prefix(Path::new(&path), dialect).as_str(),
                    format!("\"{}\" crtool", display_path(&path))
                );
            }
        }
    }

    #[test]
    fn relative_paths_are_refused() {
        for path in [Path::new("cyril"), Path::new("../bin/cyril")] {
            assert!(matches!(
                CrtoolPrefix::from_executable(path, ShellDialect::Posix),
                Err(PrefixError::RelativePath { .. })
            ));
        }
    }

    #[test]
    fn every_shell_hazard_is_refused() {
        for character in ['"', '$', '`', '\n', '\u{0085}'] {
            let path = hazard_path(character);
            match CrtoolPrefix::from_executable(Path::new(&path), ShellDialect::Posix) {
                Err(PrefixError::UnsafeCharacter {
                    character: actual, ..
                }) => assert_eq!(actual, character),
                Err(error) => panic!("wrong error for {path:?}: {error}"),
                Ok(value) => panic!("unsafe path unexpectedly produced {}", value.as_str()),
            }
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn surviving_backslashes_are_refused_in_native_posix_paths() {
        assert!(matches!(
            CrtoolPrefix::from_executable(Path::new("/tmp/cyril\\bin"), ShellDialect::Posix),
            Err(PrefixError::SurvivingBackslash { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_refused_without_lossy_conversion() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', b'c', b'y', b'r', b'i', b'l', 0xff,
        ]));
        assert!(matches!(
            CrtoolPrefix::from_executable(&path, ShellDialect::Posix),
            Err(PrefixError::NonUtf8Path { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_drive_paths_are_normalized_to_forward_slashes() {
        let path = Path::new(r"\\?\C:\Program Files\Cyril\cyril.exe");
        assert_eq!(
            prefix(path, ShellDialect::Pwsh).as_str(),
            "& \"C:/Program Files/Cyril/cyril.exe\" crtool"
        );
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_unc_paths_are_normalized_without_the_verbatim_marker() {
        let path = Path::new(r"\\?\UNC\server\share\Cyril\cyril.exe");
        assert_eq!(
            prefix(path, ShellDialect::WindowsPowerShell).as_str(),
            "& \"//server/share/Cyril/cyril.exe\" crtool"
        );
    }
    #[cfg(windows)]
    #[test]
    fn unsupported_verbatim_device_paths_are_refused() {
        let path = Path::new(r"\\?\Volume{1234}\Cyril\cyril.exe");
        assert!(matches!(
            CrtoolPrefix::from_executable(path, ShellDialect::Pwsh),
            Err(PrefixError::UnsupportedVerbatimPrefix { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn generated_posix_prefix_runs_as_a_native_shell_command() {
        use std::process::Command;

        let directory = crate::test_support::must_succeed(
            tempfile::tempdir(),
            "create native shell fixture directory",
        );
        let executable = directory.path().join("prefix fixture");
        // An immutable fixture avoids fork-inherited writable executable
        // descriptors while preserving real argv checks in parallel tests.
        crate::test_support::must_succeed(
            std::os::unix::fs::symlink(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/native-prefix.sh"
                ),
                &executable,
            ),
            "link native shell fixture",
        );
        let command = prefix(&executable, ShellDialect::Posix);
        let output = crate::test_support::must_succeed(
            Command::new("/bin/sh")
                .args(["-c", command.as_str()])
                .output(),
            "start native shell fixture",
        );
        assert!(output.status.success(), "native prefix failed: {output:?}");
        assert_eq!(output.stdout, b"crtool");
        assert!(output.stderr.is_empty());
    }
}

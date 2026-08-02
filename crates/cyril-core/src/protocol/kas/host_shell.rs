use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    Posix,
    Fish,
    Pwsh,
    WindowsPowerShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostPlatform {
    Unix,
    Windows,
}

impl HostPlatform {
    const fn name(self) -> &'static str {
        match self {
            Self::Unix => "Unix",
            Self::Windows => "Windows",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum HostShellError {
    #[error("unsupported host shell `{configured}` on {platform}")]
    Unsupported {
        configured: String,
        platform: &'static str,
    },
    #[error("configured host shell `{configured}` is not runnable on {platform}")]
    Unavailable {
        configured: String,
        platform: &'static str,
    },
    #[error("no runnable supported host shell found on {platform}")]
    NotFound { platform: &'static str },
}

trait HostEnvironment {
    fn var_os(&self, name: &str) -> Option<OsString>;
    fn is_runnable(&self, path: &Path) -> bool;
}

struct SystemHost;

impl HostEnvironment for SystemHost {
    fn var_os(&self, name: &str) -> Option<OsString> {
        std::env::var_os(name)
    }

    fn is_runnable(&self, path: &Path) -> bool {
        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
}

/// One startup-resolved host shell used for KAS reporting and execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostShell {
    kind: ShellKind,
    executable: PathBuf,
}

impl HostShell {
    fn new(kind: ShellKind, executable: PathBuf) -> Self {
        Self { kind, executable }
    }

    pub(crate) fn resolve(configured: Option<&str>) -> Result<Self, HostShellError> {
        let platform = if cfg!(windows) {
            HostPlatform::Windows
        } else {
            HostPlatform::Unix
        };
        resolve_for(platform, configured, &SystemHost)
    }

    #[cfg(test)]
    pub(crate) fn test_posix() -> Self {
        Self::new(ShellKind::Posix, PathBuf::from("/bin/sh"))
    }

    pub(crate) fn wire_name(&self) -> &'static str {
        match self.kind {
            ShellKind::Posix => "posix",
            ShellKind::Fish => "fish",
            ShellKind::Pwsh | ShellKind::WindowsPowerShell => "powershell",
        }
    }
}

const MAX_PATH_ENTRIES: usize = 256;

fn resolve_for(
    platform: HostPlatform,
    configured: Option<&str>,
    host: &impl HostEnvironment,
) -> Result<HostShell, HostShellError> {
    match platform {
        HostPlatform::Unix => resolve_unix(configured.unwrap_or("auto"), host),
        HostPlatform::Windows => resolve_windows(configured.unwrap_or("auto"), host),
    }
}

fn resolve_unix(
    configured: &str,
    host: &impl HostEnvironment,
) -> Result<HostShell, HostShellError> {
    match configured {
        "auto" => {
            if let Some(shell) = host.var_os("SHELL")
                && !shell.is_empty()
                && let Some(kind) = classify_unix(&shell)
                && let Some(executable) = resolve_candidate(&shell, host)
            {
                return Ok(HostShell::new(kind, executable));
            }
            find_on_path(OsStr::new("bash"), host)
                .map(|path| HostShell::new(ShellKind::Posix, path))
                .ok_or(HostShellError::NotFound { platform: "Unix" })
        }
        "bash" | "fish" => find_on_path(OsStr::new(configured), host)
            .map(|path| {
                let kind = if configured == "fish" {
                    ShellKind::Fish
                } else {
                    ShellKind::Posix
                };
                HostShell::new(kind, path)
            })
            .ok_or_else(|| HostShellError::Unavailable {
                configured: configured.to_string(),
                platform: "Unix",
            }),
        other => Err(HostShellError::Unsupported {
            configured: other.to_string(),
            platform: HostPlatform::Unix.name(),
        }),
    }
}

fn resolve_windows(
    configured: &str,
    host: &impl HostEnvironment,
) -> Result<HostShell, HostShellError> {
    let pwsh = || {
        find_on_path(OsStr::new("pwsh.exe"), host).or_else(|| {
            let candidate = PathBuf::from(host.var_os("ProgramFiles")?)
                .join("PowerShell")
                .join("7")
                .join("pwsh.exe");
            host.is_runnable(&candidate).then_some(candidate)
        })
    };
    let windows_powershell = || {
        host.var_os("PSModulePath").filter(|v| !v.is_empty())?;
        let candidate = PathBuf::from(host.var_os("WINDIR")?)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        host.is_runnable(&candidate).then_some(candidate)
    };

    let resolved = match configured {
        "auto" => pwsh()
            .map(|path| (ShellKind::Pwsh, path))
            .or_else(|| windows_powershell().map(|path| (ShellKind::WindowsPowerShell, path))),
        "pwsh" => pwsh().map(|path| (ShellKind::Pwsh, path)),
        "powershell" => windows_powershell().map(|path| (ShellKind::WindowsPowerShell, path)),
        other => {
            return Err(HostShellError::Unsupported {
                configured: other.to_string(),
                platform: HostPlatform::Windows.name(),
            });
        }
    };
    resolved
        .map(|(kind, path)| HostShell::new(kind, path))
        .ok_or_else(|| {
            if configured == "auto" {
                HostShellError::NotFound {
                    platform: "Windows",
                }
            } else {
                HostShellError::Unavailable {
                    configured: configured.to_string(),
                    platform: "Windows",
                }
            }
        })
}

fn classify_unix(value: &OsStr) -> Option<ShellKind> {
    match Path::new(value).file_name()?.to_str()? {
        "fish" => Some(ShellKind::Fish),
        "sh" | "bash" | "dash" | "zsh" | "ksh" => Some(ShellKind::Posix),
        _ => None,
    }
}

fn resolve_candidate(value: &OsStr, host: &impl HostEnvironment) -> Option<PathBuf> {
    let path = Path::new(value);
    if path.components().count() > 1 {
        return host.is_runnable(path).then(|| path.to_path_buf());
    }
    find_on_path(value, host)
}

fn find_on_path(name: &OsStr, host: &impl HostEnvironment) -> Option<PathBuf> {
    std::env::split_paths(&host.var_os("PATH")?)
        .take(MAX_PATH_ENTRIES)
        .map(|directory| directory.join(name))
        .find(|candidate| host.is_runnable(candidate))
}

#[cfg(test)]
mod tests {
    use super::{HostEnvironment, HostPlatform, HostShell, ShellKind, resolve_for};
    use std::cell::Cell;
    use std::collections::{HashMap, HashSet};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    #[derive(Default)]
    struct FakeHost {
        vars: HashMap<String, OsString>,
        runnable: HashSet<PathBuf>,
        probes: Cell<usize>,
    }

    impl FakeHost {
        fn var(mut self, name: &str, value: &str) -> Self {
            self.vars.insert(name.to_string(), value.into());
            self
        }

        fn path(mut self, dirs: &[&str]) -> Self {
            let value = match std::env::join_paths(dirs) {
                Ok(value) => value,
                Err(error) => panic!("invalid fake PATH: {error}"),
            };
            self.vars.insert("PATH".into(), value);
            self
        }

        fn runnable(mut self, path: &str) -> Self {
            self.runnable.insert(path.into());
            self
        }
    }

    impl HostEnvironment for FakeHost {
        fn var_os(&self, name: &str) -> Option<OsString> {
            self.vars.get(name).cloned()
        }

        fn is_runnable(&self, path: &Path) -> bool {
            self.probes.set(self.probes.get() + 1);
            self.runnable.contains(path)
        }
    }

    fn require_shell(result: Result<HostShell, super::HostShellError>) -> HostShell {
        match result {
            Ok(shell) => shell,
            Err(error) => panic!("expected resolved shell, got {error}"),
        }
    }

    fn error_message(result: Result<HostShell, super::HostShellError>) -> String {
        match result {
            Err(error) => error.to_string(),
            Ok(shell) => panic!("expected resolution error, got {shell:?}"),
        }
    }

    fn assert_shell(
        platform: HostPlatform,
        configured: Option<&str>,
        host: &FakeHost,
        kind: ShellKind,
        executable: &str,
    ) {
        let shell = require_shell(resolve_for(platform, configured, host));
        assert_eq!(shell.kind, kind);
        assert_eq!(shell.executable, PathBuf::from(executable));
    }

    #[test]
    fn every_shell_kind_maps_to_the_exact_kas_wire_vocabulary() {
        let cases = [
            (ShellKind::Posix, "posix", "/bin/bash"),
            (ShellKind::Fish, "fish", "/bin/fish"),
            (ShellKind::Pwsh, "powershell", "pwsh"),
            (ShellKind::WindowsPowerShell, "powershell", "powershell.exe"),
        ];

        let actual: Vec<_> = cases
            .into_iter()
            .map(|(kind, _, executable)| {
                HostShell::new(kind, PathBuf::from(executable)).wire_name()
            })
            .collect();
        let expected: Vec<_> = cases.into_iter().map(|(_, token, _)| token).collect();

        assert_eq!(actual, expected);
        assert!(!actual.contains(&"bash"));
        assert!(!actual.contains(&"cmd"));
    }

    #[test]
    fn unix_resolution_matches_the_signed_matrix() {
        for shell_env in [None, Some("")] {
            let mut host = FakeHost::default()
                .path(&["/fallback"])
                .runnable("/fallback/bash");
            if let Some(value) = shell_env {
                host = host.var("SHELL", value);
            }
            assert_shell(
                HostPlatform::Unix,
                None,
                &host,
                ShellKind::Posix,
                "/fallback/bash",
            );
        }

        for (value, kind, path) in [
            ("/bin/zsh", ShellKind::Posix, "/bin/zsh"),
            ("/odd ü/zsh", ShellKind::Posix, "/odd ü/zsh"),
            ("fish", ShellKind::Fish, "/shells/fish"),
        ] {
            let host = FakeHost::default()
                .var("SHELL", value)
                .path(&["/shells"])
                .runnable(path);
            assert_shell(HostPlatform::Unix, None, &host, kind, path);
        }

        for stale_or_unknown in ["/bin/fish", "/bin/nu"] {
            let host = FakeHost::default()
                .var("SHELL", stale_or_unknown)
                .path(&["/fallback"])
                .runnable("/fallback/bash");
            assert_shell(
                HostPlatform::Unix,
                Some("auto"),
                &host,
                ShellKind::Posix,
                "/fallback/bash",
            );
        }

        let bash = FakeHost::default()
            .path(&["/configured"])
            .runnable("/configured/bash");
        assert_shell(
            HostPlatform::Unix,
            Some("bash"),
            &bash,
            ShellKind::Posix,
            "/configured/bash",
        );
        let fish = FakeHost::default()
            .path(&["/configured"])
            .runnable("/configured/fish");
        assert_shell(
            HostPlatform::Unix,
            Some("fish"),
            &fish,
            ShellKind::Fish,
            "/configured/fish",
        );

        let fallback_only = FakeHost::default()
            .path(&["/configured"])
            .runnable("/configured/bash");
        assert_eq!(
            error_message(resolve_for(
                HostPlatform::Unix,
                Some("fish"),
                &fallback_only,
            )),
            "configured host shell `fish` is not runnable on Unix"
        );
        assert_eq!(
            error_message(resolve_for(
                HostPlatform::Unix,
                Some("pwsh"),
                &fallback_only,
            )),
            "unsupported host shell `pwsh` on Unix"
        );
        assert_eq!(
            error_message(resolve_for(HostPlatform::Unix, None, &FakeHost::default(),)),
            "no runnable supported host shell found on Unix"
        );
    }

    #[test]
    fn unix_path_search_is_bounded_to_256_runnable_probes() {
        let dirs: Vec<_> = (0..300).map(|n| format!("/p{n}")).collect();
        let refs: Vec<_> = dirs.iter().map(String::as_str).collect();
        let host = FakeHost::default().path(&refs).runnable("/p299/bash");

        let err = error_message(resolve_for(HostPlatform::Unix, None, &host));
        assert_eq!(err, "no runnable supported host shell found on Unix");
        assert_eq!(host.probes.get(), 256);
    }

    #[test]
    fn windows_resolution_matches_the_signed_matrix_without_comspec() {
        let path_pwsh = FakeHost::default()
            .path(&["/path"])
            .var("ProgramFiles", "/Program Files")
            .runnable("/path/pwsh.exe")
            .runnable("/Program Files/PowerShell/7/pwsh.exe");
        assert_shell(
            HostPlatform::Windows,
            None,
            &path_pwsh,
            ShellKind::Pwsh,
            "/path/pwsh.exe",
        );

        let program_files = FakeHost::default()
            .var("ProgramFiles", "/Program Files")
            .runnable("/Program Files/PowerShell/7/pwsh.exe");
        assert_shell(
            HostPlatform::Windows,
            Some("pwsh"),
            &program_files,
            ShellKind::Pwsh,
            "/Program Files/PowerShell/7/pwsh.exe",
        );

        let windows_ps = FakeHost::default()
            .var("PSModulePath", "signaled")
            .var("WINDIR", "/Windows")
            .runnable("/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
        assert_shell(
            HostPlatform::Windows,
            Some("powershell"),
            &windows_ps,
            ShellKind::WindowsPowerShell,
            "/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
        );

        for host in [
            FakeHost::default()
                .var("PSModulePath", "signaled")
                .var("WINDIR", "/Windows"),
            FakeHost::default()
                .var("WINDIR", "/Windows")
                .runnable("/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"),
            FakeHost::default()
                .path(&["/path"])
                .var("COMSPEC", "/poison/cmd.exe")
                .runnable("/path/cmd.exe"),
        ] {
            assert_eq!(
                error_message(resolve_for(HostPlatform::Windows, None, &host)),
                "no runnable supported host shell found on Windows"
            );
        }

        for configured in ["bash", "fish", "cmd", "future-shell"] {
            assert_eq!(
                error_message(resolve_for(
                    HostPlatform::Windows,
                    Some(configured),
                    &FakeHost::default(),
                )),
                format!("unsupported host shell `{configured}` on Windows")
            );
        }
    }
}

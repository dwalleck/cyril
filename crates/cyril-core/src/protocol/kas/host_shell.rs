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
    fn current_dir(&self) -> Option<PathBuf>;
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
            nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    fn current_dir(&self) -> Option<PathBuf> {
        match std::env::current_dir() {
            Ok(cwd) => Some(cwd),
            Err(error) => {
                tracing::warn!(%error, "cannot resolve relative host-shell PATH entry");
                None
            }
        }
    }
}

/// One startup-resolved host shell used for KAS reporting and execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostShell {
    kind: ShellKind,
    executable: PathBuf,
}

pub(crate) struct ShellCommand<'a> {
    pub(crate) program: &'a Path,
    pub(crate) args: Vec<String>,
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

    #[cfg(test)]
    pub(crate) fn test_posix_at(executable: impl Into<PathBuf>) -> Self {
        Self::new(ShellKind::Posix, executable.into())
    }

    #[cfg(test)]
    pub(crate) fn test_fish_at(executable: impl Into<PathBuf>) -> Self {
        Self::new(ShellKind::Fish, executable.into())
    }

    pub(crate) fn wire_name(&self) -> &'static str {
        match self.kind {
            ShellKind::Posix => "posix",
            ShellKind::Fish => "fish",
            ShellKind::Pwsh | ShellKind::WindowsPowerShell => "powershell",
        }
    }

    pub(crate) fn command(&self, command: &str, args: &[String]) -> ShellCommand<'_> {
        let shell_args = match self.kind {
            ShellKind::Posix | ShellKind::Fish => {
                vec!["-l".into(), "-c".into(), self.render_command(command, args)]
            }
            ShellKind::Pwsh | ShellKind::WindowsPowerShell => {
                let compound = std::iter::once(command)
                    .chain(args.iter().map(String::as_str))
                    .any(is_operator);
                let mut source = String::with_capacity(command.len() + 256);
                source.push_str("$global:LASTEXITCODE = $null; ");
                self.append_command(&mut source, command, args);
                source.push_str("; $cyrilSuccess = $?; ");
                if compound {
                    source.push_str("if ($cyrilSuccess) { exit 0 } else { exit 1 }");
                } else {
                    source.push_str("$cyrilExitCode = $LASTEXITCODE; if ($null -ne $cyrilExitCode) { exit $cyrilExitCode }; if ($cyrilSuccess) { exit 0 } else { exit 1 }");
                }
                vec![
                    "-NoLogo".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    source,
                ]
            }
        };
        ShellCommand {
            program: &self.executable,
            args: shell_args,
        }
    }

    fn render_command(&self, command: &str, args: &[String]) -> String {
        let mut rendered = String::with_capacity(command.len() + args.len() * 3);
        self.append_command(&mut rendered, command, args);
        rendered
    }

    fn append_command(&self, rendered: &mut String, command: &str, args: &[String]) {
        let powershell = matches!(self.kind, ShellKind::Pwsh | ShellKind::WindowsPowerShell);
        let mut command_position = powershell;
        for token in std::iter::once(command).chain(args.iter().map(String::as_str)) {
            if !rendered.is_empty() && !rendered.ends_with(' ') {
                rendered.push(' ');
            }
            if is_operator(token) {
                rendered.push_str(token);
                command_position |= powershell && is_command_separator(token);
            } else {
                if command_position {
                    rendered.push_str("& ");
                    command_position = false;
                }
                if is_variable(self.kind, token) {
                    rendered.push_str(token);
                } else {
                    push_quoted(rendered, self.kind, token);
                }
            }
        }
    }
}

fn is_operator(token: &str) -> bool {
    matches!(
        token,
        "|" | ">" | ">>" | "<" | "&&" | "||" | ";" | "&" | "2>" | "2>>" | "2>&1"
    )
}

fn is_command_separator(token: &str) -> bool {
    matches!(token, "|" | "&&" | "||" | ";" | "&")
}

fn is_variable(kind: ShellKind, token: &str) -> bool {
    let plain = token
        .strip_prefix('$')
        .filter(|value| !value.starts_with('{'));
    let braced = token
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'));
    match kind {
        ShellKind::Posix => plain.or(braced).is_some_and(valid_name),
        ShellKind::Fish => plain.is_some_and(valid_name),
        ShellKind::Pwsh | ShellKind::WindowsPowerShell => token
            .strip_prefix("$env:")
            .or_else(|| braced.and_then(|value| value.strip_prefix("env:")))
            .is_some_and(valid_name),
    }
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn push_quoted(rendered: &mut String, kind: ShellKind, token: &str) {
    rendered.push('\'');
    let replacement = if matches!(kind, ShellKind::Posix | ShellKind::Fish) {
        "'\\''"
    } else {
        "''"
    };
    for (index, part) in token.split('\'').enumerate() {
        if index > 0 {
            rendered.push_str(replacement);
        }
        rendered.push_str(part);
    }
    rendered.push('\'');
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
        .find_map(|candidate| {
            host.is_runnable(&candidate).then(|| {
                if candidate.is_absolute() {
                    Some(candidate)
                } else {
                    host.current_dir().map(|cwd| cwd.join(candidate))
                }
            })?
        })
}

#[cfg(test)]
mod tests {
    use super::{HostEnvironment, HostPlatform, HostShell, ShellKind, SystemHost, resolve_for};
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

        fn current_dir(&self) -> Option<PathBuf> {
            std::env::current_dir().ok()
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

    #[test]
    fn relative_path_entry_is_retained_as_an_absolute_snapshot() {
        let host = FakeHost::default()
            .path(&["relative-shells"])
            .runnable("relative-shells/bash");
        let expected = match std::env::current_dir() {
            Ok(cwd) => cwd.join("relative-shells/bash"),
            Err(error) => panic!("read test current directory: {error}"),
        };

        let shell = require_shell(resolve_for(HostPlatform::Unix, Some("bash"), &host));
        assert_eq!(shell.executable, expected);
    }

    #[cfg(unix)]
    #[test]
    fn system_host_checks_effective_execute_permission() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("create permission fixture: {error}"),
        };
        let candidate = dir.path().join("owner-cannot-execute");
        if let Err(error) = std::fs::write(&candidate, "#!/bin/sh\n") {
            panic!("write permission fixture: {error}");
        }
        if let Err(error) =
            std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o001))
        {
            panic!("set permission fixture: {error}");
        }

        assert!(
            !SystemHost.is_runnable(&candidate),
            "an execute bit for another permission class does not make the file runnable"
        );
    }

    fn shell(kind: ShellKind) -> HostShell {
        HostShell::new(kind, PathBuf::from(format!("/test/{kind:?}")))
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn renderer_preserves_literals_and_only_the_closed_syntax_set() {
        let common = strings(&[
            "two words",
            "line\nbreak",
            "a'b",
            "雪",
            "|",
            ">",
            ">>",
            "<",
            "&&",
            "||",
            ";",
            "&",
            "2>",
            "2>>",
            "2>&1",
        ]);
        assert_eq!(
            shell(ShellKind::Posix).render_command("echo", &common),
            "'echo' 'two words' 'line\nbreak' 'a'\\''b' '雪' | > >> < && || ; & 2> 2>> 2>&1"
        );
        assert_eq!(
            shell(ShellKind::Pwsh).render_command("echo", &common),
            "& 'echo' 'two words' 'line\nbreak' 'a''b' '雪' | > >> < && || ; & 2> 2>> 2>&1"
        );
    }

    #[test]
    fn renderer_expands_only_pure_family_valid_variables() {
        let args = strings(&[
            "$NAME",
            "${NAME}",
            "$env:NAME",
            "${env:NAME}",
            "prefix-$HOME",
            "*.rs",
            "$(whoami)",
        ]);
        assert_eq!(
            shell(ShellKind::Posix).render_command("echo", &args),
            "'echo' $NAME ${NAME} '$env:NAME' '${env:NAME}' 'prefix-$HOME' '*.rs' '$(whoami)'"
        );
        assert_eq!(
            shell(ShellKind::Fish).render_command("echo", &args),
            "'echo' $NAME '${NAME}' '$env:NAME' '${env:NAME}' 'prefix-$HOME' '*.rs' '$(whoami)'"
        );
        assert_eq!(
            shell(ShellKind::WindowsPowerShell).render_command("echo", &args),
            "& 'echo' '$NAME' '${NAME}' $env:NAME ${env:NAME} 'prefix-$HOME' '*.rs' '$(whoami)'"
        );

        let pipeline = strings(&["left", "|", "echo", "$env:NAME"]);
        assert_eq!(
            shell(ShellKind::Pwsh).render_command("echo", &pipeline),
            "& 'echo' 'left' | & 'echo' $env:NAME"
        );
    }

    #[test]
    fn launch_plan_uses_one_profile_aware_shell_process() {
        let args = strings(&["done"]);
        let posix_shell = shell(ShellKind::Posix);
        let posix = posix_shell.command("echo", &args);
        assert_eq!(posix.args, strings(&["-l", "-c", "'echo' 'done'"]));
        let fish_shell = shell(ShellKind::Fish);
        let fish = fish_shell.command("echo", &args);
        assert_eq!(fish.args, strings(&["-l", "-c", "'echo' 'done'"]));
        for kind in [ShellKind::Pwsh, ShellKind::WindowsPowerShell] {
            let host_shell = shell(kind);
            let plan = host_shell.command("echo", &args);
            assert_eq!(&plan.args[..3], ["-NoLogo", "-NonInteractive", "-Command"]);
            assert!(plan.args[3].starts_with("$global:LASTEXITCODE = $null; & 'echo' 'done';"));
            assert!(plan.args[3].contains("exit $cyrilExitCode"));
            assert!(plan.args[3].contains("if ($cyrilSuccess) { exit 0 } else { exit 1 }"));
        }
    }

    #[test]
    fn powershell_operator_sequence_does_not_reuse_a_stale_native_exit_code() {
        let args = strings(&["-c", "exit 42", ";", "Write-Output", "final-success"]);
        let host_shell = shell(ShellKind::Pwsh);
        let plan = host_shell.command("/bin/sh", &args);
        let source = &plan.args[3];

        assert!(
            !source.contains("$cyrilExitCode"),
            "a compound PowerShell statement must use its final `$?`, not a stale prior `$LASTEXITCODE`: {source}"
        );
        assert!(source.contains("if ($cyrilSuccess) { exit 0 } else { exit 1 }"));
    }

    #[test]
    fn renderer_handles_the_64_kib_token_ceiling_under_two_ms() {
        let token = "x".repeat(255);
        let args = vec![token; 256];
        let started = std::time::Instant::now();
        let rendered = shell(ShellKind::Posix).render_command("echo", &args);
        let elapsed = started.elapsed();
        assert_eq!(rendered.matches(' ').count(), 256);
        assert!(
            elapsed < std::time::Duration::from_millis(2),
            "64 KiB render took {elapsed:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn installed_shells_preserve_external_exit_42() {
        let dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("create isolated shell home: {error}"),
        };
        let args = strings(&["-c", "exit 42"]);
        for (kind, executable) in [
            (ShellKind::Posix, "/bin/sh"),
            (ShellKind::Fish, "/usr/bin/fish"),
            (ShellKind::Pwsh, "/usr/bin/pwsh"),
        ] {
            if !Path::new(executable).exists() {
                continue;
            }
            let host_shell = HostShell::new(kind, executable.into());
            let plan = host_shell.command("sh", &args);
            let status = match std::process::Command::new(plan.program)
                .args(&plan.args)
                .env("HOME", dir.path())
                .env("XDG_CONFIG_HOME", dir.path())
                .status()
            {
                Ok(status) => status,
                Err(error) => panic!("run {kind:?} exit fixture: {error}"),
            };
            assert_eq!(
                status.code(),
                Some(42),
                "{kind:?} must preserve external exit 42"
            );
        }
    }
}

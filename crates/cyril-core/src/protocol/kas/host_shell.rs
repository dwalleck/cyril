use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellKind {
    Posix,
    Fish,
    Pwsh,
    WindowsPowerShell,
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

    pub(crate) fn wire_name(&self) -> &'static str {
        match self.kind {
            ShellKind::Posix => "posix",
            ShellKind::Fish => "fish",
            ShellKind::Pwsh | ShellKind::WindowsPowerShell => "powershell",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostShell, ShellKind};
    use std::path::PathBuf;

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
}

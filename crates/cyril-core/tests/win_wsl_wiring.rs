//! Wiring fences for WSL-internal path translation (cyril-8tq6, claim C8).
//!
//! This is a dedicated integration-test binary on purpose: `to_native` /
//! `to_agent` resolve the process WSL distro once (env + cwd → `OnceLock`),
//! so these fences need a process whose distro state no other test has
//! already initialized.
//!
//! The Windows env fence spawns the test binary as a CHILD with
//! `CYRIL_WSL_DISTRO` set via `Command::env` — never `std::env::set_var`,
//! which is `unsafe` in Rust 2024 and forbidden workspace-wide.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use cyril_core::platform::path::{to_agent, to_native};

/// The `cfg!` no-op guarantee: off Windows, translation never rewrites a path,
/// whatever it looks like. Guards against an inverted gate or an OnceLock that
/// resolves a distro on Linux.
#[cfg(not(target_os = "windows"))]
#[test]
fn linux_translation_is_noop() {
    for p in ["/home/u/f", "/mnt/c/x", r"\\wsl$\Ubuntu\home\u"] {
        assert_eq!(to_native(Path::new(p)), PathBuf::from(p));
        assert_eq!(to_agent(Path::new(p)), PathBuf::from(p));
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    /// Drive translation through the real `to_native` chain — distro-independent,
    /// so it needs no env setup. Runs on the Windows CI runner.
    #[test]
    fn drive_wiring_through_real_chain() {
        assert_eq!(to_native(Path::new("/mnt/c/x")), PathBuf::from(r"C:\x"));
        assert_eq!(
            to_agent(Path::new(r"C:\Users\u")),
            PathBuf::from("/mnt/c/Users/u")
        );
    }

    /// The env→OnceLock→translation wiring, proven in a child process whose
    /// environment carries `CYRIL_WSL_DISTRO` from birth.
    #[test]
    fn env_distro_wiring_via_child_process() {
        const MARKER: &str = "CYRIL_8TQ6_WIRING_CHILD";
        if std::env::var(MARKER).is_ok() {
            // Child: the process was born with CYRIL_WSL_DISTRO=Ubuntu.
            assert_eq!(
                to_native(Path::new("/home/u")),
                PathBuf::from(r"\\wsl$\Ubuntu\home\u")
            );
            assert_eq!(
                to_agent(Path::new(r"\\wsl$\Ubuntu\home\u")),
                PathBuf::from("/home/u")
            );
            return;
        }
        // Parent: re-run this exact test in a child with the env set. A drive
        // cwd keeps cwd-derivation out of the picture (env wins regardless).
        let exe = std::env::current_exe().expect("current_exe");
        let out = std::process::Command::new(exe)
            .args([
                "windows::env_distro_wiring_via_child_process",
                "--exact",
                "--nocapture",
            ])
            .env(MARKER, "1")
            .env("CYRIL_WSL_DISTRO", "Ubuntu")
            .current_dir(std::env::temp_dir())
            .output()
            .expect("spawn child test process");
        assert!(
            out.status.success(),
            "child wiring test failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

//! Wiring fences for WSL-boundary path translation: the cyril-8tq6 distro
//! machinery and the cyril-jxmv agent-location gate.
//!
//! This is a dedicated integration-test binary on purpose: `to_native` /
//! `to_agent` consume process-global state — the distro `OnceLock` (env +
//! cwd, resolved once) and the agent-location atomic — so these fences need
//! processes whose state no other test has already initialized. Fences that
//! assert a specific global state run in CHILD processes (the parent
//! re-invokes this binary with a marker env var); the atomic is settable
//! in-process, but plain `cargo test` runs tests on shared threads, so
//! child isolation keeps the fences honest under both runners.
//!
//! Env vars reach children via `Command::env` / `env_remove` — never
//! `std::env::set_var`, which is `unsafe` in Rust 2024 and forbidden
//! workspace-wide.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use cyril_core::platform::path::{
    AgentLocation, agent_location, bind_agent_location, set_agent_location, to_agent, to_native,
};
use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{AgentCommand, Notification};

/// Re-run `test_name` in a child process with `envs` set and the location
/// env vars scrubbed (the fences control their own state). Panics if the
/// child's assertions fail.
fn run_child(test_name: &str, marker: &str, envs: &[(&str, &str)]) {
    let exe = std::env::current_exe().expect("current_exe");
    let mut cmd = std::process::Command::new(exe);
    cmd.args([test_name, "--exact", "--nocapture"])
        .env_remove("CYRIL_AGENT_LOCATION")
        .env_remove("CYRIL_WSL_DISTRO")
        .env(marker, "1")
        // A drive/temp cwd keeps distro cwd-derivation out of the picture.
        .current_dir(std::env::temp_dir());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn child test process");
    assert!(
        out.status.success(),
        "child fence {test_name} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The `cfg!` no-op guarantee: off Windows, translation never rewrites a
/// path — even with a WSL agent location explicitly bound (the adversarial
/// state; catches a gate that dropped the host-OS term, cyril-jxmv C3).
#[cfg(not(target_os = "windows"))]
#[test]
fn linux_translation_is_noop() {
    set_agent_location(AgentLocation::Wsl);
    for p in ["/home/u/f", "/mnt/c/x", r"\\wsl$\Ubuntu\home\u"] {
        assert_eq!(to_native(Path::new(p)), PathBuf::from(p));
        assert_eq!(to_agent(Path::new(p)), PathBuf::from(p));
    }
}

/// The env override reaches `bind_agent_location` (cyril-jxmv C5 wiring):
/// a child born with `CYRIL_AGENT_LOCATION=native` binds Native even for
/// the `wsl` launcher program. Platform-independent — bind and the getter
/// have no cfg gates.
#[test]
fn env_override_wiring_via_child_process() {
    const MARKER: &str = "CYRIL_JXMV_ENV_OVERRIDE_CHILD";
    if std::env::var(MARKER).is_ok() {
        bind_agent_location("wsl");
        assert_eq!(agent_location(), Some(AgentLocation::Native));
        return;
    }
    run_child(
        "env_override_wiring_via_child_process",
        MARKER,
        &[("CYRIL_AGENT_LOCATION", "native")],
    );
}

/// cyril-jxmv C8: `run_bridge` binds the agent location from the RESOLVED
/// spawn command BEFORE the exec attempt — a failed spawn still leaves the
/// wsl-launcher classification bound. The missing path carries the launcher
/// basename, so exec fails on every platform while the classification is
/// still Wsl. Lives in this binary because it asserts the process-global
/// atomic: every in-process writer here binds Wsl too, so no thread
/// interleave under plain `cargo test` can flip the asserted value
/// (bridge.rs's own real-spawn test binds Native, which is why the fence
/// does not live there).
#[tokio::test]
async fn bridge_spawn_binds_agent_location_before_exec() {
    let cmd = AgentCommand::try_from_argv(vec![
        "/cyril-jxmv-no-such-dir/wsl.exe".to_string(),
        "kiro-cli".to_string(),
    ])
    .expect("argv");
    let handle = spawn_bridge(cmd, SpawnConfig::default(), std::env::temp_dir())
        .expect("bridge thread spawns");
    let (_sender, mut rx, _perm) = handle.split();
    let routed = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
        .await
        .expect("notification within 10s of spawn failure")
        .expect("channel open");
    assert!(
        matches!(routed.notification, Notification::BridgeDisconnected { .. }),
        "spawn of a missing binary must disconnect"
    );
    assert_eq!(
        agent_location(),
        Some(AgentLocation::Wsl),
        "location must be bound from the resolved program before exec"
    );
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    /// Drive translation through the real `to_native` chain with the gate
    /// ON — distro-independent, so it needs no env setup. Runs on the
    /// Windows CI runner. (cyril-jxmv C2: the pre-gate assertions hold
    /// verbatim once a WSL location is bound.)
    #[test]
    fn drive_wiring_through_real_chain() {
        set_agent_location(AgentLocation::Wsl);
        assert_eq!(to_native(Path::new("/mnt/c/x")), PathBuf::from(r"C:\x"));
        assert_eq!(
            to_agent(Path::new(r"C:\Users\u")),
            PathBuf::from("/mnt/c/Users/u")
        );
    }

    /// The env→OnceLock→translation distro wiring (cyril-8tq6), proven in a
    /// child whose environment carries `CYRIL_WSL_DISTRO` from birth. The
    /// child binds a WSL location first — the 8tq6 assertions are C2's
    /// gated-on corpus; the assertions themselves are unchanged.
    #[test]
    fn env_distro_wiring_via_child_process() {
        const MARKER: &str = "CYRIL_8TQ6_WIRING_CHILD";
        if std::env::var(MARKER).is_ok() {
            set_agent_location(AgentLocation::Wsl);
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
        run_child(
            "windows::env_distro_wiring_via_child_process",
            MARKER,
            &[("CYRIL_WSL_DISTRO", "Ubuntu")],
        );
    }

    /// Native-agent fence (cyril-jxmv C1/C7): in a child with NOTHING set,
    /// translation is identity — first with the location unset (the
    /// do-no-harm default; catches a default-Wsl regression, i.e. today's
    /// bug surviving), then with Native bound explicitly.
    #[test]
    fn native_agent_no_rewrite_via_child_process() {
        const MARKER: &str = "CYRIL_JXMV_NATIVE_CHILD";
        if std::env::var(MARKER).is_ok() {
            let shapes = [r"C:\Users\u\repos\proj", "/mnt/c/x", "/home/u"];
            for p in shapes {
                assert_eq!(to_native(Path::new(p)), PathBuf::from(p), "unset to_native");
                assert_eq!(to_agent(Path::new(p)), PathBuf::from(p), "unset to_agent");
            }
            set_agent_location(AgentLocation::Native);
            for p in shapes {
                assert_eq!(
                    to_native(Path::new(p)),
                    PathBuf::from(p),
                    "native to_native"
                );
                assert_eq!(to_agent(Path::new(p)), PathBuf::from(p), "native to_agent");
            }
            return;
        }
        run_child(
            "windows::native_agent_no_rewrite_via_child_process",
            MARKER,
            &[],
        );
    }

    /// Gate-on with no distro (cyril-jxmv C2 × cyril-8tq6): drive mounts
    /// translate unconditionally, WSL-internal paths pass through — the
    /// unknown-distro semantics are preserved under an explicit WSL
    /// location.
    #[test]
    fn wsl_location_without_distro_via_child_process() {
        const MARKER: &str = "CYRIL_JXMV_WSL_NO_DISTRO_CHILD";
        if std::env::var(MARKER).is_ok() {
            set_agent_location(AgentLocation::Wsl);
            assert_eq!(to_native(Path::new("/mnt/c/x")), PathBuf::from(r"C:\x"));
            assert_eq!(to_native(Path::new("/home/u")), PathBuf::from("/home/u"));
            return;
        }
        run_child(
            "windows::wsl_location_without_distro_via_child_process",
            MARKER,
            &[],
        );
    }
}

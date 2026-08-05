//! cyril-jxmv probe — what does Windows-cyril's path translation do to the
//! paths a NATIVE Windows agent (kiro-cli.exe) exchanges? Runs the REAL
//! cyril-core code (linked against libcyril_core.rlib).
//!
//! `to_agent` / `to_native` are one-line `cfg!(target_os = "windows")`
//! dispatches (platform/path.rs:25-42): Windows arm = `win_to_wsl` /
//! `wsl_to_win`, other-OS arm = identity. Both arm bodies are OS-independent
//! pure functions and public API, so this Linux process can execute the exact
//! Windows behavior; the GATE section then shows the real entry points are
//! identity on this host — i.e. the dispatch keys on host OS alone. Nothing
//! about the agent command reaches any of these functions (their entire input
//! surface is the path argument + CYRIL_WSL_DISTRO/process-cwd, see probe.sh).
//!
//! Build/run: see probe.sh next to this file.

use std::path::Path;

use cyril_core::platform::path::{to_agent, to_native, win_to_wsl, wsl_to_win};

fn main() {
    // Q1 — OUTBOUND. bridge.rs:1048: `to_agent(&session_cwd)` is the
    // session/new cwd wire value. Windows arm = win_to_wsl. A native
    // kiro-cli.exe resolves cwd on the Windows filesystem: correct wire
    // value is the input, byte-identical.
    for p in [
        r"C:\Users\u\repos\proj",
        r"C:\",
        r"D:\data",
        r"\\?\C:\Users\u\repos\proj",
    ] {
        println!("OUT|{p}|{}", win_to_wsl(Path::new(p)).display());
    }

    // Q2 — INBOUND. KAS host-io `to_native_checked` -> `to_native`; Windows
    // arm = wsl_to_win. Shapes a native Windows agent plausibly sends.
    for p in [
        r"C:\Users\u\repos\proj\file.rs",
        "C:/Users/u/x.txt",
        r"D:\data\f",
    ] {
        println!("IN|{p}|{}", wsl_to_win(p).display());
    }

    // Q3 — GATE. Same inputs through the real entry points on THIS (Linux)
    // host: identity. Only cfg!(target_os) separates Q3 from Q1/Q2 — no
    // agent-location input exists.
    for p in [r"C:\Users\u\repos\proj", "/mnt/c/x"] {
        println!(
            "GATE|{p}|to_agent={}|to_native={}",
            to_agent(Path::new(p)).display(),
            to_native(Path::new(p)).display()
        );
    }
}

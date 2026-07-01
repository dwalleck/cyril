//! KAS-5b probe2 — edge-case falsifiers run at design time (cheap, offline).
//! Kills three claims empirically against real tokio::process:
//!   C4  create with a nonexistent command -> Err, not panic.
//!   C7s a signal-killed command -> exit_code None, signal Some (not exitCode 0).
//!   C8  output is COMBINED stdout+stderr, not stdout-only.
//! Single-threaded runtime, mirroring the bridge. Ugly on purpose.

use std::os::unix::process::ExitStatusExt;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // C4: spawning a command that does not exist must surface an Err (which cyril
    // maps to -32603), NOT panic. tokio's spawn returns Err here.
    let bad = tokio::process::Command::new("definitely-not-a-real-binary-xyz").spawn();
    println!("C4   spawn(nonexistent) is_err = {}", bad.is_err());

    // C7s: a process killed by a signal reports code()=None and signal()=Some.
    // Spawn a long sleeper, kill it, await exit.
    let mut child = tokio::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep should spawn");
    child.start_kill().expect("start_kill");
    let status = child.wait().await.expect("wait");
    println!(
        "C7s  killed -> code={:?} signal={:?}",
        status.code(),
        status.signal()
    );

    // C8: combined stdout+stderr. Command writes OUT to stdout and ERR to stderr.
    let out = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("echo OUT; echo ERR 1>&2")
        .output()
        .await
        .expect("output");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");
    println!(
        "C8   stdout={:?} stderr={:?} combined_has_both={}",
        stdout.trim(),
        stderr.trim(),
        combined.contains("OUT") && combined.contains("ERR")
    );
}

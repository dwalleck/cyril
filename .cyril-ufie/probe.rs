//! KAS-5b (cyril-ufie) prove-it probe — terminal host-callback responders.
//!
//! Smallest question: when KAS asks cyril to run a command via the terminal
//! lifecycle (create -> wait_for_exit -> output), what EXACT JSON does cyril put
//! on the wire if it builds replies from the typed acp 0.10.2 terminal types and
//! executes via tokio::process — and does that match (a) the real exit code and
//! (b) the reply shapes KAS provably accepts (the KAS-5a probe, clean turn)?
//!
//! Sharpened with a NON-ZERO exit code (42): an exit-0 echo can't tell a correct
//! exit-status reply from a dropped one. Standalone; uses no cyril abstractions.
//! Ugly on purpose (.unwrap allowed — this is not cyril code).

use agent_client_protocol as acp;
use std::collections::HashMap;

// Mirror the bridge runtime: a SINGLE-THREADED current_thread runtime. If
// wait_for_exit blocked (std::process) instead of awaiting (tokio::process), a
// long command would starve this thread — the regression KAS-5b must avoid.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    // (1) REQUEST ROUND-TRIP — genuine captured wire (.cyril-7bdu/host_callbacks_2.10.0.json)
    // must deserialize into the typed acp requests. Fails on any camelCase/field drift.
    let create_wire =
        r#"{"sessionId":"sess_x","command":"echo","args":["done-42"],"cwd":"/tmp/work"}"#;
    let create_req: acp::CreateTerminalRequest = serde_json::from_str(create_wire).unwrap();
    let wait_wire = r#"{"sessionId":"sess_x","terminalId":"term-1"}"#;
    let _wait_req: acp::WaitForTerminalExitRequest = serde_json::from_str(wait_wire).unwrap();
    println!(
        "REQ   create  -> command={:?} args={:?} cwd={:?}",
        create_req.command, create_req.args, create_req.cwd
    );

    // (2) EXECUTION — run a command that EXITS 42 via tokio::process (cyril's mechanism),
    // in an explicit cwd, capturing stdout + exit code.
    let cwd = std::env::temp_dir();
    let out = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("printf out-42; exit 42")
        .current_dir(&cwd)
        .output()
        .await
        .unwrap();
    let exit_code: Option<i32> = out.status.code();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    println!("EXEC  stdout={stdout:?} exit_code={exit_code:?}");

    // (3) STATEFUL REGISTRY — terminal is stateful (unlike fs). terminalIds unique;
    // an unknown id must be a lookup miss (-> error), never a panic.
    let mut terms: HashMap<String, (String, Option<i32>)> = HashMap::new();
    terms.insert("term-1".to_string(), (stdout.clone(), exit_code));
    println!(
        "REG   known(term-1)={} unknown(term-99)_is_miss={}",
        terms.contains_key("term-1"),
        terms.get("term-99").is_none()
    );

    // (4) WIRE — build the typed acp replies and serialize. THIS is what cyril emits
    // if its acp::Client overrides return the typed responses.
    let exit = acp::TerminalExitStatus::new().exit_code(exit_code.map(|c| c as u32));
    let create_resp = acp::CreateTerminalResponse::new("term-1");
    let output_resp = acp::TerminalOutputResponse::new(stdout, false).exit_status(exit.clone());
    let wait_resp = acp::WaitForTerminalExitResponse::new(exit);
    let release_resp = acp::ReleaseTerminalResponse::new();
    println!(
        "WIRE  create  = {}",
        serde_json::to_string(&create_resp).unwrap()
    );
    println!(
        "WIRE  output  = {}",
        serde_json::to_string(&output_resp).unwrap()
    );
    println!(
        "WIRE  wait    = {}",
        serde_json::to_string(&wait_resp).unwrap()
    );
    println!(
        "WIRE  release = {}",
        serde_json::to_string(&release_resp).unwrap()
    );
}

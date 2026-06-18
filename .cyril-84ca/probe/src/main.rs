// PROBE (prove-it-prototype, cyril-84ca). Throwaway, ugly-but-honest.
//
// Proves the FIX mechanism, not the bug (.k1b-steering already proved the bug):
// drive conn.prompt() OFF the command loop via spawn_local, then issue
// ext_method("session/steer") on the SAME Rc-shared connection ~1.5s later, and
// check the steer reaches kiro BEFORE the prompt's turn-end response.
//
// System under test: agent-client-protocol 0.10.2 ClientSideConnection (request
// multiplexing). Counterparty: REAL kiro-cli 2.8.0 behind the wire-tee oracle.
//
// Run (from repo root):  cd .cyril-84ca/probe && cargo run --bin probe-84ca
//   (the crate also has a `cancel` bin, so `cargo run` alone is ambiguous)
// Oracle: ../../.k1b-steering/wire_shim.py logs every frame to /tmp/k1b_wire.log,
// independent of this probe. Compare C2A `_session/steer` time vs A2C
// `session/prompt` result time.
use std::process::Stdio;
use std::rc::Rc;
use std::time::{Duration, Instant};

use agent_client_protocol::{self as acp, Agent};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

// Minimal Client: only request_permission + session_notification lack defaults.
struct Probe;

#[async_trait::async_trait(?Send)]
impl acp::Client for Probe {
    async fn request_permission(
        &self,
        _args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        // --trust-all-tools means this should never fire; cancel if it somehow does.
        Ok(acp::RequestPermissionResponse::new(
            acp::RequestPermissionOutcome::Cancelled,
        ))
    }
    async fn session_notification(&self, _args: acp::SessionNotification) -> acp::Result<()> {
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, run())
}

async fn run() -> anyhow::Result<()> {
    // wire_shim.py spawns `kiro-cli acp --trust-all-tools` and tees frames.
    let shim = "../../.k1b-steering/wire_shim.py";
    let mut child = tokio::process::Command::new("python3")
        .arg(shim)
        .arg("acp")
        .arg("--trust-all-tools")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");

    let (conn, io) = acp::ClientSideConnection::new(
        Probe,
        stdin.compat_write(),
        stdout.compat(),
        |fut| {
            tokio::task::spawn_local(fut);
        },
    );
    tokio::task::spawn_local(io);
    let conn = Rc::new(conn);

    // Handshake + session.
    conn.initialize(
        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .client_info(acp::Implementation::new("probe-84ca", "0.0.0"))
            .client_capabilities(acp::ClientCapabilities::new()),
    )
    .await?;
    let cwd = std::env::current_dir()?;
    let sess = conn.new_session(acp::NewSessionRequest::new(cwd)).await?;
    let sid = sess.session_id;
    eprintln!("[probe] session = {sid}");

    let t0 = Instant::now();
    let ms = move || t0.elapsed().as_millis();

    // 1) Prompt driven OFF the loop (this is the proposed fix's shape).
    let prompt_text = "You have the execute_bash tool. Run these three commands one at a \
        time, waiting for each to finish before the next: (1) sleep 2 && echo ONE \
        (2) sleep 2 && echo TWO (3) sleep 2 && echo THREE. Then reply DONE.";
    let preq = acp::PromptRequest::new(
        sid.clone(),
        vec![acp::ContentBlock::Text(acp::TextContent::new(prompt_text))],
    );
    let cprompt = conn.clone();
    let prompt_task = tokio::task::spawn_local(async move { cprompt.prompt(preq).await });
    println!("[+{:>5}ms] prompt dispatched off-loop (spawn_local)", ms());

    // 2) Mid-turn steer on the SAME shared connection, while prompt is pending.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    println!("[+{:>5}ms] >>> sending steer on shared conn", ms());
    let params = serde_json::json!({
        "sessionId": sid.to_string(),
        "message": "STEERING: stop running commands now and reply only with HALTED."
    });
    let raw = serde_json::value::RawValue::from_string(serde_json::to_string(&params)?)?;
    let steer = conn
        .ext_method(acp::ExtRequest::new("session/steer", raw.into()))
        .await;
    let steer_ms = ms();
    println!(
        "[+{steer_ms:>5}ms] <<< steer RESPONSE: {}",
        match &steer {
            Ok(_) => "Ok".to_string(),
            Err(e) => format!("Err({e})"),
        }
    );

    // 3) Now wait for the prompt to finish (turn end).
    let presult = prompt_task.await?;
    let turn_ms = ms();
    println!(
        "[+{turn_ms:>5}ms] prompt RESPONSE (turn end): {}",
        match &presult {
            Ok(r) => format!("{:?}", r.stop_reason),
            Err(e) => format!("Err({e})"),
        }
    );

    println!("\n================ PROBE VERDICT ================");
    if steer.is_ok() && steer_ms < turn_ms {
        println!(
            "steer round-tripped at +{steer_ms}ms, BEFORE turn end at +{turn_ms}ms\n\
             => connection multiplexes; the off-loop fix sends steer MID-TURN."
        );
    } else if steer.is_ok() {
        println!(
            "steer ok but at +{steer_ms}ms, NOT before turn end +{turn_ms}ms\n\
             => investigate: connection may serialize, or kiro deferred the response."
        );
    } else {
        println!("steer errored: {steer:?}");
    }
    println!("Cross-check /tmp/k1b_wire.log (oracle) for C2A _session/steer vs A2C prompt result.");
    println!("==============================================");

    child.start_kill().ok();
    Ok(())
}

// PROBE variant (cyril-84ca, claim C3). Same off-loop prompt shape, but issues
// session/cancel mid-turn instead of a steer, and checks the in-flight prompt
// RESOLVES (no hang) and PROMPTLY (well before the ~6s natural turn) — proving
// cancel aborts a busy turn. Cyril's cancel-while-busy has never actually run
// mid-turn (the command-loop bug blocked it), so this is unproven until now.
//
// Run: cd .cyril-84ca/probe && cargo run --bin cancel
// Oracle: ../../.k1b-steering/wire_shim.py -> /tmp/k1b_wire.log (C2A session/cancel
// time vs A2C prompt result time).
use std::process::Stdio;
use std::rc::Rc;
use std::time::{Duration, Instant};

use agent_client_protocol::{self as acp, Agent};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

struct Probe;

#[async_trait::async_trait(?Send)]
impl acp::Client for Probe {
    async fn request_permission(
        &self,
        _args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
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
    let mut child = tokio::process::Command::new("python3")
        .arg("../../.k1b-steering/wire_shim.py")
        .arg("acp")
        .arg("--trust-all-tools")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");

    let (conn, io) = acp::ClientSideConnection::new(Probe, stdin.compat_write(), stdout.compat(), |fut| {
        tokio::task::spawn_local(fut);
    });
    tokio::task::spawn_local(io);
    let conn = Rc::new(conn);

    conn.initialize(
        acp::InitializeRequest::new(acp::ProtocolVersion::V1)
            .client_info(acp::Implementation::new("probe-84ca-cancel", "0.0.0"))
            .client_capabilities(acp::ClientCapabilities::new()),
    )
    .await?;
    let sess = conn.new_session(acp::NewSessionRequest::new(std::env::current_dir()?)).await?;
    let sid = sess.session_id;

    let t0 = Instant::now();
    let ms = move || t0.elapsed().as_millis();

    let prompt_text = "You have the execute_bash tool. Run these three commands one at a \
        time, waiting for each to finish before the next: (1) sleep 2 && echo ONE \
        (2) sleep 2 && echo TWO (3) sleep 2 && echo THREE. Then reply DONE.";
    let preq = acp::PromptRequest::new(
        sid.clone(),
        vec![acp::ContentBlock::Text(acp::TextContent::new(prompt_text))],
    );
    let cprompt = conn.clone();
    let prompt_task = tokio::task::spawn_local(async move { cprompt.prompt(preq).await });
    println!("[+{:>5}ms] prompt dispatched off-loop", ms());

    tokio::time::sleep(Duration::from_millis(1500)).await;
    println!("[+{:>5}ms] >>> sending session/cancel on shared conn", ms());
    conn.cancel(acp::CancelNotification::new(sid.clone())).await?;

    // C3: prompt must RESOLVE, and promptly. Bound it well under the ~6s natural turn.
    let resolved = tokio::time::timeout(Duration::from_secs(4), prompt_task).await;
    let turn_ms = ms();

    println!("\n================ PROBE VERDICT (C3 cancel) ================");
    match resolved {
        Ok(Ok(Ok(r))) => {
            let prompt_aborted = turn_ms < 5000; // natural turn ~6s; cancel at 1.5s
            println!(
                "prompt RESOLVED at +{turn_ms}ms, stop_reason={:?}",
                r.stop_reason
            );
            if prompt_aborted {
                println!("=> cancel ABORTED the turn promptly (resolved {}ms after cancel). C3 holds.", turn_ms.saturating_sub(1500));
            } else {
                println!("=> resolved, but not faster than a natural turn — cancel may be a no-op. Investigate.");
            }
        }
        Ok(Ok(Err(e))) => println!("prompt resolved with ERR at +{turn_ms}ms: {e}"),
        Ok(Err(join)) => println!("prompt task JOIN error: {join}"),
        Err(_) => println!(
            "TIMEOUT: prompt did NOT resolve within 4s of cancel => HANG RISK. C3 FALSIFIED."
        ),
    }
    println!("Cross-check /tmp/k1b_wire.log: C2A session/cancel vs A2C prompt result.");
    println!("==========================================================");

    child.start_kill().ok();
    Ok(())
}

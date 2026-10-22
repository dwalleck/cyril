//! A real owned agent + descendant: helper-only aborts do not prove cleanup.

use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::protocol::bridge::create_channel_pair;
use crate::protocol::domain_mediator::{DomainConfig, DomainMediator};
use crate::protocol::engine::V2Engine;
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::protocol::transport::AgentProcess;
use crate::types::{AgentCommand, BridgeCommand, Notification, SpawnEnvironment};

fn alive(pid: u32) -> bool {
    let output = crate::test_support::must_succeed(
        std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output(),
        "portable Unix liveness probe",
    );
    let status = String::from_utf8_lossy(&output.stdout);
    output.status.success() && !status.trim().is_empty() && !status.trim().starts_with('Z')
}

#[tokio::test]
async fn caar_startup_timeout_kills_owned_agent_and_live_descendant() {
    tokio::task::LocalSet::new().run_until(async {
        let directory = crate::test_support::must_succeed(tempfile::tempdir(), "startup process workspace");
        let pids = directory.path().join("pids");
        // The child waits even after stdin EOF; only the owned process-group
        // lifecycle can kill its still-live descendant after the shutdown grace.
        let script = r#"
sleep 300 &
descendant=$!
printf '%s %s\n' "$$" "$descendant" > "$1"
while IFS= read -r line; do
  id=${line##*\"id\":\"}
  id=${id%%\"*}
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true}},"id":"' "$id" '"}'
      ;;
    *'"method":"session/new"'*)
      printf '%s\n' '{"jsonrpc":"2.0","method":"_kiro.dev/mcp/server_initialized","params":{"sessionId":"unanswered-candidate","serverName":"process-ready"}}'
      ;;
  esac
done
wait
"#;
        let command = AgentCommand::new("sh").with_args(vec![
            "-c".into(), script.into(), "caar-owned-startup".into(), pids.to_string_lossy().into_owned(),
        ]);
        let process = crate::test_support::must_succeed(AgentProcess::spawn(&command, directory.path(), &SpawnEnvironment::Inherit)
            .await, "spawn owned agent");
        let (handle, bridge) = create_channel_pair();
        let (sender, mut notifications, _permissions, _sources, _completion) = handle.split();
        let (mediator, channels) = crate::test_support::must_succeed(DomainMediator::new(DomainConfig {
            engine: Rc::new(V2Engine), cwd: directory.path().to_owned(), present_as: None,
            stall_threshold: Duration::from_secs(30),
            #[cfg(feature = "kas")] host_shell: None,
            #[cfg(feature = "kas")] kas_token: None,
        }, bridge), "owned startup mediator");
        let runtime = crate::test_support::must_succeed(SdkRuntime::start(process, channels, StageChain::default()).await, "owned startup SDK");
        let join = tokio::task::spawn_local(mediator.run(runtime));
        let requested_at = tokio::time::Instant::now();
        crate::test_support::must_succeed(sender.send(BridgeCommand::NewSession {cwd:directory.path().to_owned()}).await, "owned new request");
        let early = crate::test_support::must_succeed(tokio::time::timeout(Duration::from_secs(5), notifications.recv()).await, "real child startup receipt").unwrap_or_else(|| panic!("early ready event"));
        assert!(matches!(early.notification, Notification::McpServerInitialized {server_name,..} if server_name=="process-ready"));
        let ids = crate::test_support::must_succeed(std::fs::read_to_string(pids), "owned PID receipt");
        let ids: Vec<u32> = ids.split_whitespace().map(|pid| crate::test_support::must_succeed(pid.parse(), "PID")).collect();
        assert_eq!(ids.len(),2);
        assert_ne!(ids[0], ids[1], "fixture must identify a distinct descendant");
        assert!(alive(ids[0]), "child must be observed alive before timeout");
        assert!(alive(ids[1]), "descendant must be observed alive before timeout");
        // Pause only after real I/O and both alive assertions. Otherwise Tokio
        // auto-advance could expire startup while the OS child is still booting.
        tokio::time::pause();
        for _ in 0..8 {tokio::task::yield_now().await;}
        let before_deadline = requested_at + Duration::from_secs(89);
        tokio::time::advance(before_deadline.saturating_duration_since(tokio::time::Instant::now())).await;
        for _ in 0..8 {tokio::task::yield_now().await;}
        assert!(notifications.try_recv().is_err(), "owned new expired before 90s");
        tokio::time::advance(Duration::from_secs(1)).await;
        let fatal = crate::test_support::must_succeed(tokio::time::timeout(Duration::from_secs(6), notifications.recv()).await, "owned startup timeout deadline").unwrap_or_else(|| panic!("owned startup timeout"));
        assert!(matches!(fatal.notification, Notification::BridgeDisconnected {reason} if reason.contains("timed out after 90s") && reason.contains("process-ready")));
        let joined = crate::test_support::must_succeed(
            tokio::time::timeout(Duration::from_secs(5), join).await,
            "owned mediator join deadline",
        );
        let result = crate::test_support::must_succeed(joined, "owned mediator join");
        crate::test_support::must_succeed(result, "owned mediator shutdown");
        tokio::time::resume();
        assert!(sender.send(BridgeCommand::NewSession {cwd:directory.path().to_owned()}).await.is_err());
        crate::test_support::must_succeed(tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(frame) = notifications.recv().await {
                assert!(!matches!(frame.notification, Notification::SessionCreated {..} | Notification::BridgeDisconnected {..}));
            }
        }).await, "owned notification stream must close");
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive(ids[0]) || alive(ids[1]) {
            assert!(Instant::now()<deadline, "owned process tree survived fatal startup: {ids:?}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await;
}

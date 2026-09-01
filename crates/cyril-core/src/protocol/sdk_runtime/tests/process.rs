use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;

use super::super::process::RecordingReader;
use super::super::{SdkRuntime, StageChain};
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::domain_mediator::{DomainChannels, DomainConfig, DomainMediator};
use crate::protocol::engine::V2Engine;
use crate::protocol::source_observer::IngressTracker;
use crate::protocol::transport::AgentProcess;
use crate::types::{AgentCommand, BridgeCommand, Notification};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

const FUTURE_FRAME: &str = r#" { "jsonrpc": "2.0", "method": "session/update", "params": {"sessionId":"process-fixture","update":{"sessionUpdate":"future_process_update","payload":{"kept":true}}} }"#;
const PROCESS_BATCH: &str = r#"[{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"process-fixture","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"batch-0"}}}},{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"process-fixture","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"batch-1"}}}}]"#;

#[tokio::test]
async fn c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes() {
    let expected = b"{\"number\":1e400}\n[{\"id\":1},{\"id\":2}]\n{\"malformed\":\xff\n".to_vec();
    let (mut writer, reader) = tokio::io::duplex(8);
    let write_bytes = expected.clone();
    let write_task = tokio::spawn(async move {
        for chunk in write_bytes.chunks(3) {
            writer.write_all(chunk).await?;
            tokio::task::yield_now().await;
        }
        writer.shutdown().await
    });
    let capture = Arc::new(Mutex::new(Vec::new()));
    let mut recording = RecordingReader::new(reader, Some(Arc::clone(&capture)));
    let mut delivered = Vec::new();
    recording
        .read_to_end(&mut delivered)
        .await
        .unwrap_or_else(|error| panic!("recording reader delivery: {error}"));
    write_task
        .await
        .unwrap_or_else(|error| panic!("recording writer join: {error}"))
        .unwrap_or_else(|error| panic!("recording writer: {error}"));
    assert_eq!(delivered, expected);
    assert_eq!(
        capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        expected.as_slice()
    );
}

#[cfg(unix)]
#[test]
fn c4_process_adapter_captures_invalid_frames_before_sdk_rejection() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| panic!("parser-boundary runtime build: {error}"));
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let script_for = |name: &str, frame: &str| {
            r#"
{
  IFS= read -r line
  id=${line##*\"id\":\"}
  id=${id%%\"*}
  printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true},"authMethods":[]},"id":"' "$id" '"}'
  printf '%s\n' '__FRAME__'
  printf '%s\n' '{"jsonrpc":"2.0","method":"_probe/after_invalid","params":{"name":"__NAME__"}}'
} | tee "$1"
sleep 0.05
"#
            .replace("__FRAME__", frame)
            .replace("__NAME__", name)
        };
        for (name, frame) in [
            (
                "extreme-number",
                r#"{"jsonrpc":"2.0","method":"_probe/number","params":{"value":1e400}}"#,
            ),
            (
                "malformed-json",
                r#"{"jsonrpc":"2.0","method":"_probe/malformed","params":"#,
            ),
        ] {
            assert!(
                serde_json::from_str::<agent_client_protocol::RawJsonRpcMessage>(frame).is_err(),
                "{name} fixture must be rejected by the SDK raw-message parser"
            );
            let directory = tempfile::tempdir()
                .unwrap_or_else(|error| panic!("{name} fixture tempdir: {error}"));
            let expected_path = directory.path().join(format!("{name}-expected.jsonl"));
            let command = AgentCommand::new("sh").with_args(vec![
                "-c".to_owned(),
                script_for(name, frame),
                "cyril-parser-fixture".to_owned(),
                expected_path.to_string_lossy().into_owned(),
            ]);
            let process = AgentProcess::spawn(&command, directory.path())
                .await
                .unwrap_or_else(|error| panic!("{name} process spawn: {error}"));
            let capture = Arc::new(Mutex::new(Vec::new()));
            let (channels, mut work_rx, _host_rx) =
                DomainChannels::new(IngressTracker::new())
                    .unwrap_or_else(|error| panic!("{name} domain channels: {error}"));
            let sdk = SdkRuntime::start_recording_process_for_test(
                process,
                channels,
                StageChain::default(),
                Arc::clone(&capture),
            )
            .await
            .unwrap_or_else(|error| panic!("{name} SDK runtime: {error}"));
            sdk.connection()
                .send_request(acp::InitializeRequest::new(
                    agent_client_protocol::schema::ProtocolVersion::V1,
                ))
                .block_task()
                .await
                .unwrap_or_else(|error| panic!("{name} initialize: {error}"));
            let work = tokio::time::timeout(Duration::from_secs(5), work_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("{name} post-rejection sentinel timed out"))
                .unwrap_or_else(|| panic!("{name} work channel closed"));
            assert!(
                matches!(
                    work,
                    crate::protocol::domain_mediator::DomainWork::ExtensionNotification(
                        ref message
                    ) if message.method() == "_probe/after_invalid"
                        && message.params["name"] == name
                ),
                "{name} invalid frame must be rejected before the following valid frame: {work:?}"
            );
            let closed = tokio::time::timeout(Duration::from_secs(5), work_rx.recv())
                .await
                .unwrap_or_else(|_| panic!("{name} rejection/EOF timed out"))
                .unwrap_or_else(|| panic!("{name} work channel closed before EOF"));
            assert!(
                matches!(
                    closed,
                    crate::protocol::domain_mediator::DomainWork::TransportClosed
                ),
                "{name} must not create domain work before clean EOF: {closed:?}"
            );
            sdk.shutdown().await;
            let expected = std::fs::read(&expected_path)
                .unwrap_or_else(|error| panic!("{name} expected capture read: {error}"));
            assert_eq!(
                capture
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .as_slice(),
                expected.as_slice(),
                "{name} ProcessAdapter capture must exactly match child stdout"
            );
        }
    });
}

#[cfg(unix)]
#[test]
fn process_adapter_preserves_raw_ingress_and_clean_eof() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap_or_else(|error| panic!("process fixture runtime build: {error}"));
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let directory =
            tempfile::tempdir().unwrap_or_else(|error| panic!("process fixture tempdir: {error}"));
        let script = r#"
while IFS= read -r line; do
  id=${line##*\"id\":\"}
  id=${id%%\"*}
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true,"promptCapabilities":{"image":false,"audio":false,"embeddedContext":false},"mcpCapabilities":{"http":false,"sse":false},"sessionCapabilities":{}},"authMethods":[],"agentInfo":{"name":"process-fixture","version":"1"}},"id":"' "$id" '"}'
      ;;
    *'"method":"session/new"'*)
      printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"sessionId":"process-fixture","models":{"currentModelId":"process-model","availableModels":[{"modelId":"process-model","name":"Process Model","description":"fixture"}]}},"id":"' "$id" '"}'
      printf '%s\n' ' { "jsonrpc": "2.0", "method": "session/update", "params": {"sessionId":"process-fixture","update":{"sessionUpdate":"future_process_update","payload":{"kept":true}}} }'
      printf '%s\n' '[{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"process-fixture","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"batch-0"}}}},{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"process-fixture","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"batch-1"}}}}]'
      ;;
    *'"method":"session/prompt"'*)
      exit 0
      ;;
  esac
done | tee "$1"
"#;
        let expected_path = directory.path().join("expected-stdout.jsonl");
        let command = AgentCommand::new("sh").with_args(vec![
            "-c".to_owned(),
            script.to_owned(),
            "cyril-process-fixture".to_owned(),
            expected_path.to_string_lossy().into_owned(),
        ]);
        let process = AgentProcess::spawn(&command, directory.path())
            .await
            .unwrap_or_else(|error| panic!("process fixture spawn: {error}"));
        let stderr_tail = process.stderr_tail();
        let capture = Arc::new(Mutex::new(Vec::new()));
        let (handle, bridge) = create_channel_pair();
        let (sender, mut notifications, _permissions, _sources, _completion) = handle.split();
        let config = DomainConfig {
            engine: Rc::new(V2Engine),
            cwd: directory.path().to_path_buf(),
            present_as: None,
            stall_threshold: Duration::from_secs(30),
            #[cfg(feature = "kas")]
            host_shell: None,
        };
        let (mediator, channels) = DomainMediator::new(config, bridge)
            .unwrap_or_else(|error| panic!("process fixture domain channels: {error}"));
        let runtime = SdkRuntime::start_recording_process_for_test(
            process,
            channels,
            StageChain::default(),
            Arc::clone(&capture),
        )
        .await
        .unwrap_or_else(|error| panic!("process fixture runtime: {error}"));
        let loop_handle = tokio::task::spawn_local(mediator.run(runtime));

        sender
            .send(BridgeCommand::NewSession {
                cwd: directory.path().to_path_buf(),
            })
            .await
            .unwrap_or_else(|error| panic!("process fixture session command: {error}"));
        // The fixture emits the batch chunks right after the session/new
        // response; the chunks ride the notification path while the response
        // resolves on a spawned command task, so their interleaving with
        // SessionCreated is not fixed. Batch ORDER among chunks still is.
        let mut session_id = None;
        let mut batches = Vec::new();
        while session_id.is_none() || batches.len() < 2 {
            let routed = tokio::time::timeout(Duration::from_secs(5), notifications.recv())
                .await
                .unwrap_or_else(|_| panic!("process fixture session notification timeout"))
                .unwrap_or_else(|| panic!("process fixture session channel closed"));
            match routed.notification {
                Notification::SessionCreated {
                    session_id: created,
                    current_model,
                    available_models,
                    ..
                } => {
                    assert_eq!(created.as_str(), "process-fixture");
                    assert_eq!(current_model.as_deref(), Some("process-model"));
                    assert_eq!(available_models.len(), 1);
                    assert_eq!(available_models[0].id().as_str(), "process-model");
                    session_id = Some(created);
                }
                Notification::AgentMessage(message) => batches.push(message.text),
                Notification::UsageSessionStarted { .. } => {}
                other => panic!("unexpected process fixture session notification: {other:?}"),
            }
        }
        let session_id =
            session_id.unwrap_or_else(|| panic!("process fixture session never created"));
        assert_eq!(
            batches,
            ["batch-0", "batch-1"],
            "ProcessAdapter batch member order changed"
        );
        sender
            .send(BridgeCommand::SendPrompt {
                session_id,
                prompt: crate::types::PromptEnvelope::prepared(
                    vec!["close during prompt".to_owned()],
                    None,
                ),
            })
            .await
            .unwrap_or_else(|error| panic!("process fixture prompt command: {error}"));

        let mut phase = 0;
        while phase < 3 {
            let routed =
                match tokio::time::timeout(Duration::from_secs(5), notifications.recv()).await {
                    Ok(Some(routed)) => routed,
                    outcome => {
                        let raw = capture
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        panic!(
                            "process fixture terminal failed in phase {phase}: {outcome:?}; raw={:?}; stderr={:?}; loop_finished={}",
                            String::from_utf8_lossy(&raw),
                            stderr_tail.snapshot(),
                            loop_handle.is_finished()
                        );
                    }
                };
            match (phase, routed.notification) {
                (
                    0,
                    Notification::BridgeError {
                        operation,
                        message: _,
                    },
                ) => {
                    assert_eq!(operation, "prompt");
                    phase = 1;
                }
                (
                    1,
                    Notification::TurnCompleted {
                        stop_reason: crate::types::StopReason::EndTurn,
                    },
                ) => phase = 2,
                (2, Notification::BridgeDisconnected { reason }) => {
                    assert_eq!(reason, "agent connection closed unexpectedly");
                    phase = 3;
                }
                (expected, other) => {
                    panic!("unexpected process fixture phase {expected} notification: {other:?}")
                }
            }
        }
        loop_handle
            .await
            .unwrap_or_else(|error| panic!("process fixture loop join: {error}"))
            .unwrap_or_else(|error| panic!("process fixture loop: {error}"));

        let raw = capture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let expected = std::fs::read(&expected_path)
            .unwrap_or_else(|error| panic!("process fixture expected capture read: {error}"));
        assert_eq!(
            raw.as_slice(),
            expected.as_slice(),
            "ProcessAdapter raw capture must exactly preserve child stdout"
        );
        let raw = std::str::from_utf8(&raw)
            .unwrap_or_else(|error| panic!("process fixture output utf8: {error}"));
        let lines = raw.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 4, "unexpected ProcessAdapter frame count: {raw:?}");
        assert_eq!(lines[2], FUTURE_FRAME);
        assert_eq!(lines[3], PROCESS_BATCH);
    });
}

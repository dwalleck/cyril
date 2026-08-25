use std::collections::HashSet;
use std::time::Duration;

use cyril_core::types::{
    SourceTurnDisposition as CoreDisposition, SourceTurnEvent as CoreEvent,
    SourceTurnEventKind as CoreEventKind,
};
use cyril_memory::{
    CaptureBatch, SourceSessionId, SourceTurnDisposition, SourceTurnEvent, SourceTurnEventKind,
    SourceTurnId,
};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::memory_runtime::ProjectMemory;

const MAX_BATCH_EVENTS: usize = 16;
const MAX_BATCH_BYTES: usize = 256 * 1024;
pub(crate) const CAPTURE_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) struct CaptureForwarder {
    task: JoinHandle<()>,
}

impl CaptureForwarder {
    pub(crate) fn spawn(source_rx: mpsc::Receiver<CoreEvent>, memory: ProjectMemory) -> Self {
        Self {
            task: tokio::spawn(run(source_rx, memory)),
        }
    }
    pub(crate) fn discard(mut source_rx: mpsc::Receiver<CoreEvent>) -> Self {
        Self {
            task: tokio::spawn(async move { while source_rx.recv().await.is_some() {} }),
        }
    }

    pub(crate) async fn drain(self) {
        let mut task = self.task;
        match tokio::time::timeout(CAPTURE_DRAIN_TIMEOUT, &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::warn!(%error, "capture forwarder task failed"),
            Err(_) => {
                task.abort();
                tracing::warn!("capture forwarder drain timed out");
            }
        }
    }
}

async fn run(mut source_rx: mpsc::Receiver<CoreEvent>, memory: ProjectMemory) {
    let mut pending = None;
    let mut failed_turns = HashSet::new();
    loop {
        let first = match pending.take() {
            Some(event) => event,
            None => match source_rx.recv().await {
                Some(event) => event,
                None => break,
            },
        };
        let identity = first.source_turn_id();
        if failed_turns.contains(&identity) {
            if matches!(first.kind(), CoreEventKind::Finished { .. }) {
                failed_turns.remove(&identity);
            }
            continue;
        }
        let mut approximate_bytes = event_bytes(&first);
        let mut raw = vec![first];
        while raw.len() < MAX_BATCH_EVENTS {
            let next = match source_rx.try_recv() {
                Ok(event) => event,
                Err(mpsc::error::TryRecvError::Empty | mpsc::error::TryRecvError::Disconnected) => {
                    break;
                }
            };
            let next_bytes = event_bytes(&next);
            let contiguous = next.source_turn_id() == identity
                && next.sequence()
                    == raw
                        .last()
                        .map_or(0, |event| event.sequence().saturating_add(1));
            if !contiguous || approximate_bytes.saturating_add(next_bytes) > MAX_BATCH_BYTES {
                pending = Some(next);
                break;
            }
            approximate_bytes = approximate_bytes.saturating_add(next_bytes);
            raw.push(next);
        }

        let batch = raw
            .into_iter()
            .map(convert_event)
            .collect::<Result<Vec<_>, _>>()
            .and_then(CaptureBatch::new);
        match batch {
            Ok(batch) => {
                if let Err(error) = memory.capture_batch(batch).await {
                    failed_turns.insert(identity);
                    tracing::warn!(source_turn_id = %identity, %error, "source turn capture failed");
                }
            }
            Err(error) => {
                failed_turns.insert(identity);
                tracing::warn!(source_turn_id = %identity, %error, "source turn event rejected");
            }
        }
    }
}

fn convert_event(event: CoreEvent) -> Result<SourceTurnEvent, cyril_memory::SourceTurnError> {
    let session_id = SourceSessionId::new(event.session_id().as_str().to_owned())
        .map_err(|_| cyril_memory::SourceTurnError::InvalidEvent)?;
    let source_turn_id = SourceTurnId::from_bytes(event.source_turn_id().as_bytes());
    let sequence = event.sequence();
    let kind = match event.into_kind() {
        CoreEventKind::Started {
            bridge_turn_id,
            started_at_ms,
            block_count,
        } => SourceTurnEventKind::Started {
            bridge_turn_id,
            started_at_ms,
            block_count,
        },
        CoreEventKind::PromptFragment {
            block_index,
            fragment_index,
            text,
            is_last,
        } => SourceTurnEventKind::PromptFragment {
            block_index,
            fragment_index,
            text,
            is_last,
        },
        CoreEventKind::AssistantFragment {
            fragment_index,
            text,
        } => SourceTurnEventKind::AssistantFragment {
            fragment_index,
            text,
        },
        CoreEventKind::ToolSnapshot {
            tool_index,
            tool_id,
            name,
            status,
            input,
            result,
            source_truncated_chars,
        } => SourceTurnEventKind::ToolSnapshot {
            tool_index,
            tool_id,
            name,
            status,
            input,
            result,
            source_truncated_chars,
        },
        CoreEventKind::Finished {
            disposition,
            finished_at_ms,
        } => SourceTurnEventKind::Finished {
            disposition: convert_disposition(disposition),
            finished_at_ms,
        },
    };
    SourceTurnEvent::new(session_id, source_turn_id, sequence, kind)
}

fn convert_disposition(value: CoreDisposition) -> SourceTurnDisposition {
    match value {
        CoreDisposition::Completed => SourceTurnDisposition::Completed,
        CoreDisposition::Interrupted => SourceTurnDisposition::Interrupted,
        CoreDisposition::Failed => SourceTurnDisposition::Failed,
        CoreDisposition::Abandoned => SourceTurnDisposition::Abandoned,
        CoreDisposition::CaptureOverflow => SourceTurnDisposition::CaptureOverflow,
    }
}

fn event_bytes(event: &CoreEvent) -> usize {
    let payload = match event.kind() {
        CoreEventKind::Started { .. } | CoreEventKind::Finished { .. } => 32,
        CoreEventKind::PromptFragment { text, .. }
        | CoreEventKind::AssistantFragment { text, .. } => text.len(),
        CoreEventKind::ToolSnapshot {
            tool_id,
            name,
            status,
            input,
            result,
            ..
        } => tool_id.len() + name.len() + status.len() + input.len() + result.len(),
    };
    64_usize
        .saturating_add(event.session_id().as_str().len())
        .saturating_add(payload)
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used)]

    use super::CaptureForwarder;
    use cyril_core::types::{
        SessionId, SourceTurnDisposition, SourceTurnEvent, SourceTurnEventKind, SourceTurnId,
    };
    use std::time::{Duration, Instant};
    use tokio::sync::mpsc;

    #[cfg(unix)]
    #[tokio::test]
    async fn c9_forwarder_batches_and_drains_before_runtime_shutdown() {
        let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
        let workspace = tempfile::tempdir().expect("workspace");
        let memory = runtime.bind(workspace.path());
        let source_turn_id = SourceTurnId::from_bytes([0x29; 16]);
        let session_id = SessionId::new("forwarded-session");
        let (tx, rx) = mpsc::channel(32);
        let forwarder = CaptureForwarder::spawn(rx, memory.clone());
        let batch_started = Instant::now();
        for event in [
            SourceTurnEvent::for_tests(
                session_id.clone(),
                source_turn_id,
                0,
                SourceTurnEventKind::Started {
                    bridge_turn_id: 7,
                    started_at_ms: 100,
                    block_count: 1,
                },
            ),
            SourceTurnEvent::for_tests(
                session_id.clone(),
                source_turn_id,
                1,
                SourceTurnEventKind::PromptFragment {
                    block_index: 0,
                    fragment_index: 0,
                    text: "forward this decision".to_owned(),
                    is_last: true,
                },
            ),
            SourceTurnEvent::for_tests(
                session_id,
                source_turn_id,
                2,
                SourceTurnEventKind::Finished {
                    disposition: SourceTurnDisposition::Completed,
                    finished_at_ms: 200,
                },
            ),
        ] {
            tx.send(event).await.expect("source event");
        }
        drop(tx);
        forwarder.drain().await;
        let batch_elapsed = batch_started.elapsed();
        assert!(
            batch_elapsed <= Duration::from_millis(100),
            "C9 forwarder batch exceeded budget: {batch_elapsed:?}"
        );
        let stored = memory
            .inspect_turn(cyril_memory::SourceTurnId::from_bytes(
                source_turn_id.as_bytes(),
            ))
            .await
            .expect("forwarded turn");
        assert_eq!(stored.prompt(), "forward this decision", "C9 prompt");
        assert_eq!(
            stored.status(),
            cyril_memory::SourceTurnStatus::Completed,
            "C9 terminal"
        );
        runtime.shutdown().await;
    }
}

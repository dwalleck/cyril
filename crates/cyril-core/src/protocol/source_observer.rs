use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tokio::sync::{Notify, mpsc};

use crate::types::source_turn::{SOURCE_FRAGMENT_BYTES, tool_status};
use crate::types::{
    Notification, RoutedNotification, SessionId, SourceTurnDisposition, SourceTurnEvent,
    SourceTurnEventKind, SourceTurnId, ToolCall, ToolCallContent, TurnId,
};

const TOOL_ID_BYTES: usize = 1024;
const TOOL_NAME_BYTES: usize = 8 * 1024;
const TOOL_STATUS_BYTES: usize = 256;
const TOOL_INPUT_BYTES: usize = 24 * 1024;
const TOOL_RESULT_BYTES: usize = 24 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct IngressTracker {
    state: Arc<IngressState>,
}

#[derive(Debug)]
struct IngressState {
    active: AtomicUsize,
    epoch: AtomicU64,
    notify: Notify,
}

pub(crate) struct IngressGuard {
    state: Arc<IngressState>,
}

impl IngressTracker {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(IngressState {
                active: AtomicUsize::new(0),
                epoch: AtomicU64::new(0),
                notify: Notify::new(),
            }),
        }
    }

    pub(crate) fn enter(&self) -> IngressGuard {
        self.state.active.fetch_add(1, Ordering::AcqRel);
        self.state.epoch.fetch_add(1, Ordering::AcqRel);
        IngressGuard {
            state: Arc::clone(&self.state),
        }
    }

    pub(crate) async fn wait_quiescent(&self) {
        loop {
            tokio::task::yield_now().await;
            // cyril-p3t3: create-then-check. The Notified future must be
            // REGISTERED (enable) before the `active` load, or a guard
            // dropping between the load and the await is a lost wakeup —
            // `notify_waiters` stores no permit for late registrants, and
            // only the caller's rescue timeout would end the sleep.
            let notified = self.state.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let epoch = self.state.epoch.load(Ordering::Acquire);
            if self.state.active.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
                if self.state.active.load(Ordering::Acquire) == 0
                    && self.state.epoch.load(Ordering::Acquire) == epoch
                {
                    return;
                }
            } else {
                notified.await;
            }
        }
    }
}

impl Drop for IngressGuard {
    fn drop(&mut self) {
        self.state.active.fetch_sub(1, Ordering::AcqRel);
        self.state.epoch.fetch_add(1, Ordering::AcqRel);
        self.state.notify.notify_waiters();
    }
}

#[derive(Clone)]
pub(crate) struct SourceObserver {
    tx: mpsc::Sender<SourceTurnEvent>,
    active: Rc<RefCell<Option<ActiveTurn>>>,
}

struct ActiveTurn {
    session_id: SessionId,
    source_turn_id: SourceTurnId,
    next_sequence: u64,
    assistant_fragment: usize,
    tool_indices: BTreeMap<[u8; 32], usize>,
    lost: bool,
}

impl SourceObserver {
    pub(crate) fn new(tx: mpsc::Sender<SourceTurnEvent>) -> Self {
        Self {
            tx,
            active: Rc::new(RefCell::new(None)),
        }
    }

    pub(crate) fn begin(
        &self,
        session_id: SessionId,
        bridge_turn_id: TurnId,
        original_blocks: &[String],
    ) -> Result<SourceTurnId, getrandom::Error> {
        let source_turn_id = SourceTurnId::generate()?;
        let mut active = ActiveTurn {
            session_id,
            source_turn_id,
            next_sequence: 0,
            assistant_fragment: 0,
            tool_indices: BTreeMap::new(),
            lost: false,
        };
        self.try_emit(
            &mut active,
            SourceTurnEventKind::Started {
                bridge_turn_id: bridge_turn_id.get(),
                started_at_ms: now_ms(),
                block_count: original_blocks.len(),
            },
        );
        for (block_index, block) in original_blocks.iter().enumerate() {
            let fragments = utf8_fragments(block, SOURCE_FRAGMENT_BYTES);
            let last = fragments.len().saturating_sub(1);
            for (fragment_index, text) in fragments.into_iter().enumerate() {
                self.try_emit(
                    &mut active,
                    SourceTurnEventKind::PromptFragment {
                        block_index,
                        fragment_index,
                        text: text.to_owned(),
                        is_last: fragment_index == last,
                    },
                );
            }
        }
        *self.active.borrow_mut() = Some(active);
        Ok(source_turn_id)
    }

    pub(crate) fn observe(&self, routed: &RoutedNotification) {
        let mut slot = self.active.borrow_mut();
        let Some(active) = slot.as_mut() else {
            return;
        };
        if routed.session_id.as_ref() != Some(&active.session_id) {
            return;
        }
        match &routed.notification {
            Notification::AgentMessage(message) => {
                for text in utf8_fragments(&message.text, SOURCE_FRAGMENT_BYTES) {
                    let fragment_index = active.assistant_fragment;
                    active.assistant_fragment = active.assistant_fragment.saturating_add(1);
                    self.try_emit(
                        active,
                        SourceTurnEventKind::AssistantFragment {
                            fragment_index,
                            text: text.to_owned(),
                        },
                    );
                }
            }
            Notification::ToolCallStarted(tool) | Notification::ToolCallUpdated(tool) => {
                self.observe_tool(active, tool);
            }
            Notification::ToolCallChunk {
                tool_call_id,
                title,
                kind,
                ..
            } => {
                let tool_id = tool_call_id.as_str();
                let index = tool_index(active, tool_id);
                self.try_emit(
                    active,
                    bounded_tool_snapshot(
                        index,
                        tool_id,
                        title,
                        kind,
                        (String::new(), 0),
                        (String::new(), 0),
                    ),
                );
            }
            // Thoughts, replayed user messages, terminals, locations, usage,
            // and presentation-only notifications are deliberately not source.
            _ => {}
        }
    }

    pub(crate) fn finish(&self, disposition: SourceTurnDisposition) {
        let Some(active) = self.active.borrow_mut().take() else {
            return;
        };
        let final_disposition = if active.lost {
            SourceTurnDisposition::CaptureOverflow
        } else {
            disposition
        };
        let event = active.event(SourceTurnEventKind::Finished {
            disposition: final_disposition,
            finished_at_ms: now_ms(),
        });
        match self.tx.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(event)) => {
                let tx = self.tx.clone();
                tokio::task::spawn_local(async move {
                    if let Err(error) = tx.send(event).await {
                        tracing::debug!(%error, "source overflow marker dropped: App gone");
                    }
                });
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!("source terminal dropped: App gone");
            }
        }
    }

    fn observe_tool(&self, active: &mut ActiveTurn, tool: &ToolCall) {
        let tool_id = tool.id().as_str();
        let index = tool_index(active, tool_id);
        let input = match tool.raw_input() {
            Some(value) => match bounded_json(value, TOOL_INPUT_BYTES) {
                Ok(encoded) => encoded,
                Err(error) => {
                    tracing::warn!(%error, tool_call_id = %tool.id(), "tool input serialization failed");
                    (String::new(), 0)
                }
            },
            None => (String::new(), 0),
        };
        let result = tool_result(tool);
        self.try_emit(
            active,
            bounded_tool_snapshot(
                index,
                tool_id,
                tool.title(),
                tool_status(tool.status()),
                input,
                result,
            ),
        );
    }

    fn try_emit(&self, active: &mut ActiveTurn, kind: SourceTurnEventKind) {
        if active.lost {
            return;
        }
        let event = active.event(kind);
        match self.tx.try_send(event) {
            Ok(()) => active.next_sequence = active.next_sequence.saturating_add(1),
            Err(mpsc::error::TrySendError::Full(_)) => active.lost = true,
            Err(mpsc::error::TrySendError::Closed(_)) => active.lost = true,
        }
    }
}

impl ActiveTurn {
    fn event(&self, kind: SourceTurnEventKind) -> SourceTurnEvent {
        SourceTurnEvent::new(
            self.session_id.clone(),
            self.source_turn_id,
            self.next_sequence,
            kind,
        )
    }
}

fn tool_index(active: &mut ActiveTurn, tool_id: &str) -> usize {
    let identity = Sha256::digest(tool_id.as_bytes()).into();
    let next = active.tool_indices.len();
    *active.tool_indices.entry(identity).or_insert(next)
}

fn tool_result(tool: &ToolCall) -> (String, usize) {
    if let Some(raw) = tool.raw_output() {
        match bounded_json(raw, TOOL_RESULT_BYTES) {
            Ok(encoded) => return encoded,
            Err(error) => {
                tracing::warn!(%error, tool_call_id = %tool.id(), "tool output serialization failed");
            }
        }
    }
    let mut result = BoundedText::new(TOOL_RESULT_BYTES);
    for content in tool.content() {
        match content {
            ToolCallContent::Text(text) => result.push_str(text),
            ToolCallContent::Diff {
                path,
                old_text,
                new_text,
            } => {
                result.push_str(path);
                result.push_str("\n");
                if let Some(old) = old_text {
                    result.push_str(old);
                    result.push_str("\n");
                }
                result.push_str(new_text);
            }
        }
    }
    result.finish()
}

fn bounded_json(
    value: &serde_json::Value,
    max_bytes: usize,
) -> Result<(String, usize), serde_json::Error> {
    let mut output = BoundedText::new(max_bytes);
    serde_json::to_writer(&mut output, value)?;
    Ok(output.finish())
}

fn bounded_tool_snapshot(
    tool_index: usize,
    tool_id: &str,
    name: &str,
    status: &str,
    input: (String, usize),
    result: (String, usize),
) -> SourceTurnEventKind {
    let (tool_id, tool_id_dropped) = BoundedText::from_str(tool_id, TOOL_ID_BYTES);
    let (name, name_dropped) = BoundedText::from_str(name, TOOL_NAME_BYTES);
    let (status, status_dropped) = BoundedText::from_str(status, TOOL_STATUS_BYTES);
    let (input, input_dropped) = input;
    let (result, result_dropped) = result;
    let source_truncated_chars = tool_id_dropped
        .saturating_add(name_dropped)
        .saturating_add(status_dropped)
        .saturating_add(input_dropped)
        .saturating_add(result_dropped);
    SourceTurnEventKind::ToolSnapshot {
        tool_index,
        tool_id,
        name,
        status,
        input,
        result,
        source_truncated_chars,
    }
}

struct BoundedText {
    value: String,
    max_bytes: usize,
    total_chars: usize,
}

impl BoundedText {
    fn new(max_bytes: usize) -> Self {
        Self {
            value: String::with_capacity(max_bytes),
            max_bytes,
            total_chars: 0,
        }
    }

    fn from_str(value: &str, max_bytes: usize) -> (String, usize) {
        let mut bounded = Self::new(max_bytes);
        bounded.push_str(value);
        bounded.finish()
    }

    fn push_str(&mut self, value: &str) {
        self.total_chars = self.total_chars.saturating_add(value.chars().count());
        let available = self.max_bytes.saturating_sub(self.value.len());
        if available == 0 {
            return;
        }
        let mut end = value.len().min(available);
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        self.value.push_str(&value[..end]);
    }

    fn finish(self) -> (String, usize) {
        let kept_chars = self.value.chars().count();
        (self.value, self.total_chars.saturating_sub(kept_chars))
    }
}

impl Write for BoundedText {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let value = std::str::from_utf8(buffer)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        self.push_str(value);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn utf8_fragments(value: &str, max_bytes: usize) -> Vec<&str> {
    if value.is_empty() {
        return vec![""];
    }
    let mut fragments = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let mut end = value.len().min(start.saturating_add(max_bytes));
        while end > start && !value.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = value[start..]
                .char_indices()
                .nth(1)
                .map_or(value.len(), |(offset, _)| start + offset);
        }
        fragments.push(&value[start..end]);
        start = end;
    }
    fragments
}

fn now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => match i64::try_from(duration.as_millis()) {
            Ok(timestamp) => timestamp,
            Err(error) => {
                tracing::warn!(%error, "source timestamp exceeds i64 milliseconds");
                i64::MAX
            }
        },
        Err(error) => {
            tracing::warn!(%error, "system clock predates Unix epoch");
            0
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used)]

    use super::{
        IngressTracker, SourceObserver, TOOL_ID_BYTES, TOOL_INPUT_BYTES, bounded_json,
        utf8_fragments,
    };
    use crate::types::{
        AgentMessage, AgentThought, Notification, PromptEnvelope, RoutedNotification, SessionId,
        SourceTurnDisposition, SourceTurnEvent, SourceTurnEventKind, ToolCall, ToolCallContent,
        ToolCallId, ToolCallStatus, ToolKind, TurnId, UserMessage,
    };
    use tokio::sync::mpsc;

    #[test]
    fn c1_fragmentation_is_utf8_safe_and_bounded() {
        let text = format!("{}z", "🦀".repeat(20_000));
        let fragments = utf8_fragments(&text, 64 * 1024);
        assert_eq!(fragments.concat(), text, "C1 source bytes changed");
        assert!(
            fragments.iter().all(|fragment| fragment.len() <= 64 * 1024),
            "C1 fragment exceeded channel contract"
        );
        let (tx, _rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        let started = std::time::Instant::now();
        observer
            .begin(SessionId::new("main"), TurnId::new(1), &[text])
            .expect("source id");
        let elapsed = started.elapsed();
        assert!(
            elapsed <= std::time::Duration::from_millis(10),
            "source observation budget exceeded: {elapsed:?}"
        );
    }
    fn drain(rx: &mut mpsc::Receiver<SourceTurnEvent>) -> Vec<SourceTurnEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[test]
    fn c1_accepted_prompt_capture_precedes_ui_and_excludes_context() {
        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        let prompt = PromptEnvelope::prepared(
            vec!["original user prompt".to_owned()],
            Some("<CYRIL_LESSONS>private context</CYRIL_LESSONS>".to_owned()),
        );
        observer
            .begin(
                SessionId::new("main"),
                TurnId::new(7),
                prompt.original_blocks(),
            )
            .expect("source id");
        let events = drain(&mut rx);
        assert!(matches!(
            events.first().map(SourceTurnEvent::kind),
            Some(SourceTurnEventKind::Started { .. })
        ));
        assert!(matches!(
            events.get(1).map(SourceTurnEvent::kind),
            Some(SourceTurnEventKind::PromptFragment { text, .. })
                if text == "original user prompt"
        ));
        assert_eq!(events.len(), 2, "C1 prepared context entered capture");
    }

    #[test]
    fn c2_terminal_disposition_never_false_completes() {
        for disposition in [
            SourceTurnDisposition::Completed,
            SourceTurnDisposition::Interrupted,
            SourceTurnDisposition::Failed,
            SourceTurnDisposition::Abandoned,
        ] {
            let (tx, mut rx) = mpsc::channel(8);
            let observer = SourceObserver::new(tx);
            observer
                .begin(SessionId::new("main"), TurnId::new(1), &["p".to_owned()])
                .expect("source id");
            observer.finish(disposition);
            let events = drain(&mut rx);
            assert!(matches!(
                events.last().map(SourceTurnEvent::kind),
                Some(SourceTurnEventKind::Finished {
                    disposition: actual,
                    ..
                }) if *actual == disposition
            ));
        }
    }

    #[test]
    fn c6_stream_tool_tail_assembles_without_thoughts_or_secrets() {
        let (tx, mut rx) = mpsc::channel(16);
        let observer = SourceObserver::new(tx);
        let session = SessionId::new("main");
        observer
            .begin(session.clone(), TurnId::new(1), &["prompt".to_owned()])
            .expect("source id");
        observer.observe(&RoutedNotification::scoped(
            session.clone(),
            Notification::AgentThought(AgentThought {
                text: "private reasoning".to_owned(),
            }),
        ));

        observer.observe(&RoutedNotification::scoped(
            session.clone(),
            Notification::UserMessage(UserMessage {
                text: "replayed prompt".to_owned(),
                is_streaming: false,
            }),
        ));
        observer.observe(&RoutedNotification::scoped(
            session.clone(),
            Notification::AgentMessage(AgentMessage {
                text: "assistant tail".to_owned(),
                is_streaming: true,
            }),
        ));
        observer.observe(&RoutedNotification::scoped(
            session,
            Notification::ToolCallStarted(
                ToolCall::new(
                    ToolCallId::new("tool-1"),
                    "read".to_owned(),
                    ToolKind::Read,
                    ToolCallStatus::Completed,
                    Some(serde_json::json!({"path": "src/lib.rs"})),
                )
                .with_content(vec![ToolCallContent::Text("done".to_owned())]),
            ),
        ));
        observer.finish(SourceTurnDisposition::Completed);
        let events = drain(&mut rx);
        assert!(events.iter().any(|event| matches!(
            event.kind(),
            SourceTurnEventKind::AssistantFragment { text, .. } if text == "assistant tail"
        )));
        assert!(events.iter().any(|event| matches!(
            event.kind(),
            SourceTurnEventKind::ToolSnapshot { tool_id, result, .. }
                if tool_id == "tool-1" && result == "done"
        )));
        assert!(!events.iter().any(|event| match event.kind() {
            SourceTurnEventKind::PromptFragment { text, .. }
            | SourceTurnEventKind::AssistantFragment { text, .. } => {
                text.contains("private reasoning") || text.contains("replayed prompt")
            }
            _ => false,
        }));
    }
    #[test]
    fn c3_tool_snapshot_payload_is_bounded_with_truncation_metadata() {
        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        let session = SessionId::new("main");
        observer
            .begin(session.clone(), TurnId::new(1), &["prompt".to_owned()])
            .expect("source id");
        let raw_input = serde_json::json!({"payload": "🦀".repeat(20_000)});
        let (expected_input, input_dropped) =
            bounded_json(&raw_input, TOOL_INPUT_BYTES).expect("bounded JSON");
        assert_eq!(expected_input.len(), TOOL_INPUT_BYTES);
        assert_eq!(input_dropped, 13_861, "C3 dropped input scalars");
        observer.observe(&RoutedNotification::scoped(
            session,
            Notification::ToolCallStarted(
                ToolCall::new(
                    ToolCallId::new("tool-1"),
                    "🦀".repeat(3_000),
                    ToolKind::Read,
                    ToolCallStatus::Completed,
                    Some(raw_input),
                )
                .with_content(vec![ToolCallContent::Text("🦀".repeat(20_000))]),
            ),
        ));
        let events = drain(&mut rx);
        let snapshot = events
            .iter()
            .find_map(|event| match event.kind() {
                SourceTurnEventKind::ToolSnapshot {
                    tool_id,
                    name,
                    status,
                    input,
                    result,
                    source_truncated_chars,
                    ..
                } => Some((tool_id, name, status, input, result, source_truncated_chars)),
                _ => None,
            })
            .expect("tool snapshot");
        let payload_bytes = snapshot.0.len()
            + snapshot.1.len()
            + snapshot.2.len()
            + snapshot.3.len()
            + snapshot.4.len();
        assert!(
            payload_bytes <= crate::types::source_turn::SOURCE_FRAGMENT_BYTES,
            "C3 tool snapshot exceeded source event bound"
        );
        assert_eq!(snapshot.3, &expected_input, "C3 emitted input prefix");
        assert!(snapshot.3.is_char_boundary(snapshot.3.len()));
        assert_eq!(*snapshot.5, 28_669, "C3 aggregate dropped scalar metadata");
        assert!(*snapshot.5 > 0, "C3 source truncation was not recorded");
    }

    #[test]
    fn oversized_tool_ids_are_bounded_and_remain_distinct() {
        let (tx, mut rx) = mpsc::channel(8);
        let observer = SourceObserver::new(tx);
        let session = SessionId::new("main");
        observer
            .begin(session.clone(), TurnId::new(1), &["prompt".to_owned()])
            .expect("source id");
        let prefix = "🦀".repeat(TOOL_ID_BYTES / 4);
        let suffix_chars = 256 * 1024;
        let first_id = format!("{prefix}{}", "a".repeat(suffix_chars));
        let second_id = format!("{prefix}{}", "b".repeat(suffix_chars));
        for tool_call_id in [&first_id, &second_id, &first_id] {
            observer.observe(&RoutedNotification::scoped(
                session.clone(),
                Notification::ToolCallChunk {
                    tool_call_id: ToolCallId::new(tool_call_id.clone()),
                    title: "read".to_owned(),
                    kind: "read".to_owned(),
                    session_id: None,
                },
            ));
        }
        let snapshots: Vec<_> = drain(&mut rx)
            .into_iter()
            .filter_map(|event| match event.kind() {
                SourceTurnEventKind::ToolSnapshot {
                    tool_index,
                    tool_id,
                    source_truncated_chars,
                    ..
                } => Some((*tool_index, tool_id.clone(), *source_truncated_chars)),
                _ => None,
            })
            .collect();
        assert_eq!(
            snapshots
                .iter()
                .map(|snapshot| snapshot.0)
                .collect::<Vec<_>>(),
            [0, 1, 0],
            "bounded identity must deduplicate repeats without merging equal prefixes"
        );
        assert!(snapshots.iter().all(|snapshot| snapshot.1 == prefix));
        assert!(
            snapshots.iter().all(|snapshot| snapshot.2 == suffix_chars),
            "oversized ID truncation metadata must count dropped Unicode scalars"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn c9_slow_capture_is_bounded_and_shutdown_drains_in_order() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let (tx, mut rx) = mpsc::channel(1);
                let observer = SourceObserver::new(tx);
                observer
                    .begin(
                        SessionId::new("main"),
                        TurnId::new(1),
                        &["prompt".to_owned()],
                    )
                    .expect("source id");
                observer.finish(SourceTurnDisposition::Completed);
                let started = rx.recv().await.expect("started");
                assert!(matches!(
                    started.kind(),
                    SourceTurnEventKind::Started { .. }
                ));
                let terminal = rx.recv().await.expect("overflow terminal");
                assert!(matches!(
                    terminal.kind(),
                    SourceTurnEventKind::Finished {
                        disposition: SourceTurnDisposition::CaptureOverflow,
                        ..
                    }
                ));
            })
            .await;
    }
    #[tokio::test(flavor = "current_thread")]
    async fn c9_ingress_quiescence_stays_within_bridge_budget() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let tracker = IngressTracker::new();
                let guards: Vec<_> = (0..32).map(|_| tracker.enter()).collect();
                tokio::task::spawn_local(async move {
                    tokio::task::yield_now().await;
                    drop(guards);
                });
                let started = std::time::Instant::now();
                tracker.wait_quiescent().await;
                let elapsed = started.elapsed();
                assert!(
                    elapsed <= std::time::Duration::from_millis(50),
                    "C9 ingress barrier exceeded budget: {elapsed:?}"
                );
            })
            .await;
    }

    #[test]
    fn c12_source_identity_survives_numeric_reuse_and_ignores_history() {
        let (tx, mut rx) = mpsc::channel(16);
        let observer = SourceObserver::new(tx);
        let session = SessionId::new("main");
        let first = observer
            .begin(session.clone(), TurnId::new(1), &["one".to_owned()])
            .expect("first id");
        observer.finish(SourceTurnDisposition::Completed);
        let second = observer
            .begin(session, TurnId::new(2), &["two".to_owned()])
            .expect("second id");
        observer.finish(SourceTurnDisposition::Completed);
        assert_ne!(first, second, "C12 source identity reused");
        let started_ids: Vec<_> = drain(&mut rx)
            .into_iter()
            .filter(|event| matches!(event.kind(), SourceTurnEventKind::Started { .. }))
            .map(|event| event.source_turn_id())
            .collect();
        assert_eq!(started_ids, [first, second], "C12 prior turn replayed");
    }
    mod current_runtime_contract;
}

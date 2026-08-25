use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::redaction::redact;

pub const MAX_SOURCE_EVENT_TEXT_CHARS: usize = 65_536;
pub const MAX_CAPTURE_EVENTS: usize = 16;
pub const MAX_CAPTURE_BYTES: usize = 256 * 1024;
pub const MAX_TOOLS: usize = 128;
pub const MAX_TOOL_CHARS: usize = 16_000;
pub const MAX_TOTAL_TOOL_CHARS: usize = 256_000;
pub const MAX_QUERY_CHARS: usize = 4_096;
pub const MAX_QUERY_TERMS: usize = 64;
pub const MAX_EPISODES: usize = 3;
pub const MAX_EPISODE_CHARS: usize = 1_200;
pub const MAX_EPISODE_TOTAL_CHARS: usize = 3_600;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceTurnId([u8; 16]);

impl SourceTurnId {
    pub fn generate() -> Result<Self, SourceTurnError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(SourceTurnError::Random)?;
        Ok(Self(bytes))
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl FromStr for SourceTurnId {
    type Err = SourceTurnIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        crate::encoding::decode_fixed_hex::<16>(value)
            .map(Self)
            .ok_or(SourceTurnIdParseError)
    }
}

impl fmt::Display for SourceTurnId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

impl fmt::Debug for SourceTurnId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SourceTurnId({self})")
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("source turn identity must be exactly 32 hexadecimal characters")]
pub struct SourceTurnIdParseError;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceSessionId(String);

impl SourceSessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, SourceSessionIdError> {
        let value = value.into();
        if value.is_empty() || value.chars().count() > 256 || value.chars().any(char::is_control) {
            return Err(SourceSessionIdError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for SourceSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SourceSessionId")
            .field(&self.0)
            .finish()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("source session identity must be non-empty, printable, and at most 256 characters")]
pub struct SourceSessionIdError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTurnDisposition {
    Completed,
    Interrupted,
    Failed,
    Abandoned,
    CaptureOverflow,
}

impl SourceTurnDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
            Self::CaptureOverflow => "capture_overflow",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum SourceTurnEventKind {
    Started {
        bridge_turn_id: u64,
        started_at_ms: i64,
        block_count: usize,
    },
    PromptFragment {
        block_index: usize,
        fragment_index: usize,
        text: String,
        is_last: bool,
    },
    AssistantFragment {
        fragment_index: usize,
        text: String,
    },
    ToolSnapshot {
        tool_index: usize,
        tool_id: String,
        name: String,
        status: String,
        input: String,
        result: String,
    },
    Finished {
        disposition: SourceTurnDisposition,
        finished_at_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTurnEvent {
    pub(crate) session_id: SourceSessionId,
    pub(crate) source_turn_id: SourceTurnId,
    pub(crate) sequence: u64,
    pub(crate) kind: SourceTurnEventKind,
}

impl SourceTurnEvent {
    pub fn new(
        session_id: SourceSessionId,
        source_turn_id: SourceTurnId,
        sequence: u64,
        kind: SourceTurnEventKind,
    ) -> Result<Self, SourceTurnError> {
        let event = Self {
            session_id,
            source_turn_id,
            sequence,
            kind,
        };
        event.validate_shape()?;
        Ok(event)
    }

    pub fn session_id(&self) -> &SourceSessionId {
        &self.session_id
    }

    pub const fn source_turn_id(&self) -> SourceTurnId {
        self.source_turn_id
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn kind(&self) -> &SourceTurnEventKind {
        &self.kind
    }

    fn validate_shape(&self) -> Result<(), SourceTurnError> {
        match &self.kind {
            SourceTurnEventKind::Started {
                started_at_ms,
                block_count,
                ..
            } => {
                if *started_at_ms < 0 || *block_count == 0 || *block_count > 256 {
                    return Err(SourceTurnError::InvalidEvent);
                }
            }
            SourceTurnEventKind::PromptFragment {
                block_index,
                fragment_index,
                text,
                ..
            } => {
                if *block_index >= 256
                    || *fragment_index > MAX_SOURCE_EVENT_TEXT_CHARS
                    || !valid_source_text(text)
                {
                    return Err(SourceTurnError::EventTooLarge);
                }
            }
            SourceTurnEventKind::AssistantFragment {
                fragment_index,
                text,
            } => {
                if *fragment_index > MAX_SOURCE_EVENT_TEXT_CHARS || !valid_source_text(text) {
                    return Err(SourceTurnError::EventTooLarge);
                }
            }
            SourceTurnEventKind::ToolSnapshot {
                tool_id,
                name,
                status,
                input,
                result,
                ..
            } => {
                if tool_id.is_empty()
                    || tool_id.chars().count() > 256
                    || name.chars().count() > MAX_SOURCE_EVENT_TEXT_CHARS
                    || status.is_empty()
                    || status.chars().count() > 64
                    || ![tool_id, name, status, input, result]
                        .iter()
                        .all(|value| valid_source_text(value))
                {
                    return Err(SourceTurnError::EventTooLarge);
                }
            }
            SourceTurnEventKind::Finished { finished_at_ms, .. } => {
                if *finished_at_ms < 0 {
                    return Err(SourceTurnError::InvalidEvent);
                }
            }
        }
        Ok(())
    }

    fn approximate_bytes(&self) -> usize {
        let base = 64 + self.session_id.as_str().len();
        base + match &self.kind {
            SourceTurnEventKind::Started { .. } | SourceTurnEventKind::Finished { .. } => 32,
            SourceTurnEventKind::PromptFragment { text, .. }
            | SourceTurnEventKind::AssistantFragment { text, .. } => text.len(),
            SourceTurnEventKind::ToolSnapshot {
                tool_id,
                name,
                status,
                input,
                result,
                ..
            } => tool_id.len() + name.len() + status.len() + input.len() + result.len(),
        }
    }
}

fn valid_source_text(value: &str) -> bool {
    value.chars().count() <= MAX_SOURCE_EVENT_TEXT_CHARS
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureBatch {
    source_turn_id: SourceTurnId,
    session_id: SourceSessionId,
    events: Vec<SourceTurnEvent>,
    encoded_bytes: usize,
}

impl CaptureBatch {
    pub fn new(events: Vec<SourceTurnEvent>) -> Result<Self, SourceTurnError> {
        if events.is_empty() {
            return Err(SourceTurnError::EmptyBatch);
        }
        if events.len() > MAX_CAPTURE_EVENTS {
            return Err(SourceTurnError::TooManyEvents);
        }
        let source_turn_id = events[0].source_turn_id;
        let session_id = events[0].session_id.clone();
        let mut encoded_bytes = 0_usize;
        let mut previous: Option<u64> = None;
        for event in &events {
            event.validate_shape()?;
            if event.source_turn_id != source_turn_id || event.session_id != session_id {
                return Err(SourceTurnError::MixedIdentity);
            }
            if previous.is_some_and(|value| event.sequence != value.saturating_add(1)) {
                return Err(SourceTurnError::InvalidSequence);
            }
            previous = Some(event.sequence);
            encoded_bytes = encoded_bytes
                .checked_add(event.approximate_bytes())
                .ok_or(SourceTurnError::BatchTooLarge)?;
        }
        if encoded_bytes > MAX_CAPTURE_BYTES {
            return Err(SourceTurnError::BatchTooLarge);
        }
        Ok(Self {
            source_turn_id,
            session_id,
            events,
            encoded_bytes,
        })
    }

    pub fn events(&self) -> &[SourceTurnEvent] {
        &self.events
    }

    pub const fn source_turn_id(&self) -> SourceTurnId {
        self.source_turn_id
    }

    pub fn session_id(&self) -> &SourceSessionId {
        &self.session_id
    }

    pub const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRecord {
    pub tool_id: String,
    pub name: String,
    pub status: String,
    pub input: String,
    pub result: String,
    pub truncated_chars: usize,
}

impl ToolRecord {
    fn char_count(&self) -> usize {
        self.tool_id.chars().count()
            + self.name.chars().count()
            + self.status.chars().count()
            + self.input.chars().count()
            + self.result.chars().count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceTurnState {
    Incomplete,
    Finished(SourceTurnDisposition),
}

impl SourceTurnState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Finished(disposition) => disposition.as_str(),
        }
    }
}

#[derive(Debug, Error)]
pub enum SourceTurnError {
    #[error("source capture batch cannot be empty")]
    EmptyBatch,
    #[error("source capture batch contains too many events")]
    TooManyEvents,
    #[error("source capture batch is too large")]
    BatchTooLarge,
    #[error("source event payload is too large or malformed")]
    EventTooLarge,
    #[error("source event is malformed")]
    InvalidEvent,
    #[error("source events contain mixed identities")]
    MixedIdentity,
    #[error("source event sequence is not contiguous")]
    InvalidSequence,
    #[error("source event order is invalid")]
    InvalidOrder,
    #[error("source event conflicts with immutable replay data")]
    ImmutableConflict,
    #[error("source event is duplicated after a terminal event")]
    DuplicateTerminal,
    #[error("source turn identity generation failed")]
    Random(#[source] getrandom::Error),
}

pub struct SourceTurnDraft {
    pub(crate) source_turn_id: SourceTurnId,
    pub(crate) session_id: SourceSessionId,
    pub(crate) bridge_turn_id: u64,
    pub(crate) started_at_ms: i64,
    pub(crate) finished_at_ms: Option<i64>,
    pub(crate) block_count: usize,
    pub(crate) next_sequence: u64,
    pub(crate) state: SourceTurnState,
    pub(crate) events: BTreeMap<u64, SourceTurnEventKind>,
    prompt_blocks: Vec<String>,
    prompt_fragment_indices: Vec<usize>,
    prompt_closed: Vec<bool>,
    assistant: String,
    assistant_fragment_index: usize,
    tools: Vec<ToolRecord>,
    omitted_tool_indices: BTreeSet<usize>,
}

impl SourceTurnDraft {
    pub fn from_batch(batch: &CaptureBatch) -> Result<Self, SourceTurnError> {
        let first = batch.events().first().ok_or(SourceTurnError::EmptyBatch)?;
        let SourceTurnEventKind::Started {
            bridge_turn_id,
            started_at_ms,
            block_count,
        } = first.kind()
        else {
            return Err(SourceTurnError::InvalidOrder);
        };
        if first.sequence() != 0 {
            return Err(SourceTurnError::InvalidSequence);
        }
        let mut draft = Self {
            source_turn_id: first.source_turn_id(),
            session_id: first.session_id().clone(),
            bridge_turn_id: *bridge_turn_id,
            started_at_ms: *started_at_ms,
            finished_at_ms: None,
            block_count: *block_count,
            next_sequence: 0,
            state: SourceTurnState::Incomplete,
            events: BTreeMap::new(),
            prompt_blocks: vec![String::new(); *block_count],
            prompt_fragment_indices: vec![0; *block_count],
            prompt_closed: vec![false; *block_count],
            assistant: String::new(),
            assistant_fragment_index: 0,
            tools: Vec::new(),
            omitted_tool_indices: BTreeSet::new(),
        };
        draft.apply_batch(batch)?;
        Ok(draft)
    }

    pub fn from_events(events: &[SourceTurnEvent]) -> Result<Self, SourceTurnError> {
        let first = events.first().ok_or(SourceTurnError::EmptyBatch)?;
        let mut draft = Self::from_batch(&CaptureBatch::new(vec![first.clone()])?)?;
        for event in &events[1..] {
            draft.apply_batch(&CaptureBatch::new(vec![event.clone()])?)?;
        }
        Ok(draft)
    }

    pub fn apply_batch(&mut self, batch: &CaptureBatch) -> Result<(), SourceTurnError> {
        if batch.source_turn_id() != self.source_turn_id || batch.session_id() != &self.session_id {
            return Err(SourceTurnError::MixedIdentity);
        }
        for event in batch.events() {
            let normalized_kind = redacted_kind(event.kind());
            if event.sequence() < self.next_sequence {
                let existing = self
                    .events
                    .get(&event.sequence())
                    .ok_or(SourceTurnError::InvalidSequence)?;
                if existing != &normalized_kind {
                    return Err(SourceTurnError::ImmutableConflict);
                }
                continue;
            }
            if event.sequence() != self.next_sequence {
                return Err(SourceTurnError::InvalidSequence);
            }
            self.apply_new(event.sequence(), &normalized_kind)?;
            self.events.insert(event.sequence(), normalized_kind);
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .ok_or(SourceTurnError::InvalidSequence)?;
        }
        Ok(())
    }

    fn apply_new(
        &mut self,
        sequence: u64,
        kind: &SourceTurnEventKind,
    ) -> Result<(), SourceTurnError> {
        if !matches!(self.state, SourceTurnState::Incomplete) {
            return Err(SourceTurnError::DuplicateTerminal);
        }
        match kind {
            SourceTurnEventKind::Started {
                bridge_turn_id,
                started_at_ms,
                block_count,
            } => {
                if sequence != 0
                    || *bridge_turn_id != self.bridge_turn_id
                    || *started_at_ms != self.started_at_ms
                    || *block_count != self.block_count
                {
                    return Err(SourceTurnError::InvalidOrder);
                }
            }
            SourceTurnEventKind::PromptFragment {
                block_index,
                fragment_index,
                text,
                is_last,
            } => {
                if *block_index >= self.block_count
                    || *fragment_index != self.prompt_fragment_indices[*block_index]
                    || self.prompt_closed[*block_index]
                {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.prompt_blocks[*block_index].push_str(text);
                self.prompt_fragment_indices[*block_index] = self.prompt_fragment_indices
                    [*block_index]
                    .checked_add(1)
                    .ok_or(SourceTurnError::InvalidSequence)?;
                if *is_last {
                    self.prompt_closed[*block_index] = true;
                }
            }
            SourceTurnEventKind::AssistantFragment {
                fragment_index,
                text,
            } => {
                if *fragment_index != self.assistant_fragment_index {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.assistant.push_str(text);
                self.assistant_fragment_index = self
                    .assistant_fragment_index
                    .checked_add(1)
                    .ok_or(SourceTurnError::InvalidSequence)?;
            }
            SourceTurnEventKind::ToolSnapshot {
                tool_index,
                tool_id,
                name,
                status,
                input,
                result,
            } => self.merge_tool(*tool_index, tool_id, name, status, input, result)?,
            SourceTurnEventKind::Finished {
                disposition,
                finished_at_ms,
            } => {
                if *finished_at_ms < self.started_at_ms
                    || self.prompt_closed.iter().any(|closed| !closed)
                {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.finished_at_ms = Some(*finished_at_ms);
                self.state = SourceTurnState::Finished(*disposition);
            }
        }
        Ok(())
    }

    fn merge_tool(
        &mut self,
        tool_index: usize,
        tool_id: &str,
        name: &str,
        status: &str,
        input: &str,
        result: &str,
    ) -> Result<(), SourceTurnError> {
        if tool_index >= MAX_TOOLS {
            self.omitted_tool_indices.insert(tool_index);
            return Ok(());
        }
        if tool_index > self.tools.len() {
            return Err(SourceTurnError::InvalidOrder);
        }
        let existing_chars = self.tools.get(tool_index).map_or(0, ToolRecord::char_count);
        let used_without_existing = self.total_tool_chars().saturating_sub(existing_chars);
        let remaining_total = MAX_TOTAL_TOOL_CHARS.saturating_sub(used_without_existing);
        let budget = MAX_TOOL_CHARS.min(remaining_total);
        let record = bounded_tool_record(tool_id, name, status, input, result, budget);
        if tool_index == self.tools.len() {
            self.tools.push(record);
        } else {
            if self.tools[tool_index].tool_id != record.tool_id {
                return Err(SourceTurnError::ImmutableConflict);
            }
            self.tools[tool_index] = record;
        }
        Ok(())
    }

    fn total_tool_chars(&self) -> usize {
        self.tools.iter().map(ToolRecord::char_count).sum()
    }

    pub fn original_prompt(&self) -> String {
        self.prompt_blocks.join("\n")
    }

    pub fn assistant_text(&self) -> &str {
        &self.assistant
    }

    pub fn tools_in_order(&self) -> &[ToolRecord] {
        &self.tools
    }

    pub fn canonical_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"cyril-source-turn-v1");
        put_string(&mut hasher, &self.original_prompt());
        put_string(&mut hasher, &self.assistant);
        for tool in &self.tools {
            put_string(&mut hasher, &tool.tool_id);
            put_string(&mut hasher, &tool.name);
            put_string(&mut hasher, &tool.status);
            put_string(&mut hasher, &tool.input);
            put_string(&mut hasher, &tool.result);
            hasher.update((tool.truncated_chars as u64).to_be_bytes());
        }
        hasher.update((self.omitted_tool_indices.len() as u64).to_be_bytes());
        hasher.finalize().into()
    }
}

fn bounded_tool_record(
    tool_id: &str,
    name: &str,
    status: &str,
    input: &str,
    result: &str,
    budget: usize,
) -> ToolRecord {
    let mut remaining = budget;
    let mut truncated_chars = 0;
    let (tool_id, dropped) = take_bounded(tool_id, &mut remaining);
    truncated_chars += dropped;
    let (name, dropped) = take_bounded(name, &mut remaining);
    truncated_chars += dropped;
    let (status, dropped) = take_bounded(status, &mut remaining);
    truncated_chars += dropped;
    let (input, dropped) = take_bounded(input, &mut remaining);
    truncated_chars += dropped;
    let (result, dropped) = take_bounded(result, &mut remaining);
    truncated_chars += dropped;
    ToolRecord {
        tool_id,
        name,
        status,
        input,
        result,
        truncated_chars,
    }
}

fn take_bounded(value: &str, remaining: &mut usize) -> (String, usize) {
    let total = value.chars().count();
    let taken = total.min(*remaining);
    let text = value.chars().take(taken).collect();
    *remaining -= taken;
    (text, total - taken)
}

fn redacted_kind(kind: &SourceTurnEventKind) -> SourceTurnEventKind {
    match kind {
        SourceTurnEventKind::Started {
            bridge_turn_id,
            started_at_ms,
            block_count,
        } => SourceTurnEventKind::Started {
            bridge_turn_id: *bridge_turn_id,
            started_at_ms: *started_at_ms,
            block_count: *block_count,
        },
        SourceTurnEventKind::PromptFragment {
            block_index,
            fragment_index,
            text,
            is_last,
        } => SourceTurnEventKind::PromptFragment {
            block_index: *block_index,
            fragment_index: *fragment_index,
            text: redact(text),
            is_last: *is_last,
        },
        SourceTurnEventKind::AssistantFragment {
            fragment_index,
            text,
        } => SourceTurnEventKind::AssistantFragment {
            fragment_index: *fragment_index,
            text: redact(text),
        },
        SourceTurnEventKind::ToolSnapshot {
            tool_index,
            tool_id,
            name,
            status,
            input,
            result,
        } => SourceTurnEventKind::ToolSnapshot {
            tool_index: *tool_index,
            tool_id: redact(tool_id),
            name: redact(name),
            status: redact(status),
            input: redact(input),
            result: redact(result),
        },
        SourceTurnEventKind::Finished {
            disposition,
            finished_at_ms,
        } => SourceTurnEventKind::Finished {
            disposition: *disposition,
            finished_at_ms: *finished_at_ms,
        },
    }
}

fn put_string(hasher: &mut Sha256, value: &str) {
    let bytes = value.as_bytes();
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureBatch, MAX_TOOL_CHARS, SourceSessionId, SourceTurnDisposition, SourceTurnDraft,
        SourceTurnError, SourceTurnEvent, SourceTurnEventKind, SourceTurnId,
    };

    fn event(
        session: &SourceSessionId,
        source_turn_id: SourceTurnId,
        sequence: u64,
        kind: SourceTurnEventKind,
    ) -> SourceTurnEvent {
        SourceTurnEvent::new(session.clone(), source_turn_id, sequence, kind)
            .expect("valid source event fixture")
    }

    #[test]
    fn c3_source_turn_restart_retry_is_exact_once_and_conflict_safe() {
        let session = SourceSessionId::new("session-1").expect("session identity");
        let source_turn_id = SourceTurnId::from_bytes([7; 16]);
        let first = CaptureBatch::new(vec![
            event(
                &session,
                source_turn_id,
                0,
                SourceTurnEventKind::Started {
                    bridge_turn_id: 9,
                    started_at_ms: 100,
                    block_count: 1,
                },
            ),
            event(
                &session,
                source_turn_id,
                1,
                SourceTurnEventKind::PromptFragment {
                    block_index: 0,
                    fragment_index: 0,
                    text: "password=hunter2 choose the boring path".to_owned(),
                    is_last: true,
                },
            ),
        ])
        .expect("first batch");
        let second = CaptureBatch::new(vec![
            event(
                &session,
                source_turn_id,
                2,
                SourceTurnEventKind::AssistantFragment {
                    fragment_index: 0,
                    text: "done".to_owned(),
                },
            ),
            event(
                &session,
                source_turn_id,
                3,
                SourceTurnEventKind::Finished {
                    disposition: SourceTurnDisposition::Completed,
                    finished_at_ms: 200,
                },
            ),
        ])
        .expect("second batch");

        let mut draft = SourceTurnDraft::from_batch(&first).expect("staged draft");
        draft.apply_batch(&second).expect("completed draft");
        let hash = draft.canonical_hash();
        assert_eq!(
            draft.original_prompt(),
            "password=[REDACTED] choose the boring path",
            "C3 source prompt must be redacted"
        );

        let mut restarted = SourceTurnDraft::from_events(
            &first
                .events()
                .iter()
                .chain(second.events())
                .cloned()
                .collect::<Vec<_>>(),
        )
        .expect("restart reconstruction");
        restarted
            .apply_batch(&first)
            .expect("identical first replay");
        restarted
            .apply_batch(&second)
            .expect("identical second replay");
        assert_eq!(restarted.canonical_hash(), hash, "C3 replay hash");

        let conflict = CaptureBatch::new(vec![event(
            &session,
            source_turn_id,
            1,
            SourceTurnEventKind::PromptFragment {
                block_index: 0,
                fragment_index: 0,
                text: "different".to_owned(),
                is_last: true,
            },
        )])
        .expect("conflicting batch shape");
        assert!(
            matches!(
                restarted.apply_batch(&conflict),
                Err(SourceTurnError::ImmutableConflict)
            ),
            "C3 conflicting replay must not replace immutable source"
        );
        assert_eq!(restarted.canonical_hash(), hash, "C3 conflict hash");
    }

    #[test]
    fn c3_tool_snapshots_are_bounded_and_hashed() {
        let session = SourceSessionId::new("session-tools").expect("session identity");
        let id = SourceTurnId::from_bytes([8; 16]);
        let batch = CaptureBatch::new(vec![
            event(
                &session,
                id,
                0,
                SourceTurnEventKind::Started {
                    bridge_turn_id: 1,
                    started_at_ms: 1,
                    block_count: 1,
                },
            ),
            event(
                &session,
                id,
                1,
                SourceTurnEventKind::PromptFragment {
                    block_index: 0,
                    fragment_index: 0,
                    text: "query".to_owned(),
                    is_last: true,
                },
            ),
            event(
                &session,
                id,
                2,
                SourceTurnEventKind::ToolSnapshot {
                    tool_index: 0,
                    tool_id: "tool-1".to_owned(),
                    name: "read".to_owned(),
                    status: "completed".to_owned(),
                    input: "x".repeat(MAX_TOOL_CHARS),
                    result: "y".repeat(MAX_TOOL_CHARS),
                },
            ),
        ])
        .expect("tool batch");
        let draft = SourceTurnDraft::from_batch(&batch).expect("tool draft");
        let tool = &draft.tools_in_order()[0];
        assert!(tool.char_count() <= MAX_TOOL_CHARS, "C3 per-tool bound");
        assert!(tool.truncated_chars > 0, "C3 truncation metadata");
    }
}

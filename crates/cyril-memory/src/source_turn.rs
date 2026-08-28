//! Source-turn domain: validated identities, normalized capture events,
//! bounded assembly, terminal transitions, canonical hashing, and replay
//! integrity. Nothing here knows about SQL, the wire, or the UI.
//!
//! Every text payload is normalized exactly once, at [`SourceTurnEvent::new`]:
//! CRLF/ANSI noise is stripped, the raw ingress bound is applied, and shared
//! credential redaction runs. Stored data is therefore already normalized and
//! is never re-bounded or re-redacted on reload. Whole-turn redaction runs a
//! second time over the assembled prompt and assistant text when a turn
//! reaches a terminal state (and on every read of an incomplete turn), which
//! closes secrets that straddle a fragment boundary.

use std::collections::BTreeSet;
use std::fmt;
use std::iter::Peekable;
use std::str::{Chars, FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::encoding::decode_fixed_hex;
use crate::redaction::redact;

/// Ingress bound on one raw text payload, measured after sanitization and
/// before redaction. Redaction may lengthen text slightly (`[REDACTED]`
/// replaces shorter values), so stored payloads can exceed this by a small
/// factor; they are never re-checked against it.
pub const MAX_SOURCE_EVENT_TEXT_CHARS: usize = 65_536;
pub const MAX_CAPTURE_EVENTS: usize = 16;
/// Approximate encoded size of one batch after normalization and redaction.
pub const MAX_CAPTURE_BYTES: usize = 256 * 1024;
pub const MAX_PROMPT_BLOCKS: usize = 256;
pub const MAX_TOOLS: usize = 128;
pub const MAX_TOOL_CHARS: usize = 16_000;
pub const MAX_TOTAL_TOOL_CHARS: usize = 256_000;
pub const MAX_TOOL_ID_CHARS: usize = 256;
pub const MAX_TOOL_STATUS_CHARS: usize = 64;
pub const MAX_QUERY_CHARS: usize = 4_096;
pub const MAX_QUERY_TERMS: usize = 64;
pub const MAX_EPISODES: usize = 3;
pub const MAX_EPISODE_CHARS: usize = 1_200;
pub const MAX_EPISODE_TOTAL_CHARS: usize = 3_600;

const HASH_DOMAIN: &[u8] = b"cyril-source-turn-v2";
const DIGEST_DOMAIN: &[u8] = b"cyril-source-event-v1";

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
        decode_fixed_hex::<16>(value)
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

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

/// Stable identity of one tool call inside a source turn. Opaque: it is never
/// redacted or truncated, because it is what replay equality keys on.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceToolId(String);

impl SourceToolId {
    pub fn new(value: impl Into<String>) -> Result<Self, SourceToolIdError> {
        let value = value.into();
        if !Self::is_valid_text(&value) {
            return Err(SourceToolIdError);
        }
        Ok(Self(value))
    }

    fn is_valid_text(value: &str) -> bool {
        !value.is_empty()
            && value.chars().count() <= MAX_TOOL_ID_CHARS
            && !value.chars().any(char::is_control)
    }

    /// Deserialization bypasses [`Self::new`]; event normalization re-checks.
    fn is_valid(&self) -> bool {
        Self::is_valid_text(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceToolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for SourceToolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SourceToolId")
            .field(&self.0)
            .finish()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("tool identity must be non-empty, printable, and at most 256 characters")]
pub struct SourceToolIdError;

/// Free-form first-prompt text normalized into a bounded recall query.
///
/// [`Self::from_prompt`] never fails: control characters (including the
/// newlines of a pasted stack trace) become spaces and the text is cut to
/// [`MAX_QUERY_CHARS`] scalars. The wire accepts only already-normalized
/// queries so a raw frame cannot smuggle an unbounded one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptQuery(String);

impl PromptQuery {
    pub fn from_prompt(text: &str) -> Self {
        Self(
            text.chars()
                .take(MAX_QUERY_CHARS)
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .collect(),
        )
    }

    pub(crate) fn from_wire(text: String) -> Result<Self, PromptQueryError> {
        if text.chars().count() > MAX_QUERY_CHARS || text.chars().any(char::is_control) {
            return Err(PromptQueryError);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("prompt query must be printable and at most 4096 characters")]
pub struct PromptQueryError;

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

    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        Some(match value {
            "completed" => Self::Completed,
            "interrupted" => Self::Interrupted,
            "failed" => Self::Failed,
            "abandoned" => Self::Abandoned,
            "capture_overflow" => Self::CaptureOverflow,
            _ => return None,
        })
    }
}

/// Durable lifecycle state of a source turn: the single vocabulary shared by
/// assembly, storage, the wire, and inspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceTurnStatus {
    Incomplete,
    Finished(SourceTurnDisposition),
}

impl SourceTurnStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Finished(disposition) => disposition.as_str(),
        }
    }

    pub(crate) fn from_stored(value: &str) -> Option<Self> {
        if value == "incomplete" {
            return Some(Self::Incomplete);
        }
        SourceTurnDisposition::from_stored(value).map(Self::Finished)
    }

    /// Only an authoritative end-of-turn completion feeds recall.
    pub const fn is_recall_eligible(self) -> bool {
        matches!(self, Self::Finished(SourceTurnDisposition::Completed))
    }
}

/// One normalized capture event. This is also the wire shape of an event
/// payload; construction through [`SourceTurnEvent::new`] is what makes a
/// decoded value trustworthy.
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
        tool_id: SourceToolId,
        name: String,
        status: String,
        input: String,
        result: String,
        source_truncated_chars: usize,
    },
    Finished {
        disposition: SourceTurnDisposition,
        finished_at_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTurnEvent {
    session_id: SourceSessionId,
    source_turn_id: SourceTurnId,
    sequence: u64,
    kind: SourceTurnEventKind,
}

impl SourceTurnEvent {
    /// Validate, sanitize, bound, and redact one event. The only constructor.
    pub fn new(
        session_id: SourceSessionId,
        source_turn_id: SourceTurnId,
        sequence: u64,
        kind: SourceTurnEventKind,
    ) -> Result<Self, SourceTurnError> {
        Ok(Self {
            session_id,
            source_turn_id,
            sequence,
            kind: normalize_kind(kind)?,
        })
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
            } => tool_id.as_str().len() + name.len() + status.len() + input.len() + result.len(),
        }
    }
}

fn normalize_kind(kind: SourceTurnEventKind) -> Result<SourceTurnEventKind, SourceTurnError> {
    Ok(match kind {
        SourceTurnEventKind::Started {
            bridge_turn_id,
            started_at_ms,
            block_count,
        } => {
            if started_at_ms < 0
                || block_count == 0
                || block_count > MAX_PROMPT_BLOCKS
                || i64::try_from(bridge_turn_id).is_err()
            {
                return Err(SourceTurnError::InvalidEvent);
            }
            SourceTurnEventKind::Started {
                bridge_turn_id,
                started_at_ms,
                block_count,
            }
        }
        SourceTurnEventKind::PromptFragment {
            block_index,
            fragment_index,
            text,
            is_last,
        } => {
            if block_index >= MAX_PROMPT_BLOCKS || fragment_index > MAX_SOURCE_EVENT_TEXT_CHARS {
                return Err(SourceTurnError::InvalidEvent);
            }
            SourceTurnEventKind::PromptFragment {
                block_index,
                fragment_index,
                text: normalized_source_text(&text)?,
                is_last,
            }
        }
        SourceTurnEventKind::AssistantFragment {
            fragment_index,
            text,
        } => {
            if fragment_index > MAX_SOURCE_EVENT_TEXT_CHARS {
                return Err(SourceTurnError::InvalidEvent);
            }
            SourceTurnEventKind::AssistantFragment {
                fragment_index,
                text: normalized_source_text(&text)?,
            }
        }
        SourceTurnEventKind::ToolSnapshot {
            tool_index,
            tool_id,
            name,
            status,
            input,
            result,
            source_truncated_chars,
        } => {
            if !tool_id.is_valid() {
                return Err(SourceTurnError::InvalidEvent);
            }
            let status = sanitize_source_text(&status);
            if status.is_empty() || status.chars().count() > MAX_TOOL_STATUS_CHARS {
                return Err(SourceTurnError::InvalidEvent);
            }
            SourceTurnEventKind::ToolSnapshot {
                tool_index,
                tool_id,
                name: normalized_source_text(&name)?,
                status,
                input: normalized_source_text(&input)?,
                result: normalized_source_text(&result)?,
                source_truncated_chars,
            }
        }
        SourceTurnEventKind::Finished {
            disposition,
            finished_at_ms,
        } => {
            if finished_at_ms < 0 {
                return Err(SourceTurnError::InvalidEvent);
            }
            SourceTurnEventKind::Finished {
                disposition,
                finished_at_ms,
            }
        }
    })
}

/// Sanitize, apply the raw ingress bound, then redact.
fn normalized_source_text(value: &str) -> Result<String, SourceTurnError> {
    let sanitized = sanitize_source_text(value);
    if sanitized.chars().count() > MAX_SOURCE_EVENT_TEXT_CHARS {
        return Err(SourceTurnError::EventTooLarge);
    }
    Ok(redact(&sanitized))
}

/// Normalize line endings to LF, strip ANSI escape sequences, and drop every
/// other control character except LF and TAB. Tool output, terminal streams,
/// and attached files arrive with all of these and none of them is content.
fn sanitize_source_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    output.push('\n');
                }
            }
            '\u{1b}' => skip_escape_sequence(&mut chars),
            '\n' | '\t' => output.push(character),
            other if other.is_control() => {}
            other => output.push(other),
        }
    }
    output
}

/// Consume the remainder of an escape sequence whose ESC was just read: a
/// CSI (`ESC [ … final`), an OSC (`ESC ] … BEL|ESC \`), or a two-character
/// escape. Any control character ends a malformed sequence without being
/// consumed, so a stray ESC cannot swallow the rest of the text.
fn skip_escape_sequence(chars: &mut Peekable<Chars<'_>>) {
    match chars.next() {
        Some('[') => {
            while let Some(&next) = chars.peek() {
                if next.is_control() {
                    break;
                }
                chars.next();
                if ('\u{40}'..='\u{7e}').contains(&next) {
                    break;
                }
            }
        }
        Some(']') => {
            while let Some(&next) = chars.peek() {
                if next == '\u{07}' {
                    chars.next();
                    break;
                }
                if next == '\u{1b}' {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                    }
                    break;
                }
                if next.is_control() {
                    break;
                }
                chars.next();
            }
        }
        // A two-character escape (`ESC c`) is complete; an intermediate byte
        // (`ESC ( B`) is followed by more intermediates and one final byte.
        Some(intermediate) if ('\u{20}'..='\u{2f}').contains(&intermediate) => {
            while let Some(&next) = chars.peek() {
                if next.is_control() {
                    break;
                }
                chars.next();
                if !('\u{20}'..='\u{2f}').contains(&next) {
                    break;
                }
            }
        }
        Some(_) | None => {}
    }
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

/// One merged, bounded tool lifecycle as persisted with the turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolRecord {
    pub(crate) tool_id: SourceToolId,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) input: String,
    pub(crate) result: String,
    /// Unicode scalar values retained across every field, kept so a reload
    /// does not recount the whole tool list.
    pub(crate) retained_chars: usize,
    pub(crate) truncated_chars: usize,
}

/// SHA-256 over one normalized event kind. Replay equality compares digests,
/// so stored turns keep no event text beyond their assembled projection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventDigest([u8; 32]);

impl EventDigest {
    fn of(kind: &SourceTurnEventKind) -> Result<Self, SourceTurnError> {
        let encoded = serde_json::to_vec(kind).map_err(SourceTurnError::Encoding)?;
        let mut hasher = Sha256::new();
        hasher.update(DIGEST_DOMAIN);
        hasher.update(encoded);
        Ok(Self(hasher.finalize().into()))
    }
}

impl fmt::Debug for EventDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "EventDigest({})", hex::encode(self.0))
    }
}

impl Serialize for EventDigest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for EventDigest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        decode_fixed_hex::<32>(&text)
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom("event digest is not 32-byte hexadecimal"))
    }
}

/// Resumable assembly state persisted beside the turn's projections. Text is
/// not duplicated here except prompt blocks, whose boundaries the joined
/// prompt projection cannot recover.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceTurnAssembly {
    prompt_blocks: Vec<String>,
    prompt_fragment_indices: Vec<usize>,
    prompt_closed: Vec<bool>,
    assistant_fragment_index: usize,
    omitted_tool_indices: BTreeSet<usize>,
    event_digests: Vec<EventDigest>,
}

/// Everything the store persists for one turn, in typed form.
pub(crate) struct StoredSourceTurn {
    pub(crate) source_turn_id: SourceTurnId,
    pub(crate) session_id: SourceSessionId,
    pub(crate) bridge_turn_id: u64,
    pub(crate) started_at_ms: i64,
    pub(crate) finished_at_ms: Option<i64>,
    pub(crate) block_count: usize,
    pub(crate) next_sequence: u64,
    pub(crate) status: SourceTurnStatus,
    pub(crate) assistant: String,
    pub(crate) tools: Vec<ToolRecord>,
    pub(crate) assembly: SourceTurnAssembly,
}

/// Columns derived from a draft for storage and indexing.
pub(crate) struct SourceTurnProjection {
    pub(crate) prompt: String,
    pub(crate) assistant: String,
    pub(crate) tools_text: String,
    pub(crate) source_hash: Option<[u8; 32]>,
}

#[derive(Debug, Error)]
pub enum SourceTurnError {
    #[error("source capture batch cannot be empty")]
    EmptyBatch,
    #[error("source capture batch contains too many events")]
    TooManyEvents,
    #[error("source capture batch is too large")]
    BatchTooLarge,
    #[error("source event payload is too large")]
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
    #[error("stored source assembly is inconsistent: {0}")]
    InconsistentAssembly(&'static str),
    #[error("source event could not be encoded")]
    Encoding(#[source] serde_json::Error),
    #[error("source turn identity generation failed")]
    Random(#[source] getrandom::Error),
}

pub(crate) struct SourceTurnDraft {
    source_turn_id: SourceTurnId,
    session_id: SourceSessionId,
    bridge_turn_id: u64,
    started_at_ms: i64,
    finished_at_ms: Option<i64>,
    block_count: usize,
    next_sequence: u64,
    status: SourceTurnStatus,
    assistant: String,
    tools: Vec<ToolRecord>,
    tool_chars: usize,
    assembly: SourceTurnAssembly,
}

struct ToolSnapshotParts<'a> {
    tool_index: usize,
    tool_id: &'a SourceToolId,
    name: &'a str,
    status: &'a str,
    input: &'a str,
    result: &'a str,
    source_truncated_chars: usize,
}

impl SourceTurnDraft {
    /// Open an empty draft from the `Started` header of a turn's first batch.
    /// The batch is NOT applied: the caller applies it exactly once.
    pub(crate) fn begin(batch: &CaptureBatch) -> Result<Self, SourceTurnError> {
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
        Ok(Self {
            source_turn_id: first.source_turn_id(),
            session_id: first.session_id().clone(),
            bridge_turn_id: *bridge_turn_id,
            started_at_ms: *started_at_ms,
            finished_at_ms: None,
            block_count: *block_count,
            next_sequence: 0,
            status: SourceTurnStatus::Incomplete,
            assistant: String::new(),
            tools: Vec::new(),
            tool_chars: 0,
            assembly: SourceTurnAssembly {
                prompt_blocks: vec![String::new(); *block_count],
                prompt_fragment_indices: vec![0; *block_count],
                prompt_closed: vec![false; *block_count],
                assistant_fragment_index: 0,
                omitted_tool_indices: BTreeSet::new(),
                event_digests: Vec::new(),
            },
        })
    }

    /// Resume a persisted draft. Stored data is trusted as already normalized;
    /// only structural consistency is checked.
    pub(crate) fn from_stored(stored: StoredSourceTurn) -> Result<Self, SourceTurnError> {
        let StoredSourceTurn {
            source_turn_id,
            session_id,
            bridge_turn_id,
            started_at_ms,
            finished_at_ms,
            block_count,
            next_sequence,
            status,
            assistant,
            tools,
            assembly,
        } = stored;
        if block_count == 0 || block_count > MAX_PROMPT_BLOCKS {
            return Err(SourceTurnError::InconsistentAssembly(
                "block count out of range",
            ));
        }
        if assembly.prompt_blocks.len() != block_count
            || assembly.prompt_fragment_indices.len() != block_count
            || assembly.prompt_closed.len() != block_count
        {
            return Err(SourceTurnError::InconsistentAssembly(
                "prompt block vectors disagree with block count",
            ));
        }
        if usize::try_from(next_sequence).ok() != Some(assembly.event_digests.len())
            || next_sequence == 0
        {
            return Err(SourceTurnError::InconsistentAssembly(
                "event digests disagree with next sequence",
            ));
        }
        if tools.len() > MAX_TOOLS {
            return Err(SourceTurnError::InconsistentAssembly("too many tools"));
        }
        if finished_at_ms.is_some() != matches!(status, SourceTurnStatus::Finished(_)) {
            return Err(SourceTurnError::InconsistentAssembly(
                "finish time disagrees with status",
            ));
        }
        let tool_chars = tools.iter().map(|tool| tool.retained_chars).sum();
        Ok(Self {
            source_turn_id,
            session_id,
            bridge_turn_id,
            started_at_ms,
            finished_at_ms,
            block_count,
            next_sequence,
            status,
            assistant,
            tools,
            tool_chars,
            assembly,
        })
    }

    /// Apply a batch. Events below `next_sequence` must replay identically
    /// (by digest); the next expected event extends the turn; anything else
    /// is rejected. A failed batch leaves the draft unusable: callers discard
    /// it rather than persisting a partial application.
    pub(crate) fn apply_batch(&mut self, batch: &CaptureBatch) -> Result<(), SourceTurnError> {
        if batch.source_turn_id() != self.source_turn_id || batch.session_id() != &self.session_id {
            return Err(SourceTurnError::MixedIdentity);
        }
        for event in batch.events() {
            let digest = EventDigest::of(event.kind())?;
            if event.sequence() < self.next_sequence {
                let index = usize::try_from(event.sequence())
                    .map_err(|_| SourceTurnError::InvalidSequence)?;
                let existing = self
                    .assembly
                    .event_digests
                    .get(index)
                    .ok_or(SourceTurnError::InvalidSequence)?;
                if *existing != digest {
                    return Err(SourceTurnError::ImmutableConflict);
                }
                continue;
            }
            if event.sequence() != self.next_sequence {
                return Err(SourceTurnError::InvalidSequence);
            }
            self.apply_new(event.sequence(), event.kind())?;
            self.assembly.event_digests.push(digest);
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
        if !matches!(self.status, SourceTurnStatus::Incomplete) {
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
                let index = *block_index;
                if index >= self.block_count
                    || *fragment_index != self.assembly.prompt_fragment_indices[index]
                    || self.assembly.prompt_closed[index]
                {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.assembly.prompt_blocks[index].push_str(text);
                self.assembly.prompt_fragment_indices[index] =
                    self.assembly.prompt_fragment_indices[index]
                        .checked_add(1)
                        .ok_or(SourceTurnError::InvalidSequence)?;
                if *is_last {
                    self.assembly.prompt_closed[index] = true;
                }
            }
            SourceTurnEventKind::AssistantFragment {
                fragment_index,
                text,
            } => {
                if *fragment_index != self.assembly.assistant_fragment_index {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.assistant.push_str(text);
                self.assembly.assistant_fragment_index = self
                    .assembly
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
                source_truncated_chars,
            } => self.merge_tool(ToolSnapshotParts {
                tool_index: *tool_index,
                tool_id,
                name,
                status,
                input,
                result,
                source_truncated_chars: *source_truncated_chars,
            })?,
            SourceTurnEventKind::Finished {
                disposition,
                finished_at_ms,
            } => {
                if *finished_at_ms < self.started_at_ms
                    || self.assembly.prompt_closed.iter().any(|closed| !closed)
                {
                    return Err(SourceTurnError::InvalidOrder);
                }
                self.finished_at_ms = Some(*finished_at_ms);
                self.status = SourceTurnStatus::Finished(*disposition);
            }
        }
        Ok(())
    }

    fn merge_tool(&mut self, snapshot: ToolSnapshotParts<'_>) -> Result<(), SourceTurnError> {
        let ToolSnapshotParts {
            tool_index,
            tool_id,
            name,
            status,
            input,
            result,
            source_truncated_chars,
        } = snapshot;
        if tool_index >= MAX_TOOLS {
            self.assembly.omitted_tool_indices.insert(tool_index);
            return Ok(());
        }
        if tool_index > self.tools.len() {
            return Err(SourceTurnError::InvalidOrder);
        }
        let existing_chars = match self.tools.get(tool_index) {
            Some(existing) if existing.tool_id != *tool_id => {
                return Err(SourceTurnError::ImmutableConflict);
            }
            Some(existing) => existing.retained_chars,
            None => 0,
        };
        let used_without_existing = self.tool_chars.saturating_sub(existing_chars);
        let remaining_total = MAX_TOTAL_TOOL_CHARS.saturating_sub(used_without_existing);
        let budget = MAX_TOOL_CHARS.min(remaining_total);
        let mut record = bounded_tool_record(tool_id, name, status, input, result, budget);
        record.truncated_chars = record
            .truncated_chars
            .saturating_add(source_truncated_chars);
        self.tool_chars = used_without_existing.saturating_add(record.retained_chars);
        if tool_index == self.tools.len() {
            self.tools.push(record);
        } else {
            self.tools[tool_index] = record;
        }
        Ok(())
    }

    pub(crate) const fn source_turn_id(&self) -> SourceTurnId {
        self.source_turn_id
    }

    pub(crate) fn session_id(&self) -> &SourceSessionId {
        &self.session_id
    }

    pub(crate) const fn bridge_turn_id(&self) -> u64 {
        self.bridge_turn_id
    }

    pub(crate) const fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }

    pub(crate) const fn finished_at_ms(&self) -> Option<i64> {
        self.finished_at_ms
    }

    pub(crate) const fn block_count(&self) -> usize {
        self.block_count
    }

    pub(crate) const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub(crate) const fn status(&self) -> SourceTurnStatus {
        self.status
    }

    pub(crate) fn tools(&self) -> &[ToolRecord] {
        &self.tools
    }

    pub(crate) fn omitted_tool_count(&self) -> usize {
        self.assembly.omitted_tool_indices.len()
    }

    pub(crate) fn assembly(&self) -> &SourceTurnAssembly {
        &self.assembly
    }

    /// Prompt and assistant text with whole-turn redaction applied, so a
    /// secret split across fragments is closed regardless of state.
    pub(crate) fn redacted_view(&self) -> (String, String) {
        let prompt = self
            .assembly
            .prompt_blocks
            .iter()
            .map(|block| redact(block))
            .collect::<Vec<_>>()
            .join("\n");
        (prompt, redact(&self.assistant))
    }

    /// What the store persists: an incomplete turn keeps its fragment-level
    /// text (readers re-redact it); a finished turn is written whole-redacted
    /// with its canonical hash, and never changes again.
    pub(crate) fn storage_projection(&self) -> SourceTurnProjection {
        let (prompt, assistant) = match self.status {
            SourceTurnStatus::Incomplete => (
                self.assembly.prompt_blocks.join("\n"),
                self.assistant.clone(),
            ),
            SourceTurnStatus::Finished(_) => self.redacted_view(),
        };
        SourceTurnProjection {
            prompt,
            assistant,
            tools_text: self.tools_text(),
            source_hash: self.canonical_hash(),
        }
    }

    /// Plain text of every tool's name, input, and result, for indexing.
    /// Neither identifiers nor status words are indexed: they are not what a
    /// later prompt talks about.
    fn tools_text(&self) -> String {
        let mut text = String::new();
        for tool in &self.tools {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&tool.name);
            text.push('\n');
            text.push_str(&tool.input);
            text.push('\n');
            text.push_str(&tool.result);
        }
        text
    }

    /// SHA-256 over a versioned, length-prefixed canonical encoding of the
    /// redacted prompt blocks, redacted assistant text, ordered bounded tool
    /// lifecycle with truncation counts, omitted tool count, and the terminal
    /// disposition. `None` until the turn is finished.
    pub(crate) fn canonical_hash(&self) -> Option<[u8; 32]> {
        let SourceTurnStatus::Finished(disposition) = self.status else {
            return None;
        };
        let (prompt_blocks, assistant) = (
            self.assembly
                .prompt_blocks
                .iter()
                .map(|block| redact(block))
                .collect::<Vec<_>>(),
            redact(&self.assistant),
        );
        let mut hasher = Sha256::new();
        hasher.update(HASH_DOMAIN);
        put_len(&mut hasher, prompt_blocks.len());
        for block in &prompt_blocks {
            put_string(&mut hasher, block);
        }
        put_string(&mut hasher, &assistant);
        put_len(&mut hasher, self.tools.len());
        for tool in &self.tools {
            put_string(&mut hasher, tool.tool_id.as_str());
            put_string(&mut hasher, &tool.name);
            put_string(&mut hasher, &tool.status);
            put_string(&mut hasher, &tool.input);
            put_string(&mut hasher, &tool.result);
            put_len(&mut hasher, tool.truncated_chars);
        }
        put_len(&mut hasher, self.assembly.omitted_tool_indices.len());
        put_string(&mut hasher, disposition.as_str());
        Some(hasher.finalize().into())
    }
}

fn bounded_tool_record(
    tool_id: &SourceToolId,
    name: &str,
    status: &str,
    input: &str,
    result: &str,
    budget: usize,
) -> ToolRecord {
    let mut remaining = budget;
    let mut retained_chars = tool_id.as_str().chars().count();
    let mut truncated_chars = 0;
    let mut take = |value: &str| {
        let (text, kept, dropped) = take_bounded(value, remaining);
        remaining -= kept;
        retained_chars += kept;
        truncated_chars += dropped;
        text
    };
    let name = take(name);
    let status = take(status);
    let input = take(input);
    let result = take(result);
    ToolRecord {
        tool_id: tool_id.clone(),
        name,
        status,
        input,
        result,
        retained_chars,
        truncated_chars,
    }
}

/// `(text, kept, dropped)` for the first `limit` scalars of `value`.
fn take_bounded(value: &str, limit: usize) -> (String, usize, usize) {
    let total = value.chars().count();
    let kept = total.min(limit);
    (value.chars().take(kept).collect(), kept, total - kept)
}

fn put_string(hasher: &mut Sha256, value: &str) {
    let bytes = value.as_bytes();
    put_len(hasher, bytes.len());
    hasher.update(bytes);
}

fn put_len(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used)]

    use super::{
        CaptureBatch, MAX_QUERY_CHARS, MAX_SOURCE_EVENT_TEXT_CHARS, MAX_TOOL_CHARS, PromptQuery,
        SourceSessionId, SourceToolId, SourceTurnDisposition, SourceTurnDraft, SourceTurnError,
        SourceTurnEvent, SourceTurnEventKind, SourceTurnId, SourceTurnStatus, StoredSourceTurn,
        sanitize_source_text,
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

    fn started(session: &SourceSessionId, id: SourceTurnId, block_count: usize) -> SourceTurnEvent {
        event(
            session,
            id,
            0,
            SourceTurnEventKind::Started {
                bridge_turn_id: 9,
                started_at_ms: 100,
                block_count,
            },
        )
    }

    fn prompt(
        session: &SourceSessionId,
        id: SourceTurnId,
        sequence: u64,
        block_index: usize,
        fragment_index: usize,
        text: &str,
        is_last: bool,
    ) -> SourceTurnEvent {
        event(
            session,
            id,
            sequence,
            SourceTurnEventKind::PromptFragment {
                block_index,
                fragment_index,
                text: text.to_owned(),
                is_last,
            },
        )
    }

    fn assistant(
        session: &SourceSessionId,
        id: SourceTurnId,
        sequence: u64,
        fragment_index: usize,
        text: &str,
    ) -> SourceTurnEvent {
        event(
            session,
            id,
            sequence,
            SourceTurnEventKind::AssistantFragment {
                fragment_index,
                text: text.to_owned(),
            },
        )
    }

    fn finished(
        session: &SourceSessionId,
        id: SourceTurnId,
        sequence: u64,
        disposition: SourceTurnDisposition,
    ) -> SourceTurnEvent {
        event(
            session,
            id,
            sequence,
            SourceTurnEventKind::Finished {
                disposition,
                finished_at_ms: 200,
            },
        )
    }

    fn stored(draft: &SourceTurnDraft) -> StoredSourceTurn {
        let projection = draft.storage_projection();
        StoredSourceTurn {
            source_turn_id: draft.source_turn_id(),
            session_id: draft.session_id().clone(),
            bridge_turn_id: draft.bridge_turn_id(),
            started_at_ms: draft.started_at_ms(),
            finished_at_ms: draft.finished_at_ms(),
            block_count: draft.block_count(),
            next_sequence: draft.next_sequence(),
            status: draft.status(),
            assistant: projection.assistant,
            tools: draft.tools().to_vec(),
            assembly: draft.assembly().clone(),
        }
    }

    #[test]
    fn c3_source_turn_restart_retry_is_exact_once_and_conflict_safe() {
        let session = SourceSessionId::new("session-1").expect("session identity");
        let id = SourceTurnId::from_bytes([7; 16]);
        let first = CaptureBatch::new(vec![
            started(&session, id, 1),
            prompt(
                &session,
                id,
                1,
                0,
                0,
                "password=hunter2 choose the boring path",
                true,
            ),
        ])
        .expect("first batch");
        let second = CaptureBatch::new(vec![
            assistant(&session, id, 2, 0, "done"),
            finished(&session, id, 3, SourceTurnDisposition::Completed),
        ])
        .expect("second batch");

        let mut draft = SourceTurnDraft::begin(&first).expect("opened draft");
        assert_eq!(
            draft.next_sequence(),
            0,
            "C3 begin must not apply the batch"
        );
        draft.apply_batch(&first).expect("staged draft");
        assert_eq!(draft.next_sequence(), 2);
        assert!(
            draft.canonical_hash().is_none(),
            "C3 no hash before terminal"
        );
        draft.apply_batch(&second).expect("completed draft");
        let hash = draft.canonical_hash().expect("terminal hash");
        assert_eq!(
            draft.redacted_view().0,
            "password=[REDACTED] choose the boring path",
            "C3 source prompt must be redacted"
        );

        let mut restarted = SourceTurnDraft::from_stored(stored(&draft)).expect("reload");
        restarted
            .apply_batch(&first)
            .expect("identical first replay");
        restarted
            .apply_batch(&second)
            .expect("identical second replay");
        assert_eq!(restarted.canonical_hash(), Some(hash), "C3 replay hash");
        assert_eq!(restarted.next_sequence(), 4);

        let conflict = CaptureBatch::new(vec![prompt(&session, id, 1, 0, 0, "different", true)])
            .expect("conflicting batch shape");
        assert!(
            matches!(
                restarted.apply_batch(&conflict),
                Err(SourceTurnError::ImmutableConflict)
            ),
            "C3 conflicting replay must not replace immutable source"
        );
        assert_eq!(restarted.canonical_hash(), Some(hash), "C3 conflict hash");

        let after_terminal =
            CaptureBatch::new(vec![assistant(&session, id, 4, 1, "late")]).expect("shape");
        assert!(matches!(
            restarted.apply_batch(&after_terminal),
            Err(SourceTurnError::DuplicateTerminal)
        ));
    }

    #[test]
    fn c6_secrets_straddling_fragment_boundaries_are_closed_on_assembly() {
        let session = SourceSessionId::new("session-c6").expect("session identity");
        let id = SourceTurnId::from_bytes([6; 16]);
        let batch = CaptureBatch::new(vec![
            started(&session, id, 1),
            prompt(&session, id, 1, 0, 0, "password=hun", false),
            prompt(&session, id, 2, 0, 1, "ter2 please", true),
            assistant(&session, id, 3, 0, "use token=ghp_abcdefghijkl"),
            assistant(&session, id, 4, 1, "mnopqrstuvwxyz123456 ok"),
            finished(&session, id, 5, SourceTurnDisposition::Completed),
        ])
        .expect("batch");
        let mut draft = SourceTurnDraft::begin(&batch).expect("draft");
        draft.apply_batch(&batch).expect("applied");
        let projection = draft.storage_projection();
        assert!(
            !projection.prompt.contains("hunter2"),
            "C6 prompt leaked: {}",
            projection.prompt
        );
        assert!(
            !projection.assistant.contains("mnopqrstuvwxyz123456"),
            "C6 assistant leaked: {}",
            projection.assistant
        );
        // An incomplete turn is stored fragment-redacted; its read view is not.
        let mut incomplete = SourceTurnDraft::begin(&batch).expect("draft");
        incomplete
            .apply_batch(&CaptureBatch::new(batch.events()[..3].to_vec()).expect("prefix batch"))
            .expect("staged");
        assert!(!incomplete.redacted_view().0.contains("hunter2"));
    }

    #[test]
    fn c6_redaction_growth_near_the_ingress_cap_survives_reload() {
        let session = SourceSessionId::new("session-cap").expect("session identity");
        let id = SourceTurnId::from_bytes([5; 16]);
        let raw = "token=a1 ".repeat(7_281);
        assert!(raw.chars().count() <= MAX_SOURCE_EVENT_TEXT_CHARS);
        let first = CaptureBatch::new(vec![
            started(&session, id, 1),
            prompt(&session, id, 1, 0, 0, &raw, true),
        ])
        .expect("first batch");
        let stored_len = match first.events()[1].kind() {
            SourceTurnEventKind::PromptFragment { text, .. } => text.chars().count(),
            other => panic!("unexpected kind {other:?}"),
        };
        assert!(
            stored_len > MAX_SOURCE_EVENT_TEXT_CHARS,
            "C6 growth premise"
        );
        let oversized = "x".repeat(MAX_SOURCE_EVENT_TEXT_CHARS + 1);
        assert!(matches!(
            SourceTurnEvent::new(
                session.clone(),
                id,
                1,
                SourceTurnEventKind::AssistantFragment {
                    fragment_index: 0,
                    text: oversized,
                },
            ),
            Err(SourceTurnError::EventTooLarge)
        ));

        let mut draft = SourceTurnDraft::begin(&first).expect("draft");
        draft.apply_batch(&first).expect("staged");
        let mut reloaded = SourceTurnDraft::from_stored(stored(&draft)).expect("C6 reload");
        reloaded
            .apply_batch(
                &CaptureBatch::new(vec![finished(
                    &session,
                    id,
                    2,
                    SourceTurnDisposition::Completed,
                )])
                .expect("terminal batch"),
            )
            .expect("C6 terminal after reload");
        assert!(reloaded.canonical_hash().is_some());
        reloaded
            .apply_batch(&first)
            .expect("C6 replay after reload");
    }

    #[test]
    fn c6_crlf_and_ansi_noise_is_normalized_not_rejected() {
        assert_eq!(sanitize_source_text("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(
            sanitize_source_text("\u{1b}[31merror\u{1b}[0m \u{1b}]0;title\u{07}x \u{1b}(B"),
            "error x "
        );
        assert_eq!(
            sanitize_source_text("keep\ttab\u{0}drop\u{7f}"),
            "keep\ttabdrop"
        );
        assert_eq!(sanitize_source_text("stray \u{1b}[\nnext"), "stray \nnext");
        let session = SourceSessionId::new("session-ansi").expect("session identity");
        let id = SourceTurnId::from_bytes([4; 16]);
        let snapshot = event(
            &session,
            id,
            2,
            SourceTurnEventKind::ToolSnapshot {
                tool_index: 0,
                tool_id: SourceToolId::new("t1").expect("tool id"),
                name: "execute_bash".to_owned(),
                status: "completed\r\n".to_owned(),
                input: "cargo build".to_owned(),
                result: "   Compiling foo\r\n\u{1b}[1mFinished\u{1b}[0m".to_owned(),
                source_truncated_chars: 0,
            },
        );
        match snapshot.kind() {
            SourceTurnEventKind::ToolSnapshot { status, result, .. } => {
                assert_eq!(status, "completed\n");
                assert_eq!(result, "   Compiling foo\nFinished");
            }
            other => panic!("unexpected kind {other:?}"),
        }
        assert!(matches!(
            SourceTurnEvent::new(
                session,
                id,
                2,
                SourceTurnEventKind::ToolSnapshot {
                    tool_index: 0,
                    tool_id: SourceToolId::new("t1").expect("tool id"),
                    name: String::new(),
                    status: "\u{1b}[0m".to_owned(),
                    input: String::new(),
                    result: String::new(),
                    source_truncated_chars: 0,
                },
            ),
            Err(SourceTurnError::InvalidEvent)
        ));
    }

    #[test]
    fn c3_tool_snapshots_are_bounded_hashed_and_identity_checked() {
        let session = SourceSessionId::new("session-tools").expect("session identity");
        let id = SourceTurnId::from_bytes([8; 16]);
        let tool = |sequence: u64, tool_id: &str| {
            event(
                &session,
                id,
                sequence,
                SourceTurnEventKind::ToolSnapshot {
                    tool_index: 0,
                    tool_id: SourceToolId::new(tool_id).expect("tool id"),
                    name: "read".to_owned(),
                    status: "completed".to_owned(),
                    input: "x".repeat(MAX_TOOL_CHARS),
                    result: "y".repeat(MAX_TOOL_CHARS),
                    source_truncated_chars: 0,
                },
            )
        };
        let batch = CaptureBatch::new(vec![
            started(&session, id, 1),
            prompt(&session, id, 1, 0, 0, "query", true),
            tool(2, "tool-1"),
        ])
        .expect("tool batch");
        let mut draft = SourceTurnDraft::begin(&batch).expect("tool draft");
        draft.apply_batch(&batch).expect("applied");
        let record = &draft.tools()[0];
        assert!(record.retained_chars <= MAX_TOOL_CHARS + "tool-1".len());
        assert!(record.truncated_chars > 0, "C3 truncation metadata");
        assert_eq!(draft.tool_chars, record.retained_chars);
        let conflict = CaptureBatch::new(vec![tool(3, "tool-2")]).expect("shape");
        assert!(matches!(
            draft.apply_batch(&conflict),
            Err(SourceTurnError::ImmutableConflict)
        ));
        assert!(SourceToolId::new("").is_err());
        assert!(SourceToolId::new("bad\u{0}").is_err());
    }

    #[test]
    fn c3_canonical_hash_covers_block_structure_and_disposition() {
        let session = SourceSessionId::new("session-hash").expect("session identity");
        let id = SourceTurnId::from_bytes([3; 16]);
        let complete = |blocks: &[&str], disposition| {
            let mut events = vec![started(&session, id, blocks.len())];
            for (index, block) in blocks.iter().enumerate() {
                let sequence = u64::try_from(index + 1).expect("small index");
                events.push(prompt(&session, id, sequence, index, 0, block, true));
            }
            let sequence = u64::try_from(events.len()).expect("small index");
            events.push(finished(&session, id, sequence, disposition));
            let batch = CaptureBatch::new(events).expect("batch");
            let mut draft = SourceTurnDraft::begin(&batch).expect("draft");
            draft.apply_batch(&batch).expect("applied");
            draft.canonical_hash().expect("finished hash")
        };
        let joined = complete(&["a\nb"], SourceTurnDisposition::Completed);
        let split = complete(&["a", "b"], SourceTurnDisposition::Completed);
        let interrupted = complete(&["a\nb"], SourceTurnDisposition::Interrupted);
        assert_ne!(joined, split, "C3 block boundaries must be hashed");
        assert_ne!(joined, interrupted, "C3 disposition must be hashed");
        assert_eq!(
            joined,
            complete(&["a\nb"], SourceTurnDisposition::Completed)
        );
    }

    #[test]
    fn prompt_query_normalizes_free_text_and_wire_stays_strict() {
        let pasted = format!(
            "fix this:\n\tstack trace\r\n{}",
            "x".repeat(MAX_QUERY_CHARS)
        );
        let query = PromptQuery::from_prompt(&pasted);
        assert_eq!(query.as_str().chars().count(), MAX_QUERY_CHARS);
        assert!(query.as_str().starts_with("fix this:  stack trace  x"));
        assert!(!query.as_str().chars().any(char::is_control));
        assert!(PromptQuery::from_wire(query.as_str().to_owned()).is_ok());
        assert!(PromptQuery::from_wire("a\nb".to_owned()).is_err());
        assert!(PromptQuery::from_wire("x".repeat(MAX_QUERY_CHARS + 1)).is_err());
        assert_eq!(PromptQuery::from_prompt("").as_str(), "");
    }

    #[test]
    fn stored_turn_consistency_is_checked_on_reload() {
        let session = SourceSessionId::new("session-reload").expect("session identity");
        let id = SourceTurnId::from_bytes([2; 16]);
        let batch = CaptureBatch::new(vec![
            started(&session, id, 1),
            prompt(&session, id, 1, 0, 0, "hello", true),
        ])
        .expect("batch");
        let mut draft = SourceTurnDraft::begin(&batch).expect("draft");
        draft.apply_batch(&batch).expect("applied");
        let mut broken = stored(&draft);
        broken.next_sequence = 5;
        assert!(matches!(
            SourceTurnDraft::from_stored(broken),
            Err(SourceTurnError::InconsistentAssembly(_))
        ));
        let mut broken = stored(&draft);
        broken.status = SourceTurnStatus::Finished(SourceTurnDisposition::Completed);
        assert!(matches!(
            SourceTurnDraft::from_stored(broken),
            Err(SourceTurnError::InconsistentAssembly(_))
        ));
        assert!(SourceTurnDraft::from_stored(stored(&draft)).is_ok());
        assert_eq!(
            SourceTurnStatus::from_stored("completed"),
            Some(SourceTurnStatus::Finished(SourceTurnDisposition::Completed))
        );
        assert_eq!(
            SourceTurnStatus::from_stored("incomplete"),
            Some(SourceTurnStatus::Incomplete)
        );
        assert_eq!(SourceTurnStatus::from_stored("other"), None);
        assert!(SourceTurnStatus::Finished(SourceTurnDisposition::Completed).is_recall_eligible());
        assert!(!SourceTurnStatus::Finished(SourceTurnDisposition::Failed).is_recall_eligible());
    }

    #[test]
    fn event_bounds_reject_unrepresentable_headers() {
        let session = SourceSessionId::new("session-bounds").expect("session identity");
        let id = SourceTurnId::from_bytes([1; 16]);
        assert!(matches!(
            SourceTurnEvent::new(
                session,
                id,
                0,
                SourceTurnEventKind::Started {
                    bridge_turn_id: u64::MAX,
                    started_at_ms: 1,
                    block_count: 1,
                },
            ),
            Err(SourceTurnError::InvalidEvent)
        ));
    }
    #[test]
    fn c6_stream_tool_tail_assembles_without_thoughts_or_secrets() {
        let session = SourceSessionId::new("session-redaction").expect("session identity");
        let id = SourceTurnId::from_bytes([9; 16]);
        let secret = "ghp_abcdefghijklmnopqrstuvwxyz123456";
        let batch = CaptureBatch::new(vec![
            event(
                &session,
                id,
                0,
                SourceTurnEventKind::Started {
                    bridge_turn_id: 9,
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
                    text: format!("prompt token: {secret}"),
                    is_last: true,
                },
            ),
            event(
                &session,
                id,
                2,
                SourceTurnEventKind::AssistantFragment {
                    fragment_index: 0,
                    text: format!("assistant token: {secret}"),
                },
            ),
            event(
                &session,
                id,
                3,
                SourceTurnEventKind::ToolSnapshot {
                    tool_index: 0,
                    tool_id: SourceToolId::new("tool-1").expect("tool id"),
                    name: "read".to_owned(),
                    status: "completed".to_owned(),
                    input: format!("token={secret}"),
                    result: format!("token={secret}"),
                    source_truncated_chars: 0,
                },
            ),
        ])
        .expect("redaction batch");
        let mut draft = SourceTurnDraft::begin(&batch).expect("redacted draft");
        draft.apply_batch(&batch).expect("applied redacted batch");
        let (prompt, assistant) = draft.redacted_view();
        let combined = format!(
            "{}\n{}\n{}\n{}",
            prompt,
            assistant,
            draft.tools()[0].input,
            draft.tools()[0].result,
        );
        assert!(
            !combined.contains(secret),
            "C6 secret persisted: {combined}"
        );
        assert!(
            combined.contains("[REDACTED]"),
            "C6 redaction marker missing"
        );
    }
}

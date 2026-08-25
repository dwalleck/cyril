use std::fmt;

use crate::types::{SessionId, ToolCallStatus};

pub const SOURCE_EVENT_CHANNEL_CAPACITY: usize = 32;
pub const SOURCE_FRAGMENT_BYTES: usize = 64 * 1024;

/// Original prompt blocks plus optional opaque memory context. The bridge may
/// prepend the context on wire, but source capture can only see `original_blocks`.
#[derive(Debug)]
pub struct PromptEnvelope {
    original_blocks: Vec<String>,
    prepared_context: Option<String>,
}

impl PromptEnvelope {
    pub fn original(original_blocks: Vec<String>) -> Self {
        Self {
            original_blocks,
            prepared_context: None,
        }
    }

    pub fn prepared(original_blocks: Vec<String>, prepared_context: Option<String>) -> Self {
        Self {
            original_blocks,
            prepared_context,
        }
    }

    pub fn original_blocks(&self) -> &[String] {
        &self.original_blocks
    }

    pub fn into_wire_blocks(mut self) -> Vec<String> {
        if let Some(context) = self.prepared_context {
            self.original_blocks.insert(0, context);
        }
        self.original_blocks
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceTurnId([u8; 16]);

impl SourceTurnId {
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Display for SourceTurnId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for SourceTurnId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SourceTurnId({self})")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceTurnDisposition {
    Completed,
    Interrupted,
    Failed,
    Abandoned,
    CaptureOverflow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceTurnEvent {
    session_id: SessionId,
    source_turn_id: SourceTurnId,
    sequence: u64,
    kind: SourceTurnEventKind,
}

impl SourceTurnEvent {
    pub(crate) fn new(
        session_id: SessionId,
        source_turn_id: SourceTurnId,
        sequence: u64,
        kind: SourceTurnEventKind,
    ) -> Self {
        Self {
            session_id,
            source_turn_id,
            sequence,
            kind,
        }
    }
    #[cfg(feature = "test-support")]
    pub fn for_tests(
        session_id: SessionId,
        source_turn_id: SourceTurnId,
        sequence: u64,
        kind: SourceTurnEventKind,
    ) -> Self {
        Self::new(session_id, source_turn_id, sequence, kind)
    }

    pub fn session_id(&self) -> &SessionId {
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
    pub fn into_kind(self) -> SourceTurnEventKind {
        self.kind
    }
}

pub(crate) fn tool_status(status: ToolCallStatus) -> &'static str {
    match status {
        ToolCallStatus::InProgress => "in_progress",
        ToolCallStatus::Pending => "pending",
        ToolCallStatus::Completed => "completed",
        ToolCallStatus::Failed => "failed",
    }
}

//! Owned chronological messages with stable retained positions and coalesced changes.
//!
//! Positions remain stable through append, edit and prefix eviction. Clear and
//! prepend start a new epoch; revisions from another owner or epoch request a
//! reset rather than accidentally reusing prepared presentation data.

use std::collections::{BTreeMap, btree_map};
use std::ops::{Bound, Range};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::traits::ChatMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MessagePosition(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageRevision(u64);

/// An opaque cursor belonging to exactly one history owner and reset epoch.
/// The default cursor is invalid and always requests a complete reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistoryRevision {
    identity: u64,
    epoch: u64,
    sequence: u64,
}

/// Independent revisions for the two live presentation sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LiveRevision {
    pub text: u64,
    pub thought: u64,
}

impl LiveRevision {
    pub(crate) fn bump_text(&mut self) {
        let Some(next) = self.text.checked_add(1) else {
            panic!("live text revision exhausted");
        };
        self.text = next;
    }

    pub(crate) fn bump_thought(&mut self) {
        let Some(next) = self.thought.checked_add(1) else {
            panic!("live thought revision exhausted");
        };
        self.thought = next;
    }
}

pub struct HistoryChanges<'a> {
    pub reset: bool,
    pub retained: Range<MessagePosition>,
    pub changed: ChangedPositions<'a>,
}

/// Borrows the latest-change index; repeated edits never allocate a change list.
pub struct ChangedPositions<'a> {
    entries: btree_map::Range<'a, u64, MessagePosition>,
}

impl Iterator for ChangedPositions<'_> {
    type Item = MessagePosition;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next().map(|(_, position)| *position)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

/// The sole mutable owner of committed display messages.
///
/// Intentionally not `Clone`: moving a stream preserves ownership, while making
/// a new history from cloned message data must receive a distinct identity.
#[derive(Debug)]
pub struct MessageHistory {
    messages: Vec<ChatMessage>,
    revisions: Vec<MessageRevision>,
    latest_changes: BTreeMap<u64, MessagePosition>,
    start: usize,
    revision: HistoryRevision,
}

impl MessageHistory {
    pub fn new() -> Self {
        static NEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);
        let identity =
            match NEXT_IDENTITY.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            }) {
                Ok(identity) => identity,
                Err(_) => panic!("message history identities exhausted"),
            };
        Self {
            messages: Vec::new(),
            revisions: Vec::new(),
            latest_changes: BTreeMap::new(),
            start: 0,
            revision: HistoryRevision {
                identity,
                epoch: 0,
                sequence: 0,
            },
        }
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn revision(&self) -> HistoryRevision {
        self.revision
    }

    pub fn retained_range(&self) -> Range<MessagePosition> {
        let Some(end) = self.start.checked_add(self.messages.len()) else {
            panic!("message positions exhausted");
        };
        MessagePosition(self.start)..MessagePosition(end)
    }

    pub fn get(&self, position: MessagePosition) -> Option<(&ChatMessage, MessageRevision)> {
        let index = position.0.checked_sub(self.start)?;
        self.messages
            .get(index)
            .map(|message| (message, self.revisions[index]))
    }

    pub fn changed_since(&self, revision: HistoryRevision) -> HistoryChanges<'_> {
        let reset = revision.identity != self.revision.identity
            || revision.epoch != self.revision.epoch
            || revision.sequence > self.revision.sequence;
        let lower = if reset {
            Bound::Unbounded
        } else {
            Bound::Excluded(revision.sequence)
        };
        HistoryChanges {
            reset,
            retained: self.retained_range(),
            changed: ChangedPositions {
                entries: self.latest_changes.range((lower, Bound::Unbounded)),
            },
        }
    }

    fn next_revision(&mut self) -> MessageRevision {
        let Some(next) = self.revision.sequence.checked_add(1) else {
            panic!("message history revision exhausted");
        };
        self.revision.sequence = next;
        MessageRevision(self.revision.sequence)
    }

    pub(crate) fn push(&mut self, message: ChatMessage) -> MessagePosition {
        let position = self.retained_range().end;
        assert!(position.0 < usize::MAX, "message positions exhausted");
        let revision = self.next_revision();
        self.messages.push(message);
        self.revisions.push(revision);
        self.latest_changes.insert(revision.0, position);
        position
    }

    pub(crate) fn edit(
        &mut self,
        position: MessagePosition,
        edit: impl FnOnce(&mut ChatMessage),
    ) -> bool {
        let Some(index) = position
            .0
            .checked_sub(self.start)
            .filter(|&index| index < self.messages.len())
        else {
            return false;
        };
        let revision = self.next_revision();
        self.latest_changes.remove(&self.revisions[index].0);
        self.revisions[index] = revision;
        self.latest_changes.insert(revision.0, position);
        edit(&mut self.messages[index]);
        true
    }

    pub(crate) fn drop_prefix(&mut self, count: usize) {
        let count = count.min(self.messages.len());
        if count == 0 {
            return;
        }
        for revision in self.revisions.drain(..count) {
            self.latest_changes.remove(&revision.0);
        }
        self.messages.drain(..count);
        self.start += count;
        self.next_revision();
    }

    pub(crate) fn clear(&mut self) {
        self.start = self.retained_range().end.0;
        self.messages.clear();
        self.revisions.clear();
        self.latest_changes.clear();
        self.reset_epoch();
    }

    fn reset_epoch(&mut self) {
        let Some(next) = self.revision.epoch.checked_add(1) else {
            panic!("message history epoch exhausted");
        };
        self.revision.epoch = next;
        self.next_revision();
    }

    pub(crate) fn prepend(&mut self, mut earlier: MessageHistory) {
        if earlier.messages.is_empty() {
            return;
        }
        earlier.messages.append(&mut self.messages);
        self.messages = earlier.messages;
        self.start = 0;
        self.revisions.clear();
        self.latest_changes.clear();
        self.reset_epoch();
        self.revisions.reserve(self.messages.len());
        for index in 0..self.messages.len() {
            let revision = self.next_revision();
            self.revisions.push(revision);
            self.latest_changes
                .insert(revision.0, MessagePosition(index));
        }
    }
}

impl Default for MessageHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<ChatMessage>> for MessageHistory {
    fn from(messages: Vec<ChatMessage>) -> Self {
        let mut history = Self::new();
        history.messages = messages;
        history.revisions.reserve(history.messages.len());
        for index in 0..history.messages.len() {
            let revision = history.next_revision();
            history.revisions.push(revision);
            history
                .latest_changes
                .insert(revision.0, MessagePosition(index));
        }
        history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ChatMessageKind;

    fn message(text: &str) -> ChatMessage {
        ChatMessage::agent_text(text.to_owned())
    }

    #[test]
    fn edit_metadata_stays_bounded_across_repeated_edits_and_evictions() {
        let mut history = MessageHistory::new();
        for index in 0..1_000 {
            let position = history.push(message("row"));
            for _ in 0..4 {
                history.edit(position, |message| {
                    message.kind = ChatMessageKind::AgentText("changed".into())
                });
            }
            history.drop_prefix(history.messages().len().saturating_sub(3));
            assert_eq!(history.latest_changes.len(), history.messages().len());
            assert_eq!(history.revisions.len(), history.messages().len());
            assert_eq!(history.messages().len(), (index + 1).min(3));
        }
        let invalid = HistoryRevision {
            sequence: u64::MAX,
            ..history.revision()
        };
        assert!(history.changed_since(invalid).reset);
        history.clear();
        assert!(history.latest_changes.is_empty());
        assert!(history.revisions.is_empty());
    }

    #[test]
    fn new_default_and_copied_data_have_distinct_owners() {
        let first = MessageHistory::new();
        let second = MessageHistory::default();
        let original = MessageHistory::from(vec![message("copied source")]);
        let copied = MessageHistory::from(original.messages().to_vec());
        assert!(first.messages().is_empty());
        assert_eq!(
            first.retained_range(),
            MessagePosition(0)..MessagePosition(0)
        );
        assert!(first.changed_since(HistoryRevision::default()).reset);
        assert!(second.changed_since(first.revision()).reset);
        assert!(copied.changed_since(original.revision()).reset);
        assert_eq!(copied.messages().len(), 1);
        assert_ne!(first.revision(), HistoryRevision::default());
        let original_revision = original.revision();
        let moved = original;
        assert_eq!(
            moved.revision(),
            original_revision,
            "a move preserves ownership"
        );
    }

    #[test]
    fn append_and_edit_expose_borrowed_data_and_coalesce_latest_changes() {
        let mut history = MessageHistory::new();
        let initial = history.revision();
        let a = history.push(message("a"));
        let b = history.push(message("b"));
        let a_revision = history.get(a).map(|(_, revision)| revision);
        assert!(a_revision.is_some());
        let before_edit = history.revision();
        for _ in 0..1_000 {
            assert!(history.edit(a, |message| message.kind =
                ChatMessageKind::AgentText("edited".into())));
        }
        assert_eq!(history.latest_changes.len(), 2);
        assert_eq!(history.revisions.len(), 2);
        assert_ne!(history.get(a).map(|(_, revision)| revision), a_revision);
        let changes = history.changed_since(initial);
        assert!(!changes.reset);
        assert_eq!(changes.retained, a..MessagePosition(b.0 + 1));
        assert_eq!(changes.changed.collect::<Vec<_>>(), [b, a]);
        assert_eq!(
            history
                .changed_since(before_edit)
                .changed
                .collect::<Vec<_>>(),
            [a]
        );
        assert!(
            history
                .changed_since(history.revision())
                .changed
                .next()
                .is_none()
        );
        assert!(
            matches!(history.get(a).map(|(message, _)| message.kind()), Some(ChatMessageKind::AgentText(text)) if text == "edited")
        );
        assert!(
            history
                .get(a)
                .is_some_and(|(message, _)| { std::ptr::eq(message, &history.messages()[0]) }),
            "get borrows the authoritative message allocation"
        );
        assert!(history.get(MessagePosition(99)).is_none());
        let revision = history.revision();
        assert!(!history.edit(MessagePosition(99), |_| panic!("must not run")));
        assert_eq!(history.revision(), revision);
    }

    #[test]
    fn pruning_preserves_surviving_positions_and_revisions() {
        let mut history = MessageHistory::from(vec![message("a"), message("b"), message("c")]);
        let b = MessagePosition(1);
        let b_revision = history.get(b).map(|(_, revision)| revision);
        assert!(b_revision.is_some());
        let before = history.revision();
        history.drop_prefix(0);
        assert_eq!(history.revision(), before);
        history.drop_prefix(1);
        assert!(!history.changed_since(before).reset);
        assert!(history.changed_since(before).changed.next().is_none());
        assert_ne!(history.revision(), before);
        assert!(history.get(MessagePosition(0)).is_none());
        let after_prune = history.revision();
        assert!(!history.edit(MessagePosition(0), |_| panic!(
            "retired row must not be edited"
        )));
        assert_eq!(history.revision(), after_prune);
        assert_eq!(history.get(b).map(|(_, revision)| revision), b_revision);
        assert_eq!(history.latest_changes.len(), 2);
        let d = history.push(message("d"));
        assert_eq!(d, MessagePosition(3));
        history.drop_prefix(usize::MAX);
        assert_eq!(
            history.retained_range(),
            MessagePosition(4)..MessagePosition(4)
        );
        assert!(history.latest_changes.is_empty());
        assert!(history.revisions.is_empty());
        let empty_revision = history.revision();
        history.drop_prefix(1);
        assert_eq!(history.revision(), empty_revision);
        assert_eq!(history.push(message("e")), MessagePosition(4));
    }

    #[test]
    fn clear_resets_epoch_and_never_reuses_retired_positions() {
        let mut history = MessageHistory::from(vec![message("old")]);
        let before = history.revision();
        history.clear();
        assert!(history.changed_since(before).reset);
        assert!(history.latest_changes.is_empty());
        assert!(history.get(MessagePosition(0)).is_none());
        assert_eq!(history.push(message("new")), MessagePosition(1));
        let before = history.revision();
        history.clear();
        history.clear();
        assert!(history.changed_since(before).reset);
    }

    #[test]
    fn prepend_moves_messages_and_resets_only_for_nonempty_donor() {
        let mut history = MessageHistory::from(vec![message("later")]);
        let original = history.revision();
        history.prepend(MessageHistory::new());
        assert_eq!(history.revision(), original);
        let earlier = MessageHistory::from(vec![message("earlier")]);
        let donor_revision = earlier.revision();
        history.prepend(earlier);
        assert!(history.changed_since(original).reset);
        assert!(history.changed_since(donor_revision).reset);
        assert_eq!(
            history.retained_range(),
            MessagePosition(0)..MessagePosition(2)
        );
        assert_eq!(
            history.changed_since(original).changed.collect::<Vec<_>>(),
            [MessagePosition(0), MessagePosition(1)]
        );
        assert!(
            matches!(history.messages()[0].kind(), ChatMessageKind::AgentText(text) if text == "earlier")
        );
        assert!(
            matches!(history.messages()[1].kind(), ChatMessageKind::AgentText(text) if text == "later")
        );
        assert_eq!(history.latest_changes.len(), history.messages().len());
        let mut empty = MessageHistory::new();
        let before_empty = empty.revision();
        empty.prepend(MessageHistory::from(vec![message("only donor")]));
        assert!(empty.changed_since(before_empty).reset);
        assert_eq!(empty.messages().len(), 1);
        assert!(empty.get(empty.retained_range().start).is_some());
    }

    #[test]
    fn live_revisions_advance_independently() {
        let mut live = LiveRevision::default();
        live.bump_text();
        assert_eq!(
            live,
            LiveRevision {
                text: 1,
                thought: 0
            }
        );
        live.bump_thought();
        assert_eq!(
            live,
            LiveRevision {
                text: 1,
                thought: 1
            }
        );
    }
}

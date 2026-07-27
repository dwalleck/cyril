//! Per-turn ownership identity (cyril-a71q).
//!
//! A KAS turn can be terminated by two independent signals — the wire
//! `session_info_update{kind:"turn_end"}` and the `session/prompt` RPC response.
//! Before this type, the only thing distinguishing one turn from another was the
//! session id, which is *not* per-turn unique: turn A's late completion matched a
//! newly-started turn B and wrongly cleared it.
//!
//! [`TurnId`] is that missing identity, and [`TurnAllocator`] mints it.

use std::fmt;

/// Identity of one accepted turn. Allocated at dispatch, never reused.
///
/// Newtype per the CLAUDE.md rule — a raw `u64` here would be interchangeable
/// with counts, indices and every other integer in the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TurnId(u64);

impl TurnId {
    /// Construct from a raw value. Tests and the allocator only — production
    /// code obtains ids from [`TurnAllocator::allocate`] so uniqueness is
    /// centrally enforced.
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn#{}", self.0)
    }
}

/// Mints strictly increasing [`TurnId`]s, and refuses rather than repeat.
///
/// # Contract
///
/// **No value is ever issued twice.** This is load-bearing for correctness, not
/// a sanity hint: a reissued id makes a stale completion match a live turn,
/// which is exactly the defect cyril-a71q exists to fix. Enforcement is a
/// runtime `checked_add` that survives release builds — a `debug_assert!` would
/// compile out and turn the contract into a fiction.
///
/// On exhaustion [`allocate`](Self::allocate) returns `None` **forever** (fail
/// closed). It deliberately does not:
///
/// - wrap to `0` (`wrapping_add`) — silently recreates live owners;
/// - pin at `u64::MAX` (`saturating_add`) — silently reissues one owner forever.
///
/// The second is the subtle one, and it is the idiom used elsewhere in this
/// crate (`kas::terminal_io`, `SessionContext::turn_count`), where repetition is
/// harmless. Here it is not, so this type diverges deliberately.
///
/// Exhaustion is unreachable in practice — a session issues on the order of
/// 10^3 ids — so the branch exists to fail loudly rather than silently if the
/// unreachable happens.
#[derive(Debug, Default)]
pub struct TurnAllocator {
    next: u64,
    exhausted: bool,
}

impl TurnAllocator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next id, or `None` once the space is exhausted.
    ///
    /// Once `None` is returned it is returned for every subsequent call: an
    /// allocator that resumed issuing after exhaustion would reissue live ids.
    pub fn allocate(&mut self) -> Option<TurnId> {
        if self.exhausted {
            return None;
        }
        let id = TurnId(self.next);
        match self.next.checked_add(1) {
            Some(next) => self.next = next,
            // `next` was u64::MAX: this id is the last one the space contains.
            None => self.exhausted = true,
        }
        Some(id)
    }

    /// Test-only constructor for exercising the exhaustion boundary without
    /// 2^64 allocations. `pub` (not `pub(crate)`) because the bridge harness in
    /// the binary crate is a separate compilation unit and cannot see
    /// `pub(crate)` items — see CLAUDE.md on lib/bin test visibility.
    pub fn starting_at(next: u64) -> Self {
        Self {
            next,
            exhausted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_allocator_issues_zero_one_two_with_no_gaps() {
        let mut a = TurnAllocator::new();
        let got: Vec<u64> = (0..3)
            .filter_map(|_| a.allocate())
            .map(TurnId::get)
            .collect();
        assert_eq!(got, vec![0, 1, 2]);
    }

    #[test]
    fn ids_are_never_reissued() {
        let mut a = TurnAllocator::new();
        let ids: Vec<TurnId> = (0..1000).filter_map(|_| a.allocate()).collect();
        let unique: std::collections::HashSet<TurnId> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 1000, "allocator stopped early");
        assert_eq!(unique.len(), ids.len(), "an id was issued twice");
        assert!(
            ids.windows(2).all(|w| w[0] < w[1]),
            "not strictly monotonic"
        );
    }

    /// STRESS FIXTURE (plan slice 1). Expected outcome written before the
    /// implementation: allocation N succeeds and yields `u64::MAX`; N+1 fails
    /// closed; the allocator neither wraps to 0 nor reissues `u64::MAX`.
    #[test]
    fn turn_owner_exhaustion_fails_closed() {
        let mut a = TurnAllocator::starting_at(u64::MAX - 1);

        assert_eq!(a.allocate().map(TurnId::get), Some(u64::MAX - 1));
        assert_eq!(a.allocate().map(TurnId::get), Some(u64::MAX));

        // The two bug classes, asserted as explicit negatives so a future
        // refactor to wrapping_add/saturating_add fails here rather than in
        // production.
        let after = a.allocate();
        assert_eq!(after, None, "must fail closed, not wrap or saturate");
        assert_ne!(after.map(TurnId::get), Some(0), "wrapping_add regression");
        assert_ne!(
            after.map(TurnId::get),
            Some(u64::MAX),
            "saturating_add regression"
        );

        // Exhaustion is permanent — a resuming allocator would reissue live ids.
        assert_eq!(a.allocate(), None, "exhaustion must be sticky");
    }

    /// Emits the boundary sequence for the independent oracle
    /// (`.cyril-a71q/probes/alloc_oracle.py --check`). Run with `--nocapture`;
    /// the oracle compares this against its own from-scratch model, so
    /// agreement is evidence rather than the implementation checking itself.
    #[test]
    fn emit_boundary_sequence_for_oracle() {
        let mut a = TurnAllocator::starting_at(u64::MAX - 1);
        println!("ALLOC-SEQ-BEGIN");
        for _ in 0..3 {
            match a.allocate() {
                Some(id) => println!("{}", id.get()),
                None => println!("EXHAUSTED"),
            }
        }
        println!("ALLOC-SEQ-END");
    }

    #[test]
    fn distinct_ids_are_not_equal() {
        assert_ne!(TurnId::new(0), TurnId::new(1));
        assert_eq!(TurnId::new(7), TurnId::new(7));
    }
}

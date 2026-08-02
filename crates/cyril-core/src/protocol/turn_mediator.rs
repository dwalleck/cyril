//! Turn mediation (cyril-b4y4): the bridge policy deciding what each inbound
//! notification does to the active turn — forward it, release the turn,
//! absorb a companion terminal, or drop it (stale or unowned).
//!
//! This is a pure, synchronous state machine in the `SessionController`
//! mold: no async, no channels, no I/O beyond tracing. The bridge `run_loop`
//! remains the single place terminals are *observed* (ADR-0004); this module
//! owns what an observation *means*. Terminal-source authority is an Engine
//! fact (CONTEXT.md "Turn-end"): the engine answers
//! [`Engine::emits_wire_turn_end`](crate::protocol::engine::Engine::emits_wire_turn_end)
//! once per dispatch, and the answer is snapshotted on the turn record —
//! this module never names an engine kind (the enum-match pattern ADR-0001
//! rejected).
//!
//! Vocabulary is pinned in CONTEXT.md: "Turn owner", "Companion terminal",
//! "Turn mediation".

use crate::types::SessionId;
use crate::types::turn::{TurnAllocator, TurnId};

/// The turn currently occupying the bridge (cyril-a71q).
///
/// Holds the per-turn `owner` identity, the terminal-source shape the bound
/// engine declared at dispatch, and the session the turn was dispatched on.
/// `session` is deliberately a snapshot: the loop's `active_session_id` can
/// be retargeted mid-turn by a `NewSession`/`LoadSession`, and cancel must
/// still reach the turn that is actually running.
///
/// `expects_wire_terminal` is likewise a dispatch-time snapshot of
/// `Engine::emits_wire_turn_end()` (cyril-upjh): the release decision reads
/// *the turn's* properties, and a release that consulted a live engine
/// handle instead would silently become wrong the day an engine is rebound.
#[derive(Debug)]
struct ActiveTurn {
    owner: TurnId,
    expects_wire_terminal: bool,
    session: SessionId,
}

/// Outcome of [`TurnMediator::begin_turn`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BeginTurn {
    /// The turn was accepted and recorded; the id is its owner identity.
    Accepted(TurnId),
    /// A turn is already in flight (ADR-0004: at most one). Checked BEFORE
    /// allocation, so a busy mediator with an exhausted allocator reports
    /// `Busy` — the caller-actionable condition — and burns no id.
    Busy,
    /// The turn identity space is exhausted (cyril-a71q C8): refuse the turn
    /// rather than run one whose completions could match somebody else's.
    Exhausted,
}

/// Pure state machine for turn ownership mediation (cyril-b4y4).
///
/// Held and driven solely by the bridge `run_loop`; `pub(crate)` so no other
/// crate can reach it (compile-fenced, design claim C7).
#[derive(Debug, Default)]
pub(crate) struct TurnMediator {
    active: Option<ActiveTurn>,
    alloc: TurnAllocator,
}

impl TurnMediator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Test-only: seed the allocator so exhaustion cells are drivable without
    /// 2^64 allocations (cyril-ns0o's mediator half; the bridge-level fence
    /// through `run_loop` stays tracked there).
    #[cfg(test)]
    fn with_allocator(alloc: TurnAllocator) -> Self {
        Self {
            active: None,
            alloc,
        }
    }

    /// Gate and record a prompt dispatch. Order is load-bearing: the busy
    /// guard runs before allocation (see [`BeginTurn::Busy`]).
    pub(crate) fn begin_turn(
        &mut self,
        session: SessionId,
        expects_wire_terminal: bool,
    ) -> BeginTurn {
        if self.active.is_some() {
            return BeginTurn::Busy;
        }
        let Some(owner) = self.alloc.allocate() else {
            return BeginTurn::Exhausted;
        };
        self.active = Some(ActiveTurn {
            owner,
            expects_wire_terminal,
            session,
        });
        BeginTurn::Accepted(owner)
    }

    /// The in-flight turn's own session — the dispatch-time snapshot, immune
    /// to mid-turn `active_session_id` retargeting (cyril-84ca / ADR-0004).
    /// `None` when no turn is in flight; the loop falls back to
    /// `active_session_id` there.
    pub(crate) fn cancel_target(&self) -> Option<&SessionId> {
        self.active.as_ref().map(|t| &t.session)
    }

    /// Is a turn in flight? Drives the SendPrompt busy-guard and the io-death
    /// defer-vs-exit decision in the loop.
    pub(crate) fn is_busy(&self) -> bool {
        self.active.is_some()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    /// Probe cells p1–p2 (`.cyril-b4y4/probes/probe-output.txt`): a fresh
    /// mediator accepts turn#0, then refuses a second prompt while busy —
    /// and the record carries exactly what dispatch snapshotted (the fields
    /// `observe()` will key releases and companion registration off).
    #[test]
    fn accepts_then_busy() {
        let mut m = TurnMediator::new();
        assert_eq!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert!(m.is_busy());
        let recorded = m.active.as_ref().expect("a turn was just accepted");
        assert_eq!(recorded.owner, TurnId::new(0));
        assert!(recorded.expects_wire_terminal);
        assert_eq!(recorded.session, sid("s"));
        assert_eq!(m.begin_turn(sid("s"), true), BeginTurn::Busy);
    }

    /// STRESS FIXTURE (plan S2a): busy AND exhausted must report `Busy`.
    /// Buggy implementation this fails under: allocate-before-guard reorder,
    /// which burns the last id and reports `Exhausted` for a condition the
    /// caller could fix by waiting.
    #[test]
    fn busy_wins_over_exhaustion() {
        let mut m = TurnMediator::with_allocator(TurnAllocator::starting_at(u64::MAX));
        assert_eq!(
            m.begin_turn(sid("s"), false),
            BeginTurn::Accepted(TurnId::new(u64::MAX)),
            "the last id in the space is still issuable"
        );
        assert_eq!(m.begin_turn(sid("s"), false), BeginTurn::Busy);
    }

    /// STRESS FIXTURE (plan S2c): an exhausted allocator fails closed while
    /// idle — `Exhausted`, sticky, no turn recorded.
    #[test]
    fn exhaustion_fails_closed_while_idle() {
        let mut drained = TurnAllocator::starting_at(u64::MAX);
        assert!(drained.allocate().is_some(), "consume the final id");
        let mut m = TurnMediator::with_allocator(drained);
        assert_eq!(m.begin_turn(sid("s"), true), BeginTurn::Exhausted);
        assert!(!m.is_busy(), "a refused turn must not occupy the bridge");
        assert_eq!(m.begin_turn(sid("s"), true), BeginTurn::Exhausted);
    }

    /// STRESS FIXTURE (plan S2b, idle/busy halves): cancel targets the
    /// dispatch-time session snapshot; idle has no target. (The post-release
    /// half of the snapshot cell lands with `observe()` in slice 4's matrix —
    /// release does not exist yet in this slice.)
    #[test]
    fn cancel_target_is_dispatch_snapshot() {
        let mut m = TurnMediator::new();
        assert_eq!(m.cancel_target(), None);
        assert_eq!(
            m.begin_turn(sid("s1"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(m.cancel_target(), Some(&sid("s1")));
    }
}

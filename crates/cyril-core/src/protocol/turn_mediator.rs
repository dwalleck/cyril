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

use crate::types::event::{Notification, RoutedNotification};
use crate::types::turn::{TurnAllocator, TurnId};
use crate::types::{SessionId, StopReason};

/// Which terminal source the mediator is still expecting for a released turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompanionSource {
    /// The KAS wire `session_info_update{kind:"turn_end"}` — identity-free, so
    /// it can only be matched by session.
    Wire,
    /// The completion the bridge synthesizes from the prompt RPC — carries its
    /// owner, so it is matched by id.
    Synthesized,
}

impl CompanionSource {
    /// The other terminal source of the same turn — what a release still owes
    /// after this one arrived.
    fn counterpart(self) -> Self {
        match self {
            Self::Wire => Self::Synthesized,
            Self::Synthesized => Self::Wire,
        }
    }
}

/// One terminal signal's observation — which source arrived, carrying what
/// stop reason. The cyril-pnwb evidence unit: an absorption reports two of
/// these (first arrival, then the companion) and deliberately selects no
/// precedence between them — that decision belongs to cyril-pnwb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalEvidence {
    pub(crate) source: CompanionSource,
    pub(crate) reason: StopReason,
}

/// The one companion terminal still owed for a turn that already released
/// (cyril-a71q C6; CONTEXT.md "Companion terminal").
///
/// A KAS turn has two terminal sources. The first to arrive releases the turn
/// (first-source-wins, retained from cyril-j16p); the second is not a duplicate
/// to be dropped blindly but an *expected* signal to be absorbed — absorbing is
/// what lets both `{source, reason}` observations be recorded for cyril-pnwb
/// instead of the second one being lost.
///
/// At most one entry exists at any time: registering a new expectation replaces
/// a dangling one, so the ledger is bounded by construction rather than by a
/// cleanup pass.
#[derive(Debug)]
struct Companion {
    owner: TurnId,
    session: SessionId,
    awaiting: CompanionSource,
    /// Evidence of the signal that already arrived; its counterpart is
    /// reported alongside it on absorption.
    first: TerminalEvidence,
}

impl Companion {
    /// The single registration site (design C8): a release leaves behind an
    /// expectation for the arrived source's counterpart, with the arrival
    /// recorded as evidence.
    fn after_arrival(
        owner: TurnId,
        session: SessionId,
        arrived: CompanionSource,
        reason: StopReason,
    ) -> Self {
        Self {
            owner,
            session,
            awaiting: arrived.counterpart(),
            first: TerminalEvidence {
                source: arrived,
                reason,
            },
        }
    }
}

/// What one observed notification means for the active turn (cyril-b4y4).
///
/// The three non-forward outcomes are distinct variants deliberately: absorb
/// and drop both leave nothing on the notification channel, which is exactly
/// the observational blindness cyril-ri8q filed — the variant IS the seam
/// that makes them distinguishable.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Disposition {
    /// Pass through untouched — non-terminals, and terminals for sessions
    /// other than the active turn's (their own consumer needs them).
    Forward,
    /// Pass through; the active turn released on this signal. The loop's
    /// deferred-disconnect handling keys off this variant.
    ForwardTurnComplete,
    /// Pass through; a turn the AGENT started on the main session (cyril-lki9
    /// C7 — a KAS workflow auto-wake, or any `turn_start` cyril did not
    /// dispatch) is now the active turn. The loop arms turn liveness for it,
    /// exactly as it does for an accepted dispatch.
    BeginServerTurn,
    /// The expected companion terminal — consumed, not forwarded. Carries the
    /// cyril-pnwb `{source, reason}` evidence pair for both signals.
    Absorb {
        owner: TurnId,
        first: TerminalEvidence,
        second: TerminalEvidence,
    },
    /// An owner-stamped terminal matching neither the companion ledger nor
    /// the active turn — a stale duplicate. Dropped.
    DropStale { stale: TurnId },
    /// An identity-free terminal with no active turn and nothing owed.
    /// Dropped — forwarding would make the App commit streaming and metering
    /// a second time.
    DropUnowned,
}

/// The turn currently occupying the bridge (cyril-a71q).
///
/// Holds the per-turn `owner` identity, the terminal-source shape the bound
/// engine declared at dispatch (a server turn's only terminal is the wire
/// `turn_end`), the session the turn runs on, and who began it.
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
    origin: TurnOrigin,
    /// Whether this turn's wire `turn_start` has been observed (cyril-lki9
    /// C8). A dispatched turn opens unbracketed; the first `turn_start` on its
    /// session is attached to it — its own start, or a wake KAS began just
    /// before cyril's prompt reached it (the wake is then pre-empted and its
    /// `turn_end{cancelled}` releases this turn, leaving the prompt's own
    /// bracket to begin a server turn). A server turn opens bracketed.
    bracket_open: bool,
}

/// Who began the active turn (cyril-lki9). ADR-0004's single active turn no
/// longer implies "cyril dispatched it": KAS starts turns on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnOrigin {
    /// cyril's own `SendPrompt`: two terminal sources (the prompt RPC result
    /// and, on KAS, the wire `turn_end`).
    Dispatched,
    /// The agent's own `turn_start` with no dispatch in flight: the wire
    /// `turn_end` is its only terminal — there is no prompt RPC to answer it.
    Server,
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
    /// cyril-a71q C6: at most ONE outstanding companion expectation. See
    /// [`Companion`].
    companion: Option<Companion>,
    /// cyril-lki9 C10: sessions whose CURRENT turn has already been announced
    /// as agent-initiated. Cleared for a session at its turn start and turn
    /// end, so each turn announces once. Bounded by the sessions with an
    /// agent-initiated turn in flight (observed ≤ 2: main + one woken step).
    announced: std::collections::HashSet<SessionId>,
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
            companion: None,
            announced: std::collections::HashSet::new(),
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
            origin: TurnOrigin::Dispatched,
            bracket_open: false,
        });
        BeginTurn::Accepted(owner)
    }

    /// The in-flight turn's own session — the dispatch-time snapshot, immune
    /// to mid-turn `active_session_id` retargeting (cyril-84ca / ADR-0004).
    /// `None` when no turn is in flight; the loop falls back to
    /// `active_session_id` there.
    pub(crate) fn active_turn_session(&self) -> Option<&SessionId> {
        self.active.as_ref().map(|t| &t.session)
    }

    /// Is a turn in flight? Drives the SendPrompt busy-guard and the io-death
    /// defer-vs-exit decision in the loop.
    pub(crate) fn is_busy(&self) -> bool {
        self.active.is_some()
    }

    /// What does this observed notification mean for the active turn?
    ///
    /// ABSORB-FIRST in both arms: checking the ledger before the active turn
    /// is what makes single drift safe — a stale signal is absorbed by the
    /// expectation it belongs to instead of clearing the newer turn
    /// (cyril-a71q falsifier mutation M3 fails the other order; probe cell
    /// f9). Non-terminal notifications pass through untouched, except a wire
    /// `TurnStarted` (cyril-lki9 C7/C8/C9): on `main` with no turn in flight it
    /// begins a server turn; on the dispatched turn's session it opens that
    /// turn's bracket. `main` is the loop's current main session.
    pub(crate) fn observe(
        &mut self,
        routed: &RoutedNotification,
        main: Option<&SessionId>,
    ) -> Disposition {
        if matches!(routed.notification, Notification::TurnStarted) {
            if let Some(session) = &routed.session_id {
                self.announced.remove(session);
            }
            return self.observe_turn_start(routed.session_id.as_ref(), main);
        }
        if matches!(routed.notification, Notification::TurnCompleted { .. })
            && let Some(session) = &routed.session_id
        {
            self.announced.remove(session);
        }
        let Notification::TurnCompleted {
            stop_reason: reason,
        } = routed.notification
        else {
            return Disposition::Forward;
        };
        match routed.turn {
            // Owner-stamped: synthesized by the bridge itself, matched by id.
            Some(id) => {
                if let Some(d) =
                    self.absorb_if(|c| c.awaiting == CompanionSource::Synthesized && c.owner == id)
                {
                    return d.second(CompanionSource::Synthesized, reason);
                }
                match self.active.as_ref() {
                    Some(active) if active.owner == id => {
                        // cyril-upjh: only a turn whose engine emits a wire
                        // turn_end still owes one. Assigning None on the
                        // other branch is the clear: the ledger must be
                        // empty after a single-terminal release, or any
                        // later unstamped completion for this session is
                        // eaten as a phantom companion.
                        self.companion = active.expects_wire_terminal.then(|| {
                            Companion::after_arrival(
                                active.owner,
                                active.session.clone(),
                                CompanionSource::Synthesized,
                                reason,
                            )
                        });
                        tracing::debug!(owner = %id, "turn completed");
                        self.active = None;
                        Disposition::ForwardTurnComplete
                    }
                    other => {
                        tracing::debug!(
                            stale_owner = %id,
                            active = ?other.map(|t| t.owner),
                            "dropping stale completion"
                        );
                        Disposition::DropStale { stale: id }
                    }
                }
            }
            // Identity-free: the KAS wire `turn_end`. The frame is
            // `{kind, stopReason}` with no execution id — confirmed against
            // the KAS emitter, not inferred. Matched by session.
            None => {
                if let Some(d) = self.absorb_if(|c| {
                    c.awaiting == CompanionSource::Wire
                        && routed.session_id.as_ref() == Some(&c.session)
                }) {
                    return d.second(CompanionSource::Wire, reason);
                }
                match self.active.as_ref() {
                    // Scoped to the ACTIVE turn's own session -> release. A
                    // dispatched turn still owes its synthesized twin (the
                    // prompt RPC result), registered here. A server turn has
                    // no prompt RPC, so it registers nothing — and it must
                    // not CLEAR the ledger either: after a wake pre-empted
                    // cyril's dispatch, the dispatch's synthesized twin is
                    // still owed while the prompt's own turn runs as a server
                    // turn (race `D@0` in the C8 model), and wiping it here
                    // would drop that response as stale instead of absorbing
                    // it. The owed entry is id-matched, so it cannot eat a
                    // wire terminal.
                    Some(active) if routed.session_id.as_ref() == Some(&active.session) => {
                        if active.origin == TurnOrigin::Dispatched {
                            self.companion = Some(Companion::after_arrival(
                                active.owner,
                                active.session.clone(),
                                CompanionSource::Wire,
                                reason,
                            ));
                        }
                        tracing::debug!(
                            owner = %active.owner,
                            origin = ?active.origin,
                            "turn completed (wire turn_end)"
                        );
                        self.active = None;
                        Disposition::ForwardTurnComplete
                    }
                    // cyril-a71q C3: a FOREIGN session's terminal. Forward it
                    // once so that session's own consumer (a subagent stream)
                    // sees it, but touch nothing on the main turn — the
                    // cross-session split-brain was this signal clearing the
                    // main busy guard.
                    Some(active) => {
                        tracing::debug!(
                            foreign = ?routed.session_id,
                            active_owner = %active.owner,
                            "forwarding foreign terminal; main turn untouched"
                        );
                        Disposition::Forward
                    }
                    // No active turn, nothing owed: a late or duplicate
                    // terminal for a turn that already ended. Logged now
                    // (cyril-b4y4 probe finding 1 — this was the one silent
                    // disposition) and dropped.
                    None => {
                        tracing::debug!(
                            scope = ?routed.session_id,
                            "dropping unowned terminal — no active turn, nothing owed"
                        );
                        Disposition::DropUnowned
                    }
                }
            }
        }
    }

    /// Should the bridge announce this frame's turn as agent-initiated before
    /// forwarding it (cyril-lki9 C10)? `true` exactly once per session per
    /// turn: on the first frame carrying an `origin` since that session's turn
    /// start (or since it was last announced and ended). Frames without an
    /// origin, or with no session scope, never announce.
    pub(crate) fn announce(&mut self, routed: &RoutedNotification) -> bool {
        match (&routed.origin, &routed.session_id) {
            (Some(_), Some(session)) => self.announced.insert(session.clone()),
            _ => false,
        }
    }

    /// A wire `turn_start` (cyril-lki9 C7/C8/C9). Never a terminal and never
    /// dropped: every arm forwards, and only the main-session idle arm changes
    /// ownership.
    fn observe_turn_start(
        &mut self,
        session: Option<&SessionId>,
        main: Option<&SessionId>,
    ) -> Disposition {
        let Some(session) = session else {
            tracing::debug!("TurnStarted with no session scope; forwarded");
            return Disposition::Forward;
        };
        match self.active.as_mut() {
            None if main == Some(session) => {
                let Some(owner) = self.alloc.allocate() else {
                    // Same refusal as `BeginTurn::Exhausted`: never run a turn
                    // whose completions could match somebody else's.
                    tracing::warn!(session = %session, "turn id space exhausted; server turn not tracked");
                    return Disposition::Forward;
                };
                self.active = Some(ActiveTurn {
                    owner,
                    expects_wire_terminal: true,
                    session: session.clone(),
                    origin: TurnOrigin::Server,
                    bracket_open: true,
                });
                tracing::debug!(owner = %owner, session = %session, "server turn began (wire TurnStarted)");
                Disposition::BeginServerTurn
            }
            Some(active) if &active.session == session && !active.bracket_open => {
                active.bracket_open = true;
                tracing::debug!(owner = %active.owner, "TurnStarted attached to dispatched turn");
                Disposition::Forward
            }
            Some(active) if &active.session == session => {
                // KAS pre-empts rather than nests; a second start inside an
                // open bracket is unexpected wire drift, not a new owner.
                tracing::warn!(owner = %active.owner, "TurnStarted inside an open turn bracket; forwarded");
                Disposition::Forward
            }
            // A foreign session's turn (workflow step, subagent) or a main
            // turn_start while another session's turn is active: never
            // touches the main turn.
            _ => Disposition::Forward,
        }
    }

    /// The single absorb site (design C8): if the outstanding expectation
    /// matches, consume it and report the evidence pair. The `second` half of
    /// the pair is filled in by the caller via [`PendingAbsorb::second`] —
    /// only the caller knows which source just arrived.
    fn absorb_if(&mut self, matches: impl FnOnce(&Companion) -> bool) -> Option<PendingAbsorb> {
        if self.companion.as_ref().is_some_and(matches) {
            self.companion.take().map(|c| PendingAbsorb {
                owner: c.owner,
                first: c.first,
            })
        } else {
            None
        }
    }
}

/// An absorb decision waiting for its second `{source, reason}` half.
struct PendingAbsorb {
    owner: TurnId,
    first: TerminalEvidence,
}

impl PendingAbsorb {
    fn second(self, source: CompanionSource, reason: StopReason) -> Disposition {
        let second = TerminalEvidence { source, reason };
        tracing::debug!(
            owner = %self.owner,
            first = ?self.first,
            second = ?second,
            "absorbed expected companion"
        );
        Disposition::Absorb {
            owner: self.owner,
            first: self.first,
            second,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn sid(s: &str) -> SessionId {
        SessionId::new(s)
    }

    fn end_turn() -> Notification {
        Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        }
    }

    /// Every fixture terminal carries `EndTurn`, so evidence asserts only
    /// vary by source.
    fn ev(source: CompanionSource) -> TerminalEvidence {
        TerminalEvidence {
            source,
            reason: StopReason::EndTurn,
        }
    }

    /// The KAS wire `turn_end` shape: identity-free, session-scoped.
    fn wire_end(s: &str) -> RoutedNotification {
        RoutedNotification::scoped(sid(s), end_turn())
    }

    /// cyril-14ou C6 fence: `TurnStalled` is information, never a terminal.
    /// The mediator must Forward it untouched in every turn phase — active
    /// turn, no turn, and while a companion is owed — leaving the busy guard
    /// and the ledger exactly as they were. Buggy implementation this fails
    /// under: adding TurnStalled to the terminal match (early release), or
    /// absorbing it as a companion (eats the ledger entry).
    #[test]
    fn stalled_is_forwarded_nonterminal() {
        let stalled = |s: &str| {
            RoutedNotification::scoped(
                sid(s),
                Notification::TurnStalled {
                    quiet: std::time::Duration::from_secs(31),
                },
            )
        };

        // (a) mid-turn on the active session: forwarded, still busy.
        let mut m = TurnMediator::new();
        assert!(matches!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(_)
        ));
        assert_eq!(m.observe(&stalled("s"), None), Disposition::Forward);
        assert!(m.is_busy(), "stall must not release the turn");

        // (b) no active turn: forwarded (not dropped as unowned).
        let mut idle = TurnMediator::new();
        assert_eq!(idle.observe(&stalled("s"), None), Disposition::Forward);
        assert!(!idle.is_busy());

        // (c) between a wire turn_end and its owed companion: forwarded, and
        // the companion expectation survives to absorb the real twin.
        let mut owed = TurnMediator::new();
        let BeginTurn::Accepted(owner) = owed.begin_turn(sid("s"), true) else {
            panic!("begin_turn refused");
        };
        assert_eq!(
            owed.observe(&wire_end("s"), None),
            Disposition::ForwardTurnComplete
        );
        assert_eq!(owed.observe(&stalled("s"), None), Disposition::Forward);
        let twin = RoutedNotification::global(end_turn()).with_turn(owner);
        assert!(
            matches!(owed.observe(&twin, None), Disposition::Absorb { .. }),
            "companion must still be owed after a stall passed through"
        );
    }

    /// The bridge-synthesized shape: owner-stamped, global scope.
    fn stamped(n: u64) -> RoutedNotification {
        RoutedNotification::global(end_turn()).with_turn(TurnId::new(n))
    }

    /// STRESS FIXTURE (plan S3b — the cyril-upjh phantom-companion cell,
    /// probe f5/f7/f8 shapes): a stamped release leaves a Wire expectation
    /// ONLY for a turn whose engine emits a wire turn_end. Buggy
    /// implementation this fails under: unconditional registration, where a
    /// single-terminal release leaves a session-keyed phantom that eats the
    /// next unstamped completion.
    #[test]
    fn stamped_release_owes_wire_companion_only_when_expected() {
        // Dual-terminal turn: the wire twin is still owed after release.
        let mut kas = TurnMediator::new();
        assert_eq!(
            kas.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(
            kas.observe(&stamped(0), None),
            Disposition::ForwardTurnComplete
        );
        assert!(matches!(
            kas.observe(&wire_end("s"), None),
            Disposition::Absorb { .. }
        ));

        // Single-terminal turn: the ledger is empty after release, so the
        // same unstamped frame is unowned, not a companion.
        let mut v2 = TurnMediator::new();
        assert_eq!(
            v2.begin_turn(sid("s"), false),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(
            v2.observe(&stamped(0), None),
            Disposition::ForwardTurnComplete
        );
        assert_eq!(v2.observe(&wire_end("s"), None), Disposition::DropUnowned);
    }

    /// STRESS FIXTURE (plan S3c, probe f3): a stale stamp is dropped and the
    /// live turn is untouched. Buggy implementation this fails under: the
    /// original a71q defect — an id-blind "is anything running" release that
    /// lets turn A's late completion clear turn B. Also carries the
    /// post-release half of the S2 cancel-snapshot fixture.
    #[test]
    fn stale_stamp_never_clears_a_live_turn() {
        let mut m = TurnMediator::new();
        assert_eq!(
            m.begin_turn(sid("a"), false),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(
            m.observe(&stamped(0), None),
            Disposition::ForwardTurnComplete
        );
        assert_eq!(
            m.active_turn_session(),
            None,
            "released turn leaves no cancel target"
        );

        assert_eq!(
            m.begin_turn(sid("b"), false),
            BeginTurn::Accepted(TurnId::new(1))
        );
        assert_eq!(
            m.observe(&stamped(0), None),
            Disposition::DropStale {
                stale: TurnId::new(0)
            }
        );
        assert!(m.is_busy(), "the stale duplicate must not release turn#1");
        assert_eq!(
            m.active_turn_session(),
            Some(&sid("b")),
            "cancel still targets the live turn's own session"
        );
    }

    /// Probe f10+f12: owner-keyed absorb beats stale-drop while the NEXT turn
    /// runs, and the evidence pair carries both `{source, reason}` halves in
    /// arrival order (cyril-pnwb; design C9).
    #[test]
    fn owner_keyed_absorb_wins_over_stale_while_next_turn_runs() {
        let mut m = TurnMediator::new();
        assert_eq!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(
            m.observe(&wire_end("s"), None),
            Disposition::ForwardTurnComplete
        );
        assert_eq!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(1))
        );

        assert_eq!(
            m.observe(&stamped(0), None),
            Disposition::Absorb {
                owner: TurnId::new(0),
                first: ev(CompanionSource::Wire),
                second: ev(CompanionSource::Synthesized),
            },
            "turn#0's synthesized twin is evidence, not a stale duplicate"
        );
        assert!(m.is_busy(), "turn#1 must be untouched by the absorb");
    }

    /// REGRESSION FENCE (cyril-b4y4 C3/C4; closes cyril-ri8q via its option
    /// (a) observation seam). The full probe scenario f1–f12, transcribed
    /// from the pre-extraction capture `.cyril-b4y4/probes/oracle-output.txt`
    /// — expectations come from the OLD code's observed behavior, not from
    /// this implementation. Sync, no harness: this test running at all is
    /// design claim C3.
    ///
    /// DISCRIMINATING POWER, by mutation (design C4): absorb-without-clear
    /// (a71q falsifier M2) fails the f9→f10 pair — f10 would Absorb a second
    /// time instead of releasing turn#3; release-before-absorb (M3) fails f9
    /// itself — the dangling Wire expectation would clear live turn#3. Both
    /// were previously signed blindness B16 ("the bridge cannot see whether
    /// the ledger clears on absorption"); the Disposition seam is what makes
    /// them assertable.
    #[test]
    fn mediator_matrix_all_dispositions() {
        let s = "sess_fake-0";
        let mut m = TurnMediator::new();

        // Turn 1 — live-confirmed order: wire turn_end first (f1), then the
        // synthesized twin (f2).
        assert_eq!(
            m.begin_turn(sid(s), true),
            BeginTurn::Accepted(TurnId::new(0)),
            "p1"
        );
        assert_eq!(
            m.observe(&wire_end(s), None),
            Disposition::ForwardTurnComplete,
            "f1"
        );
        assert_eq!(
            m.observe(&stamped(0), None),
            Disposition::Absorb {
                owner: TurnId::new(0),
                first: ev(CompanionSource::Wire),
                second: ev(CompanionSource::Synthesized),
            },
            "f2"
        );

        // Turn 2 — stale duplicate, foreign terminal, reverse order.
        assert_eq!(
            m.begin_turn(sid(s), true),
            BeginTurn::Accepted(TurnId::new(1)),
            "p2"
        );
        assert_eq!(
            m.observe(&stamped(0), None),
            Disposition::DropStale {
                stale: TurnId::new(0)
            },
            "f3"
        );
        // C5 fence: a foreign terminal forwards but is NOT a turn completion.
        assert_eq!(
            m.observe(&wire_end("sess_foreign"), None),
            Disposition::Forward,
            "f4"
        );
        assert!(
            m.is_busy(),
            "f4: the foreign terminal must not touch the main turn"
        );
        assert_eq!(
            m.observe(&stamped(1), None),
            Disposition::ForwardTurnComplete,
            "f5"
        );
        assert_eq!(
            m.observe(&wire_end(s), None),
            Disposition::Absorb {
                owner: TurnId::new(1),
                first: ev(CompanionSource::Synthesized),
                second: ev(CompanionSource::Wire),
            },
            "f6"
        );
        assert_eq!(
            m.observe(&wire_end(s), None),
            Disposition::DropUnowned,
            "f7"
        );

        // Turn 3 + turn 4 — the absorb-first precedence block: turn#2's Wire
        // expectation dangles on the SAME session as live turn#3.
        assert_eq!(
            m.begin_turn(sid(s), true),
            BeginTurn::Accepted(TurnId::new(2)),
            "p3"
        );
        assert_eq!(
            m.observe(&stamped(2), None),
            Disposition::ForwardTurnComplete,
            "f8"
        );
        assert_eq!(
            m.begin_turn(sid(s), true),
            BeginTurn::Accepted(TurnId::new(3)),
            "p4"
        );
        assert_eq!(
            m.observe(&wire_end(s), None),
            Disposition::Absorb {
                owner: TurnId::new(2),
                first: ev(CompanionSource::Synthesized),
                second: ev(CompanionSource::Wire),
            },
            "f9 (M3: a release here means absorb-first is broken)"
        );
        assert!(m.is_busy(), "f9: live turn#3 must survive the absorb");
        assert_eq!(
            m.observe(&wire_end(s), None),
            Disposition::ForwardTurnComplete,
            "f10 (M2: an Absorb here means f9 did not clear the ledger)"
        );
        assert_eq!(
            m.observe(&RoutedNotification::global(end_turn()), None),
            Disposition::DropUnowned,
            "f11 (session-None scope, idle, only a Synthesized owed)"
        );
        assert_eq!(
            m.begin_turn(sid(s), true),
            BeginTurn::Accepted(TurnId::new(4)),
            "p5"
        );
        assert_eq!(
            m.observe(&stamped(3), None),
            Disposition::Absorb {
                owner: TurnId::new(3),
                first: ev(CompanionSource::Wire),
                second: ev(CompanionSource::Synthesized),
            },
            "f12"
        );
        assert!(m.is_busy(), "f12: live turn#4 must survive the absorb");
    }

    /// Pins the one production-unreachable input shape (design step 2, out-of-
    /// scope note): a session-less unstamped terminal WHILE a turn is live.
    /// No producer emits it today — synthesis always stamps, `convert::kas`
    /// always scopes — so this pins current behavior (foreign-shaped Forward)
    /// rather than leaving a future producer to an accidental cell.
    #[test]
    fn global_unstamped_with_live_turn_is_pinned_forward() {
        let mut m = TurnMediator::new();
        assert_eq!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(
            m.observe(&RoutedNotification::global(end_turn()), None),
            Disposition::Forward
        );
        assert!(m.is_busy());
    }

    /// Non-terminal notifications pass through untouched in every state.
    #[test]
    fn non_terminals_forward_untouched() {
        let note = || {
            RoutedNotification::global(Notification::SystemNotify {
                level: crate::types::event::SystemNotifyLevel::Info,
                message: "marker".into(),
            })
        };
        let mut m = TurnMediator::new();
        assert_eq!(m.observe(&note(), None), Disposition::Forward);
        assert_eq!(
            m.begin_turn(sid("s"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(m.observe(&note(), None), Disposition::Forward);
        assert!(m.is_busy());
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
    fn active_turn_session_is_dispatch_snapshot() {
        let mut m = TurnMediator::new();
        assert_eq!(m.active_turn_session(), None);
        assert_eq!(
            m.begin_turn(sid("s1"), true),
            BeginTurn::Accepted(TurnId::new(0))
        );
        assert_eq!(m.active_turn_session(), Some(&sid("s1")));
    }

    // ---- cyril-lki9 Slice 3: server-started turns ------------------------------

    fn turn_start(s: &str) -> RoutedNotification {
        RoutedNotification::scoped(sid(s), Notification::TurnStarted)
    }

    /// Drive one scenario of `.cyril-lki9/oracles/mediator_model.py` through the
    /// real mediator on main session `s`, logging each step in the model's own
    /// vocabulary. `D` dispatch, `S`/`E` wire turn_start/turn_end, `R` the
    /// dispatch's stamped prompt response. Returns (log, final state).
    fn run_scenario(events: &str) -> (Vec<String>, bool, bool) {
        let main = sid("s");
        let mut m = TurnMediator::new();
        let mut owner: Option<TurnId> = None;
        let mut log = Vec::new();
        for event in events.chars() {
            match event {
                'D' => match m.begin_turn(main.clone(), true) {
                    BeginTurn::Accepted(id) => {
                        owner = Some(id);
                        log.push("BEGIN cyril".to_owned());
                    }
                    BeginTurn::Busy => log.push("REJECT".to_owned()),
                    BeginTurn::Exhausted => panic!("unexpected exhaustion"),
                },
                'S' => {
                    // ATTACH is judged on the EFFECT (this frame opened a
                    // dispatched turn's bracket), never inferred from the
                    // pre-state: a mediator that skipped attachment would
                    // otherwise log the same line while warning "nested
                    // turn_start" on every ordinary prompt (mutation M8).
                    let was_unbracketed = m
                        .active
                        .as_ref()
                        .is_some_and(|a| a.origin == TurnOrigin::Dispatched && !a.bracket_open);
                    let disposition = m.observe(&turn_start("s"), Some(&main));
                    let now_bracketed = m
                        .active
                        .as_ref()
                        .is_some_and(|a| a.origin == TurnOrigin::Dispatched && a.bracket_open);
                    match disposition {
                        Disposition::BeginServerTurn => log.push("BEGIN server".to_owned()),
                        Disposition::Forward if was_unbracketed && now_bracketed => {
                            log.push("ATTACH".to_owned())
                        }
                        Disposition::Forward => log.push("FORWARD nested-start".to_owned()),
                        other => panic!("turn_start must never be {other:?}"),
                    }
                }
                'E' => log.push(match m.observe(&wire_end("s"), Some(&main)) {
                    Disposition::ForwardTurnComplete => "FORWARD_COMPLETE(wire)".to_owned(),
                    Disposition::Absorb { .. } => "ABSORB wire".to_owned(),
                    Disposition::DropUnowned => "DROP_UNOWNED".to_owned(),
                    other => format!("{other:?}"),
                }),
                'R' => {
                    // A rejected dispatch never reaches KAS: it has no response.
                    let Some(id) = owner.filter(|_| !log.contains(&"REJECT".to_owned())) else {
                        continue;
                    };
                    let response =
                        RoutedNotification::scoped(main.clone(), end_turn()).with_turn(id);
                    log.push(match m.observe(&response, Some(&main)) {
                        Disposition::ForwardTurnComplete => "FORWARD_COMPLETE(resp)".to_owned(),
                        Disposition::Absorb { .. } => "ABSORB synth".to_owned(),
                        Disposition::DropStale { .. } => "DROP_STALE".to_owned(),
                        other => format!("{other:?}"),
                    });
                }
                other => panic!("unknown event {other}"),
            }
        }
        (log, m.is_busy(), m.companion.is_some())
    }

    /// C8 (+ C7): every scenario of the independent Python model ends idle,
    /// with one forwarded completion per wire bracket, no dangling companion,
    /// and a disposition log equal to the model's item by item
    /// (`.cyril-lki9/logs/c8-model-proposed.txt`; `cyril#N` ids normalized —
    /// the model allocates ids only for dispatches, the mediator also for
    /// server turns).
    #[test]
    fn server_turn_scenarios_match_the_model() {
        let scenarios: [(&str, &str, usize, &[&str]); 10] = [
            (
                "idle-wake",
                "SE",
                1,
                &["BEGIN server", "FORWARD_COMPLETE(wire)"],
            ),
            (
                "prompt E<R",
                "DSER",
                1,
                &[
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(wire)",
                    "ABSORB synth",
                ],
            ),
            (
                "prompt R<E",
                "DSRE",
                1,
                &[
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(resp)",
                    "ABSORB wire",
                ],
            ),
            (
                "wake-cancelled",
                "SE",
                1,
                &["BEGIN server", "FORWARD_COMPLETE(wire)"],
            ),
            (
                "wake-then-dispatch-rejected",
                "SDE",
                1,
                &["BEGIN server", "REJECT", "FORWARD_COMPLETE(wire)"],
            ),
            (
                "race D@0 tail=SER",
                "DSESER",
                2,
                &[
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(wire)",
                    "BEGIN server",
                    "FORWARD_COMPLETE(wire)",
                    "ABSORB synth",
                ],
            ),
            (
                "race D@2 tail=SER",
                "SEDSER",
                2,
                &[
                    "BEGIN server",
                    "FORWARD_COMPLETE(wire)",
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(wire)",
                    "ABSORB synth",
                ],
            ),
            (
                "race D@0 tail=SRE",
                "DSESRE",
                2,
                &[
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(wire)",
                    "BEGIN server",
                    "ABSORB synth",
                    "FORWARD_COMPLETE(wire)",
                ],
            ),
            (
                "race D@2 tail=SRE",
                "SEDSRE",
                2,
                &[
                    "BEGIN server",
                    "FORWARD_COMPLETE(wire)",
                    "BEGIN cyril",
                    "ATTACH",
                    "FORWARD_COMPLETE(resp)",
                    "ABSORB wire",
                ],
            ),
            (
                "two-wakes",
                "SESE",
                2,
                &[
                    "BEGIN server",
                    "FORWARD_COMPLETE(wire)",
                    "BEGIN server",
                    "FORWARD_COMPLETE(wire)",
                ],
            ),
        ];
        for (name, events, brackets, want_log) in scenarios {
            let (log, busy, companion) = run_scenario(events);
            let forwarded = log
                .iter()
                .filter(|l| l.starts_with("FORWARD_COMPLETE"))
                .count();
            assert!(
                !busy && !companion && forwarded == brackets && log == want_log,
                "C8 scenario {name:?} ({events}): busy={busy} companion={companion} \
                 forwarded={forwarded} (want {brackets})\n  got  {log:?}\n  want {want_log:?}"
            );
        }
    }

    /// C7 stress: a duplicate `turn_end` after a server turn released is
    /// dropped (nothing owed), and the server turn's release left no companion
    /// that could eat the NEXT turn's terminal.
    #[test]
    fn server_turn_owes_no_companion() {
        let main = sid("s");
        let mut m = TurnMediator::new();
        assert_eq!(
            m.observe(&turn_start("s"), Some(&main)),
            Disposition::BeginServerTurn
        );
        assert!(m.is_busy());
        assert_eq!(
            m.active_turn_session(),
            Some(&main),
            "cancel must target the server turn"
        );
        assert_eq!(
            m.observe(&wire_end("s"), Some(&main)),
            Disposition::ForwardTurnComplete
        );
        assert!(
            m.companion.is_none(),
            "a server turn owes no prompt-RPC companion"
        );
        assert_eq!(
            m.observe(&wire_end("s"), Some(&main)),
            Disposition::DropUnowned
        );
    }

    /// C9: a `turn_start` on a session other than main never begins or alters
    /// the main turn — idle, or while a main server turn is active.
    #[test]
    fn foreign_turn_start_is_forwarded() {
        let main = sid("s");
        let mut m = TurnMediator::new();
        assert_eq!(
            m.observe(&turn_start("child-7"), Some(&main)),
            Disposition::Forward
        );
        assert!(
            !m.is_busy(),
            "a step session's turn must not make main busy"
        );
        assert_eq!(
            m.observe(&turn_start("s"), Some(&main)),
            Disposition::BeginServerTurn
        );
        assert_eq!(
            m.observe(&turn_start("child-7"), Some(&main)),
            Disposition::Forward
        );
        assert_eq!(m.active_turn_session(), Some(&main));
        // No main session known yet (before the first session): never a server turn.
        let mut fresh = TurnMediator::new();
        assert_eq!(fresh.observe(&turn_start("s"), None), Disposition::Forward);
        assert!(!fresh.is_busy());
    }

    /// C10 against live wire: replaying both committed captures through the
    /// production conversion (with origins) and this mediator announces
    /// exactly the agent-initiated turns the independent per-turn probe table
    /// found (`.cyril-lki9/probe_p1_p7.py`): tail — one, on main; gate-on —
    /// one on main (workflow-complete-wake) and one on the woken creator step
    /// (send-message-wake).
    #[cfg(feature = "kas")]
    #[test]
    fn announce_matches_tagged_turn_census() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        // (trace, main session, expected announcements as (session, reason))
        type CensusRow<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);
        let census: [CensusRow; 2] = [
            (
                "experiments/conductor-spike/kas-workflow-channels-06615-restate-gateoff-tail-2.26.0.jsonl",
                "sess_dd72baef-6822-4fd0-81c8-c9e37e291bbc",
                &[(
                    "sess_dd72baef-6822-4fd0-81c8-c9e37e291bbc",
                    "workflow-complete-wake",
                )],
            ),
            (
                "experiments/conductor-spike/kas-workflow-new-06615-gateon-2.26.0.jsonl",
                "sess_f6407f82-f87d-42e3-a302-9dff0970c09c",
                &[
                    (
                        "sess_f6407f82-f87d-42e3-a302-9dff0970c09c",
                        "workflow-complete-wake",
                    ),
                    (
                        "sess_f7800342-47dd-48a6-b17f-3d38f8edf23e",
                        "send-message-wake",
                    ),
                ],
            ),
        ];
        for (rel, main, want) in census {
            let trace = std::fs::read_to_string(root.join(rel)).expect("read committed trace");
            let main = sid(main);
            let mut m = TurnMediator::new();
            let mut got: Vec<(String, String)> = Vec::new();
            for routed in crate::test_support::kas_trace_routed(&trace) {
                m.observe(&routed, Some(&main));
                if m.announce(&routed) {
                    let session = routed
                        .session_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    let reason = routed
                        .origin
                        .as_ref()
                        .and_then(crate::types::AgentInitiation::reason)
                        .unwrap_or("<none>")
                        .to_owned();
                    got.push((session, reason));
                }
            }
            let want: Vec<(String, String)> = want
                .iter()
                .map(|(s, r)| ((*s).to_owned(), (*r).to_owned()))
                .collect();
            assert_eq!(got, want, "{rel}: announcements (session, reason) in order");
        }
    }
}

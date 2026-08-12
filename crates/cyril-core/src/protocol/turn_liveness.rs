//! Turn liveness (cyril-14ou): whether the active turn is showing signs of
//! progress — the time since the last qualifying inbound frame, with host-side
//! work counted as activity (silence while cyril owes the agent a callback
//! reply is cyril's doing, not the agent's).
//!
//! Pure state machine in the `SessionController` mold: no async, no I/O, and
//! **time is an input** — `Instant`s arrive as parameters, the struct never
//! reads a clock. The bridge `run_loop` owns one of these, stamps it from the
//! notification arm, and polls [`TurnLiveness::check`] from a periodic tick,
//! passing `HostMediator::in_flight()` at check time (poll-at-tick rather than
//! dispatch/reply event feeds — the drain task is not loop-visible; treating a
//! busy-host tick as activity bounds the post-reply false-positive window to
//! one tick period).
//!
//! A stall is information, never a terminal: the bh7g capture shows a stalled
//! turn completing 16 minutes later, so nothing here touches turn ownership —
//! the emission this feeds (`Notification::TurnStalled`) is forwarded by the
//! mediator as a non-terminal. Vocabulary pinned in CONTEXT.md: "Turn
//! liveness", "Stalled turn", "Quiet period".
//!

use std::time::{Duration, Instant};

/// Liveness bookkeeping for the bridge's single active turn (ADR-0004: at most
/// one). Grounding for the threshold this feeds: healthy KAS turns were never
/// wire-silent longer than 8.2s across the bh7g corpus (12 turns, including
/// 96s-long ones — `context_usage` frames tick throughout a model leg), while
/// the captured stall was unbounded silence. See `.cyril-14ou/findings.md`.
#[derive(Debug, Default)]
pub(crate) struct TurnLiveness {
    /// Last observed activity for the active turn. `None` = no active turn
    /// (the clock is disarmed between turns — claim C5).
    last_activity: Option<Instant>,
    /// One emission per quiet period (claim C1/C2): disarmed by the emission,
    /// re-armed by the next qualifying frame.
    armed: bool,
}

impl TurnLiveness {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// A turn was dispatched: start the clock armed.
    pub(crate) fn begin(&mut self, now: Instant) {
        self.last_activity = Some(now);
        self.armed = true;
    }

    /// The active turn released (any terminal, any source): disarm entirely.
    pub(crate) fn end(&mut self) {
        self.last_activity = None;
        self.armed = false;
    }

    /// A qualifying inbound frame arrived (scoped to the active turn's session,
    /// or global). Re-arms after an emission — resumed traffic ends the quiet
    /// period (claim C2). A late frame with no active turn is ignored: there is
    /// no clock to feed and nothing to arm (stale traffic must not resurrect a
    /// released turn's liveness).
    pub(crate) fn stamp(&mut self, now: Instant) {
        if self.last_activity.is_some() {
            self.last_activity = Some(now);
            self.armed = true;
        }
    }

    /// Periodic poll: has the active turn gone quiet past `threshold`?
    ///
    /// `in_flight` is the host-side outstanding-callback count sampled at call
    /// time (`HostMediator::in_flight()`): while cyril owes the agent replies,
    /// the tick itself counts as activity (claim C3) — so the quiet clock only
    /// starts once the host is idle, at worst one tick period late.
    ///
    /// Returns `Some(quiet_duration)` at most once per quiet period (claim C1);
    /// the caller turns it into a session-scoped `TurnStalled` notification.
    pub(crate) fn check(
        &mut self,
        now: Instant,
        in_flight: usize,
        threshold: Duration,
    ) -> Option<Duration> {
        let last = self.last_activity?;
        if in_flight > 0 {
            self.last_activity = Some(now);
            return None;
        }
        let quiet = now.saturating_duration_since(last);
        if self.armed && quiet >= threshold {
            self.armed = false;
            return Some(quiet);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const T: Duration = Duration::from_secs(30);

    fn at(t0: Instant, secs: u64) -> Instant {
        t0 + Duration::from_secs(secs)
    }

    /// C1: quiet ≥ T with nothing outstanding emits exactly once.
    #[test]
    fn stall_emits_once() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        assert_eq!(l.check(at(t0, 29), 0, T), None, "below threshold");
        let fired = l.check(at(t0, 31), 0, T);
        assert_eq!(fired, Some(Duration::from_secs(31)), "fires at threshold");
        assert_eq!(
            l.check(at(t0, 36), 0, T),
            None,
            "no second fire while quiet"
        );
        assert_eq!(
            l.check(at(t0, 300), 0, T),
            None,
            "still once per quiet period"
        );
    }

    /// C2: traffic after an emission re-arms; a later quiet period fires again.
    #[test]
    fn rearm_after_traffic() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        assert!(l.check(at(t0, 31), 0, T).is_some());
        l.stamp(at(t0, 40)); // traffic resumes: quiet period over
        assert_eq!(l.check(at(t0, 45), 0, T), None, "fresh clock after resume");
        assert_eq!(
            l.check(at(t0, 71), 0, T),
            Some(Duration::from_secs(31)),
            "second quiet period fires again"
        );
    }

    /// C3: outstanding host work parks the clock — and the tick that saw it
    /// counts as activity, so the quiet clock restarts from host-idle.
    #[test]
    fn outstanding_reply_parks() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        assert_eq!(l.check(at(t0, 40), 1, T), None, "no fire while host busy");
        assert_eq!(
            l.check(at(t0, 45), 0, T),
            None,
            "no instant fire after reply"
        );
        assert_eq!(
            l.check(at(t0, 71), 0, T),
            Some(Duration::from_secs(31)),
            "fires measured from the busy-host tick"
        );
    }

    /// C5: turn end disarms; late quiet (and late frames) do nothing.
    #[test]
    fn disarm_on_turn_end() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        l.end();
        assert_eq!(l.check(at(t0, 300), 0, T), None, "no clock after end");
        l.stamp(at(t0, 301)); // stale frame for a released turn
        assert_eq!(l.check(at(t0, 900), 0, T), None, "stale frame arms nothing");
    }

    /// Stress timeline from the plan: interleaves every input; expected
    /// emission sequence written before the implementation.
    #[test]
    fn stress_interleaved_timeline() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        for s in [1, 2, 3] {
            l.stamp(at(t0, s));
        }
        // 40s of "quiet" but a callback is outstanding the whole time.
        assert_eq!(l.check(at(t0, 43), 1, T), None);
        // Reply lands; 31s after the busy tick → first fire.
        assert_eq!(l.check(at(t0, 48), 0, T), None);
        assert_eq!(l.check(at(t0, 74), 0, T), Some(Duration::from_secs(31)));
        // Traffic resumes (re-arm), second quiet period → second fire.
        l.stamp(at(t0, 80));
        assert_eq!(l.check(at(t0, 111), 0, T), Some(Duration::from_secs(31)));
        // Turn ends; eternal quiet emits nothing.
        l.end();
        assert_eq!(l.check(at(t0, 500), 0, T), None);
        // Boundary: zero elapsed never fires.
        let mut fresh = TurnLiveness::new();
        fresh.begin(t0);
        assert_eq!(fresh.check(t0, 0, T), None);
    }
}

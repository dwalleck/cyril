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
        host_transition: Option<Instant>,
        threshold: Duration,
    ) -> Option<Duration> {
        let last = self.last_activity?;
        if in_flight > 0 {
            // Host work is activity — it also re-arms (spec review, cyril-14ou
            // fix 2): a stall followed by a host-callback window and renewed
            // quiet is a NEW quiet period and must emit again.
            self.last_activity = Some(now);
            self.armed = true;
            return None;
        }
        // A callback that entered AND left the table between ticks is
        // invisible to `in_flight` sampling (PR #94 review SP4) — its
        // transition stamp is the activity record. Clamp to `now`: the stamp
        // comes from a foreign clock read and must not push the clock forward.
        if let Some(t) = host_transition
            && t > last
        {
            self.last_activity = Some(t.min(now));
            self.armed = true;
        }
        let last = self.last_activity.unwrap_or(last);
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
    #![allow(clippy::unwrap_used, clippy::expect_used)]

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
        assert_eq!(l.check(at(t0, 29), 0, None, T), None, "below threshold");
        let fired = l.check(at(t0, 31), 0, None, T);
        assert_eq!(fired, Some(Duration::from_secs(31)), "fires at threshold");
        assert_eq!(
            l.check(at(t0, 36), 0, None, T),
            None,
            "no second fire while quiet"
        );
        assert_eq!(
            l.check(at(t0, 300), 0, None, T),
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
        assert!(l.check(at(t0, 31), 0, None, T).is_some());
        l.stamp(at(t0, 40)); // traffic resumes: quiet period over
        assert_eq!(
            l.check(at(t0, 45), 0, None, T),
            None,
            "fresh clock after resume"
        );
        assert_eq!(
            l.check(at(t0, 71), 0, None, T),
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
        assert_eq!(
            l.check(at(t0, 40), 1, None, T),
            None,
            "no fire while host busy"
        );
        assert_eq!(
            l.check(at(t0, 45), 0, None, T),
            None,
            "no instant fire after reply"
        );
        assert_eq!(
            l.check(at(t0, 71), 0, None, T),
            Some(Duration::from_secs(31)),
            "fires measured from the busy-host tick"
        );
        // PR #94 review SP4: a callback that entered AND left the table
        // between ticks (invisible to in_flight sampling) still counts as
        // activity via its transition stamp — and re-arms.
        let mut short = TurnLiveness::new();
        short.begin(t0);
        // Last frame at t0; a short callback ran at t=21 (transition stamp);
        // ticks at 25 and 31 must NOT fire — quiet restarts from 21.
        assert_eq!(short.check(at(t0, 25), 0, Some(at(t0, 21)), T), None);
        assert_eq!(
            short.check(at(t0, 31), 0, Some(at(t0, 21)), T),
            None,
            "short callback at t=21 defers the stall"
        );
        assert_eq!(
            short.check(at(t0, 52), 0, Some(at(t0, 21)), T),
            Some(Duration::from_secs(31)),
            "fires measured from the transition"
        );
        // A transition stamp OLDER than real activity changes nothing.
        let mut stale = TurnLiveness::new();
        stale.begin(t0);
        stale.stamp(at(t0, 10));
        assert_eq!(
            stale.check(at(t0, 41), 0, Some(at(t0, 5)), T),
            Some(Duration::from_secs(31)),
            "stale transition must not defer"
        );

        // Review fix 2: host work after a stall RE-ARMS — a host-callback
        // window followed by renewed quiet is a new quiet period.
        assert_eq!(l.check(at(t0, 80), 1, None, T), None, "host busy: parked");
        assert_eq!(
            l.check(at(t0, 111), 0, None, T),
            Some(Duration::from_secs(31)),
            "renewed quiet after host work must emit again"
        );
    }

    /// C5: turn end disarms; late quiet (and late frames) do nothing.
    #[test]
    fn disarm_on_turn_end() {
        let t0 = Instant::now();
        let mut l = TurnLiveness::new();
        l.begin(t0);
        l.end();
        assert_eq!(l.check(at(t0, 300), 0, None, T), None, "no clock after end");
        l.stamp(at(t0, 301)); // stale frame for a released turn
        assert_eq!(
            l.check(at(t0, 900), 0, None, T),
            None,
            "stale frame arms nothing"
        );
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
        assert_eq!(l.check(at(t0, 43), 1, None, T), None);
        // Reply lands; 31s after the busy tick → first fire.
        assert_eq!(l.check(at(t0, 48), 0, None, T), None);
        assert_eq!(
            l.check(at(t0, 74), 0, None, T),
            Some(Duration::from_secs(31))
        );
        // Traffic resumes (re-arm), second quiet period → second fire.
        l.stamp(at(t0, 80));
        assert_eq!(
            l.check(at(t0, 111), 0, None, T),
            Some(Duration::from_secs(31))
        );
        // Turn ends; eternal quiet emits nothing.
        l.end();
        assert_eq!(l.check(at(t0, 500), 0, None, T), None);
        // Boundary: zero elapsed never fires.
        let mut fresh = TurnLiveness::new();
        fresh.begin(t0);
        assert_eq!(fresh.check(t0, 0, None, T), None);
    }

    /// Replay one captured turn: stamps at the recorded frame times, a check
    /// every 5s (the production tick period) until release (completed turns)
    /// or the recorder's horizon (the stalled turn — replay time must NOT
    /// stop at the last frame, the bug the design falsifier caught in its
    /// own first draft).
    fn replay_turn(events: &[f64], completed: bool, horizon: f64, threshold: Duration) -> usize {
        let t0 = Instant::now();
        let at = |s: f64| t0 + Duration::from_secs_f64(s);
        let mut l = TurnLiveness::new();
        l.begin(t0);
        let end = if completed {
            events.last().copied().expect("completed turn has frames")
        } else {
            horizon
        };
        let (mut emissions, mut ei, mut tick) = (0, 0, 5.0);
        while tick <= end {
            while ei < events.len() && events[ei] <= tick {
                l.stamp(at(events[ei]));
                ei += 1;
            }
            if l.check(at(tick), 0, None, threshold).is_some() {
                emissions += 1;
            }
            tick += 5.0;
        }
        emissions
    }

    /// C11 REGRESSION FENCE (capture-derived; design falsifier_c11.py is the
    /// one-shot form). Real bh7g wire timings: at the production threshold the
    /// 8 healthy turns emit ZERO stalls and the captured stall emits EXACTLY
    /// ONE; at a threshold below the healthy inter-frame ceiling (8s) the same
    /// healthy turns DO emit — the tight-bound guard proving this fixture can
    /// observe emissions at all (defeats a table regenerated too coarsely).
    /// Buggy implementations this fails under: the threshold constant edited
    /// below the ceiling (healthy arm), clock-advances-only-on-frames (stall
    /// arm reports zero).
    #[test]
    fn capture_replay_thresholds() {
        let raw = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/turn_liveness_timings.json"
        ))
        .expect("timing table fixture");
        let table: serde_json::Value = serde_json::from_str(&raw).expect("valid table json");
        let turns = |key: &str| -> Vec<(Vec<f64>, bool, f64)> {
            table[key]
                .as_array()
                .expect("turn array")
                .iter()
                .map(|t| {
                    (
                        t["events"]
                            .as_array()
                            .expect("events")
                            .iter()
                            .map(|v| v.as_f64().expect("secs"))
                            .collect(),
                        t["completed"].as_bool().expect("completed"),
                        t["horizon"].as_f64().expect("horizon"),
                    )
                })
                .collect()
        };

        // The REAL production default — editing the const below the healthy
        // ceiling is exactly the regression this arm exists to catch. PR #94
        // review SP8: replaying against the const alone leaves UPWARD drift
        // unpinned (the stall tail is ~976s, so 900s would still pass) — the
        // approved contract is 30 seconds, asserted exactly.
        let production = crate::protocol::bridge::DEFAULT_STALL_THRESHOLD;
        assert_eq!(
            production,
            Duration::from_secs(30),
            "the approved cyril-14ou threshold is 30s; a deliberate change must re-run the replay analysis"
        );
        let healthy_at_30: usize = turns("healthy")
            .iter()
            .map(|(e, c, h)| replay_turn(e, *c, *h, production))
            .sum();
        assert_eq!(healthy_at_30, 0, "false stall on real healthy traffic");

        let stall_at_30: usize = turns("stall")
            .iter()
            .map(|(e, c, h)| replay_turn(e, *c, *h, production))
            .sum();
        assert_eq!(stall_at_30, 1, "the captured stall must emit exactly once");

        // Tick quantization: a threshold is only guaranteed observable when a
        // gap exceeds threshold + one tick period (a tick must LAND in the
        // window). The corpus ceiling is 8.2s and ticks are 5s apart, so 3s
        // (window ≥ 5.2s) is the tightest guaranteed-observable guard.
        let tight = Duration::from_secs(3);
        let healthy_at_3: usize = turns("healthy")
            .iter()
            .map(|(e, c, h)| replay_turn(e, *c, *h, tight))
            .sum();
        assert!(
            healthy_at_3 >= 1,
            "tight-bound guard: a 3s threshold must be observable on this table"
        );
    }
}

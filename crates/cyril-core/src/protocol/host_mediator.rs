//! Host-callback mediation (cyril-g9vt, ADR-0004 amendment): the pure state
//! machine behind the bridge `run_loop`'s host-callback arm. See CONTEXT.md
//! "Host-callback mediation".
//!
//! The mediator owns callback LIFECYCLE — ordered acceptance, cancellation
//! targeting, scope sweeps, shutdown — and nothing else. It is generic over
//! [`CallbackMeta`] and never names a capability: dispatch depth accretes on
//! the Engine-selected adapter set (ADR-0001), so adding a capability touches
//! this module zero times (design C12; the default build, where the KAS
//! callback type does not exist, is the mechanical proof). Structural twin of
//! [`crate::protocol::turn_mediator`] (cyril-b4y4): `accept` is a synchronous
//! state transition returning what the caller does next; the async execution
//! stays thin at the call site.

use std::collections::HashMap;

use crate::types::SessionId;

/// Scoped cancellation key: `(kind, id)`. The kind component is load-bearing
/// for correctness — a bare-id key would let one family's cancel abort a
/// stranger's same-id operation — so scoping is enforced by this TYPE, not by
/// an assert (budgeted-plan doc-contract (a)).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CancelKey {
    kind: &'static str,
    id: String,
}

impl CancelKey {
    pub(crate) fn new(kind: &'static str, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

/// The metadata contract between the mediator and a callback type — the ONLY
/// capability knowledge the mediator has (design C12).
pub(crate) trait CallbackMeta {
    /// `Some(key)` when this envelope is a CONTROL that aborts previously
    /// accepted work, rather than new work itself.
    fn cancels(&self) -> Option<CancelKey> {
        None
    }
    /// Key under which this operation can later be cancelled, if cancellable.
    fn cancel_key(&self) -> Option<CancelKey> {
        None
    }
    /// Session scope, for scope-wide sweeps (session cancel).
    fn scope(&self) -> Option<SessionId> {
        None
    }
    /// Stable label for logs and the wiring census (design C11).
    fn kind(&self) -> &'static str;
}

/// What the caller must do after [`HostMediator::accept`] — the whole policy
/// surface, so the `run_loop` arm stays a delegation.
#[derive(Debug)]
pub(crate) enum Accept<C> {
    /// New work: registered (id minted; cancel signal wired when the callback
    /// is cancellable). The caller spawns resolution off the loop.
    Spawn(Job<C>),
    /// A control consumed by the mediator (cancel target found and signalled,
    /// or knowingly dropped — the log line already happened).
    Consumed,
}

/// A registered unit of work travelling to the spawned resolution task.
#[derive(Debug)]
pub(crate) struct Job<C> {
    pub(crate) callback: C,
    /// Job id for completion bookkeeping ([`HostMediator::complete`]).
    pub(crate) id: JobId,
    /// Resolves when a later control cancels this job; `None` for
    /// non-cancellable callbacks. The resolution task races this against the
    /// dispatch future.
    pub(crate) cancelled: Option<tokio::sync::oneshot::Receiver<()>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct JobId(u64);

struct Entry {
    cancel: Option<(CancelKey, tokio::sync::oneshot::Sender<()>)>,
    scope: Option<SessionId>,
    kind: &'static str,
}

/// The host-callback lifecycle state machine. All methods are synchronous;
/// unit tests drive every transition with no async harness (design C1).
#[derive(Default)]
pub(crate) struct HostMediator {
    next_id: u64,
    in_flight: HashMap<JobId, Entry>,
}

impl HostMediator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Accept one envelope in channel order. Registration happens HERE —
    /// before any resolution work — so a later control can always target
    /// already-accepted work (ADR-0004 "ordered acceptance").
    pub(crate) fn accept<C: CallbackMeta>(&mut self, callback: C) -> Accept<C> {
        if let Some(key) = callback.cancels() {
            let target = self.in_flight.iter().find_map(|(id, e)| match &e.cancel {
                Some((k, _)) if *k == key => Some(*id),
                _ => None,
            });
            match target {
                Some(id) => {
                    // Entry removal on cancel: the resolution task also calls
                    // `complete`, which tolerates the missing id.
                    if let Some(Entry {
                        cancel: Some((_, tx)),
                        kind,
                        ..
                    }) = self.in_flight.remove(&id)
                    {
                        // A dropped receiver means the job already finished
                        // between our lookup and this send — benign.
                        if tx.send(()).is_err() {
                            tracing::debug!(kind, "cancel raced job completion");
                        }
                    }
                }
                None => {
                    tracing::debug!(?key, "cancel target not in flight; dropped");
                }
            }
            return Accept::Consumed;
        }

        let id = JobId(self.next_id);
        self.next_id += 1;
        let (cancel_tx, cancelled) = match callback.cancel_key() {
            Some(key) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                (Some((key, tx)), Some(rx))
            }
            None => (None, None),
        };
        self.in_flight.insert(
            id,
            Entry {
                cancel: cancel_tx,
                scope: callback.scope(),
                kind: callback.kind(),
            },
        );
        Accept::Spawn(Job {
            callback,
            id,
            cancelled,
        })
    }

    /// Resolution finished (successfully, erroneously, or via responder drop):
    /// forget the lifecycle entry. Tolerates an id already removed by a
    /// cancel — that race is expected, not an error.
    pub(crate) fn complete(&mut self, id: JobId) {
        self.in_flight.remove(&id);
    }

    /// Scope-wide sweep (session cancel): signal every in-flight cancellable
    /// job in the session. Non-cancellable jobs in scope are left to finish —
    /// they hold no external resources the sweep owns.
    pub(crate) fn cancel_scope(&mut self, scope: &SessionId) {
        self.in_flight.retain(|_, e| {
            if e.scope.as_ref() == Some(scope)
                && let Some((_, tx)) = e.cancel.take()
            {
                let kind = e.kind;
                if tx.send(()).is_err() {
                    tracing::debug!(kind, "scope cancel raced job completion");
                }
                return false;
            }
            true
        });
    }

    /// Bridge shutdown: signal every cancellable in-flight job and clear the
    /// table. Spawned tasks race their cancel signal and wind down; child
    /// processes are reaped by their owners' `kill_on_drop` (ADR-0004: made
    /// explicit here, delivered by the dispatch context's registries).
    pub(crate) fn shutdown(&mut self) {
        for (_, entry) in self.in_flight.drain() {
            if let Some((_, tx)) = entry.cancel
                && tx.send(()).is_err()
            {
                tracing::debug!(kind = entry.kind, "shutdown raced job completion");
            }
        }
    }

    /// In-flight count — seam-test observability (design C7).
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Test callback: the mediator's generic contract exercised without any
    /// capability type (which is the C12 point).
    struct Cb {
        kind: &'static str,
        cancel_key: Option<CancelKey>,
        cancels: Option<CancelKey>,
        scope: Option<SessionId>,
    }

    impl Cb {
        fn work(kind: &'static str) -> Self {
            Self {
                kind,
                cancel_key: None,
                cancels: None,
                scope: None,
            }
        }
        fn cancellable(kind: &'static str, id: &str) -> Self {
            Self {
                cancel_key: Some(CancelKey::new(kind, id)),
                ..Self::work(kind)
            }
        }
        fn cancel(kind: &'static str, id: &str) -> Self {
            Self {
                cancels: Some(CancelKey::new(kind, id)),
                ..Self::work(kind)
            }
        }
        fn scoped(mut self, s: &str) -> Self {
            self.scope = Some(SessionId::new(s));
            self
        }
    }

    impl CallbackMeta for Cb {
        fn cancels(&self) -> Option<CancelKey> {
            self.cancels.clone()
        }
        fn cancel_key(&self) -> Option<CancelKey> {
            self.cancel_key.clone()
        }
        fn scope(&self) -> Option<SessionId> {
            self.scope.clone()
        }
        fn kind(&self) -> &'static str {
            self.kind
        }
    }

    fn spawned(a: Accept<Cb>) -> Job<Cb> {
        match a {
            Accept::Spawn(j) => j,
            Accept::Consumed => panic!("expected Spawn"),
        }
    }

    // Design C2 substrate / stress (a) register-after-return: the cancel
    // signal must fire for a job whose resolution has NOT been polled at all —
    // registration lives inside accept, not in the spawned task.
    #[test]
    fn cancel_after_accept_signals_unpolled_job() {
        let mut m = HostMediator::new();
        let mut job = spawned(m.accept(Cb::cancellable("hooks/executeHook", "op-1")));
        assert_eq!(m.in_flight(), 1);

        assert!(matches!(
            m.accept(Cb::cancel("hooks/executeHook", "op-1")),
            Accept::Consumed
        ));
        assert_eq!(m.in_flight(), 0, "cancel removes the entry");
        // The signal is already resolved even though nothing ever awaited.
        assert!(
            job.cancelled.take().unwrap().try_recv().is_ok(),
            "cancel signal delivered to the unpolled job"
        );
    }

    // Stress (b) duplicate cancel-key: two live ops under the same key are
    // distinct entries; one cancel takes ONE of them, not both.
    #[test]
    fn duplicate_keys_are_distinct_entries() {
        let mut m = HostMediator::new();
        let j1 = spawned(m.accept(Cb::cancellable("hooks/executeHook", "op-x")));
        let j2 = spawned(m.accept(Cb::cancellable("hooks/executeHook", "op-x")));
        assert_ne!(j1.id, j2.id);
        assert_eq!(m.in_flight(), 2);
        m.accept(Cb::cancel("hooks/executeHook", "op-x"));
        assert_eq!(m.in_flight(), 1, "one cancel consumes one entry");
    }

    // Stress (c) cancel-unknown-key: consumed, logged, state untouched, no
    // panic.
    #[test]
    fn cancel_unknown_key_is_a_clean_drop() {
        let mut m = HostMediator::new();
        let _job = spawned(m.accept(Cb::cancellable("hooks/executeHook", "op-1")));
        assert!(matches!(
            m.accept(Cb::cancel("hooks/executeHook", "op-other")),
            Accept::Consumed
        ));
        assert_eq!(m.in_flight(), 1, "unrelated entry untouched");
    }

    // Kind-scoped keys (doc-contract (a)): same id under a DIFFERENT kind is
    // not a match — the type carries the scoping.
    #[test]
    fn cancel_keys_are_kind_scoped() {
        let mut m = HostMediator::new();
        let mut job = spawned(m.accept(Cb::cancellable("hooks/executeHook", "42")));
        m.accept(Cb::cancel("terminal/create", "42"));
        assert_eq!(m.in_flight(), 1, "other-kind cancel must not match");
        assert!(
            job.cancelled.take().unwrap().try_recv().is_err(),
            "no signal delivered"
        );
    }

    // Completion bookkeeping: complete() clears; completing twice (or after a
    // cancel already removed the entry) is tolerated.
    #[test]
    fn complete_clears_and_tolerates_races() {
        let mut m = HostMediator::new();
        let job = spawned(m.accept(Cb::work("auth/getAccessToken")));
        assert_eq!(
            job.callback.kind(),
            "auth/getAccessToken",
            "the envelope carries the work to the resolution task"
        );
        assert_eq!(m.in_flight(), 1);
        m.complete(job.id);
        assert_eq!(m.in_flight(), 0);
        m.complete(job.id); // idempotent

        let job2 = spawned(m.accept(Cb::cancellable("hooks/executeHook", "op")));
        m.accept(Cb::cancel("hooks/executeHook", "op"));
        m.complete(job2.id); // cancel already removed it — no panic
        assert_eq!(m.in_flight(), 0);
    }

    // Scope sweep: cancels the session's cancellable jobs, leaves other
    // sessions and non-cancellable work alone.
    #[test]
    fn scope_sweep_is_scoped() {
        let mut m = HostMediator::new();
        let mut a = spawned(m.accept(Cb::cancellable("hooks/executeHook", "a").scoped("s1")));
        let mut b = spawned(m.accept(Cb::cancellable("hooks/executeHook", "b").scoped("s2")));
        let c = spawned(m.accept(Cb::work("auth/getAccessToken").scoped("s1")));

        m.cancel_scope(&SessionId::new("s1"));
        assert!(a.cancelled.take().unwrap().try_recv().is_ok(), "s1 swept");
        assert!(
            b.cancelled.take().unwrap().try_recv().is_err(),
            "s2 untouched"
        );
        assert_eq!(
            m.in_flight(),
            2,
            "non-cancellable s1 job + s2 job remain until completion"
        );
        m.complete(c.id);
        assert_eq!(m.in_flight(), 1);
    }

    // Shutdown: every cancellable job signalled, table cleared.
    #[test]
    fn shutdown_signals_all_and_clears() {
        let mut m = HostMediator::new();
        let mut a = spawned(m.accept(Cb::cancellable("hooks/executeHook", "a")));
        let mut b = spawned(m.accept(Cb::cancellable("terminal/create", "t")));
        let _c = spawned(m.accept(Cb::work("auth/getAccessToken")));
        m.shutdown();
        assert_eq!(m.in_flight(), 0);
        assert!(a.cancelled.take().unwrap().try_recv().is_ok());
        assert!(b.cancelled.take().unwrap().try_recv().is_ok());
    }
}

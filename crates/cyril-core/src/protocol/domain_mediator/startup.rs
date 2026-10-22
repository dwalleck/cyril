//! Generation-tagged root-start outcomes and ordered, attempt-local diagnostics.
//! A notification's session ID is evidence about that session, not correlation
//! with session/new. Only the response can identify the new root.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use super::SessionStart;
use crate::types::{
    McpServerStatus, McpSnapshot, McpUpdate, Notification, SessionId, StartupAttemptId,
    StartupObservation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootStartKind {
    New,
    Load,
}

impl RootStartKind {
    pub(super) fn operation(self) -> &'static str {
        match self {
            Self::New => "Create session",
            Self::Load => "Load session",
        }
    }
}

#[derive(Debug)]
pub(crate) enum RootStartResult {
    Started(Box<SessionStart>),
    Failed(String),
    Fatal(String),
    TimedOut { budget: Duration },
}

pub(super) enum StartupDecision {
    Started(Box<SessionStart>),
    Fatal(String),
    Recoverable(String),
    Stale,
}

#[derive(Default)]
pub(super) struct Startup {
    generation: u64,
    active: Option<Attempt>,
}

struct Attempt {
    id: StartupAttemptId,
    kind: RootStartKind,
    previous_root: Option<String>,
    target: Option<SessionId>,
    scopes: BTreeMap<Option<String>, ScopeFacts>,
}

#[derive(Default)]
struct ScopeFacts {
    // Reuse the received authoritative type, not a second inventory schema.
    inventory: Option<Arc<McpSnapshot>>,
    reports: BTreeMap<String, Report>,
}

enum Report {
    Ready,
    Failed(String),
    OAuth,
}

impl Startup {
    /// Called only after the MCP staging owner admits the root start. This
    /// owner records diagnostic generations; it has no second admission guard.
    pub(super) fn begin(
        &mut self,
        kind: RootStartKind,
        previous_root: Option<&SessionId>,
        target: Option<SessionId>,
    ) -> StartupAttemptId {
        self.generation += 1;
        let id = StartupAttemptId::new(self.generation);
        self.active = Some(Attempt {
            id,
            kind,
            previous_root: previous_root.map(|id| id.as_str().to_owned()),
            target,
            scopes: BTreeMap::new(),
        });
        id
    }

    /// Called by the serial owner before ordinary routing/staging. Waiters
    /// never capture this state: the timeout outcome joins the latest facts
    /// when it reaches that same ordered work queue.
    /// A scoped v2 MCP fact also receives presentation-only provenance for the
    /// admitted generation when its inner and routed scopes agree.
    pub(super) fn observe(
        &mut self,
        notification: &Notification,
        session_id: Option<&SessionId>,
    ) -> Option<StartupObservation> {
        let attempt = self.active.as_mut()?;
        if attempt.kind == RootStartKind::New {
            match notification {
                Notification::McpUpdate(McpUpdate::Snapshot(snapshot)) => {
                    let scope = attempt
                        .scopes
                        .entry(Some(snapshot.session_id.as_str().to_owned()))
                        .or_default();
                    scope.reports.clear();
                    scope.inventory = Some(Arc::clone(snapshot));
                }
                Notification::McpServerInitialized {
                    session_id,
                    server_name,
                } => {
                    attempt.report(session_id.as_ref(), server_name, Report::Ready);
                }
                Notification::McpServerInitFailure {
                    session_id,
                    server_name,
                    error,
                } => {
                    attempt.report(
                        session_id.as_ref(),
                        server_name,
                        Report::Failed(
                            error.clone().unwrap_or_else(|| "no detail supplied".into()),
                        ),
                    );
                }
                Notification::McpOAuthRequest {
                    session_id,
                    server_name,
                    ..
                } => {
                    attempt.report(session_id.as_ref(), server_name, Report::OAuth);
                }
                _ => {}
            }
        }
        let scope = match notification {
            Notification::McpServerInitialized {
                session_id: Some(scope),
                ..
            }
            | Notification::McpServerInitFailure {
                session_id: Some(scope),
                ..
            }
            | Notification::McpOAuthRequest {
                session_id: Some(scope),
                ..
            } if session_id == Some(scope) => scope,
            _ => return None,
        };
        match attempt.kind {
            RootStartKind::New if attempt.previous_root.as_deref() != Some(scope.as_str()) => {
                Some(StartupObservation::UnboundNew {
                    generation: attempt.id,
                })
            }
            RootStartKind::Load if attempt.target.as_ref() == Some(scope) => {
                Some(StartupObservation::BoundRequestedLoad {
                    generation: attempt.id,
                })
            }
            RootStartKind::New | RootStartKind::Load => None,
        }
    }

    pub(super) fn finish(
        &mut self,
        id: StartupAttemptId,
        result: RootStartResult,
    ) -> StartupDecision {
        if self.active.as_ref().map(|a| a.id) != Some(id) {
            return StartupDecision::Stale;
        }
        let Some(attempt) = self.active.take() else {
            return StartupDecision::Stale;
        };
        match result {
            RootStartResult::Started(start) => StartupDecision::Started(start),
            RootStartResult::Fatal(reason) => StartupDecision::Fatal(reason),
            RootStartResult::Failed(reason) => match attempt.kind {
                RootStartKind::New => StartupDecision::Fatal(reason),
                RootStartKind::Load => StartupDecision::Recoverable(reason),
            },
            RootStartResult::TimedOut { budget } => match attempt.kind {
                RootStartKind::New => StartupDecision::Fatal(attempt.timeout_reason(budget)),
                RootStartKind::Load => StartupDecision::Recoverable(format!(
                    "session/load timed out after {}s",
                    budget.as_secs()
                )),
            },
        }
    }
}

impl Attempt {
    fn report(&mut self, session: Option<&SessionId>, name: &str, report: Report) {
        self.scopes
            .entry(session.map(|id| id.as_str().to_owned()))
            .or_default()
            .reports
            .insert(name.to_owned(), report);
    }

    fn timeout_reason(&self, budget: Duration) -> String {
        let mut message = format!("Session creation timed out after {}s.", budget.as_secs());
        for (session, facts) in &self.scopes {
            message.push('\n');
            match session {
                Some(id) if self.previous_root.as_ref() == Some(id) => {
                    message.push_str(&format!(
                        "Previously active session {id:?} (not correlated to the pending root):"
                    ));
                }
                Some(id) => message.push_str(&format!(
                    "Candidate session {id:?} (not correlated to the pending root):"
                )),
                None => message.push_str(
                    "Legacy sessionless events observed during this attempt (root attribution unknown):"
                ),
            }
            facts.append_summary(&mut message);
            if facts.inventory.is_some() {
                message.push_str("\nComplete received KAS pool snapshot applies only to this candidate session, not the pending root.");
            }
        }
        // No notification session ID proves correlation with an unanswered
        // session/new. Even one candidate's complete pool is not root inventory.
        message.push_str("\nOther configured servers: unknown — the agent supplied no complete startup inventory for the pending root.");
        message
    }
}

impl ScopeFacts {
    fn append_summary(&self, message: &mut String) {
        let mut reported = Vec::new();
        let mut oauth = Vec::new();
        let mut unsettled = Vec::new();
        // Borrow the immutable KAS snapshot; normal successful startup does
        // not allocate/copy a second per-server schema merely for diagnostics.
        let mut latest = BTreeMap::new();
        if let Some(snapshot) = &self.inventory {
            for server in &snapshot.servers {
                latest.insert(server.name.as_str(), LastReport::Kas(&server.status));
            }
        }
        for (name, report) in &self.reports {
            latest.insert(name.as_str(), LastReport::V2(report));
        }
        for (name, report) in &latest {
            match report {
                LastReport::Kas(McpServerStatus::Connected { .. })
                | LastReport::V2(Report::Ready) => reported.push(format!("{name} (ready)")),
                LastReport::Kas(McpServerStatus::Failed {
                    failed_authorization: true,
                    ..
                })
                | LastReport::V2(Report::OAuth) => oauth.push(*name),
                LastReport::Kas(McpServerStatus::Failed {
                    error_message: error,
                    ..
                })
                | LastReport::V2(Report::Failed(error)) => {
                    reported.push(format!("{name} (failed: {error})"))
                }
                LastReport::Kas(McpServerStatus::Connecting) => unsettled.push(*name),
                LastReport::Kas(McpServerStatus::Disabled) => {
                    reported.push(format!("{name} (disabled)"))
                }
                LastReport::Kas(McpServerStatus::Unknown { status }) => {
                    reported.push(format!("{name} (status: {status})"))
                }
            }
        }
        if !reported.is_empty() {
            message.push_str(&format!("\nReported: {}.", reported.join(", ")));
        }
        if !oauth.is_empty() {
            message.push_str(&format!("\nAwaiting OAuth: {}.", oauth.join(", ")));
        }
        if !unsettled.is_empty() {
            message.push_str(&format!(
                "\nAnnounced but not settled: {}.",
                unsettled.join(", ")
            ));
        }
        if let Some(inventory) = &self.inventory {
            let silent = silent_names(inventory, |name| latest.contains_key(name));
            if !silent.is_empty() {
                message.push_str(&format!(
                    "\nSilent within this candidate's authoritative inventory: {}.",
                    silent.into_iter().collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }
}

enum LastReport<'a> {
    Kas(&'a McpServerStatus),
    V2(&'a Report),
}

fn silent_names(inventory: &McpSnapshot, reported: impl Fn(&str) -> bool) -> BTreeSet<&str> {
    inventory
        .servers
        .iter()
        .map(|server| server.name.as_str())
        .filter(|name| !reported(name))
        .collect()
}

impl super::DomainMediator {
    pub(super) async fn begin_root_start(
        &mut self,
        kind: RootStartKind,
        target: Option<SessionId>,
    ) -> crate::Result<Option<StartupAttemptId>> {
        if !self.begin_mcp_session(target.clone()).await? {
            return Ok(None);
        }
        Ok(Some(self.startup.begin(
            kind,
            self.active_session_id.as_ref(),
            target,
        )))
    }

    pub(super) async fn apply_root_start(
        &mut self,
        attempt: StartupAttemptId,
        result: RootStartResult,
    ) -> crate::Result<std::ops::ControlFlow<(), bool>> {
        use std::ops::ControlFlow;
        match self.startup.finish(attempt, result) {
            StartupDecision::Started(start) => {
                let SessionStart {
                    session_id,
                    origin,
                    config_options,
                    created,
                } = *start;
                // The existing MCP owner promotes only this response's exact
                // ID, after the SessionCreated/config reset boundary.
                self.publish_session_start(session_id, origin, config_options, created)
                    .await?;
                Ok(ControlFlow::Continue(false))
            }
            StartupDecision::Fatal(reason) => {
                self.mcp.fail_session();
                self.notify(Notification::BridgeDisconnected { reason }.into())
                    .await?;
                Ok(ControlFlow::Break(()))
            }
            StartupDecision::Recoverable(message) => {
                self.mcp.fail_session();
                self.notify(
                    Notification::BridgeError {
                        operation: RootStartKind::Load.operation().into(),
                        message,
                    }
                    .into(),
                )
                .await?;
                Ok(ControlFlow::Continue(false))
            }
            StartupDecision::Stale => {
                tracing::debug!(?attempt, "stale root-start outcome ignored");
                Ok(ControlFlow::Continue(false))
            }
        }
    }
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;

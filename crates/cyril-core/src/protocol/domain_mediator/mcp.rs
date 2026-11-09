//! The serial owner of root-session MCP state, early pushes and correlated RPCs.

use std::collections::{BTreeSet, HashMap};

use crate::types::SessionId;
use crate::types::mcp::*;

#[derive(Default)]
pub(super) struct McpOwner {
    pub(super) state: McpState,
    /// Claims carried across a same-id reload. Their answers skip
    /// revalidation: the reset state cannot judge them until the reloaded
    /// root re-reports its servers, and KAS's answer is real either way.
    carried: BTreeSet<u64>,
    opening: Option<Opening>,
}

struct Opening {
    target: Option<SessionId>,
    staged: HashMap<SessionId, McpState>,
}

/// A request either claims pending state or has an immediate outcome to publish.
#[cfg(feature = "kas")]
#[must_use]
pub(super) enum McpRequestPlan {
    Pending(McpPending),
    Immediate(McpOutcome),
}

impl McpOwner {
    /// The staging slot is also the single admission claim for a root start.
    /// Reject before sending: ignoring a second success would orphan its
    /// remotely created session, and replacing this slot loses early pushes.
    pub(super) fn begin_session(&mut self, target: Option<SessionId>) -> Result<(), String> {
        if let Some(opening) = &self.opening {
            let operation = if opening.target.is_some() {
                "Load session"
            } else {
                "Create session"
            };
            return Err(format!(
                "{operation} is already pending; no second session/new or session/load request was sent"
            ));
        }
        self.opening = Some(Opening {
            target,
            staged: HashMap::new(),
        });
        Ok(())
    }

    pub(super) fn fail_session(&mut self) {
        self.opening = None;
    }

    /// Reset to `session_id`'s fresh state. A same-id reload keeps its live
    /// work: pending claims (KAS still answers them on that session), claim
    /// outcomes and refusals, re-announced after the replayed status so the
    /// controller's reset copy regains them. A different root abandons its
    /// pending claims, returned so the caller can say so.
    pub(super) fn activate(&mut self, session_id: SessionId) -> (Vec<McpUpdate>, Vec<McpPending>) {
        let staged = self
            .opening
            .take()
            .and_then(|mut opening| opening.staged.remove(&session_id));
        let previous =
            std::mem::replace(&mut self.state, McpState::for_session(session_id.clone()));
        let mut updates = Vec::new();
        if let Some(staged) = staged {
            if let Some(snapshot) = staged.snapshot {
                updates.push(McpUpdate::Snapshot(snapshot));
            }
            if !staged.authorization_urls.is_empty() {
                updates.push(McpUpdate::AuthorizationUrls {
                    session_id: session_id.clone(),
                    urls: staged.authorization_urls,
                });
            }
            if let Some(policy) = staged.policy {
                updates.push(McpUpdate::Policy {
                    session_id: session_id.clone(),
                    policy,
                });
            }
            if let Some(message) = staged.diagnostic {
                updates.push(McpUpdate::Diagnostic {
                    session_id: session_id.clone(),
                    message,
                });
            }
        }
        let pending: Vec<McpPending> = previous.pending.into_values().collect();
        let abandoned = if previous.session_id.as_ref() == Some(&session_id) {
            self.carried = pending.iter().map(|pending| pending.id).collect();
            // Outcomes before claims (a claim clears its server's outcome),
            // refusals last (a claim clears its server's refusal too).
            updates.extend(previous.outcomes.into_values().map(McpUpdate::Outcome));
            updates.extend(pending.into_iter().map(McpUpdate::Pending));
            updates.extend(previous.refusals.into_values().map(McpUpdate::Refused));
            Vec::new()
        } else {
            self.carried.clear();
            pending
        };
        for update in &updates {
            self.state.apply(update);
        }
        (updates, abandoned)
    }

    pub(super) fn ingest(&mut self, update: &McpUpdate) -> bool {
        let session_id = update.session_id();
        if matches!(update, McpUpdate::ElicitationDeclined(_)) {
            return self.state.apply(update);
        }
        if let Some(opening) = &mut self.opening {
            let target_matches = opening
                .target
                .as_ref()
                .is_none_or(|target| target == session_id);
            if target_matches
                && (opening.target.is_some() || self.state.session_id.as_ref() != Some(session_id))
            {
                opening
                    .staged
                    .entry(session_id.clone())
                    .or_insert_with(|| McpState::for_session(session_id.clone()))
                    .apply(update);
            }
        }
        self.state.apply(update)
    }

    #[cfg(feature = "kas")]
    pub(super) fn request(&mut self, id: u64, request: McpRequest) -> McpRequestPlan {
        if let Err(error) = self.state.validate(&request) {
            return McpRequestPlan::Immediate(McpOutcome {
                id,
                request,
                result: Err(error),
            });
        }
        let pending = McpPending { id, request };
        self.state.apply(&McpUpdate::Pending(pending.clone()));
        McpRequestPlan::Pending(pending)
    }

    #[cfg(feature = "kas")]
    pub(super) fn finish(&mut self, mut outcome: McpOutcome) -> Option<McpOutcome> {
        if self.state.session_id.as_ref() != Some(&outcome.request.session_id)
            || self
                .state
                .pending
                .get(&outcome.request.server_name)
                .is_none_or(|pending| pending.id != outcome.id)
        {
            return None;
        }
        // Revalidate the original advertisement, not whichever row is selected
        // now. Removing pending in this small validation view avoids its own
        // duplicate guard; the real state stays pending until the final update.
        let carried = self.carried.remove(&outcome.id);
        let pending = self.state.pending.remove(&outcome.request.server_name);
        if outcome.result.is_ok()
            && !carried
            && let Err(error) = self.state.validate(&outcome.request)
        {
            outcome.result = Err(error);
        }
        if let Some(pending) = pending {
            self.state
                .pending
                .insert(outcome.request.server_name.clone(), pending);
        }
        self.state.apply(&McpUpdate::Outcome(outcome.clone()));
        Some(outcome)
    }
}

impl super::DomainMediator {
    pub(super) async fn begin_mcp_session(
        &mut self,
        target: Option<SessionId>,
    ) -> crate::Result<bool> {
        let operation = if target.is_some() {
            "Load session"
        } else {
            "Create session"
        };
        match self.mcp.begin_session(target) {
            Ok(()) => Ok(true),
            Err(message) => {
                self.notify(
                    crate::types::Notification::BridgeError {
                        operation: operation.into(),
                        message,
                    }
                    .into(),
                )
                .await?;
                Ok(false)
            }
        }
    }

    pub(super) async fn handle_mcp_update(&mut self, update: McpUpdate) -> crate::Result<()> {
        let publish = self.mcp.ingest(&update);
        if matches!(update, McpUpdate::ElicitationDeclined(_)) {
            // Workflow/early host callbacks remain visible with their actual
            // context, but cannot mutate another root session's MCP state.
            self.notify(crate::types::RoutedNotification::global(
                crate::types::Notification::McpUpdate(update),
            ))
            .await?;
        } else if publish {
            self.notify(crate::types::RoutedNotification::scoped(
                update.session_id().clone(),
                crate::types::Notification::McpUpdate(update),
            ))
            .await?;
        }
        Ok(())
    }

    pub(super) async fn activate_mcp_session(
        &mut self,
        session_id: SessionId,
    ) -> crate::Result<()> {
        let (updates, abandoned) = self.mcp.activate(session_id);
        for update in updates {
            self.notify(crate::types::RoutedNotification::scoped(
                update.session_id().clone(),
                crate::types::Notification::McpUpdate(update),
            ))
            .await?;
        }
        for pending in abandoned {
            self.notify(
                crate::types::Notification::BridgeError {
                    operation: "MCP operation".into(),
                    message: format!(
                        "the pending request to server '{}' on previous session '{}' was abandoned when the root session changed; its result will not be shown.",
                        pending.request.server_name,
                        pending.request.session_id.as_str(),
                    ),
                }
                .into(),
            )
            .await?;
        }
        Ok(())
    }

    #[cfg(feature = "kas")]
    pub(super) async fn finish_mcp_command(&mut self, outcome: McpOutcome) -> crate::Result<()> {
        if let Some(outcome) = self.mcp.finish(outcome) {
            self.notify(crate::types::RoutedNotification::scoped(
                outcome.request.session_id.clone(),
                crate::types::Notification::McpUpdate(McpUpdate::Outcome(outcome)),
            ))
            .await?;
        }
        Ok(())
    }
}

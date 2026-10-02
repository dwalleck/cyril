//! A Run authorization: armed when the operator confirms a review, it decides
//! the run's step-session permission requests without prompting.

use super::consent::PermissionConsent;
use super::policy::{Decision, PolicyScope, decide};
use crate::types::workflow::WorkflowId;

/// Where the run is in its life. Pause does not disarm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationState {
    Armed,
    Paused,
    /// The run completed, was cancelled, or cyril is exiting. Kept so that a
    /// late request from the run is denied and logged, never prompted.
    Disarmed,
}

#[derive(Clone, Debug)]
pub struct RunAuthorization {
    workflow_id: WorkflowId,
    scope: PolicyScope,
    state: AuthorizationState,
    allowed: usize,
    denials: Vec<String>,
}

impl RunAuthorization {
    pub fn arm(workflow_id: WorkflowId, scope: PolicyScope) -> Self {
        Self {
            workflow_id,
            scope,
            state: AuthorizationState::Armed,
            allowed: 0,
            denials: Vec::new(),
        }
    }

    pub fn workflow_id(&self) -> &WorkflowId {
        &self.workflow_id
    }

    pub fn scope(&self) -> &PolicyScope {
        &self.scope
    }

    pub fn state(&self) -> AuthorizationState {
        self.state
    }

    /// Armed or paused: a second review must be refused.
    pub fn is_live(&self) -> bool {
        self.state != AuthorizationState::Disarmed
    }

    pub fn pause(&mut self) {
        if self.state == AuthorizationState::Armed {
            self.state = AuthorizationState::Paused;
        }
    }

    pub fn rearm(&mut self) {
        self.state = AuthorizationState::Armed;
    }

    pub fn disarm(&mut self) {
        self.state = AuthorizationState::Disarmed;
    }

    /// Decide one request from this run's step sessions and count it.
    pub fn decide(&mut self, consent: &PermissionConsent) -> Decision {
        let decision = if self.state == AuthorizationState::Disarmed {
            Decision::denied(format!(
                "review run {} has ended; late request denied",
                self.workflow_id
            ))
        } else {
            decide(consent, &self.scope)
        };
        if decision.allowed() {
            self.allowed += 1;
        } else {
            self.denials.push(decision.reason().to_owned());
        }
        decision
    }

    pub fn allowed_count(&self) -> usize {
        self.allowed
    }

    /// Every denial reason, in order.
    pub fn denials(&self) -> &[String] {
        &self.denials
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authorization() -> RunAuthorization {
        let scope = PolicyScope::new(
            "/w".into(),
            "/w/.code-review/r".into(),
            "\"/c\" crtool".into(),
        );
        let id = WorkflowId::try_from("wf-1".to_owned()).expect("a non-empty workflow id is valid");
        RunAuthorization::arm(id, scope)
    }

    fn unknown() -> PermissionConsent {
        PermissionConsent::new(Some("web_fetch"), None, None, None, None, Vec::new())
    }

    #[test]
    fn pause_keeps_deciding_and_disarm_denies_stragglers() {
        let mut auth = authorization();
        auth.pause();
        assert_eq!(auth.state(), AuthorizationState::Paused);
        assert!(auth.is_live());
        assert!(!auth.decide(&unknown()).allowed());
        auth.disarm();
        assert!(!auth.is_live());
        let late = auth.decide(&PermissionConsent::new(
            Some("fs_read"),
            Some("/w/a.rs".into()),
            None,
            None,
            None,
            Vec::new(),
        ));
        assert!(!late.allowed());
        assert!(late.reason().contains("has ended"));
        assert_eq!(auth.denials().len(), 2);
        assert_eq!(auth.allowed_count(), 0);
    }
}

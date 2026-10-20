//! KAS workflow message relay (cyril-9104; part of cyril-fb1m).
//!
//! When a workflow session calls `send_message`, KAS delivers the message to
//! the target session and tells the client with `_kiro/session/notify`
//! (canonicalized to `kiro/session/notify` before it reaches an engine). KAS
//! 0.66.43 builds it in `vPt`:
//!
//! ```json
//! {
//!   "sessionId": "sess_<recipient>",
//!   "callerSessionId": "sess_<sender>",
//!   "message": "…",
//!   "severity": "info" | "success" | "warning" | "error",
//!   "sender": "parent" | "step",
//!   "workflowId": "wf_…", "nodeId": "…", "agentName": "…"
//! }
//! ```
//!
//! `callerSessionId` is present whenever the sender's session is known; the
//! last three name the sending step and ride only on step messages. A step's
//! `warning` is a question (KAS parks the step on it and wakes an idle parent);
//! a `parent` message is the parent model answering or instructing a step.
//! Every field is modelled ([`crate::types::WorkflowMessage`]).

use serde::Deserialize;

use crate::types::{
    Notification, SessionId, WorkflowId, WorkflowMessage, WorkflowMessageSender,
    WorkflowMessageSeverity, WorkflowNodeId,
};

/// The canonicalized method name this adapter claims.
pub(crate) const METHOD: &str = "kiro/session/notify";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNotify {
    session_id: String,
    #[serde(default)]
    caller_session_id: Option<String>,
    message: String,
    severity: String,
    sender: String,
    #[serde(default)]
    workflow_id: Option<String>,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    agent_name: Option<String>,
}

/// Convert a `kiro/session/notify` frame into [`Notification::WorkflowMessage`].
///
/// A different method is `Ok(None)` (the engine tries its next converter). A
/// frame of this method that cannot be modelled — a missing required field,
/// an empty session id, a sender or severity outside KAS's closed sets — is
/// warned and dropped (`Ok(None)`): guessing a role or a severity would
/// misattribute a question or an answer.
pub(crate) fn to_notification(
    method: &str,
    params: &serde_json::Value,
) -> crate::Result<Option<Notification>> {
    if method != METHOD {
        return Ok(None);
    }
    let wire = match WireNotify::deserialize(params) {
        Ok(wire) => wire,
        Err(error) => {
            tracing::warn!(method, %error, "malformed workflow message; dropped");
            return Ok(None);
        }
    };
    let Ok(sender) = WorkflowMessageSender::try_from(wire.sender.as_str()) else {
        tracing::warn!(
            method,
            sender = wire.sender,
            "unknown workflow message sender; dropped"
        );
        return Ok(None);
    };
    let Ok(severity) = WorkflowMessageSeverity::try_from(wire.severity.as_str()) else {
        tracing::warn!(
            method,
            severity = wire.severity,
            "unknown workflow message severity; dropped"
        );
        return Ok(None);
    };
    if wire.session_id.is_empty() {
        tracing::warn!(
            method,
            "malformed workflow message: empty sessionId; dropped"
        );
        return Ok(None);
    }
    let mut message = WorkflowMessage::new(
        SessionId::new(wire.session_id),
        wire.message,
        severity,
        sender,
    );
    // Empty means absent (CLAUDE.md "Guard partial updates").
    if let Some(caller) = wire.caller_session_id.filter(|id| !id.is_empty()) {
        message = message.with_caller(SessionId::new(caller));
    }
    // The identifier types reject only the empty string, which on the wire
    // means "absent" — not a failure worth reporting.
    let workflow_id = wire
        .workflow_id
        .and_then(|id| WorkflowId::try_from(id).ok());
    let node_id = wire
        .node_id
        .and_then(|id| WorkflowNodeId::try_from(id).ok());
    let agent_name = wire.agent_name.filter(|name| !name.is_empty());
    message = message.with_step(workflow_id, node_id, agent_name);
    Ok(Some(Notification::WorkflowMessage(Box::new(message))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_captured_logs<T>(f: impl FnOnce() -> T) -> (T, String) {
        let _capture_lock = crate::test_support::tracing_capture_lock();
        let capture = crate::test_support::CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(capture.clone())
            .finish();
        let result = tracing::subscriber::with_default(subscriber, f);
        let logs =
            crate::test_support::must_succeed(String::from_utf8(capture.captured()), "UTF-8 logs");
        (result, logs)
    }

    fn read_trace(rel: &str) -> Vec<serde_json::Value> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let trace = std::fs::read_to_string(root.join(rel)).expect("read committed trace");
        trace
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).expect("trace row"))
            .collect()
    }

    fn convert(params: &serde_json::Value) -> Option<WorkflowMessage> {
        match to_notification(METHOD, params).expect("conversion never errors") {
            Some(Notification::WorkflowMessage(message)) => Some(*message),
            Some(other) => panic!("unexpected notification {other:?}"),
            None => None,
        }
    }

    /// Every `_kiro/session/notify` of a capture, converted, with the raw
    /// params beside it.
    fn captured(rel: &str, conn: &str) -> Vec<(serde_json::Value, WorkflowMessage)> {
        read_trace(rel)
            .into_iter()
            .filter(|row| {
                row["conn"] == conn
                    && row["dir"] == "agent->client"
                    && row["msg"]["method"] == "_kiro/session/notify"
            })
            .map(|row| {
                let params = row["msg"]["params"].clone();
                let message = convert(&params).expect("captured notify converts");
                (params, message)
            })
            .collect()
    }

    /// cyril-9104 C2: every live `_kiro/session/notify` converts with every
    /// field it carries — the wfplan capture's step question (`warning`),
    /// the parent's answer (`sender:"parent"`, no step fields), and step
    /// verdicts; the gate-off capture's step notes.
    #[rstest::rstest]
    #[case::gateon_wfplan(
        "experiments/conductor-spike/audit-2.32.0/live/kas-lead-wfplan-2.32.0.jsonl",
        "wfplan"
    )]
    #[case::gateoff(
        "experiments/conductor-spike/audit-2.32.0/live/kas-workflow-gateoff-2.32.0.jsonl",
        "main"
    )]
    fn captured_notifies_convert_with_every_field(#[case] rel: &str, #[case] conn: &str) {
        let notes = captured(rel, conn);
        assert!(!notes.is_empty(), "{rel}: the capture relays messages");
        for (raw, message) in &notes {
            assert_eq!(
                Some(message.recipient().as_str()),
                raw["sessionId"].as_str()
            );
            assert_eq!(
                message.caller().map(SessionId::as_str),
                raw["callerSessionId"].as_str()
            );
            assert_eq!(Some(message.message()), raw["message"].as_str());
            assert_eq!(Some(message.severity().as_str()), raw["severity"].as_str());
            assert_eq!(Some(message.sender().as_str()), raw["sender"].as_str());
            assert_eq!(
                message.workflow_id().map(WorkflowId::as_str),
                raw["workflowId"].as_str()
            );
            assert_eq!(
                message.node_id().map(WorkflowNodeId::as_str),
                raw["nodeId"].as_str()
            );
            assert_eq!(message.agent_name(), raw["agentName"].as_str());
        }
    }

    /// cyril-9104 C2: the two frames the feature is built on, by value.
    #[test]
    fn wfplan_question_and_parent_answer() {
        let notes = captured(
            "experiments/conductor-spike/audit-2.32.0/live/kas-lead-wfplan-2.32.0.jsonl",
            "wfplan",
        );
        let question = &notes[0].1;
        assert!(question.is_step_question());
        assert_eq!(
            question.recipient().as_str(),
            "sess_df3e2821-4f61-4189-809d-064794e17d41"
        );
        assert_eq!(
            question.caller().map(SessionId::as_str),
            Some("sess_39b38845-0255-474a-83d7-d8723160f149")
        );
        assert_eq!(
            question.node_id().map(WorkflowNodeId::as_str),
            Some("create-workflow")
        );
        assert_eq!(question.agent_name(), Some("wf-workflow-creator"));
        let answer = &notes[1].1;
        assert_eq!(answer.sender(), WorkflowMessageSender::Parent);
        assert_eq!(answer.severity(), WorkflowMessageSeverity::Info);
        assert!(!answer.is_step_question());
        assert_eq!(
            answer.recipient().as_str(),
            "sess_39b38845-0255-474a-83d7-d8723160f149"
        );
        assert!(
            answer
                .message()
                .starts_with("The proposed plan is exactly right.")
        );
        assert_eq!(answer.workflow_id(), None);
    }

    /// cyril-9104 C2: a frame that cannot be modelled is dropped and
    /// warned, never guessed; another method is left to the next converter.
    #[rstest::rstest]
    #[case::missing_message(serde_json::json!({"sessionId": "s", "severity": "info", "sender": "step"}), "malformed workflow message")]
    #[case::missing_session(serde_json::json!({"message": "m", "severity": "info", "sender": "step"}), "malformed workflow message")]
    #[case::empty_session(serde_json::json!({"sessionId": "", "message": "m", "severity": "info", "sender": "step"}), "malformed workflow message")]
    #[case::unknown_sender(serde_json::json!({"sessionId": "s", "message": "m", "severity": "info", "sender": "sibling"}), "unknown workflow message sender")]
    #[case::unknown_severity(serde_json::json!({"sessionId": "s", "message": "m", "severity": "need_input", "sender": "step"}), "unknown workflow message severity")]
    fn unmodellable_notify_is_dropped_and_warned(
        #[case] params: serde_json::Value,
        #[case] logged: &str,
    ) {
        let (converted, logs) = with_captured_logs(|| convert(&params));
        assert_eq!(converted, None);
        assert!(logs.contains(logged), "missing `{logged}` log: {logs}");
    }

    #[test]
    fn other_methods_pass_through() {
        let result = to_notification("kiro/session/notified", &serde_json::json!({}));
        assert!(matches!(result, Ok(None)));
    }
}

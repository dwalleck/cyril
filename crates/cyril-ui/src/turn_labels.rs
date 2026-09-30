//! Header and notice text for agent-initiated turns and engine-injected
//! messages (cyril-lki9 spec B1/B3/B4/B5).
//!
//! One pure function per concern, like `workflow_format`: `UiState` and the
//! subagent streams turn each announcement or injection into exactly one
//! system line built here. Every header/notice string cyril renders for these
//! lives in this module (module-shape fence C21 R2).

use cyril_core::types::{AgentInitiation, WorkflowRunStatus};
use cyril_core::workflow::WakeLabel;

/// The header line that opens an agent-initiated turn.
///
/// - workflow completion, run known → `─── ⚙ workflow "<name>" <status> · agent follow-up ───` (B1)
/// - workflow completion, run unknown → `─── ⚙ workflow ended · agent follow-up ───` (B4)
/// - any other reason → `─── ⚙ agent-initiated · <reason> ───`, `unspecified`
///   when the engine gave none (B3)
#[must_use]
pub fn header_text(origin: &AgentInitiation, label: Option<&WakeLabel>) -> String {
    if !origin.is_workflow_completion() {
        return format!(
            "─── ⚙ agent-initiated · {} ───",
            origin.reason().unwrap_or("unspecified")
        );
    }
    match label {
        Some(label) => format!(
            "─── ⚙ workflow \"{}\" {} · agent follow-up ───",
            label.name(),
            status_word(label.status())
        ),
        None => "─── ⚙ workflow ended · agent follow-up ───".to_owned(),
    }
}

/// The notice for a workflow completion KAS injected into a busy turn (B5):
/// `⚙ workflow "<name>" <status> (noted mid-turn):` — or `⚙ workflow ended
/// (noted mid-turn):` when the run is unknown — followed by the engine's text
/// verbatim on the next line(s), when it sent any.
#[must_use]
pub fn workflow_notice_text(label: Option<&WakeLabel>, content: Option<&str>) -> String {
    let head = match label {
        Some(label) => format!(
            "⚙ workflow \"{}\" {} (noted mid-turn):",
            label.name(),
            status_word(label.status())
        ),
        None => "⚙ workflow ended (noted mid-turn):".to_owned(),
    };
    match content.filter(|c| !c.is_empty()) {
        Some(content) => format!("{head}\n{content}"),
        None => head,
    }
}

/// The notice for a step verdict KAS relayed into a turn (B5):
/// `⚙ workflow step · <severity>: <message>`. Severity defaults to `info`;
/// KAS's own `[notification/<severity>] ` prefix is removed from the message
/// (it would repeat the severity), and content without it is shown verbatim.
#[must_use]
pub fn step_notice_text(severity: Option<&str>, content: Option<&str>) -> String {
    let severity = severity.filter(|s| !s.is_empty()).unwrap_or("info");
    let message = content.map_or("", strip_notification_prefix);
    format!("⚙ workflow step · {severity}: {message}")
}

/// Removes a leading `[notification/<anything>] ` marker, if present.
fn strip_notification_prefix(content: &str) -> &str {
    content
        .strip_prefix("[notification/")
        .and_then(|rest| rest.split_once("] "))
        .map_or(content, |(_, message)| message)
}

fn status_word(status: WorkflowRunStatus) -> &'static str {
    status.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(reason: Option<&str>, workflow_completion: bool) -> AgentInitiation {
        AgentInitiation::new(reason.map(str::to_owned), workflow_completion)
    }

    fn label(name: &str, status: WorkflowRunStatus) -> WakeLabel {
        WakeLabel::new(name.to_owned(), status)
    }

    /// B1 / B3 / B4 exact strings (spec, approved literals).
    #[test]
    fn header_strings() {
        let wake = origin(Some("workflow-complete-wake"), true);
        assert_eq!(
            header_text(
                &wake,
                Some(&label(
                    "audit-channels-2.26.0",
                    WorkflowRunStatus::Completed
                ))
            ),
            "─── ⚙ workflow \"audit-channels-2.26.0\" completed · agent follow-up ───"
        );
        assert_eq!(
            header_text(
                &wake,
                Some(&label("«review» \"x\"", WorkflowRunStatus::Failed))
            ),
            "─── ⚙ workflow \"«review» \"x\"\" failed · agent follow-up ───",
            "unicode and quotes in a run name are shown as given"
        );
        assert_eq!(
            header_text(&wake, None),
            "─── ⚙ workflow ended · agent follow-up ───"
        );
        assert_eq!(
            header_text(&origin(Some("send-message-wake"), false), None),
            "─── ⚙ agent-initiated · send-message-wake ───"
        );
        assert_eq!(
            header_text(&origin(None, false), None),
            "─── ⚙ agent-initiated · unspecified ───"
        );
        assert_eq!(
            header_text(
                &origin(Some("send-message-wake"), false),
                Some(&label("ignored", WorkflowRunStatus::Completed))
            ),
            "─── ⚙ agent-initiated · send-message-wake ───",
            "a label never renames a non-workflow wake"
        );
    }

    /// B5 exact strings for both injection kinds, with the stress rows.
    #[test]
    fn notice_strings() {
        let wake_text = "A workflow you launched (\"lki9-quick\") completed. Review its results and continue if you were waiting on it.";
        assert_eq!(
            workflow_notice_text(
                Some(&label("lki9-quick", WorkflowRunStatus::Completed)),
                Some(wake_text)
            ),
            format!("⚙ workflow \"lki9-quick\" completed (noted mid-turn):\n{wake_text}")
        );
        assert_eq!(
            workflow_notice_text(None, Some(wake_text)),
            format!("⚙ workflow ended (noted mid-turn):\n{wake_text}")
        );
        assert_eq!(
            workflow_notice_text(None, Some("")),
            "⚙ workflow ended (noted mid-turn):",
            "empty content adds no blank line"
        );
        assert_eq!(
            step_notice_text(Some("success"), Some("[notification/success] OK")),
            "⚙ workflow step · success: OK"
        );
        assert_eq!(
            step_notice_text(Some("error"), Some("plain message, no prefix")),
            "⚙ workflow step · error: plain message, no prefix"
        );
        assert_eq!(
            step_notice_text(None, Some("[notification/success] OK")),
            "⚙ workflow step · info: OK",
            "absent severity defaults to info"
        );
        assert_eq!(step_notice_text(Some(""), None), "⚙ workflow step · info: ");
    }
}

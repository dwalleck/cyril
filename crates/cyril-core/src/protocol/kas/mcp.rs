//! KAS's concrete elicitation policy: visible, context-preserving valid decline.
//! There is no elicitation capability advertisement in the current KAS resolver.

use crate::types::{
    McpElicitationMode, McpElicitationNotice, McpUpdate, Notification, RoutedNotification,
    SessionId, ToolCallId,
};
use agent_client_protocol::schema::v1 as acp;

#[derive(Debug)]
pub(crate) struct ElicitationArgs {
    pub(crate) notice: McpElicitationNotice,
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("MCP elicitation requires nonempty string {key}"))
}

impl ElicitationArgs {
    pub(crate) fn parse(params: &serde_json::Value) -> Result<Self, String> {
        let session_id = SessionId::new(text(params, "sessionId")?);
        let tool_call_id = ToolCallId::new(text(params, "toolCallId")?);
        let elicitation = params
            .get("elicitation")
            .filter(|value| value.is_object())
            .ok_or_else(|| "MCP elicitation requires an elicitation object".to_owned())?;
        let message = text(elicitation, "message")?;
        let mode = text(elicitation, "mode")?;
        let (mode, url, elicitation_id) = match mode {
            "form" => {
                if !elicitation
                    .get("requestedSchema")
                    .is_some_and(serde_json::Value::is_object)
                {
                    return Err("MCP form elicitation requires requestedSchema object".into());
                }
                (McpElicitationMode::Form, None, None)
            }
            "url" => (
                McpElicitationMode::Url,
                Some(text(elicitation, "url")?.to_owned()),
                Some(text(elicitation, "elicitationId")?),
            ),
            _ => (McpElicitationMode::Unknown(mode.to_owned()), None, None),
        };
        let message = match elicitation_id {
            Some(id) => format!(
                "MCP interaction declined: Cyril does not offer interactive elicitation. Elicitation: {id}. Request: {message}"
            ),
            None => format!(
                "MCP interaction declined: Cyril does not offer interactive elicitation. Request: {message}"
            ),
        };
        Ok(Self {
            notice: McpElicitationNotice {
                session_id,
                tool_call_id,
                mode,
                message,
                url,
            },
        })
    }
}

pub(crate) async fn decline(
    args: ElicitationArgs,
    channels: &crate::protocol::domain_mediator::DomainChannels,
) -> acp::Result<acp::ExtResponse> {
    let notice = args.notice;
    let routed = RoutedNotification::scoped(
        notice.session_id.clone(),
        Notification::McpUpdate(McpUpdate::ElicitationDeclined(notice)),
    );
    channels
        .enqueue(crate::protocol::domain_mediator::DomainWork::Routed(routed))
        .await?;
    super::json_ext_response(&serde_json::json!({"action": "decline"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn callback_requires_actual_ids_and_valid_concrete_form_or_url_fields() {
        for params in [
            json!({"elicitation":{"mode":"form","message":"question","requestedSchema":{}}}),
            json!({"sessionId":"session","toolCallId":"","elicitation":{"mode":"form","message":"question","requestedSchema":{}}}),
            json!({"sessionId":"session","toolCallId":"tool","elicitation":{"mode":"form","message":"question"}}),
            json!({"sessionId":"session","toolCallId":"tool","elicitation":{"mode":"url","message":"question","url":"https://auth.invalid"}}),
        ] {
            assert!(ElicitationArgs::parse(&params).is_err());
        }
        let notice=crate::test_support::must_succeed(ElicitationArgs::parse(&json!({"sessionId":"session","toolCallId":"tool","elicitation":{"mode":"url","message":"actual question","url":"https://auth.invalid","elicitationId":"actual-id"}})),"url callback").notice;
        assert_eq!(notice.session_id.as_str(), "session");
        assert_eq!(notice.tool_call_id.as_str(), "tool");
        assert!(notice.message.contains("actual question") && notice.message.contains("actual-id"));
        assert_eq!(notice.url.as_deref(), Some("https://auth.invalid"));
    }
}

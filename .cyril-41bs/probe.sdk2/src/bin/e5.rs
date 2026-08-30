#[path = "../live_support.rs"]
mod live_support;

use std::{collections::BTreeSet, env};

use agent_client_protocol::schema::{
    ProtocolVersion,
    v1::{
        ClientCapabilities, ContentBlock, Implementation, InitializeRequest, NewSessionRequest,
        PromptRequest, TextContent,
    },
};
use agent_client_protocol::{AcpAgent, AcpAgentConfig};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::time::{Duration, timeout};

fn live_capabilities(engine: live_support::EngineKind) -> Result<ClientCapabilities> {
    live_support::capabilities(engine)
}

async fn run_live(
    name: &'static str,
    engine: live_support::EngineKind,
    config: AcpAgentConfig,
    auth: Value,
) -> Result<Value> {
    let scratch = TempDir::new().context("create live Kiro scratch directory")?;
    std::fs::write(scratch.path().join("README.md"), "SDK2 parity probe\n")?;
    let events = live_support::Events::new();
    let events_after = events.clone();
    let capabilities = live_capabilities(engine)?;
    let capability_shape = serde_json::to_value(&capabilities)?;
    let result = timeout(
        Duration::from_secs(live_support::MAX_SESSION_SECONDS),
        live_support::connect_client_with(
            events.clone(),
            auth,
            AcpAgent::new(config),
            async move |connection| {
                let initialize = connection
                    .send_request(
                        InitializeRequest::new(ProtocolVersion::V1)
                            .client_info(Implementation::new("cyril", "0").title("Cyril"))
                            .client_capabilities(capabilities),
                    )
                    .block_task()
                    .await?;
                events_after.push("response:initialize");
                let session = connection
                    .send_request(NewSessionRequest::new(scratch.path()))
                    .block_task()
                    .await?;
                events_after.push("response:session/new");
                let prompt_text = live_support::live_prompt(engine);
                let prompt = connection
                    .send_request(PromptRequest::new(
                        session.session_id.clone(),
                        vec![ContentBlock::Text(TextContent::new(prompt_text))],
                    ))
                    .block_task()
                    .await?;
                events_after.push("response:session/prompt");
                Ok::<_, agent_client_protocol::Error>((initialize, session, prompt))
            },
        ),
    )
    .await
    .with_context(|| format!("{name} live SDK path timed out after 60 seconds"))?
    .map_err(|error| anyhow::anyhow!("{name} live SDK path failed: {error:?}"))?;

    let observed = events.snapshot();
    if observed.len() > live_support::MAX_EVENTS
        || observed
            .last()
            .is_some_and(|event| event == "error:event-limit")
    {
        bail!(
            "{name} captured {} events, exceeding the 1000-event bound",
            observed.len()
        );
    }
    let normalized_events = live_support::normalize_events(&observed);
    let prompt_response_last = normalized_events
        .last()
        .is_some_and(|event| event == "response:session/prompt");
    if !prompt_response_last {
        let last = normalized_events
            .last()
            .cloned()
            .unwrap_or_else(|| "<none>".to_owned());
        bail!("{name} emitted traffic after the prompt response; last event: {last}");
    }
    let agent_message_chunks = observed
        .iter()
        .filter(|event| event.contains(r#""sessionUpdate":"agent_message_chunk""#))
        .count();
    if agent_message_chunks == 0 {
        bail!("{name} completed without an agent message chunk");
    }
    let observed_methods = normalized_events
        .iter()
        .filter_map(|event| {
            event
                .strip_prefix("request:")
                .or_else(|| event.strip_prefix("notification:"))
                .and_then(|rest| rest.split(':').next())
        })
        .collect::<BTreeSet<_>>();
    let permission_index = normalized_events
        .iter()
        .position(|event| event.starts_with("request:session/request_permission"));
    let first_permissioned_tool_index = normalized_events
        .iter()
        .position(|event| event.starts_with("request:terminal/create"));
    let permission_before_tool = match (permission_index, first_permissioned_tool_index) {
        (Some(permission), Some(tool)) => Some(permission < tool),
        _ => None,
    };
    let kas_turn_end_observed = live_support::kas_turn_end_observed(&observed);
    let turn_end_index = observed.iter().position(|event| {
        event.starts_with("notification:session/update:")
            && event.contains(r#""kind":"turn_end""#)
            && event.contains(r#""stopReason":"end_turn""#)
    });
    let response_index = observed
        .iter()
        .position(|event| event == "response:session/prompt");
    let terminal_before_prompt_response = if engine.is_kas() {
        matches!(
            (turn_end_index, response_index),
            (Some(terminal), Some(response)) if terminal < response
        )
    } else {
        response_index.is_some()
    };
    let kas_host_families = live_support::kas_host_families(&observed);
    if engine.is_kas() && !kas_host_families.values().all(|observed| *observed) {
        bail!("{name} did not exercise every KAS host callback family: {kas_host_families:?}");
    }
    if engine.is_kas() && permission_before_tool != Some(true) {
        bail!("{name} did not request permission before the first host tool callback");
    }
    if engine.is_kas() && (!kas_turn_end_observed || !terminal_before_prompt_response) {
        bail!("{name} did not observe KAS end_turn before the prompt response");
    }
    let mut divergences = Vec::new();
    if !engine.is_kas() {
        divergences.push("v2:host-callbacks-not-live-proven-without-tool-trigger".to_owned());
    }
    Ok(json!({
        "engine": name,
        "protocol_version": result.0.protocol_version,
        "client_capabilities": capability_shape,
        "session_id_present": !result.1.session_id.to_string().is_empty(),
        "stop_reason": format!("{:?}", result.2.stop_reason),
        "event_count": observed.len(),
        "events": observed,
        "normalized_events": normalized_events,
        "observed_methods": observed_methods,
        "method_counts": live_support::method_counts(&events.snapshot()),
        "agent_message_chunks": agent_message_chunks,
        "prompt_response_last": prompt_response_last,
        "permission_before_tool": permission_before_tool,
        "terminal_before_prompt_response": terminal_before_prompt_response,
        "kas_turn_end_observed": kas_turn_end_observed,
        "kas_host_families": kas_host_families,
        "not_exercised": ["typed_error", "outer_response_id", "cancellation"],
        "evidence_layers": {
            "authenticated_live": true,
            "deterministic_matrix": false,
            "capture_backed": false,
            "divergences": divergences,
        },
        "within_event_bound": observed.len() <= live_support::MAX_EVENTS,
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = env::args().nth(1).unwrap_or_else(|| "all".to_owned());
    let matrix_auth = json!({
        "accessToken": "<in-memory-probe>",
        "expiresAt": "2099-01-01T00:00:00Z",
        "profileArn": "<in-memory-probe>",
    });
    let (auth, run_v2, run_kas) = match mode.as_str() {
        "matrix" => (None, false, false),
        "v2" => (Some(live_support::load_auth_response()?), true, false),
        "kas" => (Some(live_support::load_auth_response()?), false, true),
        "all" => (Some(live_support::load_auth_response()?), true, true),
        other => bail!("unknown mode `{other}`; expected matrix, v2, kas, or all"),
    };
    let callback_matrix = live_support::run_direct_matrix(matrix_auth).await?;
    let mut live = Vec::new();
    if run_v2 {
        live.push(
            run_live(
                live_support::EngineKind::V2.label(),
                live_support::EngineKind::V2,
                AcpAgentConfig::new("kiro-cli").arg("acp"),
                auth.clone()
                    .context("v2 live authentication was not loaded")?,
            )
            .await?,
        );
    }
    if run_kas {
        live.push(
            run_live(
                live_support::EngineKind::Kas.label(),
                live_support::EngineKind::Kas,
                AcpAgentConfig::new("kiro-cli").args(["acp", "--agent-engine", "v3"]),
                auth.context("KAS live authentication was not loaded")?,
            )
            .await?,
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C2", "C7"],
            "sdk_version": "2.0.0",
            "wire_version": "V1",
            "mode": mode,
            "callback_matrix": callback_matrix,
            "live": live,
            "evidence_phases": [
                "authenticated stable-v1 lifecycle/tool-prompt",
                "deterministic direct SDK callback matrix",
                "capture-backed versioned extension references",
            ],
            "credential_logged": false,
            "production_state_modified": false,
        }))?
    );
    Ok(())
}

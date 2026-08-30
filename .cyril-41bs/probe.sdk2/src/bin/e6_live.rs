#[path = "../live_support.rs"]
mod live_support;

use std::env;

use agent_client_protocol::schema::{
    InitializeProxyRequest, ProtocolVersion,
    v1::{
        ClientCapabilities, ContentBlock, Implementation, InitializeRequest, NewSessionRequest,
        PromptRequest, TextContent,
    },
};
use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Agent, Client, Conductor, ConnectTo, Error, Proxy, Responder,
    UntypedMessage,
};
use agent_client_protocol_conductor::{ConductorImpl, ProxiesAndAgent};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

fn annotate_params(params: &mut Value) {
    let Some(params) = params.as_object_mut() else {
        return;
    };
    let meta = params.entry("_meta").or_insert_with(|| json!({}));
    if let Some(meta) = meta.as_object_mut() {
        meta.insert("cyrilProbeObserved".to_owned(), Value::Bool(true));
    }
}

#[derive(Clone)]
struct TransformingAuditProxy {
    transformed: live_support::Events,
}

impl ConnectTo<Conductor> for TransformingAuditProxy {
    async fn connect_to(self, client: impl ConnectTo<Proxy>) -> Result<(), Error> {
        let client_transformed = self.transformed.clone();
        let agent_transformed = self.transformed;
        Proxy
            .builder()
            .on_receive_request_from(
                Client,
                async |request: InitializeProxyRequest, responder, cx| {
                    cx.send_request_to(Agent, request.initialize)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Client,
                async move |mut request: UntypedMessage, responder: Responder<Value>, cx| {
                    if request.method == "session/prompt" {
                        annotate_params(&mut request.params);
                        client_transformed.push("transformed:client:session/prompt");
                    }
                    cx.send_request_to(Agent, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Agent,
                async |request: UntypedMessage, responder: Responder<Value>, cx| {
                    cx.send_request_to(Client, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification_from(
                Client,
                async |notification: UntypedMessage, cx| {
                    cx.send_notification_to(Agent, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification_from(
                Agent,
                async move |mut notification: UntypedMessage, cx| {
                    if matches!(
                        notification.method.as_str(),
                        "_kiro.dev/metadata" | "_kiro/mcp/status"
                    ) {
                        annotate_params(&mut notification.params);
                        agent_transformed
                            .push(format!("transformed:agent:{}", notification.method));
                    }
                    cx.send_notification_to(Client, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

#[derive(Clone)]
struct MatrixTransformProxy {
    transformed: live_support::Events,
}

impl ConnectTo<Conductor> for MatrixTransformProxy {
    async fn connect_to(self, client: impl ConnectTo<Proxy>) -> Result<(), Error> {
        let marker = self.transformed;
        Proxy
            .builder()
            .on_receive_request_from(
                Client,
                async |request: InitializeProxyRequest, responder, cx| {
                    cx.send_request_to(Agent, request.initialize)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Client,
                async |request: UntypedMessage, responder: Responder<Value>, cx| {
                    cx.send_request_to(Agent, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_request_from(
                Agent,
                async move |mut request: UntypedMessage, responder: Responder<Value>, cx| {
                    if request.method != live_support::MATRIX_BARRIER_METHOD
                        && request.method != live_support::MATRIX_ERROR_METHOD
                    {
                        let method = request.method.clone();
                        annotate_params(&mut request.params);
                        marker.push(format!("transformed:agent:{method}"));
                    }
                    cx.send_request_to(Client, request)
                        .forward_response_to(responder)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .on_receive_notification_from(
                Client,
                async |notification: UntypedMessage, cx| {
                    cx.send_notification_to(Agent, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_notification_from(
                Agent,
                async |notification: UntypedMessage, cx| {
                    cx.send_notification_to(Client, notification)?;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_to(client)
            .await
    }
}

async fn run_topology<C>(
    engine: live_support::EngineKind,
    topology: &'static str,
    conductor: C,
    auth: Value,
    transformed: live_support::Events,
) -> Result<Value>
where
    C: ConnectTo<Client>,
{
    let scratch = TempDir::new().context("create live conductor scratch directory")?;
    let events = live_support::Events::new();
    let events_after = events.clone();
    let events_for_run = events_after.clone();
    let capabilities: ClientCapabilities = live_support::capabilities(engine)?;
    let capability_shape = serde_json::to_value(&capabilities)?;
    let result = timeout(
        Duration::from_secs(live_support::MAX_SESSION_SECONDS),
        live_support::connect_client_with(events, auth, conductor, async move |cx| {
            let initialize = cx
                .send_request(
                    InitializeRequest::new(ProtocolVersion::V1)
                        .client_info(Implementation::new("cyril", "0").title("Cyril"))
                        .client_capabilities(capabilities),
                )
                .block_task()
                .await?;
            events_for_run.push("response:initialize");
            let session = cx
                .send_request(NewSessionRequest::new(scratch.path()))
                .block_task()
                .await?;
            events_for_run.push("response:session/new");
            let prompt = cx
                .send_request(PromptRequest::new(
                    session.session_id.clone(),
                    vec![ContentBlock::Text(TextContent::new(
                        live_support::live_prompt(engine),
                    ))],
                ))
                .block_task()
                .await?;
            events_for_run.push("response:session/prompt");
            Ok::<_, Error>((initialize, session, prompt))
        }),
    )
    .await
    .with_context(|| format!("{}/{topology} timed out after 60 seconds", engine.label()))?
    .map_err(|error| anyhow::anyhow!("{}/{topology} failed: {error:?}", engine.label()))?;
    let observed = events_after.snapshot();
    if observed.len() > live_support::MAX_EVENTS
        || observed
            .last()
            .is_some_and(|event| event == "error:event-limit")
    {
        bail!(
            "{}/{topology} captured {} events, exceeding the 1000-event bound",
            engine.label(),
            observed.len()
        );
    }
    let normalized_events = live_support::normalize_events(&observed);
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
    let agent_message_chunks = observed
        .iter()
        .filter(|event| event.contains(r#""sessionUpdate":"agent_message_chunk""#))
        .count();
    if agent_message_chunks == 0 {
        bail!(
            "{}/{topology} completed without an agent message chunk",
            engine.label()
        );
    }
    let prompt_response_last = normalized_events
        .last()
        .is_some_and(|event| event == "response:session/prompt");
    if !prompt_response_last {
        let last = normalized_events
            .last()
            .cloned()
            .unwrap_or_else(|| "<none>".to_owned());
        bail!(
            "{}/{topology} emitted traffic after prompt response; last event: {last}",
            engine.label()
        );
    }
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
        bail!(
            "{}/{topology} did not exercise every KAS host callback family: {kas_host_families:?}",
            engine.label()
        );
    }
    if engine.is_kas() && permission_before_tool != Some(true) {
        bail!(
            "{}/{topology} did not request permission before the first host tool callback",
            engine.label()
        );
    }
    if engine.is_kas() && (!kas_turn_end_observed || !terminal_before_prompt_response) {
        bail!(
            "{}/{topology} did not observe KAS end_turn before the prompt response",
            engine.label()
        );
    }
    let mut divergences = Vec::new();
    if !engine.is_kas() {
        divergences.push("v2:host-callbacks-not-live-proven-without-tool-trigger".to_owned());
    }
    Ok(json!({
        "engine": engine.label(),
        "topology": topology,
        "client_capabilities": capability_shape,
        "protocol_version": result.0.protocol_version,
        "session_id_present": !result.1.session_id.to_string().is_empty(),
        "stop_reason": result.2.stop_reason,
        "events": observed,
        "normalized_events": normalized_events,
        "method_counts": live_support::method_counts(&events_after.snapshot()),
        "agent_message_chunks": agent_message_chunks,
        "observed_methods": live_support::normalize_methods(&events_after.snapshot()),
        "proxy_transformations": transformed.snapshot(),
        "kas_turn_end_observed": kas_turn_end_observed,
        "terminal_before_prompt_response": terminal_before_prompt_response,
        "kas_host_families": kas_host_families,
        "permission_before_tool": permission_before_tool,
        "not_exercised": ["typed_error", "outer_response_id", "cancellation"],
        "prompt_response_last": prompt_response_last,
        "evidence_layers": {
            "authenticated_live": true,
            "deterministic_matrix": false,
            "capture_backed": false,
            "divergences": divergences,
        },
        "within_event_bound": observed.len() <= live_support::MAX_EVENTS,
    }))
}

async fn run_live_engine(
    engine: live_support::EngineKind,
    args: &'static [&'static str],
    auth: Value,
) -> Result<Vec<Value>> {
    let zero = ConductorImpl::new_agent(
        format!("{}-zero", engine.label()),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        )),
    );
    let zero = run_topology(
        engine,
        "zero-proxy",
        zero,
        auth.clone(),
        live_support::Events::new(),
    )
    .await?;

    let noop = ConductorImpl::new_agent(
        format!("{}-noop", engine.label()),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        ))
        .proxy(Proxy.builder()),
    );
    let noop = run_topology(
        engine,
        "no-op-proxy",
        noop,
        auth.clone(),
        live_support::Events::new(),
    )
    .await?;

    let transformed = live_support::Events::new();
    let transforming = ConductorImpl::new_agent(
        format!("{}-transforming", engine.label()),
        ProxiesAndAgent::new(AcpAgent::new(
            AcpAgentConfig::new("kiro-cli").args(args.iter().copied()),
        ))
        .proxy(TransformingAuditProxy {
            transformed: transformed.clone(),
        }),
    );
    let transforming = run_topology(
        engine,
        "transforming-proxy",
        transforming,
        auth,
        transformed,
    )
    .await?;
    Ok(vec![zero, noop, transforming])
}

async fn run_matrix_topology(
    topology: &'static str,
    kind: &'static str,
    auth: Value,
) -> Result<Value> {
    let (done_tx, done_rx) = oneshot::channel();
    let agent = live_support::MatrixAgent::new(done_tx);
    let transformed = live_support::Events::new();
    let conductor = match kind {
        "zero-proxy" => ConductorImpl::new_agent("matrix-zero", ProxiesAndAgent::new(agent)),
        "no-op-proxy" => ConductorImpl::new_agent(
            "matrix-noop",
            ProxiesAndAgent::new(agent).proxy(Proxy.builder()),
        ),
        "transforming-proxy" => ConductorImpl::new_agent(
            "matrix-transform",
            ProxiesAndAgent::new(agent).proxy(MatrixTransformProxy {
                transformed: transformed.clone(),
            }),
        ),
        other => bail!("unknown matrix topology `{other}`"),
    };
    let mut result = live_support::run_conductor_matrix(topology, conductor, auth, done_rx).await?;
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "proxy_transformations".to_owned(),
            json!(transformed.snapshot()),
        );
        object.insert("evidence_layer".to_owned(), json!("deterministic"));
    }
    Ok(result)
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = env::args().nth(1).unwrap_or_else(|| "all".to_owned());
    let matrix_auth = json!({
        "accessToken": "<in-memory-probe>",
        "expiresAt": "2099-01-01T00:00:00Z",
        "profileArn": "<in-memory-probe>",
    });
    let auth = if mode == "matrix" {
        matrix_auth.clone()
    } else {
        live_support::load_auth_response()?
    };
    let mut callback_matrices = vec![live_support::run_direct_matrix(matrix_auth.clone()).await?];
    let mut topologies = Vec::new();
    match mode.as_str() {
        "matrix" => {
            for topology in ["zero-proxy", "no-op-proxy", "transforming-proxy"] {
                callback_matrices
                    .push(run_matrix_topology(topology, topology, auth.clone()).await?);
            }
        }
        "v2" => {
            for topology in ["zero-proxy", "no-op-proxy", "transforming-proxy"] {
                callback_matrices
                    .push(run_matrix_topology(topology, topology, matrix_auth.clone()).await?);
            }
            topologies.extend(run_live_engine(live_support::EngineKind::V2, &["acp"], auth).await?);
        }
        "kas" => {
            for topology in ["zero-proxy", "no-op-proxy", "transforming-proxy"] {
                callback_matrices
                    .push(run_matrix_topology(topology, topology, matrix_auth.clone()).await?);
            }
            topologies.extend(
                run_live_engine(
                    live_support::EngineKind::Kas,
                    &["acp", "--agent-engine", "v3"],
                    auth,
                )
                .await?,
            );
        }
        "all" => {
            for topology in ["zero-proxy", "no-op-proxy", "transforming-proxy"] {
                callback_matrices
                    .push(run_matrix_topology(topology, topology, matrix_auth.clone()).await?);
            }
            let v2 = run_live_engine(live_support::EngineKind::V2, &["acp"], auth.clone()).await?;
            let kas = run_live_engine(
                live_support::EngineKind::Kas,
                &["acp", "--agent-engine", "v3"],
                auth,
            )
            .await?;
            topologies.extend(v2);
            topologies.extend(kas);
        }
        other => bail!("unknown mode `{other}`; expected matrix, v2, kas, or all"),
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C2", "C7"],
            "sdk_version": "2.0.0",
            "wire_version": "V1",
            "mode": mode,
            "callback_matrices": callback_matrices,
            "topologies": topologies,
            "evidence_phases": [
                "authenticated stable-v1 lifecycle/tool-prompt",
                "deterministic direct/conductor callback matrix",
                "capture-backed versioned extension references",
            ],
            "credential_logged": false,
            "production_state_modified": false,
        }))?
    );
    Ok(())
}

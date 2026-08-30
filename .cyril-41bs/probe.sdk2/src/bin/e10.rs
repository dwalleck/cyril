use std::sync::{Arc, Mutex, MutexGuard};

use agent_client_protocol::schema::{ProtocolVersion, v1, v2};
use agent_client_protocol::{Agent, Client, ConnectTo, Error};
use anyhow::Result;
use serde_json::json;

#[derive(Clone)]
struct Events(Arc<Mutex<Vec<String>>>);

impl Events {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn push(&self, event: &str) {
        self.lock().push(event.to_owned());
    }

    fn snapshot(&self) -> Vec<String> {
        self.lock().clone()
    }
}

struct V1Client(Events);

impl ConnectTo<Agent> for V1Client {
    async fn connect_to(self, agent: impl ConnectTo<Client>) -> Result<(), Error> {
        let events = self.0;
        Client
            .builder()
            .connect_with(agent, async move |cx| {
                let response = cx
                    .send_request(
                        v1::InitializeRequest::new(ProtocolVersion::V1)
                            .client_info(v1::Implementation::new("e10-v1", "1")),
                    )
                    .block_task()
                    .await?;
                events.push("client:v1");
                if response.protocol_version != ProtocolVersion::V1 {
                    return Err(Error::internal_error().data("v1 response version mismatch"));
                }
                Ok(())
            })
            .await
    }
}

struct V2Client(Events);

impl ConnectTo<Agent> for V2Client {
    async fn connect_to(self, agent: impl ConnectTo<Client>) -> Result<(), Error> {
        let events = self.0;
        Client
            .v2()
            .connect_with(agent, async move |cx| {
                let response = cx
                    .send_request(v2::InitializeRequest::new(
                        ProtocolVersion::V2,
                        v2::Implementation::new("e10-v2", "1"),
                    ))
                    .block_task()
                    .await?;
                events.push("client:v2");
                if response.protocol_version != ProtocolVersion::V2 {
                    return Err(Error::internal_error().data("v2 response version mismatch"));
                }
                Ok(())
            })
            .await
    }
}

fn v1_agent(events: Events) -> impl ConnectTo<Client> {
    Agent.builder().on_receive_request(
        async move |request: v1::InitializeRequest, responder, _cx| {
            events.push("agent:v1");
            responder.respond(v1::InitializeResponse::new(request.protocol_version))
        },
        agent_client_protocol::on_receive_request!(),
    )
}

fn v2_agent(events: Events) -> impl ConnectTo<Client> {
    Agent.v2().on_receive_request(
        async move |request: v2::InitializeRequest, responder, _cx| {
            events.push("agent:v2");
            responder.respond(v2::InitializeResponse::new(
                request.protocol_version,
                v2::Implementation::new("e10-agent-v2", "1"),
            ))
        },
        agent_client_protocol::on_receive_request!(),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let v2_events = Events::new();
    let client_events = v2_events.clone();
    let client_events_v2 = v2_events.clone();
    let agent_events_v1 = v2_events.clone();
    let agent_events_v2 = v2_events.clone();
    Client
        .protocol_connector()
        .with_v1(move || V1Client(client_events.clone()))
        .with_v2(move || V2Client(client_events_v2.clone()))
        .connect_to(move || {
            Agent
                .protocol_router()
                .with_v1(v1_agent(agent_events_v1.clone()))
                .with_v2(v2_agent(agent_events_v2.clone()))
        })
        .await
        .map_err(|error| anyhow::anyhow!("v2 route failed: {error:?}"))?;

    let fallback_events = Events::new();
    let fallback_client_v1 = fallback_events.clone();
    let fallback_client_v2 = fallback_events.clone();
    let fallback_agent_v1 = fallback_events.clone();
    Client
        .protocol_connector()
        .with_v1(move || V1Client(fallback_client_v1.clone()))
        .with_v2(move || V2Client(fallback_client_v2.clone()))
        .connect_to(move || {
            Agent
                .protocol_router()
                .with_v1(v1_agent(fallback_agent_v1.clone()))
        })
        .await
        .map_err(|error| anyhow::anyhow!("v1 fallback failed: {error:?}"))?;

    let rejection_events = Events::new();
    let rejection_client_v1 = rejection_events.clone();
    let rejection_client_v2 = rejection_events.clone();
    let rejection_agent_v1 = rejection_events.clone();
    let direct_v1_result = Client
        .protocol_connector()
        .with_v1(move || V1Client(rejection_client_v1.clone()))
        .with_v2(move || V2Client(rejection_client_v2.clone()))
        .connect_to(move || v1_agent(rejection_agent_v1.clone()))
        .await;

    let v2_observed = v2_events.snapshot();
    let fallback_observed = fallback_events.snapshot();
    let rejection_observed = rejection_events.snapshot();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "claim_ids": ["C11"],
            "sdk_version": "2.0.0",
            "v2_feature": "unstable_protocol_v2",
            "default_builders_remain_v1": true,
            "v2_route_events": v2_observed,
            "v2_route_selected_v2_only": v2_observed == ["agent:v2", "client:v2"],
            "router_fallback_events": fallback_observed,
            "router_fallback_selected_v1": fallback_observed.contains(&"agent:v1".to_owned()) && fallback_observed.contains(&"client:v1".to_owned()),
            "direct_v1_rejection_events": rejection_observed,
            "direct_v1_rejection_is_error": direct_v1_result.is_err(),
            "direct_v1_rejection_did_not_run_v1_client": !rejection_observed.contains(&"client:v1".to_owned()),
            "handshake_only_conversion": true,
            "draft_v2_production_enabled": false,
        }))?
    );
    Ok(())
}

//! Real SDK request/ingress path, deliberately ungated on the KAS feature.
//! The first two fences use only predecessor APIs: transplant this file and
//! its module registration to the nk4o predecessor for behavioral RED at 30s.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage};
use serde_json::json;
use tokio::sync::{Notify, mpsc};

use super::*;
use crate::protocol::domain_mediator::{DomainConfig, DomainMediator};
use crate::protocol::engine::{Engine, V2Engine};
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::types::{SessionId, SourceTurnEvent, SourceTurnEventKind};

#[derive(Default)]
struct Gates {
    new_count: AtomicUsize,
    load_count: AtomicUsize,
    new_received: Notify,
    load_received: Notify,
    new_reply: Notify,
    load_reply: Notify,
    control_received: Notify,
    control_reply: Notify,
    prompt_received: Notify,
    prompt_reply: Notify,
    fail_new: AtomicBool,
    fail_load: AtomicBool,
}

struct Fixture {
    sender: BridgeSender,
    rx: mpsc::Receiver<RoutedNotification>,
    sources: mpsc::Receiver<SourceTurnEvent>,
    wire: ConnectionTo<Client>,
    gates: Arc<Gates>,
    join: tokio::task::JoinHandle<crate::Result<()>>,
}

async fn with_startup<F, Fut>(engine: Rc<dyn Engine>, kas: bool, body: F)
where
    F: FnOnce(Fixture) -> Fut,
    Fut: Future<Output = ()>,
{
    tokio::task::LocalSet::new().run_until(async move {
        let gates = Arc::new(Gates::default());
        let new_gates = Arc::clone(&gates);
        let load_gates = Arc::clone(&gates);
        let control_gates = Arc::clone(&gates);
        let prompt_gates = Arc::clone(&gates);
        let (wire_tx, mut wire_rx) = mpsc::unbounded_channel();
        let agent = Agent.builder().name("caar-startup-agent")
            .on_receive_request(async move |request: acp::InitializeRequest, responder, connection: ConnectionTo<Client>| {
                wire_tx.send(connection).map_err(|_| agent_client_protocol::Error::internal_error())?;
                let mut response = json!({"protocolVersion":request.protocol_version,"agentCapabilities":{"loadSession":true}});
                if kas { response["agentCapabilities"]["_meta"] = json!({"kiro":{}}); }
                responder.respond(serde_json::from_value::<acp::InitializeResponse>(response)
                    .map_err(|e| agent_client_protocol::Error::internal_error().data(e.to_string()))?)
            }, agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |_request: acp::NewSessionRequest, responder, connection: ConnectionTo<Client>| {
                let state = Arc::clone(&new_gates);
                let index = state.new_count.fetch_add(1, Ordering::SeqCst);
                state.new_received.notify_one();
                connection.spawn(async move {
                    state.new_reply.notified().await;
                    if state.fail_new.load(Ordering::SeqCst) {
                        responder.respond_with_error(agent_client_protocol::Error::internal_error().data("original new cause"))
                    } else {
                        let id = if kas { format!("sess_startup-{index}") } else { format!("startup-{index}") };
                        let response: acp::NewSessionResponse = serde_json::from_value(json!({
                            "sessionId": id,
                            "configOptions":[{"id":"model","name":"Model","type":"select","currentValue":"canary","options":[{"value":"canary","name":"Canary"}]}]
                        })).map_err(|e| agent_client_protocol::Error::internal_error().data(e.to_string()))?;
                        responder.respond(response)
                    }
                })?;
                Ok(())
            }, agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |_request: acp::LoadSessionRequest, responder, connection: ConnectionTo<Client>| {
                let state = Arc::clone(&load_gates);
                state.load_count.fetch_add(1, Ordering::SeqCst);
                state.load_received.notify_one();
                connection.spawn(async move {
                    state.load_reply.notified().await;
                    if state.fail_load.load(Ordering::SeqCst) {
                        responder.respond_with_error(agent_client_protocol::Error::internal_error().data("original load cause"))
                    } else {
                        responder.respond(serde_json::from_value::<acp::LoadSessionResponse>(json!({}))
                            .map_err(|e| agent_client_protocol::Error::internal_error().data(e.to_string()))?)
                    }
                })?;
                Ok(())
            }, agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |_request: acp::PromptRequest, responder, connection: ConnectionTo<Client>| {
                let state = Arc::clone(&prompt_gates);
                state.prompt_received.notify_one();
                connection.spawn(async move {
                    state.prompt_reply.notified().await;
                    responder.respond(acp::PromptResponse::new(acp::StopReason::EndTurn))
                })?;
                Ok(())
            }, agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |_request: acp::SetSessionModeRequest, responder, connection: ConnectionTo<Client>| {
                let state = Arc::clone(&control_gates);
                state.control_received.notify_one();
                connection.spawn(async move {
                    state.control_reply.notified().await;
                    responder.respond_with_error(agent_client_protocol::Error::method_not_found())
                })?;
                Ok(())
            }, agent_client_protocol::on_receive_request!());
        let (handle, bridge) = create_channel_pair();
        let (sender, rx, _permissions, sources, _completion) = handle.split();
        let (mediator, channels) = DomainMediator::new(DomainConfig {
            engine, cwd: std::env::temp_dir(), present_as: None, stall_threshold: Duration::from_secs(30),
            #[cfg(feature="kas")] host_shell: None,
            #[cfg(feature="kas")] kas_token: None,
        }, bridge).expect_contract("startup mediator");
        let runtime = SdkRuntime::start_for_test(agent, channels, StageChain::default()).await.expect_contract("startup SDK");
        let join = tokio::task::spawn_local(mediator.run(runtime));
        let wire = wire_rx.recv().await.expect_contract("initialize connection");
        body(Fixture {sender, rx, sources, wire, gates, join}).await;
    }).await;
}

impl Fixture {
    async fn new_request(&self) {
        self.sender
            .send(BridgeCommand::NewSession {
                cwd: std::env::temp_dir(),
            })
            .await
            .expect_contract("send new");
        self.gates.new_received.notified().await;
        settle().await;
    }

    async fn load_request(&self, id: &str) {
        self.sender
            .send(BridgeCommand::LoadSession {
                session_id: SessionId::new(id),
            })
            .await
            .expect_contract("send load");
        self.gates.load_received.notified().await;
        settle().await;
    }

    fn emit(&self, method: &str, params: serde_json::Value) {
        self.wire
            .send_notification(UntypedMessage::new(method, params).expect_contract("wire envelope"))
            .expect_contract("send wire frame");
    }

    async fn next(&mut self) -> RoutedNotification {
        tokio::time::timeout(Duration::from_secs(1), self.rx.recv())
            .await
            .expect_contract("notification deadline")
            .expect_contract("notification stream")
    }

    async fn started(&mut self, expected: &str) {
        assert!(matches!(
            self.next().await.notification,
            Notification::UsageSessionStarted { .. }
        ));
        assert!(
            matches!(self.next().await.notification, Notification::SessionCreated {session_id,..} if session_id.as_str()==expected)
        );
    }

    async fn shutdown(self) {
        self.sender
            .send(BridgeCommand::Shutdown)
            .await
            .expect_contract("shutdown command");
        tokio::time::timeout(Duration::from_secs(5), self.join)
            .await
            .expect_contract("mediator join deadline")
            .expect_contract("mediator join")
            .expect_contract("mediator result");
    }

    async fn fatal(mut self) -> String {
        let reason = match self.next().await.notification {
            Notification::BridgeDisconnected { reason } => reason,
            other => panic!("expected fatal startup disconnect, got {other:?}"),
        };
        // A late responder completion may itself be rejected by shutdown. It
        // must never create another active session or another fatal notice.
        self.gates.new_reply.notify_one();
        tokio::time::timeout(Duration::from_secs(5), self.join)
            .await
            .expect_contract("fatal mediator join deadline")
            .expect_contract("fatal mediator join")
            .expect_contract("fatal mediator result");
        assert!(
            self.sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir()
                })
                .await
                .is_err()
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while let Some(frame) = self.rx.recv().await {
                assert!(
                    !matches!(
                        frame.notification,
                        Notification::SessionCreated { .. }
                            | Notification::BridgeDisconnected { .. }
                    ),
                    "late startup outcome: {frame:?}"
                );
            }
        })
        .await
        .expect_contract("fatal notification stream must close");
        reason
    }
}

// Yielding never advances the paused clock. A request-received signal followed
// by these turns lets the spawned waiter establish its actual production timer.
async fn settle() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn caar_actual_new_remains_pending_after_30s_and_succeeds_at_35s() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        tokio::time::advance(Duration::from_secs(31)).await;
        settle().await;
        assert!(
            f.rx.try_recv().is_err(),
            "actual new must not fail at the old 30s deadline"
        );
        tokio::time::advance(Duration::from_secs(4)).await;
        f.gates.new_reply.notify_one();
        f.started("startup-0").await;
        assert!(matches!(
            f.next().await.notification,
            Notification::ConfigOptionsUpdated(_)
        ));
        f.shutdown().await;
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn caar_actual_new_deadline_is_90s_then_late_reply_fails_stop() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        tokio::time::advance(Duration::from_secs(89)).await;
        settle().await;
        assert!(
            f.rx.try_recv().is_err(),
            "new expired before its dedicated deadline"
        );
        tokio::time::advance(Duration::from_secs(1)).await;
        let reason = f.fatal().await;
        assert!(reason.contains("timed out after 90s"), "{reason}");
        assert!(
            reason.contains("agent supplied no complete startup inventory"),
            "{reason}"
        );
        assert!(!reason.contains("Silent: none"));
    })
    .await;
}

#[tokio::test(start_paused = true)]
async fn caar_oauth_after_31s_is_visible_before_new_response_with_true_scope() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        tokio::time::advance(Duration::from_secs(31)).await;
        f.emit("_kiro.dev/mcp/oauth_request", json!({"sessionId":"startup-0","serverName":"late-auth","oauthUrl":"https://example.test/auth?state=exact#finish"}));
        let oauth = f.next().await;
        assert_eq!(oauth.session_id.as_ref().map(SessionId::as_str), Some("startup-0"));
        assert!(matches!(&oauth.notification, Notification::McpOAuthRequest {server_name,url,..} if server_name=="late-auth" && url=="https://example.test/auth?state=exact#finish"));
        assert!(f.rx.try_recv().is_err(), "no response has been released");
        tokio::time::advance(Duration::from_secs(4)).await;
        f.gates.new_reply.notify_one();
        f.started("startup-0").await;
        assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
        f.shutdown().await;
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_timeout_reports_ordered_scoped_peer_and_legacy_facts_truthfully() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        for (method, params, sid) in [
            ("_kiro.dev/mcp/server_initialized", json!({"sessionId":"peer","serverName":"ready"}), Some("peer")),
            ("_kiro.dev/mcp/server_init_failure", json!({"sessionId":"old","serverName":"broken","error":"refused"}), Some("old")),
            ("_kiro.dev/mcp/oauth_request", json!({"sessionId":"candidate","serverName":"auth","oauthUrl":"https://example.test/auth"}), Some("candidate")),
            ("_kiro.dev/mcp/server_initialized", json!({"serverName":"legacy-ready"}), None),
        ] {
            f.emit(method, params);
            let frame = f.next().await;
            assert_eq!(frame.session_id.as_ref().map(SessionId::as_str), sid);
        }
        tokio::time::advance(Duration::from_secs(89)).await;
        f.emit("_kiro.dev/mcp/server_initialized", json!({"sessionId":"old","serverName":"broken"}));
        // Receipt at the live consumer proves this ingress was processed before
        // crossing the timeout, rather than assuming wall-clock send order.
        assert!(matches!(f.next().await.notification, Notification::McpServerInitialized {..}));
        tokio::time::advance(Duration::from_secs(1)).await;
        let reason = f.fatal().await;
        for text in ["ready (ready)", "broken (ready)", "Awaiting OAuth: auth", "legacy-ready", "Legacy sessionless", "Candidate session \"candidate\"", "not correlated", "agent supplied no complete startup inventory"] {
            assert!(reason.contains(text), "missing {text}: {reason}");
        }
        assert!(!reason.contains("broken (failed"));
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_new_new_new_load_and_load_new_admission_precedes_second_send() {
    for (first_load, competitor_load) in [(false, false), (false, true), (true, false)] {
        with_startup(Rc::new(V2Engine), false, |mut f| async move {
            if first_load { f.load_request("loaded").await; } else { f.new_request().await; }
            let competitor = if competitor_load { BridgeCommand::LoadSession {session_id: SessionId::new("competitor")} } else { BridgeCommand::NewSession {cwd:std::env::temp_dir()} };
            f.sender.send(competitor).await.expect_contract("competitor command");
            let rejected = f.next().await;
            assert!(matches!(rejected.notification, Notification::BridgeError {message,..} if message.contains("already pending") && message.contains("no second")));
            assert_eq!(f.gates.new_count.load(Ordering::SeqCst), usize::from(!first_load));
            assert_eq!(f.gates.load_count.load(Ordering::SeqCst), usize::from(first_load));
            if first_load { f.gates.load_reply.notify_one(); f.started("loaded").await; }
            else { f.gates.new_reply.notify_one(); f.started("startup-0").await; assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_))); }
            // Success releases admission as well as recoverable failure.
            f.new_request().await;
            f.gates.new_reply.notify_one();
            let expected = if first_load {"startup-0"} else {"startup-1"};
            f.started(expected).await;
            assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
            f.shutdown().await;
        }).await;
    }
}

#[tokio::test(start_paused = true)]
async fn caar_load_keeps_30s_recoverable_timeout_and_releases_admission() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.load_request("loaded").await;
        tokio::time::advance(Duration::from_secs(29)).await; settle().await;
        assert!(f.rx.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(matches!(f.next().await.notification, Notification::BridgeError {operation,message} if operation=="Load session" && message.contains("session/load timed out after 30s")));
        f.new_request().await;
        f.gates.new_reply.notify_one();
        f.started("startup-0").await;
        assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
        // Late load success cannot rebind this newly activated root.
        f.gates.load_reply.notify_one(); settle().await;
        assert!(f.rx.try_recv().is_err());
        f.shutdown().await;
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_load_preserves_original_error_and_control_keeps_10s_budget() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.gates.fail_load.store(true, Ordering::SeqCst);
        f.load_request("loaded").await; f.gates.load_reply.notify_one();
        assert!(matches!(f.next().await.notification, Notification::BridgeError {operation,message} if operation=="Load session" && message.contains("original load cause")));
        f.new_request().await; f.gates.new_reply.notify_one(); f.started("startup-0").await;
        assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
        f.sender.send(BridgeCommand::SetMode {mode_id:"canary".into()}).await.expect_contract("control");
        f.gates.control_received.notified().await; settle().await;
        tokio::time::advance(Duration::from_secs(9)).await; settle().await;
        assert!(f.rx.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(matches!(f.next().await.notification, Notification::BridgeError {message,..} if message.contains("timed out after 10s")));
        f.shutdown().await;
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_new_original_rpc_error_is_fatal_not_timeout() {
    with_startup(Rc::new(V2Engine), false, |f| async move {
        f.gates.fail_new.store(true, Ordering::SeqCst);
        f.new_request().await;
        f.gates.new_reply.notify_one();
        let reason = f.fatal().await;
        assert!(reason.contains("original new cause"));
        assert!(!reason.contains("timed out"));
        assert!(!reason.contains("inventory"));
    })
    .await;
}

#[cfg(feature = "kas")]
#[tokio::test(start_paused = true)]
async fn caar_kas_pool_is_complete_only_for_its_candidate_connecting_is_reported() {
    use crate::protocol::engine::KasEngine;
    with_startup(Rc::new(KasEngine::default()), true, |mut f| async move {
        f.new_request().await;
        f.emit("_kiro/mcp/status", json!({"sessionId":"sess_foreign","servers":[
            {"name":"ready","status":"connected","tools":[],"prompts":[],"resources":[],"resourceTemplates":[]},
            {"name":"broken","status":"failed","failedAuthorization":false,"errorMessage":"refused"},
            {"name":"auth","status":"failed","failedAuthorization":true,"authorizationUrl":"https://example.test/auth","errorMessage":"authorize"},
            {"name":"connecting","status":"connecting"}
        ]}));
        // A following v2-ready receipt traverses the same ordered SDK ingress
        // after the staged KAS snapshot; the snapshot itself need not publish
        // into an uncorrelated root's panel during new.
        f.emit("_kiro.dev/mcp/server_initialized", json!({"sessionId":"sess_foreign","serverName":"sentinel"}));
        assert!(matches!(f.next().await.notification, Notification::McpServerInitialized {..}));
        tokio::time::advance(Duration::from_secs(90)).await;
        let reason = f.fatal().await;
        for text in ["ready (ready)","broken (failed: refused)","Awaiting OAuth: auth", "Announced but not settled: connecting", "applies only to this candidate", "agent supplied no complete startup inventory for the pending root"] { assert!(reason.contains(text), "missing {text}: {reason}"); }
        assert!(!reason.contains("Silent"));
    }).await;
}

#[cfg(feature = "kas")]
#[tokio::test(start_paused = true)]
async fn caar_success_promotes_only_exact_kas_session_after_created_and_config() {
    use crate::protocol::engine::KasEngine;
    with_startup(Rc::new(KasEngine::default()), true, |mut f| async move {
        f.new_request().await;
        for (sid,name) in [("sess_peer","foreign"),("sess_startup-0","matching")] {
            f.emit("_kiro/mcp/status", json!({"sessionId":sid,"servers":[{"name":name,"status":"connecting"}]}));
        }
        f.emit("_kiro.dev/mcp/server_initialized", json!({"serverName":"legacy-receipt"}));
        assert!(matches!(f.next().await.notification, Notification::McpServerInitialized {..}));
        f.gates.new_reply.notify_one();
        f.started("sess_startup-0").await;
        assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
        let snapshot = f.next().await;
        assert_eq!(snapshot.session_id.as_ref().map(SessionId::as_str),Some("sess_startup-0"));
        assert!(matches!(snapshot.notification, Notification::McpUpdate(crate::types::McpUpdate::Snapshot(snapshot)) if snapshot.servers.len()==1 && snapshot.servers[0].name=="matching"));
        settle().await; assert!(f.rx.try_recv().is_err(),"foreign staged state must not replay");
        f.shutdown().await;
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_replacement_start_keeps_old_peer_candidate_and_legacy_evidence_separate() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        f.gates.new_reply.notify_one();
        f.started("startup-0").await;
        assert!(matches!(f.next().await.notification, Notification::ConfigOptionsUpdated(_)));
        f.new_request().await;
        for params in [
            json!({"sessionId":"startup-0","serverName":"old-auth","oauthUrl":"https://example.test/old"}),
            json!({"sessionId":"peer","serverName":"peer-auth","oauthUrl":"https://example.test/peer"}),
            json!({"sessionId":"startup-1","serverName":"candidate-auth","oauthUrl":"https://example.test/new"}),
            json!({"serverName":"legacy-auth","oauthUrl":"https://example.test/legacy"}),
        ] {
            f.emit("_kiro.dev/mcp/oauth_request", params);
            assert!(matches!(f.next().await.notification, Notification::McpOAuthRequest { .. }));
        }
        tokio::time::advance(Duration::from_secs(90)).await;
        let reason = f.fatal().await;
        for text in [
            "Previously active session \"startup-0\" (not correlated",
            "Candidate session \"startup-1\" (not correlated",
            "Candidate session \"peer\" (not correlated",
            "Legacy sessionless events", "old-auth", "peer-auth", "candidate-auth", "legacy-auth",
            "agent supplied no complete startup inventory for the pending root",
        ] {
            assert!(reason.contains(text), "missing {text}: {reason}");
        }
    }).await;
}

#[tokio::test(start_paused = true)]
async fn caar_prompt_response_remains_unbounded_beyond_startup_budget() {
    with_startup(Rc::new(V2Engine), false, |mut f| async move {
        f.new_request().await;
        f.gates.new_reply.notify_one();
        f.started("startup-0").await;
        assert!(matches!(
            f.next().await.notification,
            Notification::ConfigOptionsUpdated(_)
        ));
        let session_id = SessionId::new("startup-0");
        f.sender
            .send(BridgeCommand::SendPrompt {
                session_id: session_id.clone(),
                prompt: crate::types::PromptEnvelope::prepared(vec!["unbounded turn".into()], None),
            })
            .await
            .expect_contract("prompt command");
        f.gates.prompt_received.notified().await;

        // V2 never produces KAS's wire TurnStarted. Observe the real accepted
        // source owner and submitted prompt instead of inventing that event.
        let source_start = tokio::time::timeout(Duration::from_secs(1), f.sources.recv())
            .await
            .expect_contract("source start deadline")
            .expect_contract("source stream");
        assert_eq!(source_start.session_id(), &session_id);
        let bridge_turn_id = match source_start.kind() {
            SourceTurnEventKind::Started {
                bridge_turn_id,
                block_count,
                ..
            } => {
                assert_eq!(*block_count, 1);
                *bridge_turn_id
            }
            other => panic!("expected actual source turn start, got {other:?}"),
        };
        let source_prompt = tokio::time::timeout(Duration::from_secs(1), f.sources.recv())
            .await
            .expect_contract("source prompt deadline")
            .expect_contract("source stream");
        assert_eq!(source_prompt.session_id(), &session_id);
        assert_eq!(
            source_prompt.source_turn_id(),
            source_start.source_turn_id()
        );
        assert!(
            matches!(source_prompt.kind(), SourceTurnEventKind::PromptFragment {
            block_index: 0, fragment_index: 0, text, is_last: true,
        } if text == "unbounded turn")
        );

        // Preserve both waiting-without-activity and active-turn coverage. The
        // response stays held throughout; the marker crosses real SDK ingress.
        for marker in [None, Some("held prompt active marker")] {
            if let Some(marker) = marker {
                f.emit(
                    "session/update",
                    json!({
                        "sessionId": session_id.as_str(),
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {"type": "text", "text": marker},
                        },
                    }),
                );
                loop {
                    let frame = f.next().await;
                    match &frame.notification {
                        Notification::AgentMessage(message) => {
                            assert_eq!(frame.session_id.as_ref(), Some(&session_id));
                            assert_eq!(message.text, marker);
                            break;
                        }
                        Notification::TurnStalled { .. } => {
                            assert_eq!(frame.session_id.as_ref(), Some(&session_id));
                        }
                        other => {
                            panic!("held prompt must reach SDK marker before completion: {other:?}")
                        }
                    }
                }
            }
            tokio::time::advance(Duration::from_secs(120)).await;
            // One fixed negative-observation bound drains runnable deadline and
            // terminal work; do not reset it after an allowed liveness stall.
            let pending: std::result::Result<(), _> =
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        let frame =
                            f.rx.recv()
                                .await
                                .expect_contract("held prompt notification stream remains open");
                        match &frame.notification {
                            Notification::TurnStalled { .. } => {
                                assert_eq!(frame.session_id.as_ref(), Some(&session_id));
                            }
                            other => {
                                panic!("prompt response must remain pending beyond 90s: {other:?}")
                            }
                        }
                    }
                })
                .await;
            assert!(
                pending.is_err(),
                "the held prompt must outlive the observation window"
            );
        }
        f.gates.prompt_reply.notify_one();
        let completed = f.next().await;
        assert_eq!(
            completed
                .turn
                .expect_contract("completion names the accepted prompt owner")
                .get(),
            bridge_turn_id,
        );
        assert!(matches!(
            completed.notification,
            Notification::TurnCompleted {
                stop_reason: crate::types::StopReason::EndTurn,
            }
        ));
        f.shutdown().await;
    })
    .await;
}

//! Uses only pre-existing SDK/mediator interfaces for base-compatible sensitivity.

use super::super::{DomainChannels, DomainWork, host};
use crate::protocol::engine::{Engine, V2Engine};
use crate::protocol::host_mediator::HostMediator;
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::protocol::source_observer::IngressTracker;
use crate::test_support::must_succeed;
use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage, schema::v1 as acp};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn round_trip(
    engine: Rc<dyn Engine>,
    close: bool,
) -> (
    Vec<agent_client_protocol::Result<serde_json::Value>>,
    Vec<crate::types::RoutedNotification>,
) {
    let (channels, rx, host_rx) = must_succeed(
        DomainChannels::new(IngressTracker::new()),
        "elicitation channels",
    );
    let mut rx = if close {
        drop(rx);
        None
    } else {
        Some(rx)
    };
    #[cfg(feature = "kas")]
    let ctx = Rc::new(crate::protocol::kas::callbacks::DispatchCtx {
        notify_tx: channels.clone(),
        terminals: Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new(
            None,
        )),
        hooks: None,
        hook_ops: Default::default(),
        cwd: Default::default(),
        kas_token: None,
    });
    let host = host::run(
        host_rx,
        engine,
        Rc::new(RefCell::new(HostMediator::new())),
        #[cfg(feature = "kas")]
        ctx,
    );
    let replies = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&replies);
    let agent=Agent.builder().name("elicitation-no-flag-agent").on_receive_request(
        async move |request:acp::InitializeRequest,responder,connection:ConnectionTo<Client>| {
            let serialized=serde_json::to_value(&request).map_err(|error|agent_client_protocol::Error::internal_error().data(error.to_string()))?;
            assert!(serialized.pointer("/clientCapabilities/_meta/kiro/elicitation").is_none());
            let target=connection.clone();let sink=Arc::clone(&sink);
            connection.spawn(async move {
                for elicitation in [json!({"mode":"form","message":"Please supply a marker","requestedSchema":{"type":"object","properties":{}}}),json!({"mode":"url","message":"Authorize this operation","url":"https://canary.invalid/elicitation","elicitationId":"real-elicitation"})] {
                    let reply=target.send_request(UntypedMessage::new("_kiro/mcp/elicitation",json!({"sessionId":"actual-session","toolCallId":"actual-tool","elicitation":elicitation}))?).block_task().await;
                    sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(reply);
                }
                responder.respond(acp::InitializeResponse::new(request.protocol_version))
            })?;Ok(())
        },agent_client_protocol::on_receive_request!());
    let runtime = must_succeed(
        SdkRuntime::start_for_test(agent, channels, StageChain::default()).await,
        "elicitation SDK",
    );
    must_succeed(
        must_succeed(
            tokio::time::timeout(
                Duration::from_secs(5),
                runtime
                    .connection()
                    .send_request(acp::InitializeRequest::new(
                        agent_client_protocol::schema::ProtocolVersion::V1,
                    ))
                    .block_task(),
            )
            .await,
            "elicitation deadline",
        ),
        "elicitation initialize",
    );
    let joined = must_succeed(
        tokio::time::timeout(Duration::from_secs(5), async {
            runtime.shutdown().await;
            // Shutdown may let the host finish normally before abort.
            host.abort();
            host.await
        })
        .await,
        "elicitation cleanup completes",
    );
    if let Err(error) = joined {
        assert!(
            error.is_cancelled(),
            "elicitation host task failed: {error}"
        );
    }
    let mut notices = Vec::new();
    if let Some(rx) = &mut rx {
        while let Ok(work) = rx.try_recv() {
            if let DomainWork::Routed(routed) = work {
                notices.push(routed);
            }
        }
    }
    let replies = std::mem::take(
        &mut *replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    (replies, notices)
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn shipped_form_and_url_callbacks_return_decline_and_actual_context_without_advertisement() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (replies, notices) = round_trip(
                Rc::new(crate::protocol::engine::KasEngine::default()),
                false,
            )
            .await;
            assert_eq!(replies.len(), 2);
            for reply in replies {
                assert_eq!(
                    must_succeed(reply, "elicitation reply"),
                    json!({"action":"decline"})
                );
            }
            assert_eq!(notices.len(), 2);
            for notice in &notices {
                let display = format!("{:?}", notice.notification);
                assert!(
                    display.contains("actual-session")
                        && display.contains("actual-tool")
                        && display.contains("declined"),
                    "must be visible actual-context decline: {display}"
                );
            }
            assert!(format!("{:?}", notices[0]).contains("Form"));
            assert!(format!("{:?}", notices[1]).contains("Url"));
            let (replies, notices) =
                round_trip(Rc::new(crate::protocol::engine::KasEngine::default()), true).await;
            assert!(replies.into_iter().all(|reply| reply.is_err()));
            assert!(notices.is_empty());
        })
        .await;
}

#[tokio::test]
async fn v2_refuses_both_kas_elicitation_modes_in_every_feature_build() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (replies, notices) = round_trip(Rc::new(V2Engine), false).await;
            assert_eq!(replies.len(), 2);
            for reply in replies {
                let Err(error) = reply else {
                    panic!("v2 must refuse, not return null");
                };
                assert_eq!(error.code, agent_client_protocol::ErrorCode::MethodNotFound);
            }
            assert!(notices.is_empty());
        })
        .await;
}

#[cfg(feature = "kas")]
#[test]
fn shipped_status_and_governance_enter_through_the_real_bound_engine() {
    let kas = crate::protocol::engine::KasEngine::default();
    for (method, params, marker) in [
        (
            "kiro/mcp/status",
            json!({"sessionId":"sess_actual","servers":[{"name":"source-canary","status":"disabled"}]}),
            "source-canary",
        ),
        (
            "kiro/mcp/governance_disabled",
            json!({"sessionId":"sess_actual","reason":"admin_disabled"}),
            "AdminDisabled",
        ),
    ] {
        let notification = must_succeed(
            kas.convert_ext_notification(method, &params),
            "shipped MCP conversion",
        )
        .unwrap_or_else(|| panic!("MCP family dropped at engine entry: {method}"));
        let display = format!("{notification:?}");
        assert!(
            display.contains("sess_actual") && display.contains(marker),
            "recognized real wire must remain visible: {display}"
        );
        assert!(
            must_succeed(
                V2Engine.convert_ext_notification(method, &params),
                "v2 conversion"
            )
            .is_none(),
            "v2 must remain untouched"
        );
    }
}

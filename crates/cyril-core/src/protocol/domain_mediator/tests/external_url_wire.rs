use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage};

use super::super::{DomainChannels, DomainWork, host};
use crate::protocol::engine::Engine;
use crate::protocol::host_mediator::HostMediator;
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::protocol::source_observer::IngressTracker;
use crate::test_support::must_succeed;
use crate::types::RoutedNotification;

/// Uses only pre-fix production interfaces so these wire assertions can be
/// transplanted onto the base without requiring the new notification variants.
pub(super) async fn external_url_round_trip(
    engine: Rc<dyn Engine>,
    params: Vec<serde_json::Value>,
    close_delivery: bool,
) -> (
    Vec<agent_client_protocol::Result<serde_json::Value>>,
    Vec<RoutedNotification>,
) {
    let (channels, domain_rx, host_rx) = must_succeed(
        DomainChannels::new(IngressTracker::new()),
        "URL callback channels",
    );
    let mut domain_rx = if close_delivery {
        drop(domain_rx);
        None
    } else {
        Some(domain_rx)
    };
    #[cfg(feature = "kas")]
    let ctx = Rc::new(crate::protocol::kas::callbacks::DispatchCtx {
        notify_tx: channels.clone(),
        terminals: Rc::new(crate::protocol::kas::terminal_io::TerminalRegistry::new(
            None,
        )),
        hooks: None,
        hook_ops: crate::protocol::kas::hooks::HookOps::default(),
        cwd: std::path::PathBuf::new(),
        kas_token: None,
    });
    let host_task = host::run(
        host_rx,
        engine,
        Rc::new(RefCell::new(HostMediator::new())),
        #[cfg(feature = "kas")]
        ctx,
    );
    let replies = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&replies);
    let agent = Agent
        .builder()
        .name("external-url-callback-test-agent")
        .on_receive_request(
            async move |request: acp::InitializeRequest,
                        responder,
                        connection: ConnectionTo<Client>| {
                let callback_connection = connection.clone();
                let sink = Arc::clone(&sink);
                let params = params.clone();
                connection.spawn(async move {
                    for params in params {
                        let reply = callback_connection
                            .send_request(UntypedMessage::new("_kiro/openExternalUrl", params)?)
                            .block_task()
                            .await;
                        sink.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .push(reply);
                    }
                    responder.respond(acp::InitializeResponse::new(request.protocol_version))
                })?;
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        );
    let runtime = must_succeed(
        SdkRuntime::start_for_test(agent, channels, StageChain::default()).await,
        "URL callback runtime",
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
            "URL callback initialization completes",
        ),
        "URL callback initialization response",
    );
    let joined = must_succeed(
        tokio::time::timeout(Duration::from_secs(5), async {
            runtime.shutdown().await;
            // Shutdown may let the host finish normally before abort.
            host_task.abort();
            host_task.await
        })
        .await,
        "URL callback cleanup completes",
    );
    if let Err(error) = joined {
        assert!(
            error.is_cancelled(),
            "URL callback host task failed: {error}"
        );
    }
    let mut notifications = Vec::new();
    if let Some(rx) = &mut domain_rx {
        while let Ok(work) = rx.try_recv() {
            if let DomainWork::Routed(routed) = work {
                notifications.push(routed);
            }
        }
    }
    let replies = std::mem::take(
        &mut *replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    (replies, notifications)
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn external_url_callback_wire_result() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (replies, _) = external_url_round_trip(
                Rc::new(crate::protocol::engine::KasEngine::default()),
                vec![serde_json::json!({"url": "https://login.invalid/authorize?state=exact#fragment"})],
                false,
            )
            .await;
            assert_eq!(replies.len(), 1);
            assert_eq!(
                must_succeed(
                    replies.into_iter().next().unwrap_or_else(|| panic!("one reply")),
                    "URL reply",
                ),
                serde_json::json!({"success": true}),
                "the actual callback must return the supported success object, never null",
            );
        })
        .await;
}

#[tokio::test]
async fn external_url_callback_refused_on_v2_with_or_without_kas() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (replies, notifications) = external_url_round_trip(
                Rc::new(crate::protocol::engine::V2Engine),
                vec![
                    serde_json::json!({"url": "https://login.invalid/"}),
                    serde_json::json!({}),
                ],
                false,
            )
            .await;
            assert_eq!(replies.len(), 2);
            for reply in replies {
                let Err(error) = reply else {
                    panic!("a v2-bound callback must be refused");
                };
                assert_eq!(error.code, agent_client_protocol::ErrorCode::MethodNotFound);
            }
            assert!(notifications.is_empty());
        })
        .await;
}

#[cfg(feature = "kas")]
#[tokio::test]
async fn external_url_callback_closed_delivery_is_not_success() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (replies, notifications) = external_url_round_trip(
                Rc::new(crate::protocol::engine::KasEngine::default()),
                vec![serde_json::json!({"url": "https://login.invalid/authorize"})],
                true,
            )
            .await;
            assert_eq!(replies.len(), 1);
            let Some(Err(error)) = replies.into_iter().next() else {
                panic!("closed delivery cannot acknowledge handling");
            };
            assert_eq!(error.code, agent_client_protocol::ErrorCode::InternalError);
            assert!(notifications.is_empty());
        })
        .await;
}

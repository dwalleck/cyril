use super::super::{DomainConfig, DomainMediator};
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::engine::KasEngine;
use crate::protocol::sdk_runtime::{SdkRuntime, StageChain};
use crate::test_support::must_succeed;
use crate::types::*;
use agent_client_protocol::{Agent, Client, ConnectionTo, UntypedMessage, schema::v1 as acp};
use serde_json::json;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn status(connected: bool) -> serde_json::Value {
    let server = if connected {
        json!({"name":"opaque / server","status":"connected","tools":[],"prompts":[{"name":"other prompt name","promptName":"prompt name","arguments":[{"name":"required","required":true},{"name":"optional"}]},{"name":"Other prompt display title","promptName":"other prompt name"}],"resources":[{"name":"text","uri":"canary:///exact?x=%2F"},{"name":"bad","uri":"canary:///malformed"},{"name":"denied","uri":"canary:///denied"}],"resourceTemplates":[]})
    } else {
        json!({"name":"opaque / server","status":"failed","failedAuthorization":true,"authorizationUrl":"https://auth.invalid/passive","errorMessage":"Unauthorized"})
    };
    json!({"sessionId":"sess_mcp-runtime-root","servers":[server]})
}

async fn next(rx: &mut tokio::sync::mpsc::Receiver<RoutedNotification>) -> RoutedNotification {
    must_succeed(
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await,
        "MCP event deadline",
    )
    .unwrap_or_else(|| panic!("MCP event channel closed"))
}

#[tokio::test(start_paused = true)]
async fn actual_sdk_reset_wait_keeps_status_and_url_live_then_reads_exact_strings_without_model_calls()
 {
    tokio::task::LocalSet::new().run_until(async {
        let records=Arc::new(Mutex::new(Vec::<(String,serde_json::Value)>::new()));let agent_records=Arc::clone(&records);
        let client=Arc::new(Mutex::new(None::<ConnectionTo<Client>>));let agent_client=Arc::clone(&client);
        let reset_gate=Arc::new(tokio::sync::Notify::new());let agent_gate=Arc::clone(&reset_gate);
        let (receipt_tx,mut receipt_rx)=tokio::sync::mpsc::unbounded_channel();
        let agent=Agent.builder().name("mcp-management-wire-agent")
            .on_receive_request(async move |request:acp::InitializeRequest,responder,_connection| {
                responder.respond(serde_json::from_value(json!({"protocolVersion":request.protocol_version,"agentCapabilities":{"_meta":{"kiro":{}}}})).map_err(|error|agent_client_protocol::Error::internal_error().data(error.to_string()))?)
            },agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |_request:acp::NewSessionRequest,responder,connection:ConnectionTo<Client>| {
                *agent_client.lock().unwrap_or_else(std::sync::PoisonError::into_inner)=Some(connection.clone());
                connection.send_notification(UntypedMessage::new("_kiro/mcp/status",json!({"sessionId":"workflow-foreign","servers":[]}))?)?;
                connection.send_notification(UntypedMessage::new("_kiro/mcp/status",status(false))?)?;
                responder.respond(acp::NewSessionResponse::new("sess_mcp-runtime-root"))
            },agent_client_protocol::on_receive_request!())
            .on_receive_request(async move |request:UntypedMessage,responder,connection:ConnectionTo<Client>| {
                agent_records.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((request.method().to_owned(),request.params.clone()));
                match request.method() {
                    "_kiro/mcp/resetServer"=> {
                        let gate=Arc::clone(&agent_gate);let receipt=receipt_tx.clone();let target=connection.clone();
                        connection.spawn(async move {
                            target.send_notification(UntypedMessage::new("_kiro/mcp/status",status(false))?)?;
                            let reply=target.send_request(UntypedMessage::new("_kiro/openExternalUrl",json!({"url":"https://auth.invalid/active?state=exact"}))?).block_task().await?;
                            assert_eq!(reply,json!({"success":true}));
                            receipt.send(()).map_err(|_|agent_client_protocol::Error::internal_error())?;
                            gate.notified().await;
                            responder.respond(json!({"success":true}))
                        })?;Ok(())
                    }
                    "_kiro/mcp/getPrompt"=>responder.respond(json!({"description":"external data","messages":[{"role":"user","content":{"type":"text","text":"canary read only"}},{"role":"assistant","content":{"type":"image","data":"AAAA","mimeType":"image/png"}}]})),
                    "_kiro/mcp/getResource"=>match request.params.get("uri").and_then(serde_json::Value::as_str) {
                        Some("canary:///malformed")=>responder.respond(json!({"contents":null})),
                        Some("canary:///denied")=>responder.respond_with_error(agent_client_protocol::Error::internal_error().data("MCP governance is disabled by your organization")),
                        _=>responder.respond(json!({"contents":[{"uri":"canary:///exact?x=%2F","text":"resource canary"},{"uri":"canary:///blob","blob":"AAAA","mimeType":"application/pdf"}]})),
                    },
                    _=>responder.respond_with_error(agent_client_protocol::Error::method_not_found()),
                }
            },agent_client_protocol::on_receive_request!());
        let directory=must_succeed(tempfile::tempdir(),"MCP workspace");
        let(handle,bridge)=create_channel_pair();let(sender,mut rx,_permissions,_source,_completion)=handle.split();
        let(mediator,channels)=must_succeed(DomainMediator::new(DomainConfig {engine:Rc::new(KasEngine::default()),cwd:directory.path().into(),present_as:None,stall_threshold:Duration::from_secs(30),host_shell:None,kas_token:None},bridge),"MCP mediator");
        let runtime=must_succeed(SdkRuntime::start_for_test(agent,channels,StageChain::default()).await,"MCP SDK");
        let task=tokio::task::spawn_local(mediator.run(runtime));
        must_succeed(sender.send(BridgeCommand::NewSession {cwd:directory.path().into()}).await,"new MCP root");
        let mut controller=crate::session::SessionController::new();
        loop {let event=next(&mut rx).await;controller.apply_notification(&event.notification);if matches!(event.notification,Notification::McpUpdate(McpUpdate::AuthorizationUrls {..})) {break;}}
        assert!(controller.mcp().snapshot.is_some(),"promotion snapshot precedes its retained authorization URLs");
        assert_eq!(controller.mcp().authorization_urls.get("opaque / server").map(String::as_str),Some("https://auth.invalid/passive"));
        assert_eq!(controller.mcp().session_id.as_ref().map(SessionId::as_str),Some("sess_mcp-runtime-root"));
        let request=|operation|McpRequest {session_id:SessionId::new("sess_mcp-runtime-root"),server_name:"opaque / server".into(),operation};
        must_succeed(sender.send(BridgeCommand::Mcp {request:request(McpOperation::Reset {start_oauth:true})}).await,"authenticate");
        let(mut pending,mut pushed,mut url)=(false,false,false);
        while !(pending&&pushed&&url) {
            let event=next(&mut rx).await;controller.apply_notification(&event.notification);
            match event.notification {
                Notification::McpUpdate(McpUpdate::Pending(_))=>pending=true,
                Notification::McpUpdate(McpUpdate::Snapshot(_))=>pushed=true,
                Notification::ExternalUrlRequested {url:actual}=>{assert_eq!(actual,"https://auth.invalid/active?state=exact");url=true;},
                other=>panic!("unexpected reset event {other:?}"),
            }
        }
        must_succeed(receipt_rx.recv().await.ok_or("URL callback receipt channel closed"),"URL callback RPC completed during reset wait");
        tokio::time::advance(Duration::from_secs(11)).await;tokio::task::yield_now().await;
        assert!(rx.try_recv().is_err(),"generic 10-second command timeout must not terminate reset");
        assert_eq!(controller.mcp().pending.len(),1);
        must_succeed(sender.send(BridgeCommand::Mcp {request:request(McpOperation::Reset {start_oauth:false})}).await,"duplicate reset");
        let duplicate=next(&mut rx).await;controller.apply_notification(&duplicate.notification);
        assert!(matches!(duplicate.notification,Notification::McpUpdate(McpUpdate::Outcome(McpOutcome {result:Err(McpOperationError {kind:McpErrorKind::AlreadyPending,..}),..}))));
        assert_eq!(controller.mcp().pending.len(),1,"duplicate failure cannot clear first claim");
        reset_gate.notify_one();
        let ack=next(&mut rx).await;controller.apply_notification(&ack.notification);
        assert!(matches!(ack.notification,Notification::McpUpdate(McpUpdate::Outcome(McpOutcome {result:Ok(McpOperationResult::Acknowledged),..}))));
        assert!(matches!(&controller.mcp().snapshot.as_ref().unwrap_or_else(||panic!("snapshot")).servers[0].status,McpServerStatus::Failed {failed_authorization:true,..}),"ACK does not mean connected");
        let connection=client.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().cloned().unwrap_or_else(||panic!("agent client"));
        must_succeed(connection.send_notification(must_succeed(UntypedMessage::new("_kiro/mcp/status",status(true)),"connected frame")),"connected status");
        controller.apply_notification(&next(&mut rx).await.notification);
        let mcp_state=controller.mcp();
        let snapshot=mcp_state.snapshot.as_ref().unwrap_or_else(||panic!("connected snapshot"));
        let McpServerStatus::Connected {catalog}=&snapshot.servers[0].status else {panic!("connected catalog");};
        let prompt_name=catalog.prompts[0].name.clone();
        assert_eq!(prompt_name,"prompt name");
        assert_eq!(catalog.prompts[0].title.as_deref(),Some("other prompt name"));
        assert_eq!(catalog.prompts[1].name,"other prompt name","display collides with another wire ID");
        for optional in [None,Some("")] {
            let mut arguments=BTreeMap::new();arguments.insert("required".into(),"  original\n{not: json}\t  ".into());if let Some(value)=optional {arguments.insert("optional".into(),value.into());}
            must_succeed(sender.send(BridgeCommand::Mcp {request:request(McpOperation::GetPrompt {name:prompt_name.clone(),arguments})}).await,"get prompt");
            assert!(matches!(next(&mut rx).await.notification,Notification::McpUpdate(McpUpdate::Pending(_))));
            assert!(matches!(next(&mut rx).await.notification,Notification::McpUpdate(McpUpdate::Outcome(McpOutcome {result:Ok(McpOperationResult::Preview(preview)),..})) if matches!(preview.as_ref(),McpPreview::Prompt {messages,..} if messages.len()==2)));
        }
        for (uri,kind) in [("canary:///exact?x=%2F",None),("canary:///malformed",Some(McpErrorKind::Malformed)),("canary:///denied",Some(McpErrorKind::Rejected))] {
            must_succeed(sender.send(BridgeCommand::Mcp {request:request(McpOperation::GetResource {uri:uri.into()})}).await,"get resource");
            assert!(matches!(next(&mut rx).await.notification,Notification::McpUpdate(McpUpdate::Pending(_))));
            let Notification::McpUpdate(McpUpdate::Outcome(outcome))=next(&mut rx).await.notification else {panic!("resource outcome");};
            match kind {Some(kind)=>assert_eq!(outcome.result.err().map(|error|error.kind),Some(kind)),None=>assert!(matches!(outcome.result,Ok(McpOperationResult::Preview(preview)) if matches!(preview.as_ref(),McpPreview::Resource {contents} if contents.len()==2)))}
        }
        must_succeed(sender.send(BridgeCommand::Shutdown).await,"MCP shutdown");must_succeed(must_succeed(task.await,"MCP join"),"MCP stopped");
        let records=records.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(records.len(),6,"no duplicate reset/model/tool/file request");
        assert_eq!(records[0],("_kiro/mcp/resetServer".into(),json!({"serverName":"opaque / server","startOAuth":true})));
        assert_eq!(records[1].1,json!({"serverName":"opaque / server","promptName":"prompt name","arguments":{"required":"  original\n{not: json}\t  "}}));
        assert_eq!(records[2].1,json!({"serverName":"opaque / server","promptName":"prompt name","arguments":{"required":"  original\n{not: json}\t  ","optional":""}}));
        assert_eq!(records[3].1,json!({"serverName":"opaque / server","uri":"canary:///exact?x=%2F"}));
    }).await;
}

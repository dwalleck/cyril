use super::*;
use cyril_ui::traits::Activity;
use ratatui::backend::TestBackend;

#[test]
fn scoped_v2_startup_progress_is_visible_before_reply_without_claiming_root() {
    let (mut app, mut commands) = test_app_with_command_rx();
    let candidate = SessionId::new("candidate-root");
    let peer = SessionId::new("candidate-peer");
    for (sid, notification) in [
        (
            candidate.clone(),
            Notification::McpServerInitialized {
                session_id: Some(candidate.clone()),
                server_name: "early-ready".into(),
            },
        ),
        (
            peer.clone(),
            Notification::McpServerInitFailure {
                session_id: Some(peer.clone()),
                server_name: "peer-broken".into(),
                error: Some("refused".into()),
            },
        ),
        (
            candidate.clone(),
            Notification::McpOAuthRequest {
                session_id: Some(candidate.clone()),
                server_name: "late-auth".into(),
                url: "https://example.test/auth?state=exact#finish".into(),
            },
        ),
    ] {
        assert!(
            app.handle_notification(RoutedNotification::scoped(sid, notification))
                .is_empty()
        );
    }
    let before = system_messages(&app);
    assert_eq!(
        before.len(),
        3,
        "all progress is visible before SessionCreated"
    );
    for (line, name, sid) in [
        (&before[0], "early-ready", "candidate-root"),
        (&before[1], "peer-broken", "candidate-peer"),
        (&before[2], "late-auth", "candidate-root"),
    ] {
        assert!(
            line.contains(name) && line.contains(sid) && line.contains("not yet correlated"),
            "{line}"
        );
    }
    assert!(before[2].contains("https://example.test/auth?state=exact#finish"));
    assert!(app.session.id().is_none());
    assert!(
        app.pending_session_notifications.is_empty(),
        "display-only progress needs no replay duplicate"
    );
    assert!(
        app.ui_state.subagent_ui().streams().is_empty(),
        "candidate IDs are not inferred peers or root"
    );
    assert!(!app.ui_state.has_modal_overlay());
    assert!(commands.try_recv().is_err());
    app.handle_notification(session_created_frame(&candidate));
    let after = system_messages(&app);
    assert_eq!(&after[..3], before.as_slice());
    assert_eq!(after.len(), 4, "no duplicate startup replay across reset");
    assert_eq!(app.session.id(), Some(&candidate));
    let mut terminal = Terminal::new(TestBackend::new(180, 24)).expect("startup terminal");
    app.draw_frame(&mut terminal, "startup progress")
        .expect("startup draw");
    let buffer = terminal.backend().buffer();
    let screen: String = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buffer[(x, y)].symbol())
        .collect();
    assert!(screen.contains("late-auth"));
    assert!(screen.contains("not yet correlated"));
}

#[test]
fn true_foreign_v2_mcp_progress_stays_in_its_stream_not_current_root() {
    let (mut app, mut commands) = test_app_with_command_rx();
    let root = SessionId::new("current-root");
    let peer = SessionId::new("true-peer");
    establish_main_session(&mut app, &root);
    app.handle_notification(RoutedNotification::scoped(
        root.clone(),
        Notification::TurnStarted,
    ));
    for notification in [
        Notification::McpServerInitialized {
            session_id: Some(peer.clone()),
            server_name: "peer-ready".into(),
        },
        Notification::McpServerInitFailure {
            session_id: Some(peer.clone()),
            server_name: "peer-broken".into(),
            error: Some("refused".into()),
        },
        Notification::McpOAuthRequest {
            session_id: Some(peer.clone()),
            server_name: "peer-auth".into(),
            url: "https://example.test/peer-auth".into(),
        },
    ] {
        app.handle_notification(RoutedNotification::scoped(peer.clone(), notification));
    }
    assert_eq!(system_messages(&app), ["Session created: current-root"]);
    assert_eq!(app.session.id(), Some(&root));
    assert_eq!(app.session.status(), &SessionStatus::Busy);
    assert_eq!(app.ui_state.activity(), Activity::Waiting);
    assert!(app.ui_state.focus_subagent(peer));
    let stream = app
        .ui_state
        .subagent_ui()
        .focused_stream()
        .expect("peer stream");
    let messages = stream
        .messages()
        .iter()
        .filter_map(|message| match message.kind() {
            ChatMessageKind::System(text) => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("peer-ready"));
    assert!(messages[1].contains("peer-broken"));
    assert!(
        messages[2].contains("peer-auth") && messages[2].contains("https://example.test/peer-auth")
    );
    assert!(commands.try_recv().is_err());
}

#[test]
fn legacy_sessionless_oauth_history_survives_creation_without_a_fake_session_id() {
    let (mut app, mut commands) = test_app_with_command_rx();
    app.handle_notification(RoutedNotification::global(Notification::McpOAuthRequest {
        session_id: None,
        server_name: "legacy-auth".into(),
        url: "https://example.test/legacy".into(),
    }));
    assert!(system_messages(&app)[0].contains("legacy-auth"));
    assert!(app.session.id().is_none());
    app.handle_notification(session_created_frame(&SessionId::new("actual-root")));
    assert_eq!(system_messages(&app).len(), 2);
    assert!(system_messages(&app)[0].contains("legacy-auth"));
    assert!(commands.try_recv().is_err());
}

// The real subprocess fixture requires POSIX sh. Keep the portable App tests
// above registered on Windows, including in all-features workspace runs.
#[cfg(unix)]
mod replacement {
    use super::*;
    use std::collections::HashMap;
    use std::panic::AssertUnwindSafe;
    use std::sync::Arc;

    use anyhow::{Context, Result};
    use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
    use futures_util::FutureExt;

    const ROOT: &str = "replacement-a";
    const TARGET: &str = "replacement-b";
    const OTHER: &str = "unrelated-c";
    const PEER: &str = "tracked-peer";
    const WORKFLOW: &str = "workflow-step";
    const POST: &str = "post-completion-peer";
    const AUTH_URL: &str = "https://example.test/candidate?state=exact%2Fvalue#finish";

    // Same string-ID extraction as sdk_runtime/tests/process.rs. The pending
    // root RPC never blocks this loop: a separate extension request releases
    // its saved responder, so cleanup needs neither a polling gate nor a child.
    const AGENT: &str = r#"
emit_scope() {
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"method\":\"_kiro.dev/mcp/server_initialized\",\"params\":{\"sessionId\":\"$1\",\"serverName\":\"$2-ready\"}}"
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"method\":\"_kiro.dev/mcp/server_init_failure\",\"params\":{\"sessionId\":\"$1\",\"serverName\":\"$2-broken\",\"error\":\"original-broken-cause\"}}"
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"method\":\"_kiro.dev/mcp/oauth_request\",\"params\":{\"sessionId\":\"$1\",\"serverName\":\"$2-auth\",\"oauthUrl\":\"https://example.test/$2?state=exact%2Fvalue#finish\"}}"
}
first_new=true
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$1"
  id=${line##*\"id\":\"}
  id=${id%%\"*}
  case "$line" in
    *'"method":"initialize"'*)
      printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":true}},"id":"' "$id" '"}'
      ;;
    *'"method":"session/new"'*|*'"method":"session/load"'*)
      if [ "$first_new" = true ]; then
        first_new=false
        printf '%s%s%s\n' '{"jsonrpc":"2.0","result":{"sessionId":"replacement-a"},"id":"' "$id" '"}'
      else
        pending=$id
        case "$line" in
          *'"method":"session/load"'*) result='{}' ;;
          *) result='{"sessionId":"replacement-b"}' ;;
        esac
        emit_scope replacement-b candidate
        emit_scope unrelated-c uncorrelated
        emit_scope tracked-peer peer
        emit_scope workflow-step workflow
        printf '%s\n' '{"jsonrpc":"2.0","method":"_kiro.dev/mcp/server_initialized","params":{"sessionId":"replacement-a","serverName":"current-ready"}}'
        printf '%s\n' '{"jsonrpc":"2.0","method":"_kiro.dev/mcp/server_initialized","params":{"serverName":"pending-receipt"}}'
      fi
      ;;
    *'"method":"_fixture/complete_success"'*)
      printf '{"jsonrpc":"2.0","result":%s,"id":"%s"}\n' "$result" "$pending"
      printf '{"jsonrpc":"2.0","result":{},"id":"%s"}\n' "$id"
      ;;
    *'"method":"_fixture/complete_failure"'*)
      printf '{"jsonrpc":"2.0","error":{"code":-32603,"message":"original-replacement-cause"},"id":"%s"}\n' "$pending"
      printf '{"jsonrpc":"2.0","result":{},"id":"%s"}\n' "$id"
      ;;
    *'"method":"_fixture/post_completion"'*)
      emit_scope post-completion-peer post
      printf '%s\n' '{"jsonrpc":"2.0","method":"_kiro.dev/mcp/server_initialized","params":{"serverName":"post-receipt"}}'
      printf '{"jsonrpc":"2.0","result":{},"id":"%s"}\n' "$id"
      ;;
  esac
done
"#;

    #[derive(Clone, Copy, Debug)]
    enum Start {
        New,
        Load,
    }

    #[derive(Clone, Copy, Debug)]
    enum TargetOwner {
        Peer,
        Workflow,
    }

    struct View {
        session: Option<SessionId>,
        registry: Arc<McpState>,
        status: SessionStatus,
        activity: Activity,
        messages: Vec<String>,
        screen: String,
        peers: HashMap<SessionId, Vec<String>>,
        workflows: HashMap<SessionId, Vec<String>>,
        modal: bool,
        pending_frames: usize,
    }

    struct Observation {
        original_registry: Arc<McpState>,
        before_reply: View,
        after_reply: View,
        post_completion: Option<View>,
        requests: Vec<serde_json::Value>,
    }

    fn stream_messages(messages: &[cyril_ui::traits::ChatMessage]) -> Vec<String> {
        messages
            .iter()
            .filter_map(|message| match message.kind() {
                ChatMessageKind::System(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn view(app: &mut App) -> Result<View> {
        let mut terminal = Terminal::new(TestBackend::new(200, 50))?;
        app.draw_frame(&mut terminal, "replacement startup draw")?;
        let buffer = terminal.backend().buffer();
        let screen = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(View {
            session: app.session.id().cloned(),
            registry: app.session.mcp(),
            status: app.session.status().clone(),
            activity: app.ui_state.activity(),
            messages: system_messages(app),
            screen,
            peers: app
                .ui_state
                .subagent_ui()
                .streams()
                .iter()
                .map(|(id, stream)| (id.clone(), stream_messages(stream.messages())))
                .collect(),
            workflows: app
                .ui_state
                .workflow_streams()
                .iter()
                .map(|(id, stream)| (id.clone(), stream_messages(stream.messages())))
                .collect(),
            modal: app.ui_state.has_modal_overlay(),
            pending_frames: app.pending_session_notifications.len(),
        })
    }

    fn apply(app: &mut App, frame: RoutedNotification) -> Result<()> {
        let deferred = app.handle_notification(frame);
        anyhow::ensure!(
            deferred.is_empty(),
            "unexpected deferred command: {deferred:?}"
        );
        Ok(())
    }

    async fn through(app: &mut App, stop: impl Fn(&Notification) -> bool) -> Result<()> {
        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frame = app
                    .notification_rx
                    .recv()
                    .await
                    .context("real bridge notification stream closed")?;
                let finished = stop(&frame.notification);
                let disconnected =
                    matches!(&frame.notification, Notification::BridgeDisconnected { .. });
                let description = format!("{:?}", frame.notification);
                received.push(description);
                apply(app, frame)?;
                if finished {
                    return Ok(());
                }
                anyhow::ensure!(
                    !disconnected,
                    "unexpected disconnect: {:?}",
                    received.last()
                );
            }
        })
        .await
        .with_context(|| format!("real bridge notification deadline; received: {received:?}"))?
    }

    async fn extension(app: &App, method: &str) -> Result<()> {
        app.bridge_sender
            .send(BridgeCommand::ExtMethod {
                method: method.into(),
                params: serde_json::json!({}),
            })
            .await?;
        Ok(())
    }

    async fn collect(
        app: &mut App,
        directory: &std::path::Path,
        start: Start,
        failure: bool,
        target_owner: Option<TargetOwner>,
    ) -> Result<Observation> {
        app.bridge_sender
            .send(BridgeCommand::NewSession {
                cwd: directory.into(),
            })
            .await?;
        through(app, |note| {
            matches!(note, Notification::SessionCreated { .. })
        })
        .await
        .context("awaiting initial root A response")?;

        // Explicit existing typed App fixture state, not a claim that the v2
        // agent emitted a KAS pool. It makes premature registry replacement
        // observable rather than comparing two indistinguishable empty pools.
        apply(
            app,
            RoutedNotification::scoped(
                SessionId::new(ROOT),
                Notification::McpUpdate(McpUpdate::Snapshot(Arc::new(McpSnapshot {
                    session_id: SessionId::new(ROOT),
                    servers: vec![McpServer {
                        name: "original-registry-server".into(),
                        auth_type: None,
                        provenance: None,
                        status: McpServerStatus::Connecting,
                    }],
                    access_mode: None,
                    registry_servers: None,
                    access_mode_filtered_servers: None,
                    unresolved_registry_servers: None,
                    provenance: None,
                }))),
            ),
        )?;
        let original_registry = app.session.mcp();
        let mut subagents = vec![SubagentInfo::new(
            SessionId::new(PEER),
            "known peer",
            "reviewer",
            "foreign control",
            SubagentStatus::Working { message: None },
        )];
        if matches!(target_owner, Some(TargetOwner::Peer)) {
            subagents.push(SubagentInfo::new(
                SessionId::new(TARGET),
                "requested load peer",
                "reviewer",
                "explicit load target",
                SubagentStatus::Working { message: None },
            ));
        }
        apply(
            app,
            RoutedNotification::global(Notification::SubagentListUpdated {
                subagents,
                pending_stages: Vec::new(),
            }),
        )?;
        apply(
            app,
            RoutedNotification::global(workflow_run_started_frame("replacement-workflow")),
        )?;
        apply(
            app,
            RoutedNotification::global(workflow_node_claim_frame(
                "replacement-workflow",
                "step",
                &SessionId::new(WORKFLOW),
            )),
        )?;
        match target_owner {
            Some(TargetOwner::Peer) => anyhow::ensure!(
                app.ui_state
                    .subagent_tracker()
                    .is_subagent(&SessionId::new(TARGET)),
                "requested Load target must already be a known peer"
            ),
            Some(TargetOwner::Workflow) => {
                apply(
                    app,
                    RoutedNotification::global(workflow_node_claim_frame(
                        "replacement-workflow",
                        "requested-target",
                        &SessionId::new(TARGET),
                    )),
                )?;
                anyhow::ensure!(
                    app.workflow_tracker
                        .session_owner(&SessionId::new(TARGET))
                        .is_some(),
                    "requested Load target must already be workflow-owned"
                );
            }
            None => {}
        }
        apply(
            app,
            RoutedNotification::scoped(SessionId::new(ROOT), Notification::TurnStarted),
        )?;
        app.bridge_sender
            .send(match start {
                Start::New => BridgeCommand::NewSession {
                    cwd: directory.into(),
                },
                Start::Load => BridgeCommand::LoadSession {
                    session_id: SessionId::new(TARGET),
                },
            })
            .await?;
        through(app, |note| {
            matches!(note, Notification::McpServerInitialized { server_name, .. } if server_name == "pending-receipt")
        })
        .await
        .context("awaiting pending replacement B progress receipt")?;
        // This actual draw and state/history capture happen while the original
        // B response is still held. Only the assertions wait for owned cleanup.
        let before_reply = view(app)?;

        extension(
            app,
            if failure {
                "fixture/complete_failure"
            } else {
                "fixture/complete_success"
            },
        )
        .await?;
        through(app, |note| {
            if failure {
                matches!(
                    note,
                    Notification::BridgeDisconnected { .. } | Notification::BridgeError { .. }
                )
            } else {
                matches!(note, Notification::SessionCreated { .. })
            }
        })
        .await
        .context("awaiting held replacement B response after fixture release")?;
        let after_reply = view(app)?;
        let post_completion = if !failure || matches!(start, Start::Load) {
            extension(app, "fixture/post_completion").await?;
            through(app, |note| {
                matches!(note, Notification::McpServerInitialized { server_name, .. } if server_name == "post-receipt")
            })
            .await
            .context("awaiting post-completion peer progress receipt")?;
            Some(view(app)?)
        } else {
            None
        };
        let requests = std::fs::read_to_string(directory.join("requests.jsonl"))?
            .lines()
            .map(serde_json::from_str)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Observation {
            original_registry,
            before_reply,
            after_reply,
            post_completion,
            requests,
        })
    }

    async fn observe(start: Start, failure: bool) -> Observation {
        observe_with_target_owner(start, failure, None).await
    }

    async fn observe_with_target_owner(
        start: Start,
        failure: bool,
        target_owner: Option<TargetOwner>,
    ) -> Observation {
        let directory = tempfile::tempdir().expect("replacement fixture directory");
        let command = AgentCommand::new("/bin/sh").with_args(vec![
            "-c".into(),
            AGENT.into(),
            "replacement-startup-agent".into(),
            directory
                .path()
                .join("requests.jsonl")
                .to_string_lossy()
                .into_owned(),
        ]);
        let bridge = spawn_bridge(
            command,
            SpawnConfig {
                engine: AgentEngine::V2,
                environment: SpawnEnvironment::Replace(std::collections::BTreeMap::new()),
                ..SpawnConfig::default()
            },
            directory.path().into(),
        )
        .expect("spawn real replacement bridge");
        let mut app = App::new(
            bridge,
            StartupInputs {
                ui: &config::UiConfig::default(),
                environment: &cyril_ui::theme::ColorEnvironment::default(),
            },
            directory.path().into(),
            cyril_core::commands::HooksCommandSource::Agent,
            cyril_core::commands::WorkflowCommandSource::None,
            UsageWiring {
                log: test_usage_log(),
                snapshot: live_snapshot_handle(),
                snapshot_rx: idle_snapshot_rx(),
            },
            AgentEngine::V2,
        );
        let collected = AssertUnwindSafe(collect(
            &mut app,
            directory.path(),
            start,
            failure,
            target_owner,
        ))
        .catch_unwind()
        .await;
        // Even baseline assertion failures and unexpected collection panics
        // cannot leave a held agent behind. Completion follows SDK/process
        // shutdown; send failure is expected after fatal New failure.
        if let Err(error) = app.bridge_sender.send(BridgeCommand::Shutdown).await {
            tracing::debug!(%error, "replacement fixture bridge already stopped");
        }
        let completion = app
            .bridge_completion_rx
            .take()
            .expect("owned bridge completion");
        tokio::time::timeout(Duration::from_secs(8), completion)
            .await
            .expect("replacement fixture cleanup deadline")
            .expect("replacement fixture bridge completed");
        match collected {
            Ok(result) => result
                .with_context(|| {
                    let wire = std::fs::read_to_string(directory.path().join("requests.jsonl"))
                        .unwrap_or_else(|error| format!("<request journal unavailable: {error}>"));
                    format!("owned fixture requests after bridge cleanup:\n{wire}")
                })
                .expect("collect real replacement startup"),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    fn candidate_history(view: &View) -> Vec<&str> {
        view.messages
            .iter()
            .filter(|line| line.contains("candidate-"))
            .map(String::as_str)
            .collect()
    }

    fn assert_candidate_history(view: &View) {
        let history = candidate_history(view);
        assert_eq!(
            history.len(),
            3,
            "one durable line per event: {:?}",
            view.messages
        );
        for (line, server) in
            history
                .iter()
                .zip(["candidate-ready", "candidate-broken", "candidate-auth"])
        {
            assert!(line.contains(server) && line.contains(TARGET), "{line}");
        }
        assert!(history[1].contains("original-broken-cause"));
        assert!(
            history[2].contains(AUTH_URL),
            "exact OAuth URL: {}",
            history[2]
        );
    }

    fn assert_foreign_controls(view: &View) {
        for (id, prefix, streams) in [
            (PEER, "peer", &view.peers),
            (WORKFLOW, "workflow", &view.workflows),
        ] {
            let messages = streams
                .get(&SessionId::new(id))
                .expect("known foreign stream");
            assert_eq!(messages.len(), 3, "{id}: {messages:?}");
            for (line, suffix) in messages.iter().zip(["ready", "broken", "auth"]) {
                let server = format!("{prefix}-{suffix}");
                assert!(line.contains(&server), "{id}: {line}");
                assert!(
                    !view.messages.iter().any(|main| main.contains(&server)),
                    "foreign progress must not globalize: {server}"
                );
            }
            assert!(messages[2].contains(&format!(
                "https://example.test/{prefix}?state=exact%2Fvalue#finish"
            )));
        }
    }

    #[rstest::rstest]
    #[case::new(Start::New)]
    #[case::load(Start::Load)]
    #[tokio::test]
    async fn caar_replacement_mcp_startup_visible_before_reply(#[case] start: Start) {
        let observed = observe(start, false).await;
        let before = &observed.before_reply;
        assert_candidate_history(before);
        assert_eq!(before.session, Some(SessionId::new(ROOT)));
        assert_eq!(before.registry, observed.original_registry);
        assert_eq!(before.status, SessionStatus::Busy);
        assert_eq!(before.activity, Activity::Waiting);
        assert!(!before.modal);
        assert_eq!(before.pending_frames, 0);
        assert!(!before.peers.contains_key(&SessionId::new(TARGET)));
        for text in [
            "candidate-ready",
            "candidate-broken",
            "candidate-auth",
            AUTH_URL,
            TARGET,
        ] {
            assert!(
                before.screen.contains(text),
                "missing {text}: {}",
                before.screen
            );
        }
        match start {
            Start::New => {
                for line in candidate_history(before) {
                    assert!(line.contains("not yet correlated"), "{line}");
                }
                assert!(
                    before.messages.iter().any(|line| {
                        line.contains("uncorrelated-ready")
                            && line.contains(OTHER)
                            && line.contains("not yet correlated")
                    }),
                    "a second unknown scope must not be guessed to be root"
                );
                assert!(!before.peers.contains_key(&SessionId::new(OTHER)));
            }
            Start::Load => {
                assert!(before.peers.contains_key(&SessionId::new(OTHER)));
                assert!(
                    !before
                        .messages
                        .iter()
                        .any(|line| line.contains("uncorrelated-"))
                );
            }
        }
    }

    #[rstest::rstest]
    #[case::new(Start::New)]
    #[case::load(Start::Load)]
    #[tokio::test]
    async fn caar_replacement_mcp_startup_history_survives_success(#[case] start: Start) {
        let observed = observe(start, false).await;
        assert_candidate_history(&observed.after_reply);
        assert_eq!(
            candidate_history(&observed.after_reply),
            candidate_history(&observed.before_reply),
            "success must preserve the already-visible history without replay"
        );
        assert_eq!(observed.after_reply.session, Some(SessionId::new(TARGET)));
        assert_eq!(
            observed.after_reply.registry.session_id,
            Some(SessionId::new(TARGET))
        );
        assert!(
            observed.after_reply.registry.snapshot.is_none(),
            "B inventory remains unreported"
        );
        assert!(
            !observed
                .after_reply
                .peers
                .contains_key(&SessionId::new(TARGET))
        );
    }

    #[rstest::rstest]
    #[case::new(Start::New)]
    #[case::load(Start::Load)]
    #[tokio::test]
    async fn caar_replacement_mcp_startup_history_survives_failure(#[case] start: Start) {
        let observed = observe(start, true).await;
        assert_candidate_history(&observed.after_reply);
        assert_eq!(
            candidate_history(&observed.after_reply),
            candidate_history(&observed.before_reply),
            "failure must preserve the early progress without replay"
        );
        assert_eq!(observed.after_reply.session, Some(SessionId::new(ROOT)));
        assert!(
            observed
                .after_reply
                .messages
                .iter()
                .any(|line| { line.contains("original-replacement-cause") }),
            "original RPC cause must remain visible"
        );
        match start {
            Start::New => assert_eq!(observed.after_reply.status, SessionStatus::Disconnected),
            Start::Load => {
                assert_eq!(observed.after_reply.registry, observed.original_registry);
                assert_ne!(observed.after_reply.status, SessionStatus::Disconnected);
            }
        }
    }

    #[rstest::rstest]
    #[case::new(Start::New)]
    #[case::load(Start::Load)]
    #[tokio::test]
    async fn caar_replacement_mcp_startup_foreign_and_post_completion_controls(
        #[case] start: Start,
    ) {
        let observed = observe(start, false).await;
        assert_foreign_controls(&observed.before_reply);
        let post = observed
            .post_completion
            .as_ref()
            .expect("post-completion receipt");
        assert_foreign_controls(post);
        let stream = post
            .peers
            .get(&SessionId::new(POST))
            .expect("optimistic peer stream");
        assert_eq!(stream.len(), 3);
        assert!(stream[0].contains("post-ready"));
        assert!(stream[1].contains("post-broken"));
        assert!(stream[2].contains("https://example.test/post?state=exact%2Fvalue#finish"));
        assert!(
            !post.messages.iter().any(|line| {
                ["post-ready", "post-broken", "post-auth"]
                    .iter()
                    .any(|name| line.contains(name))
            }),
            "completed startup must not globalize later unknown-peer progress"
        );
        let root_requests = observed
            .requests
            .iter()
            .filter(|request| {
                matches!(
                    request["method"].as_str(),
                    Some("session/new" | "session/load")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(root_requests.len(), 2, "only A and one replacement RPC");
        assert_eq!(root_requests[0]["method"], "session/new");
        assert_eq!(
            root_requests[1]["method"],
            match start {
                Start::New => "session/new",
                Start::Load => "session/load",
            },
        );
        if matches!(start, Start::Load) {
            assert_eq!(root_requests[1]["params"]["sessionId"], TARGET);
        }
        assert!(
            observed.requests.iter().all(|request| {
                matches!(
                    request["method"].as_str(),
                    Some(
                        "initialize"
                            | "session/new"
                            | "session/load"
                            | "_fixture/complete_success"
                            | "_fixture/post_completion"
                    )
                )
            }),
            "progress must not launch model, OAuth, or additional control operations"
        );
    }

    #[rstest::rstest]
    #[case::peer_success(TargetOwner::Peer, false)]
    #[case::peer_failure(TargetOwner::Peer, true)]
    #[case::workflow_success(TargetOwner::Workflow, false)]
    #[case::workflow_failure(TargetOwner::Workflow, true)]
    #[tokio::test]
    async fn caar_replacement_mcp_startup_requested_load_of_known_owner(
        #[case] owner: TargetOwner,
        #[case] failure: bool,
    ) {
        let observed = observe_with_target_owner(Start::Load, failure, Some(owner)).await;
        let before = &observed.before_reply;
        assert_candidate_history(before);
        assert_eq!(before.session, Some(SessionId::new(ROOT)));
        assert_eq!(before.registry, observed.original_registry);
        assert_eq!(before.status, SessionStatus::Busy);
        assert_eq!(before.activity, Activity::Waiting);
        assert!(!before.modal);
        assert_eq!(before.pending_frames, 0);
        for text in [
            "candidate-ready",
            "candidate-broken",
            "candidate-auth",
            AUTH_URL,
            TARGET,
        ] {
            assert!(
                before.screen.contains(text),
                "missing {text}: {}",
                before.screen
            );
        }
        assert_foreign_controls(before);
        assert!(before.peers.contains_key(&SessionId::new(OTHER)));
        assert!(
            !before
                .messages
                .iter()
                .any(|line| line.contains("uncorrelated-"))
        );
        for streams in [&before.peers, &before.workflows] {
            assert!(
                streams.get(&SessionId::new(TARGET)).is_none_or(|messages| {
                    !messages.iter().any(|line| line.contains("candidate-"))
                }),
                "the explicitly requested Load progress must not duplicate into its old owner"
            );
        }

        let after = &observed.after_reply;
        assert_candidate_history(after);
        assert_eq!(candidate_history(after), candidate_history(before));
        if failure {
            assert_eq!(after.session, Some(SessionId::new(ROOT)));
            assert_eq!(after.registry, observed.original_registry);
            assert_ne!(after.status, SessionStatus::Disconnected);
            assert!(
                after
                    .messages
                    .iter()
                    .any(|line| { line.contains("original-replacement-cause") }),
                "a known-owner Load failure retains its recoverable original cause"
            );
        } else {
            assert_eq!(after.session, Some(SessionId::new(TARGET)));
            assert_eq!(after.registry.session_id, Some(SessionId::new(TARGET)));
            assert!(
                after.registry.snapshot.is_none(),
                "requested B inventory remains unreported"
            );
        }
        let post = observed
            .post_completion
            .as_ref()
            .expect("Load bridge remains live");
        assert_foreign_controls(post);
        assert_eq!(candidate_history(post), candidate_history(before));
        assert_eq!(
            post.peers
                .get(&SessionId::new(POST))
                .expect("post-Load optimistic peer")
                .len(),
            3
        );
        assert!(!post.messages.iter().any(|line| {
            ["post-ready", "post-broken", "post-auth"]
                .iter()
                .any(|name| line.contains(name))
        }));
        let loads = observed
            .requests
            .iter()
            .filter(|request| request["method"] == "session/load")
            .collect::<Vec<_>>();
        assert_eq!(loads.len(), 1, "one actual requested Load RPC");
        assert_eq!(loads[0]["params"]["sessionId"], TARGET);
    }
}

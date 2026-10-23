use super::*;
use crate::types::{McpCatalog, McpServer};

fn timeout(startup: &mut Startup, id: StartupAttemptId) -> String {
    match startup.finish(
        id,
        RootStartResult::TimedOut {
            budget: Duration::from_secs(90),
        },
    ) {
        StartupDecision::Fatal(reason) => reason,
        _ => panic!("new timeout must be fatal"),
    }
}

fn snapshot(session: &str) -> Arc<McpSnapshot> {
    Arc::new(McpSnapshot {
        session_id: SessionId::new(session),
        servers: vec![
            McpServer {
                name: "ready".into(),
                auth_type: None,
                status: McpServerStatus::Connected {
                    catalog: Arc::new(McpCatalog::default()),
                },
                provenance: None,
            },
            McpServer {
                name: "broken".into(),
                auth_type: None,
                status: McpServerStatus::Failed {
                    failed_authorization: false,
                    authorization_url: None,
                    error_message: "refused".into(),
                    error_code: None,
                },
                provenance: None,
            },
            McpServer {
                name: "auth".into(),
                auth_type: None,
                status: McpServerStatus::Failed {
                    failed_authorization: true,
                    authorization_url: Some("https://example.test/auth".into()),
                    error_message: "authorize".into(),
                    error_code: None,
                },
                provenance: None,
            },
            McpServer {
                name: "connecting".into(),
                auth_type: None,
                status: McpServerStatus::Connecting,
                provenance: None,
            },
        ],
        access_mode: None,
        registry_servers: None,
        access_mode_filtered_servers: None,
        unresolved_registry_servers: None,
        provenance: None,
    })
}

#[test]
fn failures_keep_new_fatal_and_load_recoverable_without_admission_policy() {
    for kind in [RootStartKind::New, RootStartKind::Load] {
        let mut owner = Startup::default();
        let first = owner.begin(kind, None, None);
        match (
            kind,
            owner.finish(first, RootStartResult::Failed("original cause".into())),
        ) {
            (RootStartKind::New, StartupDecision::Fatal(reason))
            | (RootStartKind::Load, StartupDecision::Recoverable(reason)) => {
                assert_eq!(reason, "original cause")
            }
            _ => panic!("failure classification changed"),
        }
        let replacement = owner.begin(kind, None, None);
        assert_ne!(first, replacement);
        assert!(matches!(
            owner.finish(first, RootStartResult::Fatal("late".into())),
            StartupDecision::Stale
        ));
        match (
            kind,
            owner.finish(
                replacement,
                RootStartResult::TimedOut {
                    budget: Duration::from_secs(30),
                },
            ),
        ) {
            (RootStartKind::New, StartupDecision::Fatal(reason)) => {
                assert!(reason.contains("timed out after 30s"))
            }
            (RootStartKind::Load, StartupDecision::Recoverable(reason)) => {
                assert_eq!(reason, "session/load timed out after 30s")
            }
            _ => panic!("stale finish consumed the replacement generation"),
        }
    }
}

#[test]
fn latest_scoped_facts_are_joined_at_timeout_without_guessing_root() {
    let mut owner = Startup::default();
    let id = owner.begin(RootStartKind::New, Some(&SessionId::new("old")), None);
    let _observation = owner.observe(
        &Notification::McpUpdate(McpUpdate::Snapshot(snapshot("peer"))),
        None,
    );
    let _observation = owner.observe(
        &Notification::McpOAuthRequest {
            session_id: Some(SessionId::new("old")),
            server_name: "old-auth".into(),
            url: "https://example.test/old".into(),
        },
        Some(&SessionId::new("old")),
    );
    let _observation = owner.observe(
        &Notification::McpOAuthRequest {
            session_id: None,
            server_name: "legacy-auth".into(),
            url: "https://example.test/legacy".into(),
        },
        None,
    );
    // Same name, distinct scopes. An unscoped ready cannot clear scoped OAuth.
    let _observation = owner.observe(
        &Notification::McpServerInitialized {
            session_id: None,
            server_name: "auth".into(),
        },
        None,
    );
    // This is the last ordered ingress before the timeout result is applied.
    let _observation = owner.observe(
        &Notification::McpServerInitialized {
            session_id: Some(SessionId::new("peer")),
            server_name: "broken".into(),
        },
        Some(&SessionId::new("peer")),
    );
    let reason = timeout(&mut owner, id);
    assert!(reason.starts_with("Session creation timed out after 90s."));
    assert!(reason.contains("Candidate session \"peer\" (not correlated to the pending root)"));
    assert!(reason.contains("Previously active session \"old\""));
    assert!(reason.contains("Legacy sessionless events observed during this attempt"));
    assert!(reason.contains("broken (ready)"));
    assert!(
        !reason.contains("broken (failed"),
        "latest report replaces failure"
    );
    assert!(reason.contains("Awaiting OAuth: auth."));
    assert!(reason.contains("Awaiting OAuth: legacy-auth."));
    assert!(reason.contains("Announced but not settled: connecting."));
    assert!(
        !reason.contains("Silent"),
        "connecting has already reported"
    );
    assert!(reason.contains("agent supplied no complete startup inventory for the pending root"));
}

#[test]
fn authoritative_snapshot_replaces_removed_names_and_oauth() {
    let mut owner = Startup::default();
    let id = owner.begin(RootStartKind::New, None, None);
    let _observation = owner.observe(
        &Notification::McpUpdate(McpUpdate::Snapshot(snapshot("candidate"))),
        None,
    );
    let mut replacement = (*snapshot("candidate")).clone();
    replacement.servers.retain(|server| server.name == "ready");
    let _observation = owner.observe(
        &Notification::McpUpdate(McpUpdate::Snapshot(Arc::new(replacement))),
        None,
    );
    let reason = timeout(&mut owner, id);
    assert!(reason.contains("ready (ready)"));
    for removed in ["broken", "auth", "connecting"] {
        assert!(!reason.contains(removed), "removed name leaked: {reason}");
    }
}

#[test]
fn silent_requires_authoritative_expected_set_minus_actual_reports() {
    // Pure subtraction coverage, not a claim that KAS sends an inventory-only
    // frame. Actual KAS rows all report a status, including connecting.
    let inventory = snapshot("candidate");
    let reports = BTreeSet::from(["ready", "auth", "connecting"]);
    assert_eq!(
        silent_names(&inventory, |name| reports.contains(name)),
        BTreeSet::from(["broken"])
    );
    let facts = ScopeFacts {
        inventory: None,
        reports: BTreeMap::new(),
    };
    let mut unknown = String::new();
    facts.append_summary(&mut unknown);
    assert!(!unknown.contains("Silent"));
}

#[test]
fn completed_attempt_facts_do_not_enter_replacement_timeout() {
    let mut owner = Startup::default();
    let first = owner.begin(RootStartKind::New, None, None);
    let _observation = owner.observe(
        &Notification::McpServerInitialized {
            session_id: None,
            server_name: "prior".into(),
        },
        None,
    );
    assert!(matches!(
        owner.finish(first, RootStartResult::Failed("failed".into())),
        StartupDecision::Fatal(_)
    ));
    let second = owner.begin(RootStartKind::New, None, None);
    let reason = timeout(&mut owner, second);
    assert!(!reason.contains("prior"));
    assert!(reason.contains("agent supplied no complete startup inventory"));
}

#[test]
fn success_and_fingerprint_failure_finish_only_their_generation() {
    let mut owner = Startup::default();
    let first = owner.begin(RootStartKind::New, None, None);
    let session_id = SessionId::new("exact-response-id");
    let start = SessionStart {
        session_id: session_id.clone(),
        origin: crate::types::SessionOrigin::Fresh,
        config_options: None,
        created: crate::protocol::convert::session_created_from_response(
            session_id.as_str().into(),
            None,
            None,
        ),
    };
    match owner.finish(first, RootStartResult::Started(Box::new(start))) {
        StartupDecision::Started(start) => assert_eq!(start.session_id, session_id),
        _ => panic!("successful exact response must be activated"),
    }
    let load = owner.begin(RootStartKind::Load, None, None);
    assert!(matches!(
        owner.finish(first, RootStartResult::Fatal("stale fingerprint".into())),
        StartupDecision::Stale
    ));
    match owner.finish(
        load,
        RootStartResult::Fatal("original fingerprint cause".into()),
    ) {
        StartupDecision::Fatal(reason) => assert_eq!(reason, "original fingerprint cause"),
        _ => panic!("load fingerprint failure must retain its existing fatal semantics"),
    }
}

#[test]
fn new_observations_keep_actual_scope_and_end_with_their_generation() {
    let mut owner = Startup::default();
    let current = SessionId::new("current");
    let candidate = SessionId::new("candidate");
    let first = owner.begin(RootStartKind::New, Some(&current), None);
    let ready = Notification::McpServerInitialized {
        session_id: Some(candidate.clone()),
        server_name: "ready".into(),
    };
    assert_eq!(owner.observe(&ready, None), None, "no actual outer scope");
    assert_eq!(
        owner.observe(&ready, Some(&current)),
        None,
        "scopes disagree"
    );
    assert_eq!(
        owner.observe(&ready, Some(&candidate)),
        Some(StartupObservation::UnboundNew { generation: first })
    );
    assert_eq!(
        owner.observe(
            &Notification::McpServerInitialized {
                session_id: Some(current.clone()),
                server_name: "current-ready".into(),
            },
            Some(&current),
        ),
        None,
        "the existing root remains ordinary current-session progress"
    );
    assert!(matches!(
        owner.finish(first, RootStartResult::Failed("original cause".into())),
        StartupDecision::Fatal(_)
    ));
    assert_eq!(owner.observe(&ready, Some(&candidate)), None);
    let next = owner.begin(RootStartKind::New, Some(&current), None);
    assert_ne!(first, next);
    assert_eq!(
        owner.observe(&ready, Some(&candidate)),
        Some(StartupObservation::UnboundNew { generation: next })
    );
}

#[test]
fn requested_load_observations_bind_only_the_actual_requested_scope() {
    let mut owner = Startup::default();
    let current = SessionId::new("current");
    let target = SessionId::new("requested-target");
    let foreign = SessionId::new("foreign");
    let generation = owner.begin(RootStartKind::Load, Some(&current), Some(target.clone()));
    for notification in [
        Notification::McpServerInitialized {
            session_id: Some(target.clone()),
            server_name: "ready".into(),
        },
        Notification::McpServerInitFailure {
            session_id: Some(target.clone()),
            server_name: "broken".into(),
            error: Some("original cause".into()),
        },
        Notification::McpOAuthRequest {
            session_id: Some(target.clone()),
            server_name: "auth".into(),
            url: "https://example.test/actual".into(),
        },
    ] {
        assert_eq!(
            owner.observe(&notification, Some(&target)),
            Some(StartupObservation::BoundRequestedLoad { generation })
        );
        assert_eq!(owner.observe(&notification, Some(&foreign)), None);
    }
    assert_eq!(
        owner.observe(
            &Notification::McpServerInitialized {
                session_id: Some(foreign.clone()),
                server_name: "foreign-ready".into(),
            },
            Some(&foreign),
        ),
        None,
        "unrelated scope is not the explicit Load target"
    );
    assert_eq!(
        owner.observe(
            &Notification::McpServerInitialized {
                session_id: None,
                server_name: "legacy-ready".into(),
            },
            None,
        ),
        None,
        "sessionless progress keeps the legacy path"
    );
}

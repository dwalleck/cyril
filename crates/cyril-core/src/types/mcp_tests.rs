use super::*;

fn snapshot(session: &str, servers: Vec<McpServer>) -> Arc<McpSnapshot> {
    Arc::new(McpSnapshot {
        session_id: SessionId::new(session),
        servers,
        access_mode: None,
        registry_servers: None,
        access_mode_filtered_servers: None,
        unresolved_registry_servers: None,
        provenance: None,
    })
}

fn connected() -> McpServer {
    McpServer {
        name: "opaque / server".into(),
        auth_type: None,
        provenance: None,
        status: McpServerStatus::Connected {
            catalog: Arc::new(McpCatalog {
                prompts: vec![McpPrompt {
                    name: "prompt name".into(),
                    title: None,
                    description: None,
                    arguments: vec![
                        McpPromptArgument {
                            name: "required".into(),
                            required: true,
                            description: None,
                        },
                        McpPromptArgument {
                            name: "optional".into(),
                            required: false,
                            description: None,
                        },
                    ],
                }],
                resources: vec![McpResource {
                    uri: "canary:///opaque?x=%2F".into(),
                    name: "resource".into(),
                    title: None,
                    description: None,
                    mime_type: None,
                }],
                ..McpCatalog::default()
            }),
        },
    }
}

fn request(operation: McpOperation) -> McpRequest {
    McpRequest {
        session_id: SessionId::new("root"),
        server_name: "opaque / server".into(),
        operation,
    }
}

#[test]
fn current_state_is_immutable_replacement_and_foreign_updates_cannot_mutate_it() {
    let mut state = McpState::for_session(SessionId::new("root"));
    assert!(state.snapshot.is_none());
    assert!(state.apply(&McpUpdate::Snapshot(snapshot("root", vec![connected()]))));
    let held = state.snapshot.clone().unwrap_or_else(|| panic!("snapshot"));
    assert!(!state.apply(&McpUpdate::Snapshot(snapshot("workflow", vec![]))));
    assert!(Arc::ptr_eq(
        &held,
        state
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("snapshot"))
    ));
    assert!(state.apply(&McpUpdate::Diagnostic {
        session_id: SessionId::new("root"),
        message: "malformed status".into()
    }));
    assert!(Arc::ptr_eq(
        &held,
        state
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("snapshot"))
    ));
    assert!(state.diagnostic.is_some());
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![])));
    assert!(
        state
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.servers.is_empty())
    );
    assert!(state.diagnostic.is_none());
    assert_eq!(
        held.servers.len(),
        1,
        "old immutable readers remain unchanged"
    );
}

#[test]
fn validation_distinguishes_absence_from_explicitly_empty_original_strings() {
    let mut state = McpState::for_session(SessionId::new("root"));
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![connected()])));
    let absent = request(McpOperation::GetPrompt {
        name: "prompt name".into(),
        arguments: BTreeMap::new(),
    });
    assert_eq!(
        state.validate(&absent).err().map(|error| error.kind),
        Some(McpErrorKind::InvalidArguments)
    );
    let mut arguments = BTreeMap::new();
    arguments.insert("required".into(), String::new());
    let valid = request(McpOperation::GetPrompt {
        name: "prompt name".into(),
        arguments: arguments.clone(),
    });
    assert!(state.validate(&valid).is_ok());
    assert!(!arguments.contains_key("optional"));
    arguments.insert("optional".into(), " \n{not:json}\t ".into());
    assert!(
        state
            .validate(&request(McpOperation::GetPrompt {
                name: "prompt name".into(),
                arguments
            }))
            .is_ok()
    );
    assert!(
        state
            .validate(&request(McpOperation::GetResource {
                uri: "canary:///opaque?x=%2F".into()
            }))
            .is_ok()
    );
    assert_eq!(
        state
            .validate(&request(McpOperation::GetResource {
                uri: "canary:///other".into()
            }))
            .err()
            .map(|error| error.kind),
        Some(McpErrorKind::EntryMissing)
    );
    state.policy = Some(McpPolicy::Unknown("future fail-closed reason".into()));
    assert_eq!(
        state.validate(&valid).err().map(|error| error.kind),
        Some(McpErrorKind::Policy)
    );
}

#[test]
fn reset_ack_never_changes_authoritative_auth_status_and_outcomes_are_correlated() {
    let failed = McpServer {
        name: "opaque / server".into(),
        auth_type: Some("oauth".into()),
        provenance: None,
        status: McpServerStatus::Failed {
            failed_authorization: true,
            authorization_url: Some("https://auth.invalid/exact".into()),
            error_message: "Unauthorized".into(),
            error_code: None,
        },
    };
    let mut state = McpState::for_session(SessionId::new("root"));
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![failed])));
    assert_eq!(
        state
            .authorization_urls
            .get("opaque / server")
            .map(String::as_str),
        Some("https://auth.invalid/exact")
    );
    let mut reconnecting = connected();
    reconnecting.status = McpServerStatus::Connecting;
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![reconnecting])));
    assert!(state.authorization_urls.contains_key("opaque / server"));
    let request = request(McpOperation::Reset { start_oauth: true });
    let pending = McpPending {
        id: 9,
        request: request.clone(),
    };
    state.apply(&McpUpdate::Pending(pending));
    assert_eq!(
        state.validate(&request).err().map(|error| error.kind),
        Some(McpErrorKind::AlreadyPending)
    );
    assert!(!state.apply(&McpUpdate::Outcome(McpOutcome {
        id: 8,
        request: request.clone(),
        result: Ok(McpOperationResult::Acknowledged)
    })));
    state.apply(&McpUpdate::Outcome(McpOutcome {
        id: 9,
        request,
        result: Ok(McpOperationResult::Acknowledged),
    }));
    assert!(state.pending.is_empty());
    assert!(
        state.authorization_urls.contains_key("opaque / server"),
        "ACK cannot clear passive auth URL"
    );
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![connected()])));
    assert!(
        state.authorization_urls.is_empty(),
        "connected status resolves passive URL"
    );
    assert!(matches!(
        state
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("snapshot"))
            .servers[0]
            .status,
        McpServerStatus::Connected { .. }
    ));
    state.apply(&McpUpdate::Snapshot(snapshot("root", vec![])));
    assert_eq!(
        state.outcomes.len(),
        1,
        "pinned operation history survives removal"
    );
}

#[test]
fn session_controller_resets_mcp_and_shares_snapshot_without_copying_catalogs() {
    let mut controller = crate::session::SessionController::new();
    controller.apply_notification(&crate::types::Notification::SessionCreated {
        session_id: SessionId::new("root"),
        current_mode: None,
        current_model: None,
        available_modes: vec![],
        available_models: vec![],
    });
    let snapshot = snapshot("root", vec![connected()]);
    controller.apply_notification(&crate::types::Notification::McpUpdate(McpUpdate::Snapshot(
        Arc::clone(&snapshot),
    )));
    let held = controller.mcp();
    assert!(Arc::ptr_eq(
        held.snapshot.as_ref().unwrap_or_else(|| panic!("snapshot")),
        &snapshot
    ));
    controller.apply_notification(&crate::types::Notification::BridgeDisconnected {
        reason: "closed".into(),
    });
    assert!(controller.mcp().session_id.is_none());
    assert!(held.snapshot.is_some(), "prior readers are immutable");
}

#[test]
fn governance_policy_survives_empty_disabled_unknown_and_metadata_only_snapshots() {
    for policy in [
        McpPolicy::ApiFailure,
        McpPolicy::AdminDisabled,
        McpPolicy::NoEndpoint,
        McpPolicy::Unknown("future reason".into()),
    ] {
        let mut state = McpState::for_session(SessionId::new("root"));
        assert!(state.apply(&McpUpdate::Policy {
            session_id: SessionId::new("root"),
            policy: policy.clone(),
        }));
        for status in [
            None,
            Some(McpServerStatus::Disabled),
            Some(McpServerStatus::Unknown {
                status: "future status".into(),
            }),
        ] {
            let servers = status
                .map(|status| McpServer {
                    status,
                    ..connected()
                })
                .into_iter()
                .collect();
            assert!(state.apply(&McpUpdate::Snapshot(snapshot("root", servers))));
            assert_eq!(state.policy.as_ref(), Some(&policy));
            assert_eq!(
                state
                    .validate(&request(McpOperation::Reset { start_oauth: true }))
                    .err()
                    .map(|error| error.kind),
                Some(McpErrorKind::Policy)
            );
        }
        let mut metadata_only = snapshot("root", vec![]);
        let metadata = Arc::make_mut(&mut metadata_only);
        metadata.access_mode = Some(McpAccessMode::Registry);
        metadata.registry_servers = Some(vec![McpRegistryServer {
            name: "catalog-only".into(),
            version: None,
            description: None,
            enabled: true,
        }]);
        assert!(state.apply(&McpUpdate::Snapshot(metadata_only)));
        assert_eq!(state.policy.as_ref(), Some(&policy));
    }
}

#[test]
fn known_governance_policy_recovers_only_from_authoritative_active_pool_status() {
    // KAS empties desiredOrder synchronously while globally disabled. Known active
    // pool entries therefore contradict its session-start api/admin-disabled notice.
    for policy in [McpPolicy::ApiFailure, McpPolicy::AdminDisabled] {
        for server in [
            connected(),
            McpServer {
                status: McpServerStatus::Connecting,
                ..connected()
            },
            McpServer {
                status: McpServerStatus::Failed {
                    failed_authorization: true,
                    authorization_url: Some("https://auth.invalid/recovered-policy".into()),
                    error_message: "Unauthorized".into(),
                    error_code: None,
                },
                ..connected()
            },
        ] {
            let is_connected = matches!(&server.status, McpServerStatus::Connected { .. });
            let mut state = McpState::for_session(SessionId::new("root"));
            assert!(state.apply(&McpUpdate::Policy {
                session_id: SessionId::new("root"),
                policy: policy.clone(),
            }));
            assert!(state.apply(&McpUpdate::Snapshot(snapshot("root", vec![]))));
            assert_eq!(state.policy.as_ref(), Some(&policy));
            assert!(!state.apply(&McpUpdate::Snapshot(
                snapshot("foreign", vec![connected()],)
            )));
            assert_eq!(state.policy.as_ref(), Some(&policy));
            assert!(state.apply(&McpUpdate::Snapshot(snapshot("root", vec![server]))));
            assert!(
                state.policy.is_none(),
                "known active pool status must replace stale {policy:?}"
            );
            for start_oauth in [false, true] {
                assert!(
                    state
                        .validate(&request(McpOperation::Reset { start_oauth }))
                        .is_ok()
                );
            }
            if is_connected {
                assert!(
                    state
                        .validate(&request(McpOperation::GetPrompt {
                            name: "prompt name".into(),
                            arguments: [("required".into(), String::new())].into_iter().collect(),
                        }))
                        .is_ok()
                );
                assert!(
                    state
                        .validate(&request(McpOperation::GetResource {
                            uri: "canary:///opaque?x=%2F".into(),
                        }))
                        .is_ok()
                );
            } else {
                assert_eq!(
                    state
                        .validate(&request(McpOperation::GetResource {
                            uri: "canary:///opaque?x=%2F".into(),
                        }))
                        .err()
                        .map(|error| error.kind),
                    Some(McpErrorKind::Disconnected)
                );
            }
        }
    }
}

#[test]
fn unrecognized_policy_reasons_are_not_overridden_by_known_server_status() {
    for policy in [
        McpPolicy::NoEndpoint,
        McpPolicy::Unknown("future reason".into()),
    ] {
        let mut state = McpState::for_session(SessionId::new("root"));
        state.apply(&McpUpdate::Policy {
            session_id: SessionId::new("root"),
            policy: policy.clone(),
        });
        state.apply(&McpUpdate::Snapshot(snapshot("root", vec![connected()])));
        assert_eq!(state.policy.as_ref(), Some(&policy));
        assert_eq!(
            state
                .validate(&request(McpOperation::Reset { start_oauth: false }))
                .err()
                .map(|error| error.kind),
            Some(McpErrorKind::Policy)
        );
    }
}

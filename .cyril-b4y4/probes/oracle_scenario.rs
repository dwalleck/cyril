    /// TEMPORARY b4y4 PROBE-ORACLE (not a fence; removed after capture).
    /// Drives the REAL `run_loop` through the scenario in
    /// `.cyril-b4y4/probes/probe_mediation_model.py` and emits `B4Y4-ORACLE`
    /// lines for line-by-line comparison against the standalone model.
    /// Absorb/stale/unowned are invisible on the notification channel
    /// (cyril-ri8q), so a stderr tracing subscriber captures the loop's own
    /// debug lines as the disposition labels.
    #[cfg(feature = "kas")]
    #[tokio::test]
    async fn b4y4_probe_oracle_scenario() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_writer(std::io::stderr)
            .try_init();
        let script = Rc::new(RefCell::new(Script {
            block_prompt: true,
            ..Default::default()
        }));
        let probe = script.clone();
        with_engine_harness(
            Rc::new(crate::protocol::engine::KasEngine::default()),
            script,
            |sender, mut rx, _perm_rx, _gate, _loop, _kill| async move {
                let sid = start_session(&sender, &mut rx).await;
                let inbound = probe.borrow().inbound.clone().expect("seam");
                let wire_end = |sid: &crate::types::SessionId| {
                    RoutedNotification::scoped(
                        sid.clone(),
                        Notification::TurnCompleted {
                            stop_reason: StopReason::EndTurn,
                        },
                    )
                };
                let stamped = |n: u64| {
                    RoutedNotification::global(Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    })
                    .with_turn(TurnId::new(n))
                };
                let marker = |n: u32| {
                    RoutedNotification::global(Notification::SystemNotify {
                        level: crate::types::event::SystemNotifyLevel::Info,
                        message: format!("marker-{n}"),
                    })
                };
                let expect_marker = |got: Option<Notification>, want: &str, label: &str| match got {
                    Some(Notification::SystemNotify { message, .. }) if message == want => {
                        eprintln!("B4Y4-ORACLE {label}: NOT-FORWARDED ({want} arrived first)");
                    }
                    other => panic!("{label}: expected {want}, got {other:?}"),
                };

                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["one".into()],
                    })
                    .await
                    .unwrap();
                assert!(wait_for_prompt_count(&probe, 1, 5).await);
                eprintln!("B4Y4-ORACLE PROMPT-ACCEPTED turn#0");

                // f1: live order — wire turn_end first. Forwarded.
                inbound.send(wire_end(&sid)).await.unwrap();
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::TurnCompleted { .. }) => {
                        eprintln!("B4Y4-ORACLE f1: FORWARDED TurnCompleted");
                    }
                    other => panic!("f1: expected forwarded TurnCompleted, got {other:?}"),
                }

                // f2: the synthesized twin. Absorbed (marker arrives instead).
                inbound.send(stamped(0)).await.unwrap();
                inbound.send(marker(2)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-2", "f2");

                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["two".into()],
                    })
                    .await
                    .unwrap();
                assert!(wait_for_prompt_count(&probe, 2, 5).await);
                eprintln!("B4Y4-ORACLE PROMPT-ACCEPTED turn#1");

                // f3: stale duplicate of turn#0. Dropped.
                inbound.send(stamped(0)).await.unwrap();
                inbound.send(marker(3)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-3", "f3");

                // f4: foreign session's terminal. Forwarded, main turn untouched.
                inbound
                    .send(RoutedNotification::scoped(
                        crate::types::SessionId::new("sess_foreign"),
                        Notification::TurnCompleted {
                            stop_reason: StopReason::EndTurn,
                        },
                    ))
                    .await
                    .unwrap();
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::TurnCompleted { .. }) => {
                        eprintln!("B4Y4-ORACLE f4: FORWARDED TurnCompleted (foreign)");
                    }
                    other => panic!("f4: expected forwarded foreign terminal, got {other:?}"),
                }

                // f5: reverse order — response first releases turn#1.
                inbound.send(stamped(1)).await.unwrap();
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::TurnCompleted { .. }) => {
                        eprintln!("B4Y4-ORACLE f5: FORWARDED TurnCompleted");
                    }
                    other => panic!("f5: expected forwarded TurnCompleted, got {other:?}"),
                }

                // f6: wire turn_end second. Absorbed by the Wire expectation.
                inbound.send(wire_end(&sid)).await.unwrap();
                inbound.send(marker(4)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-4", "f6");

                // f7: a third terminal with nothing owed. Dropped.
                inbound.send(wire_end(&sid)).await.unwrap();
                inbound.send(marker(5)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-5", "f7");

                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["three".into()],
                    })
                    .await
                    .unwrap();
                assert!(wait_for_prompt_count(&probe, 3, 5).await);
                eprintln!("B4Y4-ORACLE PROMPT-ACCEPTED turn#2");
            },
        )
        .await;
    }

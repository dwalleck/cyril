    /// TEMPORARY b4y4 PROBE-ORACLE (not a fence; removed after capture).
    /// Extended run: f1-f7 as captured 2026-08-02, plus f8-f12 — the
    /// absorb-first precedence cells (dangling companion on the SAME session
    /// as a live turn) and the global-unstamped drop.
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

                // f8: stamped release of turn#2 — Wire expectation dangles.
                inbound.send(stamped(2)).await.unwrap();
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::TurnCompleted { .. }) => {
                        eprintln!("B4Y4-ORACLE f8: FORWARDED TurnCompleted");
                    }
                    other => panic!("f8: expected forwarded TurnCompleted, got {other:?}"),
                }

                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["four".into()],
                    })
                    .await
                    .unwrap();
                assert!(wait_for_prompt_count(&probe, 4, 5).await);
                eprintln!("B4Y4-ORACLE PROMPT-ACCEPTED turn#3");

                // f9: ABSORB-FIRST precedence — the dangling Wire expectation
                // (same session as live turn#3) eats this frame. A release here
                // would forward a TurnCompleted and falsify the model.
                inbound.send(wire_end(&sid)).await.unwrap();
                inbound.send(marker(6)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-6", "f9");

                // f10: companion gone — now this releases turn#3 by scope.
                inbound.send(wire_end(&sid)).await.unwrap();
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::TurnCompleted { .. }) => {
                        eprintln!("B4Y4-ORACLE f10: FORWARDED TurnCompleted");
                    }
                    other => panic!("f10: expected forwarded TurnCompleted, got {other:?}"),
                }

                // f11: global unstamped terminal, idle, Synthesized owed —
                // matches neither arm. Dropped.
                inbound
                    .send(RoutedNotification::global(Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    }))
                    .await
                    .unwrap();
                inbound.send(marker(7)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-7", "f11");

                sender
                    .send(BridgeCommand::SendPrompt {
                        session_id: sid.clone(),
                        content_blocks: vec!["five".into()],
                    })
                    .await
                    .unwrap();
                assert!(wait_for_prompt_count(&probe, 5, 5).await);
                eprintln!("B4Y4-ORACLE PROMPT-ACCEPTED turn#4");

                // f12: owner-keyed absorb of turn#3's synthesized twin wins
                // over stale-drop while turn#4 is active.
                inbound.send(stamped(3)).await.unwrap();
                inbound.send(marker(8)).await.unwrap();
                expect_marker(recv_notif(&mut rx, 5).await, "marker-8", "f12");
            },
        )
        .await;
    }

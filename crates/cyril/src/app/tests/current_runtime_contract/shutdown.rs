use super::*;

#[tokio::test]
async fn c10_shutdown_waits_bridge_then_drains_capture_then_stops_memory() {
    let (mut app, _commands) = test_app_with_command_rx();
    let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
    app.bridge_completion_rx = Some(completion_rx);
    let (source_tx, source_rx) = tokio::sync::mpsc::channel(1);
    app.capture_forwarder = Some(CaptureForwarder::discard(source_rx));
    app.memory_runtime = Some(MemoryRuntimeHandle::start(
        cyril_memory::MemoryConfigState::Absent,
    ));

    {
        let shutdown = app.shutdown_memory_runtime();
        tokio::pin!(shutdown);
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut shutdown)
                .await
                .is_err(),
            "C10 bridge completion gate must hold shutdown"
        );

        drop(source_tx);
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut shutdown)
                .await
                .is_err(),
            "C10 capture closure must not bypass bridge completion"
        );
        completion_tx
            .send(())
            .expect("C10 signal bridge completion");
        tokio::time::timeout(Duration::from_secs(1), &mut shutdown)
            .await
            .expect("C10 ordered shutdown completes within bound");
    }

    assert_eq!(
        app.shutdown_order,
        ["bridge-complete", "capture-drained", "memory-stopped"],
        "C10 exact shutdown phase order"
    );
    assert!(
        app.bridge_completion_rx.is_none(),
        "C10 completion receiver consumed"
    );
    assert!(app.capture_forwarder.is_none(), "C10 forwarder consumed");
    assert!(app.memory_runtime.is_none(), "C10 memory runtime consumed");
}

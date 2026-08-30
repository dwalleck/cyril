use super::*;

async fn send_and_receive(
    app: &mut App,
    commands: &mut tokio::sync::mpsc::Receiver<BridgeCommand>,
    session_id: SessionId,
    blocks: Vec<String>,
    waits_for_memory: bool,
) -> (SessionId, Vec<String>, Vec<String>) {
    app.send_prompt(session_id, blocks)
        .await
        .expect("C7 prompt accepted by App");
    if waits_for_memory {
        assert!(
            commands.try_recv().is_err(),
            "C7 memory lookup must remain off-loop and precede bridge dispatch"
        );
        drain_one_memory_result(app).await;
    }
    recv_prompt(commands).await
}

fn assert_context_present(cell: &str, wire: &[String], originals: &[String], lesson: &str) {
    assert_eq!(
        wire.len(),
        originals.len() + 1,
        "C7 {cell}: exactly one prepared context block"
    );
    assert!(
        wire[0].starts_with("<CYRIL_LESSONS") && wire[0].contains(lesson),
        "C7 {cell}: expected lesson context, got {:?}",
        wire[0]
    );
    assert_eq!(
        &wire[1..],
        originals,
        "C7 {cell}: original block bytes/order changed"
    );
    assert_eq!(
        wire[1..]
            .iter()
            .filter(|block| block.contains("<CYRIL_LESSONS"))
            .count(),
        0,
        "C7 {cell}: context was inserted twice"
    );
}

fn assert_context_absent(cell: &str, wire: &[String], originals: &[String]) {
    assert_eq!(wire, originals, "C7 {cell}: unexpected prepared context");
    assert!(
        wire.iter().all(|block| !block.contains("<CYRIL_LESSONS")),
        "C7 {cell}: hidden lesson marker leaked"
    );
}

#[tokio::test]
async fn c7_ready_memory_matrix_preserves_blocks_sessions_and_project_scope() {
    let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
    let workspace_a = tempfile::tempdir().expect("C7 workspace A");
    let workspace_b = tempfile::tempdir().expect("C7 workspace B");
    let memory_a = runtime.bind(workspace_a.path());
    let memory_b = runtime.bind(workspace_b.path());
    let lesson = "prefer Ω-safe boring Rust";
    memory_a
        .teach(cyril_memory::LessonText::new(lesson).expect("C7 lesson"))
        .await
        .expect("C7 teach project A");

    let (mut app_a, mut commands_a) = test_app_with_command_rx();
    app_a.set_memory_runtime(
        MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
        ProjectBinding::Bound(memory_a.clone()),
    );
    let session_a = SessionId::new("ready-session-a");
    app_a.handle_notification(session_created_frame(&session_a));
    let first_blocks = vec![
        "first Ω query".to_owned(),
        "attachment block 🦀 with spaces".to_owned(),
    ];
    let (sent_session, originals, wire) = send_and_receive(
        &mut app_a,
        &mut commands_a,
        session_a.clone(),
        first_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(sent_session, session_a, "C7 ready.first session identity");
    assert_eq!(originals, first_blocks, "C7 ready.first source originals");
    assert_context_present("ready.first.multiblock", &wire, &first_blocks, lesson);

    app_a.session.set_status(SessionStatus::Active);
    let subsequent_blocks = vec!["subsequent λ prompt".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app_a,
        &mut commands_a,
        session_a,
        subsequent_blocks.clone(),
        false,
    )
    .await;
    assert_eq!(
        originals, subsequent_blocks,
        "C7 ready.subsequent source originals"
    );
    assert_context_absent("ready.subsequent", &wire, &subsequent_blocks);

    let session_b = SessionId::new("ready-session-b");
    app_a.handle_notification(session_created_frame(&session_b));
    let new_session_blocks = vec!["new session prompt".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app_a,
        &mut commands_a,
        session_b,
        new_session_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(originals, new_session_blocks);
    assert_context_present("ready.new_session", &wire, &new_session_blocks, lesson);

    let (mut app_b, mut commands_b) = test_app_with_command_rx();
    app_b.set_memory_runtime(
        MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
        ProjectBinding::Bound(memory_b),
    );
    let foreign_project_session = SessionId::new("project-b-session");
    app_b.handle_notification(session_created_frame(&foreign_project_session));
    let project_b_blocks = vec!["first Ω query".to_owned(), "project B".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app_b,
        &mut commands_b,
        foreign_project_session,
        project_b_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(originals, project_b_blocks);
    assert_context_absent("ready.project_change", &wire, &project_b_blocks);

    runtime.shutdown().await;
}

#[tokio::test]
async fn c7_starting_retries_once_then_ready_context_is_exactly_once() {
    let runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
    let workspace = tempfile::tempdir().expect("C7 starting workspace");
    let memory = runtime.bind(workspace.path());
    let lesson = "remember the exact retry contract";
    memory
        .teach(cyril_memory::LessonText::new(lesson).expect("C7 starting lesson"))
        .await
        .expect("C7 starting teach");
    runtime.set_starting();

    let (mut app, mut commands) = test_app_with_command_rx();
    app.set_memory_runtime(
        MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
        ProjectBinding::Bound(memory),
    );
    let session = SessionId::new("starting-session");
    app.handle_notification(session_created_frame(&session));

    let cold_blocks = vec!["cold first".to_owned(), "attachment".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app,
        &mut commands,
        session.clone(),
        cold_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(originals, cold_blocks);
    assert_context_absent("starting.first", &wire, &cold_blocks);
    assert_eq!(
        app.first_prompt_lessons_pending.as_ref(),
        Some(&session),
        "C7 starting re-arms only this session"
    );

    runtime.set_ready();
    app.session.set_status(SessionStatus::Active);
    let ready_blocks = vec!["ready retry".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app,
        &mut commands,
        session.clone(),
        ready_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(originals, ready_blocks);
    assert_context_present("starting.ready_retry", &wire, &ready_blocks, lesson);
    assert!(
        app.first_prompt_lessons_pending.is_none(),
        "C7 successful retry consumes first-prompt eligibility"
    );

    app.session.set_status(SessionStatus::Active);
    let third_blocks = vec!["third plain".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut app,
        &mut commands,
        session,
        third_blocks.clone(),
        false,
    )
    .await;
    assert_eq!(originals, third_blocks);
    assert_context_absent("starting.third", &wire, &third_blocks);

    runtime.shutdown().await;
}

#[tokio::test]
async fn c7_unavailable_disabled_and_unbound_memory_degrade_without_prompt_loss() {
    let dead_runtime = crate::memory_runtime::test_support::InProcessRuntime::start().await;
    let dead_workspace = tempfile::tempdir().expect("C7 dead workspace");
    let dead_memory = dead_runtime.bind(dead_workspace.path());
    dead_runtime.shutdown().await;

    let (mut unavailable, mut unavailable_commands) = test_app_with_command_rx();
    unavailable.set_memory_runtime(
        MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
        ProjectBinding::Bound(dead_memory),
    );
    let unavailable_session = SessionId::new("unavailable-session");
    unavailable.handle_notification(session_created_frame(&unavailable_session));
    let unavailable_blocks = vec!["survive unavailable memory".to_owned()];
    let (_, originals, wire) = send_and_receive(
        &mut unavailable,
        &mut unavailable_commands,
        unavailable_session,
        unavailable_blocks.clone(),
        true,
    )
    .await;
    assert_eq!(originals, unavailable_blocks);
    assert_context_absent("unavailable", &wire, &unavailable_blocks);
    assert!(
        unavailable.first_prompt_lessons_pending.is_none(),
        "C7 terminal memory failure is not retried forever"
    );

    let cases = [
        ("disabled", ProjectBinding::Disabled),
        (
            "unbound",
            ProjectBinding::Unbound {
                reason: "project identity unavailable".to_owned(),
            },
        ),
    ];
    for (cell, binding) in cases {
        let (mut app, mut commands) = test_app_with_command_rx();
        app.set_memory_runtime(
            MemoryRuntimeHandle::start(cyril_memory::MemoryConfigState::Absent),
            binding,
        );
        let session = SessionId::new(format!("{cell}-session"));
        app.handle_notification(session_created_frame(&session));
        let blocks = vec![format!("{cell} Ω"), "second block".to_owned()];
        let (_, originals, wire) =
            send_and_receive(&mut app, &mut commands, session, blocks.clone(), false).await;
        assert_eq!(originals, blocks, "C7 {cell}: source originals");
        assert_context_absent(cell, &wire, &blocks);
    }
}

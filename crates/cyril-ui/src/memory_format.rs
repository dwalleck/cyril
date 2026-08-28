use cyril_core::types::{
    MemoryDisabledReason, MemoryLessonListView, MemoryLessonView, MemoryProjectBinding,
    MemorySourceTurnListView, MemorySourceTurnView, MemoryStatus, MemoryStatusView,
    MemoryTeachOperation, MemoryTeachView,
};

/// Format `/memory status` from the immutable domain view.
pub fn format_memory_status(status: &MemoryStatusView) -> String {
    let mut rendered = match status.status() {
        MemoryStatus::Disabled => match status.disabled_reason() {
            Some(MemoryDisabledReason::Absent) => {
                "Memory: disabled\nAdd `[memory] enabled = true` to Cyril's config to enable it."
                    .to_owned()
            }
            Some(MemoryDisabledReason::ConfiguredOff) | None => {
                "Memory: disabled\nSet `[memory] enabled = true` to enable it.".to_owned()
            }
        },
        MemoryStatus::Starting => {
            "Memory: starting\nWaiting for authenticated runtime health.".to_owned()
        }
        MemoryStatus::Ready => {
            let protocol = status
                .protocol_version()
                .map_or_else(|| "unknown".to_owned(), |value| value.to_string());
            let versions = status.store_versions().map_or_else(
                || "memory unknown, knowledge unknown".to_owned(),
                |versions| {
                    format!(
                        "memory {}, knowledge {}",
                        versions.memory(),
                        versions.knowledge()
                    )
                },
            );
            let instance = status.instance_id().unwrap_or("unknown");
            format!(
                "Memory: ready\nProtocol: {protocol}\nStores: {versions}\nRuntime instance: {instance}"
            )
        }
        MemoryStatus::Degraded => format!(
            "Memory: degraded\n{}\nOrdinary chat remains available.",
            status.detail().unwrap_or("Runtime health is unavailable.")
        ),
        MemoryStatus::Failed => format!(
            "Memory: failed\n{}\nFix the memory configuration or runtime and restart Cyril; ordinary chat remains available.",
            status.detail().unwrap_or("Runtime startup failed.")
        ),
    };
    // The project axis is independent of runtime health: a Ready runtime
    // with an unbound project cannot serve lesson commands, and this is the
    // one place the user can see why.
    match status.project() {
        Some(MemoryProjectBinding::Bound { display_path }) => {
            rendered.push_str("\nProject: ");
            rendered.push_str(display_path);
        }
        Some(MemoryProjectBinding::Unbound { reason }) => {
            rendered.push_str("\nProject: unbound — ");
            rendered.push_str(reason);
            rendered.push_str("\nLesson commands and first-prompt lessons are unavailable.");
        }
        None => {}
    }
    rendered
}

/// Format the outcome of `/memory teach` or `/memory teach --replace`.
pub fn format_memory_teach(result: &MemoryTeachView) -> String {
    let action = match (result.operation(), result.created()) {
        (MemoryTeachOperation::Teach, true) => "Lesson created",
        (MemoryTeachOperation::Teach, false) => "Lesson already active",
        (MemoryTeachOperation::Replace, true) => "Lesson replaced",
        (MemoryTeachOperation::Replace, false) => {
            "Lesson replaced; the new text matches this already-active lesson"
        }
    };
    format!("{action}:\n{}", format_memory_lesson(result.lesson()))
}

pub fn format_memory_list(result: &MemoryLessonListView) -> String {
    if result.lessons().is_empty() && result.corrupt_count() == 0 {
        return "No active project lessons.".to_owned();
    }
    let mut lines = Vec::with_capacity(result.lessons().len() + 3);
    lines.push("Active project lessons:".to_owned());
    lines.extend(result.lessons().iter().map(|lesson| {
        format!(
            "{} [{} / {} / {}] {}",
            lesson.id(),
            lesson.provenance().as_str(),
            lesson.trust().as_str(),
            lesson.status().as_str(),
            single_line(lesson.content())
        )
    }));
    if result.omitted_count() > 0 {
        lines.push(format!("+{} more", result.omitted_count()));
    }
    if result.corrupt_count() > 0 {
        lines.push(format!(
            "{} corrupt lesson row(s) skipped — see cyril.log",
            result.corrupt_count()
        ));
    }
    lines.join("\n")
}

pub fn format_memory_lesson(lesson: &MemoryLessonView) -> String {
    let supersedes = lesson.supersedes_id().unwrap_or("none");
    format!(
        "ID: {}\nProvenance: {}\nTrust: {}\nStatus: {}\nSupersedes: {}\nCreated: {}\nUpdated: {}\nContent: {}",
        lesson.id(),
        lesson.provenance().as_str(),
        lesson.trust().as_str(),
        lesson.status().as_str(),
        supersedes,
        lesson.created_at_ms(),
        lesson.updated_at_ms(),
        lesson.content()
    )
}

pub fn format_memory_turn_list(result: &MemorySourceTurnListView) -> String {
    if result.turns().is_empty() && result.corrupt_count() == 0 {
        return "No captured project source turns.".to_owned();
    }
    let mut lines = Vec::with_capacity(result.turns().len() + 3);
    lines.push("Captured project source turns:".to_owned());
    lines.extend(result.turns().iter().map(|turn| {
        format!(
            "{} [{}] session={} bridge_turn={} started={} {}",
            turn.id(),
            turn.status().as_str(),
            turn.session_id(),
            turn.bridge_turn_id(),
            turn.started_at_ms(),
            single_line(turn.prompt_preview())
        )
    }));
    if result.omitted_count() > 0 {
        lines.push(format!("+{} more", result.omitted_count()));
    }
    if result.corrupt_count() > 0 {
        lines.push(format!(
            "{} corrupt source turn row(s) skipped — see cyril.log",
            result.corrupt_count()
        ));
    }
    lines.join("\n")
}

fn format_memory_tools(turn: &MemorySourceTurnView) -> String {
    let tools = turn
        .tools()
        .iter()
        .map(|tool| {
            serde_json::json!({
                "tool_id": tool.tool_id(),
                "name": tool.name().text(),
                "name_truncated_chars": tool.name().truncated_chars(),
                "status": tool.status(),
                "input": tool.input().text(),
                "input_truncated_chars": tool.input().truncated_chars(),
                "result": tool.result().text(),
                "result_truncated_chars": tool.result().truncated_chars(),
                "capture_truncated_chars": tool.capture_truncated_chars(),
            })
        })
        .collect();
    let mut rendered = serde_json::Value::Array(tools).to_string();
    if turn.omitted_tool_count() > 0 {
        rendered.push_str(&format!("\n+{} omitted tool(s)", turn.omitted_tool_count()));
    }
    rendered
}

pub fn format_memory_turn(turn: &MemorySourceTurnView) -> String {
    format!(
        "ID: {}\nSession: {}\nBridge turn: {}\nStatus: {}\nStarted: {}\nFinished: {}\nNext sequence: {}\nSource hash: {}\nPrompt:\n{}\nAssistant:\n{}\nTools:\n{}",
        turn.id(),
        turn.session_id(),
        turn.bridge_turn_id(),
        turn.status().as_str(),
        turn.started_at_ms(),
        turn.finished_at_ms()
            .map_or_else(|| "none".to_owned(), |value| value.to_string()),
        turn.next_sequence(),
        turn.source_hash().unwrap_or("none"),
        turn.prompt(),
        turn.assistant(),
        format_memory_tools(turn),
    )
}

/// One list row stays one row: multi-line content shows its first line plus
/// how many lines follow, so a lesson cannot masquerade as several entries.
fn single_line(content: &str) -> String {
    let mut lines = content.lines();
    let first = lines.next().unwrap_or_default();
    let more = lines.count();
    if more == 0 {
        first.to_owned()
    } else {
        format!("{first} (+{more} more line(s))")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::{
        MemoryBoundedTextView, MemoryLessonMetadataView, MemoryLessonProvenance,
        MemoryLessonStatus, MemoryLessonTrust, MemorySourceToolView, MemorySourceTurnListView,
        MemorySourceTurnMetadataView, MemorySourceTurnStatus, MemorySourceTurnSummaryMetadataView,
        MemorySourceTurnSummaryView, MemorySourceTurnView, MemoryStoreVersions,
    };

    #[test]
    fn five_state_table_is_explicit_and_actionable() {
        let rows = [
            (
                MemoryStatusView::disabled(MemoryDisabledReason::Absent),
                "Memory: disabled",
            ),
            (MemoryStatusView::starting(), "Memory: starting"),
            (
                MemoryStatusView::ready("instance-1", 1, MemoryStoreVersions::new(1, 1)),
                "Memory: ready",
            ),
            (
                MemoryStatusView::degraded("runtime exited"),
                "Memory: degraded",
            ),
            (
                MemoryStatusView::failed("invalid memory.enabled"),
                "Memory: failed",
            ),
        ];
        for (status, label) in rows {
            let rendered = format_memory_status(&status);
            assert!(rendered.starts_with(label));
            assert!(!rendered.trim().is_empty());
            assert!(!rendered.contains("Project:"));
        }
    }

    #[test]
    fn ready_formats_protocol_versions_and_instance() {
        let rendered = format_memory_status(&MemoryStatusView::ready(
            "instance-1",
            1,
            MemoryStoreVersions::new(2, 3),
        ));
        assert!(rendered.contains("Protocol: 1"));
        assert!(rendered.contains("memory 2, knowledge 3"));
        assert!(rendered.contains("instance-1"));
    }

    #[test]
    fn project_binding_is_shown_with_its_cause() {
        let ready = MemoryStatusView::ready("instance-1", 1, MemoryStoreVersions::new(2, 1));
        let bound = format_memory_status(
            &ready
                .clone()
                .with_project(Some(MemoryProjectBinding::bound("/work/proj"))),
        );
        assert!(bound.ends_with("Project: /work/proj"), "{bound}");
        let unbound = format_memory_status(&ready.with_project(Some(
            MemoryProjectBinding::unbound("Git metadata file /work/proj/.git is invalid"),
        )));
        assert!(unbound.starts_with("Memory: ready"), "{unbound}");
        assert!(
            unbound.contains("Project: unbound — Git metadata file /work/proj/.git is invalid"),
            "{unbound}"
        );
        assert!(unbound.contains("Lesson commands and first-prompt lessons are unavailable."));
    }

    fn lesson(index: usize, status: MemoryLessonStatus) -> MemoryLessonView {
        lesson_with_content(index, status, &format!("lesson {index}"))
    }

    fn lesson_with_content(
        index: usize,
        status: MemoryLessonStatus,
        content: &str,
    ) -> MemoryLessonView {
        MemoryLessonView::new(
            format!("{index:032x}"),
            content.to_owned(),
            MemoryLessonMetadataView::new(
                MemoryLessonProvenance::UserExplicit,
                MemoryLessonTrust::Instruction,
                status,
                (index > 0).then(|| format!("{:032x}", index - 1)),
                1_000,
                2_000,
            ),
        )
    }

    #[test]
    fn lesson_command_matrix_is_bounded_and_typed() {
        let active = lesson(1, MemoryLessonStatus::Active);
        let created = format_memory_teach(&MemoryTeachView::new(
            MemoryTeachOperation::Teach,
            active.clone(),
            true,
        ));
        assert!(created.starts_with("Lesson created:"));
        let replaced = format_memory_teach(&MemoryTeachView::new(
            MemoryTeachOperation::Replace,
            active.clone(),
            true,
        ));
        assert!(replaced.starts_with("Lesson replaced:"));
        let resolved = format_memory_teach(&MemoryTeachView::new(
            MemoryTeachOperation::Replace,
            active.clone(),
            false,
        ));
        assert!(
            resolved
                .starts_with("Lesson replaced; the new text matches this already-active lesson:"),
            "{resolved}"
        );
        assert!(created.contains("Provenance: user_explicit"));
        assert!(created.contains("Trust: instruction"));
        assert!(created.contains("Status: active"));

        let duplicate = format_memory_teach(&MemoryTeachView::new(
            MemoryTeachOperation::Teach,
            active.clone(),
            false,
        ));
        assert!(duplicate.starts_with("Lesson already active:"));
        assert_eq!(
            format_memory_list(&MemoryLessonListView::new(Vec::new(), 0, 0)),
            "No active project lessons."
        );

        let many: Vec<_> = (0..100)
            .map(|index| lesson(index, MemoryLessonStatus::Active))
            .collect();
        let rendered = format_memory_list(&MemoryLessonListView::new(many, 1, 0));
        assert!(rendered.contains("+1 more"));
        assert!(!rendered.contains("corrupt"));
        assert!(rendered.len() <= 20 * 1_024);

        let invalidated = format_memory_lesson(&lesson(2, MemoryLessonStatus::Invalidated));
        assert!(invalidated.contains("Status: invalidated"));
        assert!(invalidated.contains("Supersedes: 00000000000000000000000000000001"));
    }

    #[test]
    fn list_rows_stay_single_line_and_report_corrupt_rows() {
        let multi = lesson_with_content(
            3,
            MemoryLessonStatus::Active,
            "first line\nsecond line\nthird line",
        );
        let rendered = format_memory_list(&MemoryLessonListView::new(vec![multi], 0, 2));
        let rows: Vec<&str> = rendered.lines().collect();
        assert_eq!(rows.len(), 3, "{rendered}");
        assert!(
            rows[1].ends_with("first line (+2 more line(s))"),
            "{rendered}"
        );
        assert!(!rendered.contains("second line"));
        assert_eq!(rows[2], "2 corrupt lesson row(s) skipped — see cyril.log");

        let only_corrupt = format_memory_list(&MemoryLessonListView::new(Vec::new(), 0, 1));
        assert!(only_corrupt.contains("1 corrupt lesson row(s) skipped"));
    }
    #[test]
    fn c7_turn_inspection_survives_ui_retention_and_is_scoped() {
        let turn = MemorySourceTurnView::new(
            "00112233445566778899aabbccddeeff".to_owned(),
            "first line\nsecond line".to_owned(),
            "assistant output".to_owned(),
            vec![MemorySourceToolView::new(
                "tool-1".to_owned(),
                MemoryBoundedTextView::new("read".to_owned(), 0),
                "completed".to_owned(),
                MemoryBoundedTextView::new("/tmp/input".to_owned(), 0),
                MemoryBoundedTextView::new("file contents".to_owned(), 0),
                0,
            )],
            0,
            MemorySourceTurnMetadataView::new(
                "session-1".to_owned(),
                42,
                MemorySourceTurnStatus::Completed,
                Some("deadbeef".to_owned()),
                1_000,
                Some(2_000),
                5,
            ),
        );
        let summary = MemorySourceTurnSummaryView::new(
            "00112233445566778899aabbccddeeff".to_owned(),
            "first line\nsecond line".to_owned(),
            1,
            MemorySourceTurnSummaryMetadataView::new(
                "session-1".to_owned(),
                42,
                MemorySourceTurnStatus::Completed,
                1_000,
                Some(2_000),
            ),
        );
        assert_eq!(summary.tool_count(), 1);
        assert_eq!(summary.finished_at_ms(), Some(2_000));
        let list = format_memory_turn_list(&MemorySourceTurnListView::new(vec![summary], 2, 1));
        assert!(list.contains("[completed] session=session-1 bridge_turn=42"));
        assert!(list.contains("first line (+1 more line(s))"));
        assert!(!list.contains("second line"));
        assert!(list.contains("+2 more"));
        assert!(list.contains("1 corrupt source turn row(s) skipped"));

        let inspection = format_memory_turn(&turn);
        for expected in [
            "ID: 00112233445566778899aabbccddeeff",
            "Session: session-1",
            "Bridge turn: 42",
            "Status: completed",
            "Finished: 2000",
            "Source hash: deadbeef",
            "Prompt:\nfirst line\nsecond line",
            "Assistant:\nassistant output",
            "Tools:\n[",
            "\"name\":\"read\"",
            "\"tool_id\":\"tool-1\"",
        ] {
            assert!(
                inspection.contains(expected),
                "missing {expected}: {inspection}"
            );
        }

        let large_turn = MemorySourceTurnView::new(
            "ffeeddccbbaa99887766554433221100".to_owned(),
            "p".repeat(6 * 1024),
            "a".repeat(6 * 1024),
            Vec::new(),
            0,
            MemorySourceTurnMetadataView::new(
                "session-performance".to_owned(),
                99,
                MemorySourceTurnStatus::Completed,
                Some("deadbeef".to_owned()),
                1_000,
                Some(2_000),
                5,
            ),
        );
        let large_summary = MemorySourceTurnSummaryView::new(
            "ffeeddccbbaa99887766554433221100".to_owned(),
            "p".repeat(6 * 1024),
            0,
            MemorySourceTurnSummaryMetadataView::new(
                "session-performance".to_owned(),
                99,
                MemorySourceTurnStatus::Completed,
                1_000,
                Some(2_000),
            ),
        );
        let render_started = std::time::Instant::now();
        let list = format_memory_turn_list(&MemorySourceTurnListView::new(
            vec![large_summary; 100],
            0,
            0,
        ));
        let detail = format_memory_turn(&large_turn);
        let render_elapsed = render_started.elapsed();
        assert_eq!(list.lines().count(), 101);
        assert!(detail.contains("session-performance"));
        assert!(
            render_elapsed <= std::time::Duration::from_millis(50),
            "C7 source turn render budget exceeded: {render_elapsed:?}"
        );
    }
}

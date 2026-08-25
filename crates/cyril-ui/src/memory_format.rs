use cyril_core::types::{
    MemoryDisabledReason, MemoryLessonListView, MemoryLessonProvenance, MemoryLessonStatus,
    MemoryLessonTrust, MemoryLessonView, MemoryStatus, MemoryStatusView, MemoryTeachView,
};

/// Format `/memory status` from the immutable domain view.
pub fn format_memory_status(status: &MemoryStatusView) -> String {
    match status.status() {
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
    }
}

pub fn format_memory_teach(result: &MemoryTeachView) -> String {
    let action = if result.created() {
        "Lesson created"
    } else {
        "Lesson already active"
    };
    format!("{action}:\n{}", format_memory_lesson(result.lesson()))
}
pub fn format_memory_replace(result: &MemoryTeachView) -> String {
    let action = if result.created() {
        "Lesson replaced"
    } else {
        "Lesson already active"
    };
    format!("{action}:\n{}", format_memory_lesson(result.lesson()))
}

pub fn format_memory_list(result: &MemoryLessonListView) -> String {
    if result.lessons().is_empty() {
        return "No active project lessons.".to_owned();
    }
    let mut lines = Vec::with_capacity(result.lessons().len() + 2);
    lines.push("Active project lessons:".to_owned());
    lines.extend(result.lessons().iter().map(|lesson| {
        format!(
            "{} [{} / {} / {}] {}",
            lesson.id(),
            provenance_label(lesson.provenance()),
            trust_label(lesson.trust()),
            status_label(lesson.status()),
            lesson.content()
        )
    }));
    if result.omitted_count() > 0 {
        lines.push(format!("+{} more", result.omitted_count()));
    }
    lines.join("\n")
}

pub fn format_memory_lesson(lesson: &MemoryLessonView) -> String {
    let supersedes = lesson.supersedes_id().unwrap_or("none");
    format!(
        "ID: {}\nProvenance: {}\nTrust: {}\nStatus: {}\nSupersedes: {}\nCreated: {}\nUpdated: {}\nContent: {}",
        lesson.id(),
        provenance_label(lesson.provenance()),
        trust_label(lesson.trust()),
        status_label(lesson.status()),
        supersedes,
        lesson.created_at_ms(),
        lesson.updated_at_ms(),
        lesson.content()
    )
}

const fn provenance_label(value: MemoryLessonProvenance) -> &'static str {
    match value {
        MemoryLessonProvenance::UserExplicit => "user_explicit",
        MemoryLessonProvenance::Document => "document",
    }
}

const fn trust_label(value: MemoryLessonTrust) -> &'static str {
    match value {
        MemoryLessonTrust::Instruction => "instruction",
        MemoryLessonTrust::Reference => "reference",
    }
}

const fn status_label(value: MemoryLessonStatus) -> &'static str {
    match value {
        MemoryLessonStatus::Active => "active",
        MemoryLessonStatus::Invalidated => "invalidated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::MemoryLessonMetadataView;
    use cyril_core::types::MemoryStoreVersions;

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

    fn lesson(index: usize, status: MemoryLessonStatus) -> MemoryLessonView {
        MemoryLessonView::new(
            format!("{index:032x}"),
            format!("lesson {index}"),
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
        let created = format_memory_teach(&MemoryTeachView::new(active.clone(), true));
        assert!(created.starts_with("Lesson created:"));
        let replaced = format_memory_replace(&MemoryTeachView::new(active.clone(), true));
        assert!(replaced.starts_with("Lesson replaced:"));
        assert!(created.contains("Provenance: user_explicit"));
        assert!(created.contains("Trust: instruction"));
        assert!(created.contains("Status: active"));

        let duplicate = format_memory_teach(&MemoryTeachView::new(active.clone(), false));
        assert!(duplicate.starts_with("Lesson already active:"));
        assert_eq!(
            format_memory_list(&MemoryLessonListView::new(Vec::new(), 0)),
            "No active project lessons."
        );

        let many: Vec<_> = (0..100)
            .map(|index| lesson(index, MemoryLessonStatus::Active))
            .collect();
        let rendered = format_memory_list(&MemoryLessonListView::new(many, 1));
        assert!(rendered.contains("+1 more"));
        assert!(rendered.len() <= 20 * 1_024);

        let invalidated = format_memory_lesson(&lesson(2, MemoryLessonStatus::Invalidated));
        assert!(invalidated.contains("Status: invalidated"));
        assert!(invalidated.contains("Supersedes: 00000000000000000000000000000001"));
    }
}

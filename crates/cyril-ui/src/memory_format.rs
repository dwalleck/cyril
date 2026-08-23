use cyril_core::types::{MemoryDisabledReason, MemoryStatus, MemoryStatusView};

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

#[cfg(test)]
mod tests {
    use super::*;
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
}

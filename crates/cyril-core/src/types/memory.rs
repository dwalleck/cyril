const MAX_DETAIL_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryStatus {
    Disabled,
    Starting,
    Ready,
    Degraded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryDisabledReason {
    Absent,
    ConfiguredOff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryStoreVersions {
    memory: u32,
    knowledge: u32,
}

impl MemoryStoreVersions {
    pub const fn new(memory: u32, knowledge: u32) -> Self {
        Self { memory, knowledge }
    }

    pub const fn memory(self) -> u32 {
        self.memory
    }

    pub const fn knowledge(self) -> u32 {
        self.knowledge
    }
}

/// Immutable, engine-neutral memory status projected into commands and UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryStatusView {
    status: MemoryStatus,
    disabled_reason: Option<MemoryDisabledReason>,
    detail: Option<String>,
    instance_id: Option<String>,
    protocol_version: Option<u16>,
    store_versions: Option<MemoryStoreVersions>,
}

impl Default for MemoryStatusView {
    fn default() -> Self {
        Self::disabled(MemoryDisabledReason::Absent)
    }
}

impl MemoryStatusView {
    pub const fn disabled(reason: MemoryDisabledReason) -> Self {
        Self {
            status: MemoryStatus::Disabled,
            disabled_reason: Some(reason),
            detail: None,
            instance_id: None,
            protocol_version: None,
            store_versions: None,
        }
    }

    pub const fn starting() -> Self {
        Self {
            status: MemoryStatus::Starting,
            disabled_reason: None,
            detail: None,
            instance_id: None,
            protocol_version: None,
            store_versions: None,
        }
    }

    pub fn ready(
        instance_id: impl Into<String>,
        protocol_version: u16,
        store_versions: MemoryStoreVersions,
    ) -> Self {
        let instance_id = instance_id.into();
        debug_assert!(
            !instance_id.is_empty(),
            "runtime instance ID must be non-empty"
        );
        Self {
            status: MemoryStatus::Ready,
            disabled_reason: None,
            detail: None,
            instance_id: Some(bound_text(&instance_id)),
            protocol_version: Some(protocol_version),
            store_versions: Some(store_versions),
        }
    }

    pub fn degraded(detail: impl AsRef<str>) -> Self {
        Self::with_detail(MemoryStatus::Degraded, detail.as_ref())
    }

    pub fn failed(detail: impl AsRef<str>) -> Self {
        Self::with_detail(MemoryStatus::Failed, detail.as_ref())
    }

    fn with_detail(status: MemoryStatus, detail: &str) -> Self {
        Self {
            status,
            disabled_reason: None,
            detail: Some(bound_text(detail)),
            instance_id: None,
            protocol_version: None,
            store_versions: None,
        }
    }

    pub const fn status(&self) -> MemoryStatus {
        self.status
    }

    pub const fn disabled_reason(&self) -> Option<MemoryDisabledReason> {
        self.disabled_reason
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }

    pub const fn protocol_version(&self) -> Option<u16> {
        self.protocol_version
    }

    pub const fn store_versions(&self) -> Option<MemoryStoreVersions> {
        self.store_versions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonProvenance {
    UserExplicit,
    Document,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonTrust {
    Instruction,
    Reference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonStatus {
    Active,
    Invalidated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLessonMetadataView {
    provenance: MemoryLessonProvenance,
    trust: MemoryLessonTrust,
    status: MemoryLessonStatus,
    supersedes_id: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl MemoryLessonMetadataView {
    pub fn new(
        provenance: MemoryLessonProvenance,
        trust: MemoryLessonTrust,
        status: MemoryLessonStatus,
        supersedes_id: Option<String>,
        created_at_ms: i64,
        updated_at_ms: i64,
    ) -> Self {
        Self {
            provenance,
            trust,
            status,
            supersedes_id,
            created_at_ms,
            updated_at_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLessonView {
    id: String,
    content: String,
    provenance: MemoryLessonProvenance,
    trust: MemoryLessonTrust,
    status: MemoryLessonStatus,
    supersedes_id: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl MemoryLessonView {
    pub fn new(id: String, content: String, metadata: MemoryLessonMetadataView) -> Self {
        Self {
            id,
            content,
            provenance: metadata.provenance,
            trust: metadata.trust,
            status: metadata.status,
            supersedes_id: metadata.supersedes_id,
            created_at_ms: metadata.created_at_ms,
            updated_at_ms: metadata.updated_at_ms,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub const fn provenance(&self) -> MemoryLessonProvenance {
        self.provenance
    }

    pub const fn trust(&self) -> MemoryLessonTrust {
        self.trust
    }

    pub const fn status(&self) -> MemoryLessonStatus {
        self.status
    }

    pub fn supersedes_id(&self) -> Option<&str> {
        self.supersedes_id.as_deref()
    }

    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub const fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTeachView {
    lesson: MemoryLessonView,
    created: bool,
}

impl MemoryTeachView {
    pub const fn new(lesson: MemoryLessonView, created: bool) -> Self {
        Self { lesson, created }
    }

    pub const fn lesson(&self) -> &MemoryLessonView {
        &self.lesson
    }

    pub const fn created(&self) -> bool {
        self.created
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLessonListView {
    lessons: Vec<MemoryLessonView>,
    omitted_count: usize,
}

impl MemoryLessonListView {
    pub const fn new(lessons: Vec<MemoryLessonView>, omitted_count: usize) -> Self {
        Self {
            lessons,
            omitted_count,
        }
    }

    pub fn lessons(&self) -> &[MemoryLessonView] {
        &self.lessons
    }

    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }
}

fn bound_text(value: &str) -> String {
    if value.len() <= MAX_DETAIL_BYTES {
        return value.to_owned();
    }
    let mut end = MAX_DETAIL_BYTES - 3;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &value[..end])
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn public_status_contract_covers_every_state() {
        let versions = MemoryStoreVersions::new(1, 2);
        let rows = [
            MemoryStatusView::disabled(MemoryDisabledReason::Absent),
            MemoryStatusView::disabled(MemoryDisabledReason::ConfiguredOff),
            MemoryStatusView::starting(),
            MemoryStatusView::ready("instance", 1, versions),
            MemoryStatusView::degraded("health unavailable"),
            MemoryStatusView::failed("startup failed"),
        ];
        assert_eq!(rows[0].status(), MemoryStatus::Disabled);
        assert_eq!(
            rows[1].disabled_reason(),
            Some(MemoryDisabledReason::ConfiguredOff)
        );
        assert_eq!(rows[2].status(), MemoryStatus::Starting);
        assert_eq!(rows[3].instance_id(), Some("instance"));
        assert_eq!(rows[3].protocol_version(), Some(1));
        assert_eq!(rows[3].store_versions(), Some(versions));
        assert_eq!(versions.memory(), 1);
        assert_eq!(versions.knowledge(), 2);
        assert_eq!(rows[4].detail(), Some("health unavailable"));
        assert_eq!(rows[5].status(), MemoryStatus::Failed);
        assert_send_sync::<MemoryStatusView>();
    }

    #[test]
    fn detail_is_utf8_safe_and_bounded() {
        let view = MemoryStatusView::failed("Ω".repeat(300));
        let detail = view.detail().expect("detail");
        assert!(detail.len() <= MAX_DETAIL_BYTES);
        assert!(detail.ends_with("..."));
    }
}

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

/// How the startup workspace is bound to project memory. Orthogonal to the
/// runtime lifecycle: a Ready runtime with an unbound project still cannot
/// serve lesson commands, and the user needs to see why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryProjectBinding {
    /// The workspace resolved to a project; `display_path` is its canonical
    /// location.
    Bound { display_path: String },
    /// The workspace could not be resolved; `reason` is the cause.
    Unbound { reason: String },
}

impl MemoryProjectBinding {
    pub fn bound(display_path: impl AsRef<str>) -> Self {
        Self::Bound {
            display_path: bound_text(display_path.as_ref()),
        }
    }

    pub fn unbound(reason: impl AsRef<str>) -> Self {
        Self::Unbound {
            reason: bound_text(reason.as_ref()),
        }
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
    project: Option<MemoryProjectBinding>,
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
            project: None,
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
            project: None,
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
            project: None,
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
            project: None,
        }
    }

    /// Attach the project-binding axis (absent when memory is disabled).
    #[must_use]
    pub fn with_project(mut self, project: Option<MemoryProjectBinding>) -> Self {
        self.project = project;
        self
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

    pub const fn project(&self) -> Option<&MemoryProjectBinding> {
        self.project.as_ref()
    }
}

/// Where a lesson came from. Only explicit user teaching exists today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonProvenance {
    UserExplicit,
}

impl MemoryLessonProvenance {
    /// Display vocabulary — the one place the UI spelling lives.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserExplicit => "user_explicit",
        }
    }
}

/// How a lesson is presented to the model. Only instructions exist today.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonTrust {
    Instruction,
}

impl MemoryLessonTrust {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instruction => "instruction",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryLessonStatus {
    Active,
    Invalidated,
}

impl MemoryLessonStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Invalidated => "invalidated",
        }
    }
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

/// Which lesson-writing operation produced a [`MemoryTeachView`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTeachOperation {
    Teach,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTeachView {
    operation: MemoryTeachOperation,
    lesson: MemoryLessonView,
    created: bool,
}

impl MemoryTeachView {
    pub const fn new(
        operation: MemoryTeachOperation,
        lesson: MemoryLessonView,
        created: bool,
    ) -> Self {
        Self {
            operation,
            lesson,
            created,
        }
    }

    pub const fn operation(&self) -> MemoryTeachOperation {
        self.operation
    }

    pub const fn lesson(&self) -> &MemoryLessonView {
        &self.lesson
    }

    /// `true` when a new lesson row was written; `false` when the text
    /// matched an already-active lesson, which is the lesson carried here.
    pub const fn created(&self) -> bool {
        self.created
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryLessonListView {
    lessons: Vec<MemoryLessonView>,
    omitted_count: usize,
    corrupt_count: usize,
}

impl MemoryLessonListView {
    pub const fn new(
        lessons: Vec<MemoryLessonView>,
        omitted_count: usize,
        corrupt_count: usize,
    ) -> Self {
        Self {
            lessons,
            omitted_count,
            corrupt_count,
        }
    }

    pub fn lessons(&self) -> &[MemoryLessonView] {
        &self.lessons
    }

    /// Active lessons beyond the list cap.
    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    /// Active rows the runtime skipped because their stored integrity check
    /// failed.
    pub const fn corrupt_count(&self) -> usize {
        self.corrupt_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemorySourceTurnStatus {
    Incomplete,
    Completed,
    Interrupted,
    Failed,
    Abandoned,
    CaptureOverflow,
}

impl MemorySourceTurnStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
            Self::CaptureOverflow => "capture_overflow",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySourceTurnMetadataView {
    session_id: String,
    bridge_turn_id: u64,
    status: MemorySourceTurnStatus,
    source_hash: Option<String>,
    started_at_ms: i64,
    finished_at_ms: Option<i64>,
    next_sequence: u64,
}

impl MemorySourceTurnMetadataView {
    pub fn new(
        session_id: String,
        bridge_turn_id: u64,
        status: MemorySourceTurnStatus,
        source_hash: Option<String>,
        started_at_ms: i64,
        finished_at_ms: Option<i64>,
        next_sequence: u64,
    ) -> Self {
        Self {
            session_id,
            bridge_turn_id,
            status,
            source_hash,
            started_at_ms,
            finished_at_ms,
            next_sequence,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySourceTurnView {
    id: String,
    prompt: String,
    assistant: String,
    tools: String,
    metadata: MemorySourceTurnMetadataView,
}

impl MemorySourceTurnView {
    pub fn new(
        id: String,
        prompt: String,
        assistant: String,
        tools: String,
        metadata: MemorySourceTurnMetadataView,
    ) -> Self {
        Self {
            id,
            prompt,
            assistant,
            tools,
            metadata,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn session_id(&self) -> &str {
        &self.metadata.session_id
    }

    pub const fn bridge_turn_id(&self) -> u64 {
        self.metadata.bridge_turn_id
    }

    pub const fn status(&self) -> MemorySourceTurnStatus {
        self.metadata.status
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn assistant(&self) -> &str {
        &self.assistant
    }

    pub fn tools(&self) -> &str {
        &self.tools
    }

    pub fn source_hash(&self) -> Option<&str> {
        self.metadata.source_hash.as_deref()
    }

    pub const fn started_at_ms(&self) -> i64 {
        self.metadata.started_at_ms
    }

    pub const fn finished_at_ms(&self) -> Option<i64> {
        self.metadata.finished_at_ms
    }

    pub const fn next_sequence(&self) -> u64 {
        self.metadata.next_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemorySourceTurnListView {
    turns: Vec<MemorySourceTurnView>,
    omitted_count: usize,
    corrupt_count: usize,
}

impl MemorySourceTurnListView {
    pub const fn new(
        turns: Vec<MemorySourceTurnView>,
        omitted_count: usize,
        corrupt_count: usize,
    ) -> Self {
        Self {
            turns,
            omitted_count,
            corrupt_count,
        }
    }

    pub fn turns(&self) -> &[MemorySourceTurnView] {
        &self.turns
    }

    pub const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    pub const fn corrupt_count(&self) -> usize {
        self.corrupt_count
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
        assert!(rows.iter().all(|row| row.project().is_none()));
        assert_send_sync::<MemoryStatusView>();
    }

    #[test]
    fn project_binding_is_an_orthogonal_axis() {
        let ready = MemoryStatusView::ready("instance", 1, MemoryStoreVersions::new(2, 1));
        let bound = ready
            .clone()
            .with_project(Some(MemoryProjectBinding::bound("/work/proj")));
        assert_eq!(
            bound.project(),
            Some(&MemoryProjectBinding::Bound {
                display_path: "/work/proj".to_owned()
            })
        );
        assert_eq!(bound.status(), MemoryStatus::Ready);
        let unbound = ready.with_project(Some(MemoryProjectBinding::unbound(
            "Git metadata file /work/proj/.git is invalid",
        )));
        assert_eq!(
            unbound.project(),
            Some(&MemoryProjectBinding::Unbound {
                reason: "Git metadata file /work/proj/.git is invalid".to_owned()
            })
        );
        assert_ne!(bound, unbound);
    }

    #[test]
    fn detail_is_utf8_safe_and_bounded() {
        let view = MemoryStatusView::failed("Ω".repeat(300));
        let detail = view.detail().expect("detail");
        assert!(detail.len() <= MAX_DETAIL_BYTES);
        assert!(detail.ends_with("..."));
        let MemoryProjectBinding::Unbound { reason } =
            MemoryProjectBinding::unbound("Ω".repeat(300))
        else {
            panic!("unbound expected");
        };
        assert!(reason.len() <= MAX_DETAIL_BYTES);
    }

    #[test]
    fn lesson_vocabulary_is_spelled_once() {
        assert_eq!(
            MemoryLessonProvenance::UserExplicit.as_str(),
            "user_explicit"
        );
        assert_eq!(MemoryLessonTrust::Instruction.as_str(), "instruction");
        assert_eq!(MemoryLessonStatus::Active.as_str(), "active");
        assert_eq!(MemoryLessonStatus::Invalidated.as_str(), "invalidated");
    }
}

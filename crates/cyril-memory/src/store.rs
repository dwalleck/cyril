#[cfg(unix)]
use std::fs::Permissions;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};
use thiserror::Error;

use crate::lesson::{
    ContextBlock, ContextLesson, LessonId, LessonProvenance, LessonStatus, LessonText, LessonTrust,
    render_context,
};
use crate::paths::MemoryPaths;
use crate::project::ProjectScope;

const MEMORY_SCHEMA_VERSION: u32 = 2;
const KNOWLEDGE_SCHEMA_VERSION: u32 = 1;
const STARTUP_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStoreVersions {
    memory: u32,
    knowledge: u32,
}

impl MemoryStoreVersions {
    pub const fn new(memory: u32, knowledge: u32) -> Self {
        Self { memory, knowledge }
    }
    pub(crate) const fn from_parts(memory: u32, knowledge: u32) -> Self {
        Self::new(memory, knowledge)
    }

    pub fn memory(self) -> u32 {
        self.memory
    }

    pub fn knowledge(self) -> u32 {
        self.knowledge
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store file is missing at {path}")]
    Missing { path: PathBuf },

    #[error("store file at {path} is unreadable: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("store file at {path} is malformed: {source}")]
    Malformed {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("store at {path} has invalid metadata: {reason}")]
    Invalid { path: PathBuf, reason: String },

    #[error("permission denied for store path {path}: {source}")]
    Permission {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("store root is already owned at {path}")]
    AlreadyRunning { path: PathBuf },

    #[error("could not acquire store ownership at {path}: {source}")]
    Lock {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("schema metadata is missing from store at {path}")]
    MissingMetadata { path: PathBuf },

    #[error("schema metadata is corrupt in store at {path}: {reason}")]
    CorruptSchema { path: PathBuf, reason: String },

    #[error("schema metadata is duplicated in store at {path}: {count} rows")]
    DuplicateMetadata { path: PathBuf, count: usize },

    #[error("store at {path} uses unsupported schema version {version}")]
    UnsupportedSchema { path: PathBuf, version: i64 },

    #[error("SQLite operation failed for store at {path}: {source}")]
    Sqlite {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("lesson does not exist in the bound project")]
    LessonNotFound,

    #[error("lesson was already replaced and is no longer active")]
    LessonSuperseded,

    #[error("could not generate a lesson identity")]
    Random(#[source] getrandom::Error),

    #[error("system clock is before the Unix epoch")]
    Clock(#[source] std::time::SystemTimeError),

    #[error("stored lesson data is corrupt: {reason}")]
    CorruptLesson { reason: String },
}

pub struct StoreSet {
    _memory: Connection,
    _knowledge: Connection,
    _lock: File,
    memory_path: PathBuf,
    versions: MemoryStoreVersions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoredLesson {
    sequence: u64,
    id: LessonId,
    text: LessonText,
    provenance: LessonProvenance,
    trust: LessonTrust,
    status: LessonStatus,
    supersedes_id: Option<LessonId>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

impl StoredLesson {
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn id(&self) -> LessonId {
        self.id
    }

    pub(crate) fn text(&self) -> &LessonText {
        &self.text
    }

    pub(crate) const fn status(&self) -> LessonStatus {
        self.status
    }

    pub(crate) const fn provenance(&self) -> LessonProvenance {
        self.provenance
    }

    pub(crate) const fn trust(&self) -> LessonTrust {
        self.trust
    }

    pub(crate) const fn supersedes_id(&self) -> Option<LessonId> {
        self.supersedes_id
    }

    pub(crate) const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub(crate) const fn updated_at_ms(&self) -> i64 {
        self.updated_at_ms
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeachOutcome {
    lesson: StoredLesson,
    created: bool,
}

impl TeachOutcome {
    pub(crate) fn lesson(&self) -> &StoredLesson {
        &self.lesson
    }

    pub(crate) const fn created(&self) -> bool {
        self.created
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LessonList {
    lessons: Vec<StoredLesson>,
    omitted_count: usize,
    corrupt_count: usize,
}

impl LessonList {
    pub(crate) fn lessons(&self) -> &[StoredLesson] {
        &self.lessons
    }

    pub(crate) const fn omitted_count(&self) -> usize {
        self.omitted_count
    }

    /// Active rows skipped because their stored integrity check failed.
    pub(crate) const fn corrupt_count(&self) -> usize {
        self.corrupt_count
    }
}

impl StoreSet {
    pub fn open(paths: &MemoryPaths) -> Result<Self, StoreError> {
        let lock_path = paths.lock_path();
        let lock = open_lock(lock_path)?;
        try_lock(&lock, lock_path)?;

        let memory_path = paths.memory_store_path();
        let knowledge_path = paths.knowledge_store_path();
        let memory_state = precreate_store(memory_path)?;
        let knowledge_state = precreate_store(knowledge_path)?;

        let memory = open_store(
            memory_path,
            memory_state,
            StoreKind::Memory,
            MEMORY_SCHEMA_VERSION,
        )?;
        let knowledge = open_store(
            knowledge_path,
            knowledge_state,
            StoreKind::Knowledge,
            KNOWLEDGE_SCHEMA_VERSION,
        )?;
        let versions = MemoryStoreVersions::new(MEMORY_SCHEMA_VERSION, KNOWLEDGE_SCHEMA_VERSION);

        Ok(Self {
            _lock: lock,
            _memory: memory,
            _knowledge: knowledge,
            memory_path: memory_path.to_path_buf(),
            versions,
        })
    }

    pub fn versions(&self) -> MemoryStoreVersions {
        self.versions
    }

    pub(crate) fn teach_lesson(
        &mut self,
        project: &ProjectScope,
        text: &LessonText,
    ) -> Result<TeachOutcome, StoreError> {
        let path = self.memory_path.clone();
        let now = now_ms()?;
        let transaction = self
            ._memory
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| map_sqlite(&path, source))?;
        ensure_project(&path, &transaction, project, now)?;
        if let Some(lesson) = find_active_by_hash(&path, &transaction, project, text)? {
            insert_audit(
                &path,
                &transaction,
                project,
                &lesson,
                "duplicate",
                None,
                now,
            )?;
            transaction
                .commit()
                .map_err(|source| map_sqlite(&path, source))?;
            return Ok(TeachOutcome {
                lesson,
                created: false,
            });
        }
        let lesson = insert_lesson(&path, &transaction, project, text, None, now)?;
        insert_audit(&path, &transaction, project, &lesson, "created", None, now)?;
        transaction
            .commit()
            .map_err(|source| map_sqlite(&path, source))?;
        Ok(TeachOutcome {
            lesson,
            created: true,
        })
    }

    pub(crate) fn replace_lesson(
        &mut self,
        project: &ProjectScope,
        replaced_id: LessonId,
        text: &LessonText,
    ) -> Result<TeachOutcome, StoreError> {
        let path = self.memory_path.clone();
        let now = now_ms()?;
        let transaction = self
            ._memory
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|source| map_sqlite(&path, source))?;
        ensure_project(&path, &transaction, project, now)?;
        // Look the target up in any status so "already replaced" is reported
        // as itself, not as the same "not found" a never-existing id gets.
        let replaced = find_lesson(&path, &transaction, project, replaced_id, None)?
            .ok_or(StoreError::LessonNotFound)?;
        if replaced.status() != LessonStatus::Active {
            return Err(StoreError::LessonSuperseded);
        }
        // The user asked for the target to go away: it is invalidated even
        // when the replacement text already matches another active lesson,
        // in which case that lesson is what the replacement resolves to.
        let existing = find_active_by_hash(&path, &transaction, project, text)?;
        if existing
            .as_ref()
            .is_some_and(|lesson| lesson.id() == replaced_id)
        {
            // Replacing a lesson with its own text is a no-op, not a
            // self-supersede that would leave nothing active.
            insert_audit(
                &path,
                &transaction,
                project,
                &replaced,
                "duplicate",
                Some(replaced_id),
                now,
            )?;
            transaction
                .commit()
                .map_err(|source| map_sqlite(&path, source))?;
            return Ok(TeachOutcome {
                lesson: replaced,
                created: false,
            });
        }
        // A backwards wall-clock step must not stamp the invalidated row
        // before its own creation: readers reject `updated < created`.
        let invalidated_at = now.max(replaced.created_at_ms());
        let changed = transaction
            .execute(
                "UPDATE lessons
                 SET status = 'invalidated', updated_at_ms = ?1
                 WHERE project_id = ?2 AND lesson_id = ?3 AND status = 'active'",
                params![
                    invalidated_at,
                    project.project_id().to_string(),
                    replaced_id.to_string()
                ],
            )
            .map_err(|source| map_sqlite(&path, source))?;
        if changed != 1 {
            return Err(StoreError::LessonNotFound);
        }
        let (lesson, created) = match existing {
            Some(lesson) => (lesson, false),
            None => (
                insert_lesson(&path, &transaction, project, text, Some(replaced.id()), now)?,
                true,
            ),
        };
        insert_audit(
            &path,
            &transaction,
            project,
            &lesson,
            "superseded",
            Some(replaced.id()),
            now,
        )?;
        transaction
            .commit()
            .map_err(|source| map_sqlite(&path, source))?;
        Ok(TeachOutcome { lesson, created })
    }

    pub(crate) fn list_lessons(
        &self,
        project: &ProjectScope,
        limit: usize,
    ) -> Result<LessonList, StoreError> {
        let project_id = project.project_id().to_string();
        let total = self
            ._memory
            .query_row(
                "SELECT COUNT(*) FROM lessons WHERE project_id = ?1 AND status = 'active'",
                [&project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let total = usize::try_from(total).map_err(|_| StoreError::CorruptLesson {
            reason: "active lesson count exceeds platform limits".to_owned(),
        })?;
        let sql_limit = i64::try_from(limit).map_err(|_| StoreError::CorruptLesson {
            reason: "lesson list limit exceeds SQLite limits".to_owned(),
        })?;
        let mut statement = self
            ._memory
            .prepare(
                "SELECT sequence, lesson_id, content, content_hash, provenance, trust, status,
                        supersedes_id, created_at_ms, updated_at_ms
                 FROM lessons
                 WHERE project_id = ?1 AND status = 'active'
                 ORDER BY sequence DESC
                 LIMIT ?2",
            )
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let rows = statement
            .query_map(params![project_id, sql_limit], read_raw_lesson)
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let mut lessons = Vec::with_capacity(total.min(limit));
        let mut corrupt_count = 0;
        for row in rows {
            let raw = row.map_err(|source| map_sqlite(&self.memory_path, source))?;
            match decode_lesson(raw) {
                Ok(lesson) => lessons.push(lesson),
                Err(error) => {
                    corrupt_count += 1;
                    log_corrupt_row(&error, "list");
                }
            }
        }
        Ok(LessonList {
            omitted_count: total.saturating_sub(lessons.len() + corrupt_count),
            lessons,
            corrupt_count,
        })
    }

    pub(crate) fn inspect_lesson(
        &self,
        project: &ProjectScope,
        id: LessonId,
    ) -> Result<StoredLesson, StoreError> {
        find_lesson(&self.memory_path, &self._memory, project, id, None)?
            .ok_or(StoreError::LessonNotFound)
    }

    pub(crate) fn context(
        &self,
        project: &ProjectScope,
        budget: usize,
    ) -> Result<Option<ContextBlock>, StoreError> {
        let project_id = project.project_id().to_string();
        let total = self
            ._memory
            .query_row(
                "SELECT COUNT(*) FROM lessons
                 WHERE project_id = ?1 AND status = 'active'
                   AND provenance = 'user_explicit' AND trust = 'instruction'",
                [&project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let total = usize::try_from(total).map_err(|_| StoreError::CorruptLesson {
            reason: "context lesson count exceeds platform limits".to_owned(),
        })?;
        if total == 0 {
            return Ok(None);
        }
        let mut statement = self
            ._memory
            .prepare(
                "SELECT sequence, lesson_id, content, content_hash, provenance, trust, status,
                        supersedes_id, created_at_ms, updated_at_ms
                 FROM lessons
                 WHERE project_id = ?1 AND status = 'active'
                   AND provenance = 'user_explicit' AND trust = 'instruction'
                 ORDER BY sequence DESC",
            )
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let rows = statement
            .query_map([project_id], read_raw_lesson)
            .map_err(|source| map_sqlite(&self.memory_path, source))?;
        let mut candidates = Vec::new();
        let mut candidate_chars = 0_usize;
        let mut corrupt_count = 0_usize;
        for row in rows {
            let raw = row.map_err(|source| map_sqlite(&self.memory_path, source))?;
            let lesson = match decode_lesson(raw) {
                Ok(lesson) => lesson,
                Err(error) => {
                    // One corrupt row must not blank first-prompt context for
                    // the whole project; it is skipped and reported.
                    corrupt_count += 1;
                    log_corrupt_row(&error, "context");
                    continue;
                }
            };
            let candidate = ContextLesson::new(lesson.sequence(), lesson.text().clone());
            candidate_chars = candidate_chars.saturating_add(candidate.rendered_line_chars());
            candidates.push(candidate);
            if candidate_chars > budget {
                break;
            }
        }
        Ok(render_context(
            &candidates,
            total.saturating_sub(corrupt_count),
            budget,
        ))
    }
}

fn log_corrupt_row(error: &StoreError, operation: &'static str) {
    tracing::warn!(error = %error, operation, "skipping corrupt stored lesson row");
}

struct RawLesson {
    sequence: i64,
    id: String,
    content: String,
    content_hash: String,
    provenance: String,
    trust: String,
    status: String,
    supersedes_id: Option<String>,
    created_at_ms: i64,
    updated_at_ms: i64,
}

fn read_raw_lesson(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawLesson> {
    Ok(RawLesson {
        sequence: row.get(0)?,
        id: row.get(1)?,
        content: row.get(2)?,
        content_hash: row.get(3)?,
        provenance: row.get(4)?,
        trust: row.get(5)?,
        status: row.get(6)?,
        supersedes_id: row.get(7)?,
        created_at_ms: row.get(8)?,
        updated_at_ms: row.get(9)?,
    })
}

fn decode_lesson(raw: RawLesson) -> Result<StoredLesson, StoreError> {
    let sequence = u64::try_from(raw.sequence).map_err(|_| StoreError::CorruptLesson {
        reason: "lesson sequence is negative".to_owned(),
    })?;
    let id = decode_lesson_id(&raw.id)?;
    let hash = decode_fixed::<32>(&raw.content_hash, "content hash")?;
    let text =
        LessonText::from_stored(raw.content, hash).map_err(|error| StoreError::CorruptLesson {
            reason: error.to_string(),
        })?;
    let provenance = LessonProvenance::from_stored(&raw.provenance).ok_or_else(|| {
        StoreError::CorruptLesson {
            reason: "lesson provenance is unknown".to_owned(),
        }
    })?;
    let trust = LessonTrust::from_stored(&raw.trust).ok_or_else(|| StoreError::CorruptLesson {
        reason: "lesson trust is unknown".to_owned(),
    })?;
    let status =
        LessonStatus::from_stored(&raw.status).ok_or_else(|| StoreError::CorruptLesson {
            reason: "lesson status is unknown".to_owned(),
        })?;
    let supersedes_id = raw
        .supersedes_id
        .as_deref()
        .map(decode_lesson_id)
        .transpose()?;
    Ok(StoredLesson {
        sequence,
        id,
        text,
        provenance,
        trust,
        status,
        supersedes_id,
        created_at_ms: raw.created_at_ms,
        updated_at_ms: raw.updated_at_ms,
    })
}

fn decode_lesson_id(value: &str) -> Result<LessonId, StoreError> {
    decode_fixed::<16>(value, "lesson identity").map(LessonId::from_bytes)
}

fn decode_fixed<const N: usize>(value: &str, label: &str) -> Result<[u8; N], StoreError> {
    crate::encoding::decode_fixed_hex::<N>(value).ok_or_else(|| StoreError::CorruptLesson {
        reason: format!("{label} is not {N}-byte hexadecimal"),
    })
}

fn ensure_project(
    path: &Path,
    transaction: &Transaction<'_>,
    project: &ProjectScope,
    now: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO projects (project_id, display_path, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(project_id) DO UPDATE
             SET display_path = excluded.display_path, updated_at_ms = excluded.updated_at_ms",
            params![
                project.project_id().to_string(),
                project.display_path().to_string_lossy(),
                now
            ],
        )
        .map_err(|source| map_sqlite(path, source))?;
    Ok(())
}

fn find_active_by_hash(
    path: &Path,
    connection: &Connection,
    project: &ProjectScope,
    text: &LessonText,
) -> Result<Option<StoredLesson>, StoreError> {
    let raw = connection
        .query_row(
            "SELECT sequence, lesson_id, content, content_hash, provenance, trust, status,
                    supersedes_id, created_at_ms, updated_at_ms
             FROM lessons
             WHERE project_id = ?1 AND content_hash = ?2 AND status = 'active'",
            params![
                project.project_id().to_string(),
                hex::encode(text.content_hash())
            ],
            read_raw_lesson,
        )
        .optional()
        .map_err(|source| map_sqlite(path, source))?;
    raw.map(decode_lesson).transpose()
}

fn find_lesson(
    path: &Path,
    connection: &Connection,
    project: &ProjectScope,
    id: LessonId,
    status: Option<LessonStatus>,
) -> Result<Option<StoredLesson>, StoreError> {
    let raw = match status {
        Some(status) => connection
            .query_row(
                "SELECT sequence, lesson_id, content, content_hash, provenance, trust, status,
                        supersedes_id, created_at_ms, updated_at_ms
                 FROM lessons
                 WHERE project_id = ?1 AND lesson_id = ?2 AND status = ?3",
                params![
                    project.project_id().to_string(),
                    id.to_string(),
                    status.as_str()
                ],
                read_raw_lesson,
            )
            .optional(),
        None => connection
            .query_row(
                "SELECT sequence, lesson_id, content, content_hash, provenance, trust, status,
                        supersedes_id, created_at_ms, updated_at_ms
                 FROM lessons
                 WHERE project_id = ?1 AND lesson_id = ?2",
                params![project.project_id().to_string(), id.to_string()],
                read_raw_lesson,
            )
            .optional(),
    }
    .map_err(|source| map_sqlite(path, source))?;
    raw.map(decode_lesson).transpose()
}

fn insert_lesson(
    path: &Path,
    transaction: &Transaction<'_>,
    project: &ProjectScope,
    text: &LessonText,
    supersedes_id: Option<LessonId>,
    now: i64,
) -> Result<StoredLesson, StoreError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(StoreError::Random)?;
    let id = LessonId::from_bytes(bytes);
    let provenance = LessonProvenance::UserExplicit;
    let trust = LessonTrust::Instruction;
    let status = LessonStatus::Active;
    transaction
        .execute(
            "INSERT INTO lessons (
                lesson_id, project_id, content, content_hash, provenance, trust, status,
                supersedes_id, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id.to_string(),
                project.project_id().to_string(),
                text.redacted(),
                hex::encode(text.content_hash()),
                provenance.as_str(),
                trust.as_str(),
                status.as_str(),
                supersedes_id.map(|value| value.to_string()),
                now
            ],
        )
        .map_err(|source| map_sqlite(path, source))?;
    let sequence =
        u64::try_from(transaction.last_insert_rowid()).map_err(|_| StoreError::CorruptLesson {
            reason: "inserted lesson sequence is negative".to_owned(),
        })?;
    Ok(StoredLesson {
        sequence,
        id,
        text: text.clone(),
        provenance,
        trust,
        status,
        supersedes_id,
        created_at_ms: now,
        updated_at_ms: now,
    })
}

fn insert_audit(
    path: &Path,
    transaction: &Transaction<'_>,
    project: &ProjectScope,
    lesson: &StoredLesson,
    action: &str,
    target_lesson_id: Option<LessonId>,
    now: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO memory_audit (
                project_id, lesson_id, target_lesson_id, action, content_hash, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                project.project_id().to_string(),
                lesson.id().to_string(),
                target_lesson_id.map(|id| id.to_string()),
                action,
                hex::encode(lesson.text().content_hash()),
                now
            ],
        )
        .map_err(|source| map_sqlite(path, source))?;
    Ok(())
}

fn now_ms() -> Result<i64, StoreError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(StoreError::Clock)?
        .as_millis();
    i64::try_from(millis).map_err(|_| StoreError::CorruptLesson {
        reason: "system time exceeds SQLite integer range".to_owned(),
    })
}

struct StoreFileState {
    existed: bool,
    was_nonempty: bool,
}

#[derive(Clone, Copy)]
enum StoreKind {
    Memory,
    Knowledge,
}

fn open_lock(path: &Path) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let file = options.open(path).map_err(|source| map_io(path, source))?;
    #[cfg(unix)]
    file.set_permissions(Permissions::from_mode(0o600))
        .map_err(|source| map_io(path, source))?;
    Ok(file)
}

fn try_lock(file: &File, path: &Path) -> Result<(), StoreError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(std::fs::TryLockError::WouldBlock) => Err(StoreError::AlreadyRunning {
            path: path.to_path_buf(),
        }),
        Err(std::fs::TryLockError::Error(source)) => Err(StoreError::Lock {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn precreate_store(path: &Path) -> Result<StoreFileState, StoreError> {
    let (existed, was_nonempty) = match fs::metadata(path) {
        Ok(metadata) => (true, metadata.len() != 0),
        Err(source) if source.kind() == io::ErrorKind::NotFound => (false, false),
        Err(source) => return Err(map_io(path, source)),
    };

    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let file = options.open(path).map_err(|source| map_io(path, source))?;
    #[cfg(unix)]
    file.set_permissions(Permissions::from_mode(0o600))
        .map_err(|source| map_io(path, source))?;
    drop(file);

    Ok(StoreFileState {
        existed,
        was_nonempty,
    })
}

fn open_store(
    path: &Path,
    state: StoreFileState,
    kind: StoreKind,
    target_version: u32,
) -> Result<Connection, StoreError> {
    let mut connection = Connection::open(path).map_err(|source| map_sqlite(path, source))?;
    connection
        .busy_timeout(STARTUP_BUSY_TIMEOUT)
        .map_err(|source| map_sqlite(path, source))?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|source| map_sqlite(path, source))?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|source| map_sqlite(path, source))?;
    migrate_store(path, state, transaction, kind, target_version)?;

    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|source| map_sqlite(path, source))?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|source| map_sqlite(path, source))?;
    Ok(connection)
}

fn migrate_store(
    path: &Path,
    state: StoreFileState,
    transaction: Transaction<'_>,
    kind: StoreKind,
    target_version: u32,
) -> Result<(), StoreError> {
    let objects = read_schema_objects(path, &transaction)?;
    if objects.is_empty() {
        if state.existed && state.was_nonempty {
            return Err(StoreError::MissingMetadata {
                path: path.to_path_buf(),
            });
        }
        transaction
            .execute_batch(
                "CREATE TABLE schema_version (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    version INTEGER NOT NULL CHECK (version > 0)
                );
                INSERT INTO schema_version (singleton, version) VALUES (1, 1);",
            )
            .map_err(|source| map_sqlite(path, source))?;
    } else if !objects
        .iter()
        .any(|(kind, name)| kind == "table" && name == "schema_version")
    {
        return Err(StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "schema_version is absent while other schema objects exist".to_owned(),
        });
    }

    let metadata_count = transaction
        .query_row("SELECT COUNT(*) FROM schema_version", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    if metadata_count == 0 {
        return Err(StoreError::MissingMetadata {
            path: path.to_path_buf(),
        });
    }
    if metadata_count > 1 {
        let count = usize::try_from(metadata_count).map_err(|_| StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "schema metadata row count exceeds platform limits".to_owned(),
        })?;
        return Err(StoreError::DuplicateMetadata {
            path: path.to_path_buf(),
            count,
        });
    }
    validate_schema_shape(path, &transaction)?;
    validate_schema_constraints(path, &transaction)?;
    let version = read_schema_version(path, &transaction)?;
    if version > i64::from(target_version) {
        return Err(StoreError::UnsupportedSchema {
            path: path.to_path_buf(),
            version,
        });
    }

    if version == 1 && matches!(kind, StoreKind::Memory) && target_version == 2 {
        migrate_memory_to_v2(path, &transaction)?;
    } else if version != i64::from(target_version) {
        return Err(StoreError::UnsupportedSchema {
            path: path.to_path_buf(),
            version,
        });
    }
    validate_store_objects(path, &transaction, kind, target_version)?;

    transaction
        .commit()
        .map_err(|source| map_sqlite(path, source))
}

fn migrate_memory_to_v2(path: &Path, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    transaction
        .execute_batch(
            "CREATE TABLE projects (
                project_id TEXT PRIMARY KEY CHECK (length(project_id) = 64),
                display_path TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
            CREATE TABLE lessons (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                lesson_id TEXT NOT NULL UNIQUE CHECK (length(lesson_id) = 32),
                project_id TEXT NOT NULL REFERENCES projects(project_id),
                content TEXT NOT NULL,
                content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
                provenance TEXT NOT NULL CHECK (provenance IN ('user_explicit')),
                trust TEXT NOT NULL CHECK (trust IN ('instruction')),
                status TEXT NOT NULL CHECK (status IN ('active', 'invalidated')),
                supersedes_id TEXT REFERENCES lessons(lesson_id),
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX lessons_active_content_idx
                ON lessons(project_id, content_hash) WHERE status = 'active';
            CREATE INDEX lessons_project_sequence_idx
                ON lessons(project_id, status, sequence DESC);
            CREATE TABLE memory_audit (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id TEXT NOT NULL REFERENCES projects(project_id),
                lesson_id TEXT NOT NULL REFERENCES lessons(lesson_id),
                target_lesson_id TEXT REFERENCES lessons(lesson_id),
                action TEXT NOT NULL CHECK (action IN ('created', 'duplicate', 'superseded')),
                content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
                created_at_ms INTEGER NOT NULL
            );
            UPDATE schema_version SET version = 2 WHERE singleton = 1;",
        )
        .map_err(|source| map_sqlite(path, source))
}

fn validate_store_objects(
    path: &Path,
    transaction: &Transaction<'_>,
    kind: StoreKind,
    version: u32,
) -> Result<(), StoreError> {
    let actual = read_schema_objects(path, transaction)?;
    let expected: &[(&str, &str)] = match (kind, version) {
        (StoreKind::Memory, 2) => &[
            ("table", "lessons"),
            ("index", "lessons_active_content_idx"),
            ("index", "lessons_project_sequence_idx"),
            ("table", "memory_audit"),
            ("table", "projects"),
            ("table", "schema_version"),
        ],
        (StoreKind::Knowledge, 1) => &[("table", "schema_version")],
        _ => {
            return Err(StoreError::UnsupportedSchema {
                path: path.to_path_buf(),
                version: i64::from(version),
            });
        }
    };
    if actual
        != expected
            .iter()
            .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
            .collect::<Vec<_>>()
    {
        return Err(StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "store has unexpected schema objects".to_owned(),
        });
    }
    Ok(())
}

fn read_schema_objects(
    path: &Path,
    transaction: &Transaction<'_>,
) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT type, name\n             FROM sqlite_master\n             WHERE name NOT LIKE 'sqlite_%'\n             ORDER BY name",
        )
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut objects = Vec::new();
    for row in rows {
        let object = row.map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
        objects.push(object);
    }
    Ok(objects)
}

fn validate_schema_shape(path: &Path, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let mut statement = transaction
        .prepare("PRAGMA table_info(schema_version)")
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row.map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?);
    }

    let expected = [
        ("singleton", "INTEGER", 0_i64, 1_i64),
        ("version", "INTEGER", 1_i64, 0_i64),
    ];
    if columns.len() != expected.len()
        || columns.iter().zip(expected).any(|(actual, expected)| {
            actual.0 != expected.0
                || !actual.1.eq_ignore_ascii_case(expected.1)
                || actual.2 != expected.2
                || actual.3 != expected.3
        })
    {
        return Err(StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "schema_version has unexpected columns".to_owned(),
        });
    }
    Ok(())
}

fn validate_schema_constraints(
    path: &Path,
    transaction: &Transaction<'_>,
) -> Result<(), StoreError> {
    transaction
        .execute_batch("SAVEPOINT cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;

    let singleton_rejected = constraint_rejected(
        path,
        transaction.execute(
            "INSERT INTO schema_version (singleton, version) VALUES (2, 1)",
            [],
        ),
    )?;
    transaction
        .execute_batch("ROLLBACK TO cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;
    transaction
        .execute_batch("RELEASE cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;
    if !singleton_rejected {
        return Err(StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "singleton CHECK constraint is absent".to_owned(),
        });
    }
    transaction
        .execute_batch("SAVEPOINT cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;

    let version_rejected = constraint_rejected(
        path,
        transaction.execute(
            "UPDATE schema_version SET version = 0 WHERE singleton = 1",
            [],
        ),
    )?;
    transaction
        .execute_batch("ROLLBACK TO cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;
    transaction
        .execute_batch("RELEASE cyril_schema_constraints")
        .map_err(|source| map_sqlite(path, source))?;
    if !version_rejected {
        return Err(StoreError::CorruptSchema {
            path: path.to_path_buf(),
            reason: "version CHECK constraint is absent".to_owned(),
        });
    }
    Ok(())
}
fn constraint_rejected(path: &Path, result: rusqlite::Result<usize>) -> Result<bool, StoreError> {
    match result {
        Ok(_) => Ok(false),
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == ErrorCode::ConstraintViolation =>
        {
            Ok(true)
        }
        Err(source) => Err(map_sqlite(path, source)),
    }
}

fn read_schema_version(path: &Path, transaction: &Transaction<'_>) -> Result<i64, StoreError> {
    let (singleton, version) = transaction
        .query_row(
            "SELECT singleton, version FROM schema_version ORDER BY rowid",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    if singleton != 1 || version <= 0 {
        return Err(StoreError::Invalid {
            path: path.to_path_buf(),
            reason: format!(
                "expected singleton=1 and version>0, found singleton={singleton}, version={version}"
            ),
        });
    }
    Ok(version)
}

fn map_io(path: &Path, source: io::Error) -> StoreError {
    if source.kind() == io::ErrorKind::NotFound {
        StoreError::Missing {
            path: path.to_path_buf(),
        }
    } else if source.kind() == io::ErrorKind::PermissionDenied {
        StoreError::Permission {
            path: path.to_path_buf(),
            source,
        }
    } else {
        StoreError::Unreadable {
            path: path.to_path_buf(),
            source,
        }
    }
}

fn map_sqlite(path: &Path, source: rusqlite::Error) -> StoreError {
    match source {
        rusqlite::Error::SqliteFailure(error, message)
            if matches!(
                error.code,
                ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt
            ) =>
        {
            StoreError::Malformed {
                path: path.to_path_buf(),
                source: rusqlite::Error::SqliteFailure(error, message),
            }
        }
        source => StoreError::Sqlite {
            path: path.to_path_buf(),
            source,
        },
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use rusqlite::Connection;
    use tempfile::TempDir;
    fn test_paths() -> (TempDir, MemoryPaths) {
        let root = TempDir::new().unwrap();
        let paths = MemoryPaths::prepare(Some(root.path())).unwrap();
        (root, paths)
    }

    fn user_tables(connection: &Connection) -> Vec<String> {
        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|row| row.unwrap())
            .collect()
    }

    fn assert_metadata(connection: &Connection, version: i64, tables: &[&str]) {
        assert_eq!(user_tables(connection), tables);
        assert_eq!(
            connection
                .query_row("SELECT singleton, version FROM schema_version", [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap(),
            (1, version)
        );
    }

    fn assert_store(path: &Path, version: i64, tables: &[&str]) {
        let connection = Connection::open(path).unwrap();
        assert_metadata(&connection, version, tables);
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_ascii_uppercase(),
            "WAL"
        );
    }

    fn assert_connection(connection: &Connection, version: i64, tables: &[&str]) {
        assert_metadata(connection, version, tables);
        assert_eq!(
            connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_ascii_uppercase(),
            "WAL"
        );
        assert_eq!(
            connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            250
        );
    }

    #[test]
    fn versions_are_private_and_accessible() {
        let versions = MemoryStoreVersions::new(7, 9);
        assert_eq!(versions.memory(), 7);
        assert_eq!(versions.knowledge(), 9);

        let (_root, paths) = test_paths();
        let stores = StoreSet::open(&paths).unwrap();
        assert_eq!(stores.versions().memory(), 2);
        assert_eq!(stores.versions().knowledge(), 1);
    }

    #[test]
    fn fresh_stores_are_minimal_private_and_reopenable() {
        let (_root, paths) = test_paths();
        {
            let stores = StoreSet::open(&paths).unwrap();
            assert_eq!(stores.versions(), MemoryStoreVersions::new(2, 1));
            assert_connection(
                &stores._memory,
                2,
                &["lessons", "memory_audit", "projects", "schema_version"],
            );
            assert_connection(&stores._knowledge, 1, &["schema_version"]);
        }

        assert_store(
            paths.memory_store_path(),
            2,
            &["lessons", "memory_audit", "projects", "schema_version"],
        );
        assert_store(paths.knowledge_store_path(), 1, &["schema_version"]);
        #[cfg(unix)]
        {
            assert_eq!(
                fs::metadata(paths.memory_store_path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(paths.knowledge_store_path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let stores = StoreSet::open(&paths).unwrap();
        assert_eq!(stores.versions(), MemoryStoreVersions::new(2, 1));
    }

    #[test]
    fn partial_initialization_is_completed_without_replacing_existing_store() {
        let (_root, paths) = test_paths();
        {
            let _stores = StoreSet::open(&paths).unwrap();
        }
        let memory_before = fs::read(paths.memory_store_path()).unwrap();
        fs::remove_file(paths.knowledge_store_path()).unwrap();

        let _stores = StoreSet::open(&paths).unwrap();
        assert_eq!(fs::read(paths.memory_store_path()).unwrap(), memory_before);
        assert_store(paths.knowledge_store_path(), 1, &["schema_version"]);
    }

    #[test]
    fn second_owner_is_rejected_until_first_owner_drops() {
        let (_root, paths) = test_paths();
        let stores = StoreSet::open(&paths).unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::AlreadyRunning { .. })
        ));
        drop(stores);
        assert!(StoreSet::open(&paths).is_ok());
    }

    #[test]
    fn missing_metadata_is_distinct() {
        let (_root, paths) = test_paths();
        Connection::open(paths.memory_store_path())
            .unwrap()
            .execute_batch(
                "CREATE TABLE schema_version (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    version INTEGER NOT NULL CHECK (version > 0)
                )",
            )
            .unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::MissingMetadata { .. })
        ));
    }

    #[test]
    fn duplicate_metadata_is_distinct() {
        let (_root, paths) = test_paths();
        Connection::open(paths.memory_store_path())
            .unwrap()
            .execute_batch(
                "CREATE TABLE schema_version (singleton INTEGER, version INTEGER);
                 INSERT INTO schema_version VALUES (1, 1), (1, 1);",
            )
            .unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::DuplicateMetadata { count: 2, .. })
        ));
    }

    #[test]
    fn unsupported_metadata_is_distinct() {
        let (_root, paths) = test_paths();
        Connection::open(paths.memory_store_path())
            .unwrap()
            .execute_batch(
                "CREATE TABLE schema_version (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    version INTEGER NOT NULL CHECK (version > 0)
                );
                INSERT INTO schema_version VALUES (1, 3);",
            )
            .unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::UnsupportedSchema { version: 3, .. })
        ));
    }

    #[test]
    fn corrupt_metadata_is_not_destructively_initialized() {
        let (_root, paths) = test_paths();
        Connection::open(paths.memory_store_path())
            .unwrap()
            .execute_batch(
                "CREATE TABLE schema_version (singleton TEXT, version TEXT);
                 INSERT INTO schema_version VALUES ('bad', 'bad');",
            )
            .unwrap();
        let before = fs::read(paths.memory_store_path()).unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::CorruptSchema { .. }) | Err(StoreError::Invalid { .. })
        ));
        assert_eq!(fs::read(paths.memory_store_path()).unwrap(), before);
    }

    #[test]
    fn malformed_sqlite_is_distinct() {
        let (_root, paths) = test_paths();
        fs::write(paths.memory_store_path(), b"not a sqlite database").unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::Malformed { .. })
        ));
    }

    #[test]
    fn memory_v1_migrates_atomically_to_v2() {
        let (_root, paths) = test_paths();
        Connection::open(paths.memory_store_path())
            .unwrap()
            .execute_batch(
                "CREATE TABLE schema_version (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    version INTEGER NOT NULL CHECK (version > 0)
                );
                INSERT INTO schema_version VALUES (1, 1);",
            )
            .unwrap();

        let stores = StoreSet::open(&paths).unwrap();
        assert_eq!(stores.versions(), MemoryStoreVersions::new(2, 1));
        assert_eq!(
            user_tables(&stores._memory),
            vec!["lessons", "memory_audit", "projects", "schema_version"]
        );
        assert_eq!(
            stores
                ._memory
                .query_row("SELECT version FROM schema_version", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    fn project_store() -> (TempDir, crate::ProjectScope, StoreSet) {
        let root = TempDir::new().unwrap();
        let workspace = root.path().join("project");
        fs::create_dir(&workspace).unwrap();
        let paths = MemoryPaths::prepare(Some(&root.path().join("data"))).unwrap();
        let project = crate::ProjectScope::resolve(&workspace).unwrap();
        let stores = StoreSet::open(&paths).unwrap();
        (root, project, stores)
    }

    fn last_audit(stores: &StoreSet) -> (String, Option<String>, String) {
        stores
            ._memory
            .query_row(
                "SELECT lesson_id, target_lesson_id, action
                 FROM memory_audit ORDER BY sequence DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    #[test]
    fn replacement_matching_another_lesson_still_supersedes_the_target() {
        let (_root, project, mut stores) = project_store();
        let first = stores
            .teach_lesson(&project, &crate::LessonText::new("first").unwrap())
            .unwrap()
            .lesson;
        let existing = stores
            .teach_lesson(&project, &crate::LessonText::new("existing").unwrap())
            .unwrap()
            .lesson;
        let outcome = stores
            .replace_lesson(
                &project,
                first.id(),
                &crate::LessonText::new("existing").unwrap(),
            )
            .unwrap();
        assert!(!outcome.created);
        assert_eq!(outcome.lesson.id(), existing.id());
        // The user asked for `first` to go away: it is no longer active.
        assert_eq!(
            stores
                .inspect_lesson(&project, first.id())
                .unwrap()
                .status(),
            LessonStatus::Invalidated
        );
        let active = stores.list_lessons(&project, 100).unwrap();
        assert_eq!(active.lessons().len(), 1);
        assert_eq!(active.lessons()[0].id(), existing.id());
        let (result_id, target_id, action) = last_audit(&stores);
        assert_eq!(result_id, existing.id().to_string());
        assert_eq!(target_id, Some(first.id().to_string()));
        assert_eq!(action, "superseded");
    }

    #[test]
    fn replacing_a_lesson_with_its_own_text_is_a_noop() {
        let (_root, project, mut stores) = project_store();
        let text = crate::LessonText::new("keep me").unwrap();
        let lesson = stores.teach_lesson(&project, &text).unwrap().lesson;
        let outcome = stores.replace_lesson(&project, lesson.id(), &text).unwrap();
        assert!(!outcome.created);
        assert_eq!(outcome.lesson.id(), lesson.id());
        assert_eq!(
            stores
                .inspect_lesson(&project, lesson.id())
                .unwrap()
                .status(),
            LessonStatus::Active
        );
        assert_eq!(last_audit(&stores).2, "duplicate");
    }

    #[test]
    fn replacing_an_already_replaced_lesson_is_distinct_from_not_found() {
        let (_root, project, mut stores) = project_store();
        let first = stores
            .teach_lesson(&project, &crate::LessonText::new("first").unwrap())
            .unwrap()
            .lesson;
        stores
            .replace_lesson(
                &project,
                first.id(),
                &crate::LessonText::new("second").unwrap(),
            )
            .unwrap();
        let stale = stores.replace_lesson(
            &project,
            first.id(),
            &crate::LessonText::new("third").unwrap(),
        );
        assert!(
            matches!(stale, Err(StoreError::LessonSuperseded)),
            "{stale:?}"
        );
        let unknown = stores.replace_lesson(
            &project,
            LessonId::from_bytes([9; 16]),
            &crate::LessonText::new("third").unwrap(),
        );
        assert!(
            matches!(unknown, Err(StoreError::LessonNotFound)),
            "{unknown:?}"
        );
        assert_eq!(
            StoreError::LessonSuperseded.to_string(),
            "lesson was already replaced and is no longer active"
        );
    }

    #[test]
    fn invalidation_stamp_never_precedes_creation_after_clock_step_back() {
        let (_root, project, mut stores) = project_store();
        let lesson = stores
            .teach_lesson(&project, &crate::LessonText::new("created late").unwrap())
            .unwrap()
            .lesson;
        // Simulate the wall clock stepping back after creation by pushing the
        // row's creation stamp into the future.
        let future = lesson.created_at_ms() + 5_000;
        stores
            ._memory
            .execute(
                "UPDATE lessons SET created_at_ms = ?1, updated_at_ms = ?1 WHERE lesson_id = ?2",
                params![future, lesson.id().to_string()],
            )
            .unwrap();
        stores
            .replace_lesson(
                &project,
                lesson.id(),
                &crate::LessonText::new("replacement").unwrap(),
            )
            .unwrap();
        let invalidated = stores.inspect_lesson(&project, lesson.id()).unwrap();
        assert_eq!(invalidated.status(), LessonStatus::Invalidated);
        assert!(invalidated.updated_at_ms() >= invalidated.created_at_ms());
        assert_eq!(invalidated.updated_at_ms(), future);
    }

    #[test]
    fn corrupt_rows_are_skipped_and_stale_redaction_is_healed_on_read() {
        let (_root, project, mut stores) = project_store();
        let healthy = stores
            .teach_lesson(&project, &crate::LessonText::new("healthy").unwrap())
            .unwrap()
            .lesson;
        let stale = stores
            .teach_lesson(&project, &crate::LessonText::new("stale").unwrap())
            .unwrap()
            .lesson;
        let broken = stores
            .teach_lesson(&project, &crate::LessonText::new("broken").unwrap())
            .unwrap()
            .lesson;
        // A row an older, looser redactor would have written raw, with a
        // correct hash: integrity holds, so it is served redacted.
        let raw = "use ghp_abcdefghijklmnopqrstuvwxyz1234 in CI";
        let raw_hash: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(raw.as_bytes()).into();
        stores
            ._memory
            .execute(
                "UPDATE lessons SET content = ?1, content_hash = ?2 WHERE lesson_id = ?3",
                params![raw, hex::encode(raw_hash), stale.id().to_string()],
            )
            .unwrap();
        // A row whose bytes no longer match its hash is corrupt.
        stores
            ._memory
            .execute(
                "UPDATE lessons SET content = 'tampered' WHERE lesson_id = ?1",
                [broken.id().to_string()],
            )
            .unwrap();

        let list = stores.list_lessons(&project, 100).unwrap();
        assert_eq!(list.corrupt_count(), 1);
        assert_eq!(list.omitted_count(), 0);
        let ids: Vec<_> = list.lessons().iter().map(StoredLesson::id).collect();
        assert_eq!(ids, vec![stale.id(), healthy.id()]);
        assert_eq!(list.lessons()[0].text().redacted(), "use [REDACTED] in CI");

        let context = stores.context(&project, 4_000).unwrap().unwrap();
        assert!(context.text().contains("use [REDACTED] in CI"));
        assert!(context.text().contains("healthy"));
        assert!(!context.text().contains("ghp_"));
        assert!(!context.text().contains("tampered"));
        assert_eq!(context.omitted_count(), 0);

        let inspect = stores.inspect_lesson(&project, broken.id());
        assert!(
            matches!(inspect, Err(StoreError::CorruptLesson { .. })),
            "{inspect:?}"
        );
        assert!(stores.inspect_lesson(&project, stale.id()).is_ok());
        // Teaching still works alongside a corrupt sibling row.
        assert!(
            stores
                .teach_lesson(&project, &crate::LessonText::new("after").unwrap())
                .unwrap()
                .created()
        );
    }

    #[test]
    fn lesson_lifecycle_is_scoped_idempotent_and_append_audited() {
        let root = TempDir::new().unwrap();
        let data_root = root.path().join("data");
        let workspace_a = root.path().join("project-a");
        let workspace_b = root.path().join("project-b");
        fs::create_dir_all(&workspace_a).unwrap();
        fs::create_dir_all(&workspace_b).unwrap();
        let paths = MemoryPaths::prepare(Some(&data_root)).unwrap();
        let project_a = crate::ProjectScope::resolve(&workspace_a).unwrap();
        let project_b = crate::ProjectScope::resolve(&workspace_b).unwrap();
        let first_text = crate::LessonText::new("use password=hunter2 safely").unwrap();
        let replacement_text = crate::LessonText::new("prefer boring Rust").unwrap();
        let final_text = crate::LessonText::new("prefer explicit errors").unwrap();

        let first_id;
        let replacement_id;
        let final_id;
        {
            let mut stores = StoreSet::open(&paths).unwrap();
            let first = stores.teach_lesson(&project_a, &first_text).unwrap();
            assert!(first.created());
            first_id = first.lesson().id();
            let duplicate = stores.teach_lesson(&project_a, &first_text).unwrap();
            assert!(!duplicate.created());
            assert_eq!(duplicate.lesson().id(), first_id);

            let other = stores.teach_lesson(&project_b, &first_text).unwrap();
            assert!(other.created());
            assert_ne!(other.lesson().id(), first_id);

            let replacement = stores
                .replace_lesson(&project_a, first_id, &replacement_text)
                .unwrap();
            replacement_id = replacement.lesson().id();
            assert!(replacement.created());
            assert_ne!(replacement_id, first_id);
            let final_lesson = stores
                .replace_lesson(&project_a, replacement_id, &final_text)
                .unwrap();
            final_id = final_lesson.lesson().id();
            assert!(final_lesson.created());
            assert_eq!(final_lesson.lesson().supersedes_id(), Some(replacement_id));

            let active = stores.list_lessons(&project_a, 100).unwrap();
            assert_eq!(active.omitted_count(), 0);
            assert_eq!(active.lessons().len(), 1);
            assert_eq!(active.lessons()[0].id(), final_id);
            assert_eq!(
                stores
                    .inspect_lesson(&project_a, first_id)
                    .unwrap()
                    .status(),
                crate::LessonStatus::Invalidated
            );
            assert!(stores.inspect_lesson(&project_b, first_id).is_err());
        }

        let stores = StoreSet::open(&paths).unwrap();
        assert_eq!(
            stores
                .inspect_lesson(&project_a, replacement_id)
                .unwrap()
                .status(),
            crate::LessonStatus::Invalidated
        );
        assert_eq!(
            stores
                .inspect_lesson(&project_a, final_id)
                .unwrap()
                .text()
                .redacted(),
            "prefer explicit errors"
        );
        let direct = Connection::open_with_flags(
            paths.memory_store_path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        assert_eq!(
            direct
                .query_row(
                    "SELECT COUNT(*) FROM memory_audit WHERE project_id = ?1",
                    [project_a.project_id().to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            4
        );
        assert_eq!(
            direct
                .query_row(
                    "SELECT COUNT(*) FROM lessons WHERE project_id = ?1 AND status = 'active'",
                    [project_a.project_id().to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }
}

#[cfg(unix)]
use std::fs::Permissions;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use rusqlite::{Connection, ErrorCode, Transaction, TransactionBehavior};
use thiserror::Error;

use crate::paths::MemoryPaths;

const CURRENT_SCHEMA_VERSION: u32 = 1;
const STARTUP_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const BUSY_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStoreVersions {
    memory: u32,
    knowledge: u32,
}

impl MemoryStoreVersions {
    const fn new(memory: u32, knowledge: u32) -> Self {
        Self { memory, knowledge }
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
}

pub struct StoreSet {
    _memory: Connection,
    _knowledge: Connection,
    _lock: File,
    versions: MemoryStoreVersions,
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

        let memory = open_store(memory_path, memory_state)?;
        let knowledge = open_store(knowledge_path, knowledge_state)?;
        let versions = MemoryStoreVersions::new(CURRENT_SCHEMA_VERSION, CURRENT_SCHEMA_VERSION);

        Ok(Self {
            _lock: lock,
            _memory: memory,
            _knowledge: knowledge,
            versions,
        })
    }

    pub fn versions(&self) -> MemoryStoreVersions {
        self.versions
    }
}

struct StoreFileState {
    existed: bool,
    was_nonempty: bool,
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

fn open_store(path: &Path, state: StoreFileState) -> Result<Connection, StoreError> {
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
    migrate_store(path, state, transaction)?;

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
                "CREATE TABLE schema_version (\n                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\n                    version INTEGER NOT NULL CHECK (version > 0)\n                );\n                INSERT INTO schema_version (singleton, version) VALUES (1, 1);",
            )
            .map_err(|source| map_sqlite(path, source))?;
    } else {
        let Some(schema_object) = objects.iter().find(|(_, name)| name == "schema_version") else {
            return Err(StoreError::CorruptSchema {
                path: path.to_path_buf(),
                reason: "schema_version is absent while other schema objects exist".to_owned(),
            });
        };
        if schema_object.0 != "table" || objects.len() != 1 {
            return Err(StoreError::CorruptSchema {
                path: path.to_path_buf(),
                reason: "schema_version is not the only user schema object".to_owned(),
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
        validate_schema_row(path, &transaction)?;
        validate_schema_constraints(path, &transaction)?;
    }

    transaction
        .commit()
        .map_err(|source| map_sqlite(path, source))
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

fn validate_schema_row(path: &Path, transaction: &Transaction<'_>) -> Result<(), StoreError> {
    let mut statement = transaction
        .prepare("SELECT singleton, version FROM schema_version ORDER BY rowid")
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?;
    let mut metadata = Vec::new();
    for row in rows {
        metadata.push(row.map_err(|source| StoreError::Malformed {
            path: path.to_path_buf(),
            source,
        })?);
    }

    if metadata.is_empty() {
        return Err(StoreError::MissingMetadata {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > 1 {
        return Err(StoreError::DuplicateMetadata {
            path: path.to_path_buf(),
            count: metadata.len(),
        });
    }

    let (singleton, version) = metadata[0];
    if singleton != 1 || version <= 0 {
        return Err(StoreError::Invalid {
            path: path.to_path_buf(),
            reason: format!(
                "expected singleton=1 and version>0, found singleton={singleton}, version={version}"
            ),
        });
    }
    if version != i64::from(CURRENT_SCHEMA_VERSION) {
        return Err(StoreError::UnsupportedSchema {
            path: path.to_path_buf(),
            version,
        });
    }
    Ok(())
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

    fn assert_metadata(connection: &Connection) {
        assert_eq!(
            user_tables(connection),
            vec![String::from("schema_version")]
        );
        assert_eq!(
            connection
                .query_row("SELECT singleton, version FROM schema_version", [], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap(),
            (1, 1)
        );
    }

    fn assert_store(path: &Path) {
        let connection = Connection::open(path).unwrap();
        assert_metadata(&connection);
        assert_eq!(
            connection
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_ascii_uppercase(),
            "WAL"
        );
    }

    fn assert_connection(connection: &Connection) {
        assert_metadata(connection);
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
        assert_eq!(stores.versions().memory(), 1);
        assert_eq!(stores.versions().knowledge(), 1);
    }

    #[test]
    fn fresh_stores_are_minimal_private_and_reopenable() {
        let (_root, paths) = test_paths();
        {
            let stores = StoreSet::open(&paths).unwrap();
            assert_eq!(stores.versions(), MemoryStoreVersions::new(1, 1));
            assert_connection(&stores._memory);
            assert_connection(&stores._knowledge);
        }

        assert_store(paths.memory_store_path());
        assert_store(paths.knowledge_store_path());
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
        assert_eq!(stores.versions(), MemoryStoreVersions::new(1, 1));
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
        assert_store(paths.knowledge_store_path());
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
                INSERT INTO schema_version VALUES (1, 2);",
            )
            .unwrap();
        assert!(matches!(
            StoreSet::open(&paths),
            Err(StoreError::UnsupportedSchema { version: 2, .. })
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
}

//! C10 cheapest falsifier — can `snapshot(&self)` open a deferred read
//! transaction WITHOUT changing its signature to `&mut self`?
//!
//! rusqlite's `Connection::transaction()` takes `&mut self`. `UsageLog::snapshot`
//! takes `&self`, and the App holds `UsageLog` by value, so a `&mut self`
//! snapshot would ripple into every call site and into the worker's ownership.
//! The claim is that `unchecked_transaction()` gives a real deferred
//! transaction through `&self`.
//!
//! Falsified if: the call does not compile through `&self`, or the transaction
//! it returns does not actually isolate the reader from a concurrent commit.
//!
//! Run: cp .cyril-nanu/probe_txn.rs crates/cyril-core/tests/probe_nanu_txn.rs
//!      cargo test -p cyril-core --test probe_nanu_txn -- --nocapture
//!      rm crates/cyril-core/tests/probe_nanu_txn.rs
use rusqlite::Connection;

struct FakeLog {
    connection: Connection,
}

impl FakeLog {
    /// Mirrors `UsageLog::snapshot`'s receiver exactly: `&self`, not `&mut self`.
    fn counted_twice_in_one_txn(&self) -> rusqlite::Result<(i64, i64)> {
        let txn = self.connection.unchecked_transaction()?;
        let first: i64 =
            txn.query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0))?;
        // A writer commits here, from another connection, in the test below.
        std::thread::sleep(std::time::Duration::from_millis(150));
        let second: i64 =
            txn.query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0))?;
        txn.commit()?;
        Ok((first, second))
    }
}

#[test]
fn deferred_read_transaction_works_through_shared_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("usage.sqlite3");

    let setup = Connection::open(&path).expect("open setup");
    setup
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE usage_turns (id INTEGER PRIMARY KEY, session_id TEXT NOT NULL);
             INSERT INTO usage_turns (session_id) VALUES ('a'), ('b'), ('c');",
        )
        .expect("seed");
    drop(setup);

    let reader = FakeLog {
        connection: Connection::open(&path).expect("open reader"),
    };
    reader
        .connection
        .busy_timeout(std::time::Duration::from_millis(250))
        .expect("reader busy timeout");

    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let conn = Connection::open(&writer_path).expect("open writer");
        conn.busy_timeout(std::time::Duration::from_millis(250))
            .expect("writer busy timeout");
        conn.execute("INSERT INTO usage_turns (session_id) VALUES ('mid')", [])
            .expect("writer commits while the reader's txn is open")
    });

    let (first, second) = reader
        .counted_twice_in_one_txn()
        .expect("deferred read transaction through &self");
    let written = writer.join().expect("writer thread joins");

    println!("C10: first={first} second={second} writer_rows_inserted={written}");
    assert_eq!(written, 1, "the writer really did commit mid-transaction");
    assert_eq!(first, 3, "reader sees the seeded state");
    assert_eq!(
        second, first,
        "both reads inside one deferred transaction see the same point in time"
    );

    // Positive control: outside the transaction the new row IS visible, so the
    // equality above is isolation and not a reader that simply never updates.
    let after: i64 = reader
        .connection
        .query_row("SELECT COUNT(*) FROM usage_turns", [], |row| row.get(0))
        .expect("post-commit read");
    println!("C10 control: after_txn={after}");
    assert_eq!(after, 4, "positive control: the committed row is visible once the txn ends");
}

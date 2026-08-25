use rusqlite::{params, Connection};

fn main() -> rusqlite::Result<()> {
    let connection = Connection::open_in_memory()?;
    let fts5_enabled: i64 = connection.query_row(
        "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
        [],
        |row| row.get(0),
    )?;

    connection.execute_batch(
        "CREATE VIRTUAL TABLE source_turns_fts USING fts5(
            memory_id UNINDEXED,
            project_id UNINDEXED,
            completed_at_ms UNINDEXED,
            prompt,
            assistant,
            tools,
            tokenize = 'unicode61'
        );",
    )?;

    let fixtures = [
        (
            "turn-old",
            "project-a",
            100_i64,
            "marsupial quorum",
            "Keep the decision.",
            "read_file",
        ),
        (
            "turn-new",
            "project-a",
            200_i64,
            "marsupial quorum",
            "Keep the decision.",
            "read_file",
        ),
        (
            "turn-foreign",
            "project-b",
            300_i64,
            "marsupial quorum",
            "Keep the decision.",
            "read_file",
        ),
        (
            "turn-noise",
            "project-a",
            400_i64,
            "unrelated deployment",
            "No matching terms.",
            "shell",
        ),
    ];
    for fixture in fixtures {
        connection.execute(
            "INSERT INTO source_turns_fts(
                memory_id, project_id, completed_at_ms, prompt, assistant, tools
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![fixture.0, fixture.1, fixture.2, fixture.3, fixture.4, fixture.5],
        )?;
    }

    let mut statement = connection.prepare(
        "SELECT memory_id
         FROM source_turns_fts
         WHERE source_turns_fts MATCH ?1 AND project_id = ?2
         ORDER BY bm25(source_turns_fts) ASC,
                  CAST(completed_at_ms AS INTEGER) DESC,
                  memory_id ASC",
    )?;
    let rows = statement
        .query_map(params!["\"marsupial\" \"quorum\"", "project-a"], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    println!("fts5_enabled={fts5_enabled}");
    println!("rows={}", rows.join(","));
    Ok(())
}

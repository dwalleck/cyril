#!/usr/bin/env python3
import sqlite3

connection = sqlite3.connect(":memory:")
connection.execute(
    """CREATE VIRTUAL TABLE source_turns_fts USING fts5(
        memory_id UNINDEXED,
        project_id UNINDEXED,
        completed_at_ms UNINDEXED,
        prompt,
        assistant,
        tools,
        tokenize = 'unicode61'
    )"""
)
fixtures = [
    ("turn-old", "project-a", 100, "marsupial quorum", "Keep the decision.", "read_file"),
    ("turn-new", "project-a", 200, "marsupial quorum", "Keep the decision.", "read_file"),
    ("turn-foreign", "project-b", 300, "marsupial quorum", "Keep the decision.", "read_file"),
    ("turn-noise", "project-a", 400, "unrelated deployment", "No matching terms.", "shell"),
]
connection.executemany(
    """INSERT INTO source_turns_fts(
        memory_id, project_id, completed_at_ms, prompt, assistant, tools
    ) VALUES (?, ?, ?, ?, ?, ?)""",
    fixtures,
)
rows = connection.execute(
    """SELECT memory_id
       FROM source_turns_fts
       WHERE source_turns_fts MATCH ? AND project_id = ?
       ORDER BY bm25(source_turns_fts) ASC,
                CAST(completed_at_ms AS INTEGER) DESC,
                memory_id ASC""",
    ('"marsupial" "quorum"', "project-a"),
).fetchall()
print("rows=" + ",".join(row[0] for row in rows))

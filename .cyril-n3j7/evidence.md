# Evidence: cyril-n3j7

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | The workspace's exact `rusqlite 0.39` configuration (`default-features = false`, `features = ["bundled"]`) provides operational FTS5. | Can a binary built with that exact dependency report `ENABLE_FTS5`, create an FTS5 virtual table, and execute a MATCH query? | PASS |
| P2 | FTS5 can provide the required literal-token, project-filtered, deterministic lexical floor. | Does a parameterized quoted-token MATCH over production-shaped rows return only the requested project and order equal-ranked rows by completion recency then stable ID? | PASS |
| P3 | The bridge exposes normalized lifecycle and authoritative turn ownership before App/UI projection. | N/A — current repository evidence covers this existing-system behavior: `TurnMediator` owner-stamps accepted terminal events and absorbs duplicate/stale companions; `RoutedNotification` preserves session/turn identity; bridge fake-agent tests fence mediation. | N/A — covered by current applicable repository evidence |
| P4 | Result counts, episode character budgets, framing, and completion eligibility. | N/A — these are behavior already decided by the issue Design and acceptance criteria, not claims about an existing system. | N/A — specification behavior |

## Data

- Source: production-shaped generated fixture.
- Shape: four completed source-turn search rows across two canonical project IDs, including two equal-content/equal-rank same-project rows with different completion times, one higher-recency foreign-project row, and one same-project non-match. Indexed fields match the planned prompt/assistant/tool lexical surface; unindexed fields match project, stable turn identity, and completion-time filters/tie-breakers.
- Safety: both probe and oracle use independent in-memory SQLite databases. They read the repository's declared dependency configuration but do not open, write, mutate, or delete repository/runtime databases or operator state.

## Probe

- File: `probe.rs`; supporting exact dependency manifest: `Cargo.toml` and generated `Cargo.lock` in this directory.
- Mechanism: a standalone Rust binary builds against `rusqlite 0.39` with the workspace's exact bundled feature configuration, asks SQLite for `ENABLE_FTS5`, creates/populates the production-shaped FTS table, and runs the parameterized project-filtered quoted-token query.
- Run: `CARGO_TARGET_DIR=/tmp/cyril-n3j7-probe-target cargo run --quiet --manifest-path .cyril-n3j7/Cargo.toml --bin fts5-probe`
- Output:

  ```text
  fts5_enabled=1
  rows=turn-new,turn-old
  ```

## Oracle

- P1 mechanism: inspect the independently generated debug binary's static symbol table. Presence of the linked local `sqlite3Fts5Init` initializer has a different failure mechanism from executing SQL through rusqlite and from the future production query implementation.
- P1 run: `nm --defined-only /tmp/cyril-n3j7-probe-target/debug/fts5-probe`; symbol-table row observed: `00000000001c0298 t sqlite3Fts5Init`.
- P2 mechanism: `oracle.py` uses Python's separately built standard-library SQLite binding and an independently written fixture/query, then compares the returned IDs to the hand-derived expectation: the foreign project and non-match are absent; identical same-project matches tie on BM25 and therefore order `turn-new` before `turn-old` by completion time. This differs from the Rust/rusqlite probe and future Rust storage implementation.
- P2 run: `python .cyril-n3j7/oracle.py`
- P2 output:

  ```text
  rows=turn-new,turn-old
  ```

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | `fts5_enabled=1`; FTS5 table creation and MATCH query succeeded. | Linked binary defines local `sqlite3Fts5Init`. | PASS |
| P2 | `rows=turn-new,turn-old` | Python SQLite and hand expectation both produce `rows=turn-new,turn-old`. | PASS |

## Validated / learned

- P1: Validated prior understanding — the exact workspace bundled-rusqlite configuration compiles, links, and executes FTS5 rather than merely declaring a feature flag.
- P2: Validated prior understanding — parameterized quoted-token MATCH supports strict project filtering, and explicit completion-time/stable-ID tie-breakers make equal-rank results deterministic.

## Related issues

- Consulted through one bounded native-tracker listing/search: `cyril-ct0y` (parent contract; FTS5 lexical floor), `cyril-n3j7` (this capture/episode slice), `cyril-3dqf` and `cyril-s7gn` (later scoped MCP/fact recall consumers), `cyril-39xn` (later knowledge FTS5 consumer), `cyril-nxq5` (later completed-turn consolidation), `cyril-y91y` (later vector/FTS fusion), and closed predecessor `cyril-ezgo` (lesson context and project binding).
- Filed: none — both premises passed; no substrate defect or uncovered future work was found.

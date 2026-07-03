# knowledge-panel-render

A **design sketch** (not built, not wired) for the optional cyril UI that would
surface KAS's `_kiro/knowledge/indexing{Started,Completed}` progress
notifications — the ones that fire when a custom agent's declared
`resources.knowledgeBases[]` are indexed (KAS / `@kiro/agent` 0.8.0,
kiro-cli 2.11.0). Tracked in rivets **cyril-45ld**.

It renders a progress panel styled 1:1 with
[`crew_panel`](../../crates/cyril-ui/src/widgets/crew_panel.rs) — bordered block,
cyan ` knowledge ` title, `{icon} ` + bold-white `{name:<20}` + status, and a
`+N more` overflow — to a real ratatui `TestBackend`, then dumps the buffer. So
the output is what the terminal would actually paint, not a mockup.

## Run

```sh
cargo run      # truecolor render at 80 cols + a narrow 44-col render
cargo test     # 5 assertions tying each row's content to its wire event
```

Standalone crate (its own `[workspace]`, `ratatui = "0.30"` only) so it builds
independently of cyril's pinned toolchain and lints.

## What each row maps to

| Row glyph | State | Wire notification |
|-----------|-------|-------------------|
| `⠹` yellow | downloading model (`best`, first use, ~90MB HF) | `indexingStarted {name, fileCount}`, no `Completed` yet |
| `⠋` cyan | indexing (`fast`/BM25) | `indexingStarted {name, fileCount}` |
| `✓` green | done | `indexingCompleted {status:"success", itemCount}` |
| `✗` red | failed | `indexingCompleted {status:"failed"}` (no `itemCount`) |

## Layout facts the render test surfaced

- The verbose status (`… from Hugging Face`) **clips at ≤66 cols** — trimmed to
  `~90MB` so it fits at cyril's typical widths.
- At very narrow widths ratatui **truncates the status at the inner edge
  gracefully** (no panic, no body overflow) — relevant to `cyril-mdbp`.

## To land it as the real feature

`render_knowledge_panel()` mirrors `crew_panel::render` line-for-line. To
productionize: swap the `&[Row]` argument for a `&dyn TuiState` accessor backed
by a `KnowledgeTracker` (pure state machine, `apply_notification` pattern like
`SubagentTracker`) fed from the two indexing notifications; the 5 assertions here
become the widget's test module.

# Route: cyril-jhmi

Change: `/context` items — read the camelCase `autoIncluded` flag the live wire serves so the `(auto)` tag renders; fix/extend the fixtures against the real wire shape.
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Two premises, both now covered by evidence in the repository plus the reproducible archive scan below. **P1 — the current wire serves camelCase `autoIncluded`:** `experiments/conductor-spike/v2-turn-2.17.0.jsonl` (committed b036cbe1, 2026-08-11) is a raw JSON-RPC line capture (no `{ts,dir,msg}` envelope, so no `ts` exists); its `/context` responses at 0-based frames 5 (`id: 3`), 11 (`id: 5`) and 16 (`id: 8`) carry `"autoIncluded": true` on every `~/.kiro/skills/*/SKILL.md` item (117 occurrences file-wide, 0 snake_case). **P2 — "no capture has ever shown snake_case" (the stated premise for dropping the `auto_included` read):** FALSIFIED. Committed captures with `"auto_included": true` on live `/context` items: `experiments/conductor-spike/test_bridge-2.1.0.out`, `experiments/conductor-spike/logs/conductor-2.5.1.log` (raw compact wire frame, key order `name,tokens,matched,percent,auto_included` — identical to 2.17.0's order with the camelCase key, so it is the wire, not a cyril re-serialization), `experiments/conductor-spike/test_bridge-conductor-2.7.1.out`, `experiments/conductor-spike/test_bridge-2.12.0.out` (2026-07-09). Archive scan (`.cyril-jhmi/pin-rename.py`, output in `.cyril-jhmi/pin-rename-output.txt`): `kiro-cli-chat` 2.12.0, 2.12.1, 2.12.3, 2.13.0, 2.14.0, 2.14.1, 2.14.2, 2.15.0 contain the literal `auto_included` (1×) and never `autoIncluded`; 2.16.0, 2.16.1, 2.16.2, 2.17.0 contain `autoIncluded` (1×) and never `auto_included`. **The key was renamed in kiro-cli 2.16.0.** The 2.16.0 capture (`v2-turn-2.16.0.jsonl`) has neither spelling (no auto-included items in that session) and is uninformative — the issue's "absent in 2.16.0" reading is an absence of items, not of the field. No premise remains unverified. | no |
| 2 | Structural module shape | `append_context_items` (`crates/cyril/src/app.rs:2473`, private free fn, single caller `format_command_response` at `app.rs:2567`) keeps its signature, owner and responsibility (JSON `/context` item → display row); the change is a key-name read inside one existing responsibility. No interface, schema, seam or dependency direction changes. Length gate lookup: `grep app\.rs scripts/ .github/ .githooks/` → none; no line-count gate script in the repo; the protected-parent ledgers in `.cyril-gl5s/` (2,723-line baseline) and `.cyril-v19o/` (6,857-line baseline) were change-scoped fences owned by those changes' own `oracles/`, not standing repository gates — no inherited threshold applies. Current size 7,619 lines (incl. tests); projected delta 0–1 production lines and ≈45 test lines; uncertainty ±5 lines. No length-review trigger. | no |
| 3 | Production-scale risk | Renders one `/context` response (≤ ~50 items) on demand; no latency, throughput, memory, concurrency or data-volume dimension. | no |
| 4 | Explicit behavior | The requested contract is explicit for the current wire — **G/W/T-1:** given a `/context` `commands/execute` response whose `contextFiles.items[]` entry carries `"autoIncluded": true` (kiro-cli ≥ 2.16.0), when `format_command_response("context", …)` renders it, then the item row ends with ` (auto)` (existing row format `    {name} — {tokens} tokens ({pct:.1}%){tags}\n`, unchanged). **G/W/T-2:** given an item without the flag, then no ` (auto)` tag (unchanged). **Unresolved decision D1** — given an item carrying the pre-2.16.0 spelling `"auto_included": true` (kiro-cli ≤ 2.15.0, live in four committed captures), when rendered, then: **(A)** no tag — the approved instruction "drop the `auto_included` read entirely", which was justified by the now-falsified premise P2 and therefore silently regresses the `(auto)` tag for every kiro-cli ≤ 2.15.0 user, a risk acceptance nobody has made; or **(B)** ` (auto)` — read `autoIncluded` first and fall back to `auto_included`, zero regression, one extra line, and the existing snake_case fixture is retained (renamed) as the ≤ 2.15.0 fence. D1 is a risk-acceptance/scope decision outside the approved set. | no |

Unknown tests: none

## Selected route

Structural — T4 is `no`: the approved "drop the snake_case read" decision rests on a falsified factual premise (P2), leaving D1 unresolved; interrogation of the requester is required and Local has none. Expected correction under **Direct stage entry**: once D1 is answered, T4 becomes `yes`, precedence yields **Local**, and this file is rewritten accordingly (no other verdict changes).

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — unresolved decision D1 (T4 verdict); needs the requester's answer, which the orchestrator relays — STOPPED here |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict); P1/P2 evidence is in the repository captures and `.cyril-jhmi/pin-rename-output.txt` |
| design.md | falsifiable-design | required while Structural; expected `N/A — Local route: no design gate` after D1 resolves and the route is corrected to Local |
| plan.md | budgeted-plan | required while Structural; expected `N/A — Local route: no plan gate` after D1 resolves and the route is corrected to Local |

Oracle checkpoint in `checkpointed-build`: required while Structural; expected `N/A — Local route` after the D1 re-route.

## Downstream sequence

interrogated-spec (D1 only) → re-run T4 → expected Local → implement with normal repository fix/TDD.

## Work already done (decision-neutral under both D1 branches)

- Red fence committed: `format_response_context_items_render_auto_tag_from_camel_case_wire` in `crates/cyril/src/app.rs` tests — a capture-derived fixture (frame 5 of `v2-turn-2.17.0.jsonl`, `id: 3`, verbatim subset) asserting the ` (auto)` tag; it fails on HEAD f9bc81d8 because product code reads `auto_included`.
- Existing fixture `format_response_context_breakdown_lists_files` (snake_case, `app.rs:5292`) deliberately left untouched: under D1-(A) it is converted to camelCase; under D1-(B) it is kept as the ≤ 2.15.0 legacy fence. It was capture-faithful when written (2.1.0–2.15.0 wire), not a "self-consistent wrong test".

## Terminal criterion

Structural (current) — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate. On the expected re-route to Local after D1: focused behavioral verification `env -u CARGO_TARGET_DIR cargo nextest run -p cyril format_response_context` (all three context fixtures green, the new fence red-before/green-after), then the full CI-mirror gate; append `Result: <date> | <command> | PASS|FAIL` and the final T2 recheck here.

# Spec: Render installed KAS powers in a `/powers` panel

## Request (verbatim)
> claim and implement cyril-v19o
>
> — cyril-v19o: `_kiro/powers/*`: render installed powers; refresh is declared-but-unimplemented
> Scope: consume items_changed to populate a powers view; do NOT wire a refresh
> affordance to the unimplemented method.

Answers given during interrogation, verbatim:
> surface: "/powers overlay panel"
> live_auth: "Log in now, verify live"
> fields: "Rich: title + meta + description"

## What this is

cyril has never rendered KAS "powers" — installable MCP-server bundles at `~/.kiro/powers/installed/<name>/` that the agent reports unprompted over `_kiro/powers/items_changed` at session creation (`docs/kiro-2.20.1-wire-audit.md` §3; ROADMAP KAS-8 coverage table). Today that frame reaches cyril's extension boundary and dies in the `other =>` catch-all (`crates/cyril-core/src/protocol/convert/kiro.rs:1107-1110`). This change consumes the frame, keeps the reported catalog in state, and renders it in a scrollable `/powers` overlay panel — the KAS panel pattern already used by `/code` and `/usage` (ROADMAP KAS-4, cyril-nk4o). No outbound `_kiro/powers/*` request is added: the push carries the full catalog, and `refresh` is a declared-but-unimplemented method that answers `-32603`.

## Roles

- **KAS user of cyril**: runs cyril with `--agent-engine kas` (or `[agent] engine = "kas"`), has powers installed under `~/.kiro/powers/`, and needs to see which powers the agent loaded for the session — name, what it activates (MCP servers, steering files) and what it does — without leaving the TUI or reading `installed.json` by hand.
- **cyril maintainer**: needs the powers surface to stay inside the existing KAS panel pattern (widget + `TuiState` accessor + one `render.rs` arm) and to add no wire traffic for a method the agent does not implement.

## Behavior

### B1 — `/powers` opens the panel from the pushed catalog
- **Given**: a KAS session in cyril whose agent has pushed `_kiro/powers/items_changed` with `{"sessionId": "sess_…", "status": "success", "powers": [<3 items>]}` (live-confirmed shape, this machine, kiro-cli 2.21.2)
- **When**: the user submits `/powers`
- **Then**: a bordered overlay panel opens over the chat and lists each power as three rendered lines — line 1 the power's `displayName`, line 2 the power's `name` plus each `mcpServerNames` entry prefixed `mcp` plus the token `steering` when `hasSteeringFiles` is `true`, line 3 the `description` truncated to the panel width. The panel title carries the count (e.g. `Powers — 3 installed`).

### B2 — deterministic order
- **Given**: the same pushed catalog
- **When**: the panel renders
- **Then**: powers appear ordered by `displayName` (ASCII-lowercase compare), ties broken by `name`; the order does not depend on the wire array order.

### B3 — scrolling and dismissal
- **Given**: an open powers panel whose rows exceed the panel height
- **When**: the user presses Down/Up or PageDown/PageUp, or Esc
- **Then**: Down/Up move the viewport by one row and PageDown/PageUp by one page, each clamped at both ends (no wrap, no over-scroll past the last row); Esc closes the panel and restores the chat view. The panel never covers the input line.

### B4 — a later push replaces the catalog
- **Given**: a powers catalog already held (panel open or closed)
- **When**: a further `_kiro/powers/items_changed` arrives carrying a different set (including `powers: []`)
- **Then**: the held catalog becomes exactly the newly pushed set — no merge, no duplicate rows, removed powers disappear — and an open panel redraws with the new set, its scroll offset clamped so a shorter list cannot leave the viewport past the last row.

### B5 — no catalog yet
- **Given**: a KAS session in which no `_kiro/powers/items_changed` frame has arrived yet
- **When**: the user submits `/powers`
- **Then**: no panel opens and exactly one system line is appended to the transcript stating that the agent has not reported installed powers yet.

### B6 — empty installed set
- **Given**: the agent pushed `_kiro/powers/items_changed` with `"powers": []` (the live shape on a HOME with no powers registry)
- **When**: the user submits `/powers`
- **Then**: the panel opens (the catalog IS known — it is empty) and renders the placeholder text `No powers installed` in place of rows.

### B7 — malformed push is rejected, never destructive
- **Given**: a catalog already held
- **When**: an `_kiro/powers/items_changed` frame arrives whose `powers` key is absent or is not a JSON array
- **Then**: the frame is dropped with a `warn!`-level log naming the method, the previously held catalog is unchanged, and no panel state changes.

### B8 — no outbound powers traffic
- **Given**: any cyril session, either engine
- **When**: the user opens and closes the powers panel and types anything else
- **Then**: cyril sends no `_kiro/powers/refresh` request ever, and no `_kiro/powers/list` request.

### B9 — v2 engine is unaffected
- **Given**: cyril running the default v2 engine (`kiro-cli acp`), which advertises 25 commands and emits no `_kiro/powers/*` notification (measured, `.cyril-v19o/v2-commands-2.21.2-verdict.json`)
- **When**: the user submits `/powers`
- **Then**: behavior is exactly B5 — one system line, no panel — and no v2 parsing path, notification conversion, or rendering changes.

## Success criteria

- **SC1 (live, end-to-end)**: on this machine (kiro-cli 2.21.2, 3 powers installed), a cyril KAS session followed by `/powers` renders a frame containing all three `displayName` values `Build AWS infrastructure with CDK and CloudFormation`, `Datadog Observability`, `Markdownlint`; checked by running cyril with `--agent-engine kas` and reading the rendered panel from the terminal.
- **SC2 (live, field fidelity)**: in that same rendered frame, the `datadog` entry shows its id `datadog`, its MCP server name `datadog`, and the `steering` token; `aws-infrastructure-as-code` shows `awslabs.aws-iac-mcp-server` and no `steering` token; checked in the same frame by reading the panel text.
- **SC3 (empty set)**: a widget render test drives `powers_panel::render` with a zero-power catalog and asserts the rendered buffer contains `No powers installed`; checked by `cargo test -p cyril-ui powers_panel`.
- **SC4 (unloaded vs empty are distinct)**: an app-level test submits `/powers` with no catalog held and asserts zero panels open plus exactly one added system message; a second case with an empty-but-known catalog asserts the panel opens; checked by `cargo test -p cyril`.
- **SC5 (no outbound powers traffic)**: a bridge/domain test drives a `/powers` interaction and asserts the captured outbound frame list contains no method whose name contains `powers/`; checked by that test, plus a source fence over `crates/` asserting the string `_kiro/powers/refresh` and `_kiro/powers/list` appear in no production source file.
- **SC6 (converter fidelity against the wire)**: the converter test replays the recorded 2.21.2 frame from `.cyril-v19o/kas-powers-2.21.2-2.21.2.jsonl` and asserts the three powers convert with their `displayName`, `name`, `mcpServerNames`, `hasSteeringFiles` values equal to those independently extracted from the same capture by the probe's verdict JSON; checked by `cargo test -p cyril-core` and by re-running the probe's extraction.
- **SC7 (repo invariants)**: `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` are clean; checked by running each at the change's HEAD.

## Out of scope

This change does NOT include:

- Any refresh affordance. `_kiro/powers/refresh` is declared but unimplemented (`-32603`, re-measured on 2.21.2) and MUST NOT be called or surfaced as a keybinding, panel footer, or command flag. A refresh rendered from this method could only ever fail.
- Calling `_kiro/powers/list`, even though it works and is unadvertised: the requester re-confirmed push-only after this row was challenged, and the unprompted push carries the full catalog.
- Installing, uninstalling, enabling, disabling, or configuring powers; editing a power's files; the `@powers` mention flow — **no such verb exists on this wire surface**. The KAS ext-method classification enumeration (`docs/kiro-2.14.1-wire-audit.md:322-325`) and the covenant's `_kiro/*` catalog list exactly `powers/{list,refresh,items_changed}`; install/enable is kiro-cli's own TUI/CLI surface, unreachable by an ACP client.
- Surviving `list`'s `errors[]` surface: it appears only on `list` responses, and the push's params are exactly `{powers, sessionId, status}` (measured on 2.21.2). With the pull path excluded above, no `errors[]` can reach cyril.
- The "open Agent Plugin format" investigation and any consume/supply role for cyril beyond displaying what the agent reports (that is cyril-q159, a separate open research ticket; folding it in makes this change unbounded).
- v2-engine powers support — there is no v2 wire surface: the v2 probe measured 25 advertised commands with no `powers` among them and zero `_kiro/powers/*` frames, and the 2.16.2 release note scopes powers to V3 sessions.
- Rendering powers in the toolbar, status line, crew panel, or autocomplete.

## Related issues

- **cyril-v19o**: this feature.
- **cyril-q159** (open, P2): investigates what the open Agent Plugin format IS and whether cyril should consume/supply plugins. Bearing: that issue owns the strategic question; this spec deliberately stops at display and adds no format handling.
- **cyril-nk4o** (open, P3, `kas-8`): scopes the `_kiro/mcp/*` MCP panel — "server list + state … the KAS analog of v2 /mcp". Bearing: the adopted sibling-panel pattern and the precedent that KAS catalogs surface as panels; not a dependency (different wire family).
- **cyril-58uv** (in_progress, P1): fixes the silent drop of unhandled `_kiro/*` notifications. Bearing: this change adds one consumer at the same boundary it is instrumenting; the design keeps its converter in a new KAS-side module so the two edits do not collide on `convert/kiro.rs`'s catch-all.
- **cyril-oiyt** (open, P4, `kas-7`): extends the hooks panel with the KAS host-mode registry. Bearing: same "extend a panel with a KAS-sourced catalog" family; its panel-open discipline (never auto-open from a push) is adopted here.
- **cyril-7q8u** (open, P2): live capture of pushed KAS methods, which recorded `_kiro/powers/items_changed {sessionId, status, powers:[]}`. Bearing: independent prior capture agreeing with the 2.21.2 probe.
- **docs/kiro-2.20.1-wire-audit.md §3**: the wire contract this spec consumes.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| Which surface renders the powers catalog? | `/powers` overlay panel, hooks/usage style | Requester: "/powers overlay panel"; repo pattern KAS-4 ("/code, /usage panels are the sibling pattern", ROADMAP) and cyril-nk4o scope the same shape for KAS catalogs | The change adds a widget module, a `TuiState` accessor, and one `render.rs` arm |
| How much does each power show? | Rich: `displayName` title, meta line with `name` + `mcpServerNames` + `steering` token, truncated `description` | Requester: "Rich: title + meta + description" | All seven wire fields except `keywords` and `_meta` are either rendered or drive a marker |
| Are `keywords[]` rendered? | N/A — not rendered | They exist for `@powers` mention matching, which is out of scope; the panel's job is identification, and the meta line is already width-bound | `keywords` is parsed but never displayed; the decision keeps the panel from wrapping unpredictably |
| Which engine gets the `/powers` builtin? | Registered unconditionally for both engines | Measured: v2 advertises 25 commands and `powers` is not one of them (`.cyril-v19o/v2-commands-2.21.2-verdict.json`); KAS advertises no TUI commands at all, so the name cannot shadow an agent command on either engine. `CommandRegistry::register_agent_commands` already skips a name cyril holds (`commands/mod.rs:394`) | No new command-source seam (`HooksCommandSource` is needed only because v2 DOES advertise `/hooks`); v2 users get B5's message |
| Where does the catalog come from? | Solely the unprompted `items_changed` push | Ticket scope ("consume items_changed"); the push carries the full catalog and fires at session creation (live-confirmed). Challenged by the requester and re-confirmed: "Push only (as specced)" over a one-shot `_kiro/powers/list` pull or an in-panel `r` refresh key | No `list` call, no pull path, no staleness affordance, no `errors[]` surface |
| Are install / enable / disable / `@powers` actions part of this? | N/A — not expressible on this wire surface | The KAS ext-method classification enumeration lists exactly `powers/{list,refresh,items_changed}` (`docs/kiro-2.14.1-wire-audit.md:322-325`); no client-callable install/enable verb exists | Display only; no mutation, no file I/O on `~/.kiro/powers/` |
| Is a refresh action provided? | No — explicitly out of scope | Ticket: "do NOT wire a refresh affordance"; `refresh` answers `-32603 Unknown ext method` (re-measured on 2.21.2) | No keybinding, no footer affordance, no command flag; SC5 fences it |
| What ordering does the panel use? | `displayName` case-insensitively, tie-break `name` | Matches `show_hooks_panel`'s "UiState owns the sort" discipline (`state.rs:2331-2341`); wire order is not semantically meaningful and would make render tests order-fragile | Sorting lives in the state setter, not the widget |
| Empty set (zero powers) | Panel opens with `No powers installed` | Precedent: hooks panel renders an empty placeholder (`empty_hooks_renders_placeholder`, `hooks_panel.rs:211`). A known-empty catalog is not the same as an unknown one | Requires the "catalog known" flag distinct from "catalog empty" — B5 vs B6 |
| No push received yet | `/powers` adds one system line, opens no panel | AGENTS.md "Errors are not default values" and "distinguish missing from corrupt": an empty panel would claim "no powers installed" when cyril simply has no data | The state must represent three cases: not loaded, loaded-empty, loaded-non-empty |
| Update semantics on a later push | Full replacement, never a merge | Matches `HooksChanged`'s documented contract ("carries the full new registry, so replacement not delta", `event.rs:276-289`) | Idempotent for a repeated identical push; removed powers disappear |
| Scope of the catalog: session or process? | Process-global; latest push wins | Powers are a user-level install at `~/.kiro/powers/`, not session state, and the push carries the same set for every session of one install (live: 3 items on 2.21.2) | No per-session keying; a `/new` does not clear the panel data |
| Does a push open the panel by itself? | No — a push only updates state | Precedent, same family: the hooks panel contract ("it must never pop an overlay open by itself", `app.rs:1404-1406`) | Opening stays a user action via `/powers` |
| Multi-tenancy boundaries | N/A — powers are per-user-directory (`~/.kiro/powers`); no tenant or account dimension exists on the wire | Live payload carries no tenant/account field (2.21.2 probe, item keys enumerated in `evidence.md`) | No scoping logic |
| Concurrent writes | N/A — all extension notifications reach the single serial domain owner (`domain_mediator`), and the App applies them on one event loop | `inbound.rs:127-190` runs on the serial domain owner; no cross-thread mutation exists | No locking; last write in arrival order wins |
| Permission denied / unauthenticated | N/A — the frame is agent→client and needs no client capability or permission; cyril advertises nothing for it | `client.rs:143-145` enqueues every `_`-prefixed method unconditionally | No capability gate, no error branch |
| Partial failure (one of N powers) | N/A — the payload is one atomic list; a partially corrupt item is not distinguishable on the wire | Observed item shape is uniform across the 3 live powers (identical key sets) | Item-level validation is all-or-nothing per frame (B7) |
| Retries / idempotency | N/A — no request is sent and no retry exists; repeated identical pushes converge by replacement | B8: cyril issues no powers request | Nothing to retry; a duplicated push is a no-op in observable terms |
| Soft-deleted records | N/A — no deletion concept on this wire surface; removal is expressed as absence from the replacement list | Full-replacement semantics (B4) | Removed powers simply stop rendering |
| Null / missing field | `powers` absent or non-array → warn + drop the frame, keep the previous catalog; a missing optional item field renders as absent, never as a placeholder string | AGENTS.md "Guard partial updates" and "Errors are not default values" | The converter returns an error/drop outcome rather than manufacturing an empty catalog |
| Time-zone / DST | N/A — the payload carries no timestamp | Item keys enumerated in `evidence.md`; only `sessionId`/`status`/`powers` are present | No time handling |
| Replication lag | N/A — the push is a direct stdio frame from the local agent process; no replication exists | Transport is JSON-RPC over the spawn's stdin/stdout (`client.rs:99-146`) | None |
| Cache invalidation | The push IS the invalidation: a later frame replaces the catalog wholesale | B4; the wire offers no version or etag | No TTL, no staleness check |
| Max scale (many powers) | Rendering is bounded: rows are laid out from the list and the viewport is clamped to the panel height; the panel scrolls rather than growing | Existing modal geometry (`widgets/modal.rs::place`) and hooks-panel clamping precedent | No truncation of the catalog itself; only the viewport is bounded |

## Approval

Requester approval (verbatim): "Approve spec.md"
Date: 2026-09-10

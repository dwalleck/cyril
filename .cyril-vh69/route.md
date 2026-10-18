# route.md — cyril-vh69: KAS /model picker with per-model effort, plus /mode

- Change: rivets `cyril-vh69` (P1 feature, labels: kas, kas-4, kiro-2-31-1, ready-for-agent)
- Date: 2026-10-15
- Branch/worktree: `feature/cyril-vh69-kas-model-picker` at `../cyril-wt-feature-cyril-vh69-kas-model-picker`
- Request source: rivets ticket `cyril-vh69` (filed 2026-10-15 from the kiro-cli 2.31.1 upgrade audit, cyril-4c4d parity list). Ticket description and acceptance criteria are the authoritative behavior source.

## Route tests

### T1 Empirical premise — NO

Every external fact the design depends on has LIVE capture evidence already in the repo, cited by the ticket:

- configId `"model"` pushed via `config_option_update` after session/new (frame 24 of `experiments/conductor-spike/audit-2.31.1/live/kas-live-sweep-2.31.1.jsonl`; session/new itself may omit it — timing); 21 options with `_meta.kiro.{hasEffort, effortLevels[], defaultEffortLevel, effortSchemaPath, rateMultiplier, rateUnit, thinkingToggleable, defaultThinkingEnabled}`.
- Effort merged into the model choice; `set_config_option {configId:"effortLevel", value}` is the set path (TUI 2.31.1 sessionWrites).
- configId `"mode"` (vibe/spec/quick-spec/bug-fix/plan/autonomous/custom agents); `set_mode` also works; both report only via `config_option_update`; unknown mode silently coerced to vibe.
- `set_config_option model` accepts an unoffered id unvalidated (fails at next prompt) — KAS twin of cyril-fj6j.
- `session/set_model` is -32603 on KAS.
- effortLevel zero-turn behavior (0.66.34): no effortLevel option while model=auto; appears after switching to a Claude model with that model's levels (default medium); invalid values return SUCCESS but leave `currentValue` unchanged — client must read back `currentValue`.

Verified: `docs/kiro-2.31.1-wire-audit.md` and `experiments/conductor-spike/audit-2.31.1/live/kas-live-sweep-2.31.1.jsonl` exist in-repo. No unresolved external fact could change the design. (Design stage re-confirms any fact it leans on against these captures.)

### T2 Structural module shape — YES

New user-facing command family on the KAS engine, touching multiple owners:

- **cyril-core commands** (`crates/cyril-core/src/commands/`): `/model`, `/effort`, `/mode` must dispatch on KAS via config options instead of the v2 `kiro.dev/commands/*` path. Engine-conditional command behavior is new responsibility for the registry/executor.
- **cyril-ui picker overlay**: a two-step flow (model select → effort select when `hasEffort`) is a new picker responsibility beyond single-shot option lists.
- **cyril-ui state** (`crates/cyril-ui/src/state.rs`): `ConfigOptionsUpdated` handling exists (~line 1023) but must additionally support validation-against-offered-list, effort re-derive on model switch, and read-back of `currentValue` after each set.
- **Toolbar**: must reflect `config_option_update` for model/effort/mode on KAS.

No dependency-direction change, no new crate, no module split/merge. This is a new responsibility cluster added to existing owners (orchestrator/facade extension) → Structural.

### T3 Production-scale risk — NO

Command dispatch over existing bounded channels; picker over ≤ ~21 options. No latency, throughput, memory, concurrency, or volume risk introduced.

### T4 Explicit behavior — YES

The ticket's acceptance criteria are the complete outcome contract (verbatim from `rivets show cyril-vh69`):

- On a KAS session: `/model` opens a picker over configId `"model"` options (from the latest `config_option_update`), validates the choice against the offered list, and sends `set_config_option model`.
- When the chosen model `hasEffort`, an effort step offers only its `effortLevels` (default marked) and sends `set_config_option effortLevel`.
- `/effort` alone offers the current model's levels.
- `/mode` offers configId `"mode"` options.
- Never sends `session/set_model` on KAS.
- After every set, the shown value is the returned `currentValue` (KAS silently ignores invalid effort levels).
- Effort is re-derived after every model switch (cyril-838u constraint).
- Toolbar reflects `config_option_update`.
- Tests cover option parsing incl. `_meta.kiro` effort metadata, validation, and the effort re-derive.

Minor decisions left to design (not requester-level): exact engine-gating mechanism, behavior of `/effort` when the current model offers no effortLevel option, picker UX for the two-step flow, v2-engine behavior preservation (must stay on the `kiro.dev` path).

## Selected route: **Structural**

T2 yes with explicit T4 behavior and no unresolved empirical premise. Per change-workflow: Structural with explicit behavior → `falsifiable-design` next.

## Scope vs. related open issues (tracker taxonomy)

The ticket says it "builds on" four issues; all four are OPEN, and none is registered as a blocking dependency (cyril-vh69 is on the `ready` list). Disposition:

- **cyril-838u** (never carry effortLevel across a model switch) — **absorbed** by this change's acceptance criteria ("Effort is re-derived after every model switch"). Close cyril-838u as covered when this ships.
- **cyril-cxwb** (lift config_options from session/new response) — **not in scope**; the contract sources options from `config_option_update`, which the ticket notes is the reliable channel (session/new may omit the model option). Stays open.
- **cyril-lxuo** (per-model effort/credit badges in picker) — **not in scope**; display polish beyond the acceptance criteria (default-effort marking IS in scope; credit-rate badges are not). Stays open.
- **cyril-v2ol** (sticky-default side effect surfacing) — **not in scope**. Stays open.

## Required artifacts and owning stages

| Artifact | Owning stage |
|---|---|
| `route.md` (this file) | change-workflow |
| `design.md` | falsifiable-design |
| `plan.md` | budgeted-plan |

No `spec.md` (T4 explicit — ticket acceptance criteria are the outcome contract). No `evidence.md`/probe (T1 resolved from existing captures).

## Downstream sequence and terminal criterion

1. falsifiable-design → `design.md` (user approval required)
2. budgeted-plan → `plan.md`
3. checkpointed-build per slice

**Terminal criterion (Structural):** required artifacts satisfy their owning criteria and checkpointed-build's final assembled behavior, consumer/platform/repository, and independent-review proof is valid, with no blocking defect or missing required check.

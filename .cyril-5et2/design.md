# KAS-2b (cyril-5et2) — prove-it-prototype output

Feature: render KAS's live per-category **context_usage** breakdown bar
(Files / Tools / Prompts / Responses) in the toolbar, from the proactively
pushed `session/update → session_info_update` (`_meta.kiro.kind == "context_usage"`).

## Step 0 — tracker prior art (bounded)

- `cyril-5et2` — the KAS-2b task itself.
- `cyril-nhzw` (related, not blocking) — KAS settings handshake; would turn *more*
  feature flags on but doesn't gate context_usage rendering.
- No prior bug filed about context_usage parsing. Memory note
  `reference_kiro_context_usage_breakdown.md` corroborates "tools bucket
  aggregate-only on v2 + KAS."

## Probe

`.cyril-5et2/probe.rs` — run as a throwaway `#[ignore]` test inside
`cyril-core` (locks the *exact* acp 0.10 version cyril uses), then removed.
It exercises cyril's real path with **no** KAS-2b abstraction:
`serde_json::from_str::<acp::SessionNotification>` → match `SessionInfoUpdate(siu)`
→ navigate `siu.meta["kiro"]["breakdown"][bucket]` for `{tokens, percent, items?}`.

Input: `.cyril-5et2/context_usage_raw.json` — a **genuine** frame captured live
from `kiro-cli-chat 2.10.0` (`acp --agent-engine v3`) via
`.cyril-5et2/probe_live_capture.py` (the user's binary in use; AWS IdC auth).

## Oracle

`.cyril-5et2/oracle.sh` — navigates the **same raw bytes** with `jq` (a parser
entirely independent of Rust/serde), extracting the same 6 values. Probe (serde)
and oracle (jq) **agree on every value**, including the load-bearing
items-absent-vs-empty split:

```
                       PROBE (serde)        ORACLE (jq)
usagePercentage        4.3                  4.3
contextFiles           0 / 0   / items[0]   0 / 0   / items[0]
tools                  4662 / 2.3 / ABSENT  4662 / 2.3 / ABSENT
yourPrompts            4096 / 2   / ABSENT  4096 / 2   / ABSENT
kiroResponses          0 / 0   / ABSENT     0 / 0   / ABSENT
sessionFiles           0 / 0   / items[0]   0 / 0   / items[0]
```

Independent agreement on a non-trivial slice (6 values × 3 dims) ⇒ cyril's serde
path extracts the breakdown correctly from real wire data.

## What I learned (not obvious before probing)

1. **The covenant/issue shape is incomplete.** Documented as
   `_meta.kiro = {usagePercentage, breakdown?}`, but the real 2.10.0 wire also
   carries an **undocumented `contextUsage: {usagePercentage}` wrapper** — so
   `usagePercentage` appears twice (nested + flat, both `4.3`, agreeing). Design
   reads the flat `kiro.usagePercentage`; flag that the `contextUsage` wrapper
   may accrete fields later.
2. **The aggregate-only set is exactly 3 of 5 buckets.** Only `contextFiles` and
   `sessionFiles` carry `items` (`[]` here); `tools`, `yourPrompts`, and
   `kiroResponses` have **no** `items` key. Confirms + sharpens constraint #1 —
   the UI must treat tools/prompts/responses as aggregate-only, never drill-in.
3. **Shape is stable 2.8.0 → 2.10.0** (byte-identical structure across the
   `@kiro/agent` 0.3.234→0.3.299 bundle change; only a token count differs by
   turn-noise) — we're not designing against a moving target.

## Honest gaps (carry into falsifiable-design)

- **Constraint #2 not re-confirmed on 2.10.0.** This turn pushed only **1**
  context_usage frame (the 2.8.0 run saw 6, with breakdown-absent frames). The
  "breakdown optional per frame; retain-last, absence ≠ cleared" rule is from the
  2.8.0 log, not re-captured here. `context_usage_absent_raw.json` was not
  produced. → falsifiable-design must keep the retain-last design falsifiable,
  or re-capture a multi-frame turn.

## Hard gate
- [x] Probe runs against the real codebase (cyril-core serde + live 2.10.0 frame)
- [x] Oracle defined, produces output (jq)
- [x] Probe and oracle agree on a non-trivial slice
- [x] Learned something non-obvious (the `contextUsage` wrapper + dual usagePercentage)

---

# Falsifiable design

## Decisions (open forks resolved, for user approval)

- **Type modeling → new type, not extend `ContextUsage`.** `ContextUsage` is a
  shared, scalar-only newtype (`Option<f64>` flows from v2 `MetadataUpdated` +
  `UsageUpdated` into the toolbar `Context: N%`). The breakdown is KAS-only, so:
  - `ContextBucket { tokens: u64, percent: f64 }` (no `items` — encodes the
    "aggregate-only" probe finding; per-file items drill-in is a separate feature,
    **cyril-1116**).
  - `ContextBreakdown { context_files, session_files, tools, your_prompts,
    kiro_responses: ContextBucket }`.
  - New `Notification::ContextBreakdownUpdated { usage_percentage: f64,
    breakdown: Option<ContextBreakdown> }`. The existing scalar `context_usage`
    path is **untouched**; under KAS the converter feeds the scalar from this
    notification's `usage_percentage` (KAS sends no `kiro.dev/metadata`).
- **UI state → retain-last.** `UiState.context_breakdown: Option<ContextBreakdown>`;
  a breakdown-absent frame updates the scalar but does NOT clear the stored
  breakdown (mirrors the `effort`-field discipline). New
  `TuiState::context_breakdown() -> Option<&ContextBreakdown>`.
- **Toolbar UX (user sign-off: 5 distinct labels).** A categorized bar with one
  label per wire bucket — **Context Files** (contextFiles), **Session Files**
  (sessionFiles), **Tools**, **Prompts** (yourPrompts), **Responses**
  (kiroResponses) — each showing its percent. No merging. Aggregate-only, no
  drill-in.

## Input shapes (converter input = a `session_info_update` siu)

1. `kind==context_usage`, breakdown present, 5 buckets, mixed items → C1,C2,C7 (the captured 2.10.0 frame).
2. `kind==context_usage`, breakdown **absent**, usagePercentage only → C3,C4 (2.8.0-observed; constraint #2).
3. `kind==context_usage`, breakdown present but a bucket missing tokens/percent → **out-of-scope-defensive**: treated as breakdown-absent (None), not an error (production-unreachable per probe; every bucket had tokens+percent). Covered by C3's "absent OR unparseable → None".
4. `kind==context_usage`, flat `usagePercentage` present, nested `contextUsage.usagePercentage` present/agreeing/disagreeing → C2 (read flat).
5. `kind==turn_end` → must still map to TurnCompleted (KAS-2a) → C6.
6. `kind==turn_completion` / other 16 sub-kinds → must stay None → C6.
7. `_meta`/`kiro`/`kind` absent → None (KAS-2a behavior, unchanged).

## Subtractive sweep

**Additive.** Adds a converter arm (context_usage was dropped to `None`), a
Notification variant, a UiState field, and a toolbar widget. Removes no lock,
guard, ordering, or uniqueness property. The `turn_end` and v2-scalar paths are
untouched — C6 fences turn_end against accidental capture by the new arm.

## Falsification

| # | Claim | Falsifier (input → falsifying result) | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|----------------------------------------|----------------------|------|--------|------------------|
| 1 | A breakdown-present context_usage frame → `ContextBreakdownUpdated{breakdown:Some}` with all 5 buckets' exact tokens/percent. | Feed `.cyril-5et2/context_usage_raw.json`; if any bucket tokens/percent ≠ the frame, false. Buggy impl: transpose tokens/percent, or read `contextUsage.usagePercentage` per bucket. | `jq` on the fixture (prove-it oracle) | 5m | pending | unit `kas::context_usage_maps_breakdown` (fixture) |
| 2 | `usage_percentage` reads flat `_meta.kiro.usagePercentage`, not the nested wrapper. | Frame with flat=9.9, nested `contextUsage.usagePercentage`=1.1; if result≠9.9, false. Buggy impl: reads the nested wrapper → 1.1. | hand-set divergent values | 5m | pending | unit `kas::usage_reads_flat_not_nested` |
| 3 | A breakdown-**absent** context_usage frame → `Some(ContextBreakdownUpdated{breakdown:None})` carrying the scalar (not dropped, not Err). | Feed a breakdown-absent frame; if None/Err or scalar missing, false. Buggy impl: `breakdown.unwrap()` or returns `None`. | hand-constructed frame | 5m | pending | unit `kas::context_usage_breakdown_absent` |
| 4 | UiState retains the last breakdown: present-then-absent leaves `context_breakdown` Some, scalar updated (absence≠clear). | Apply present then absent; if `context_breakdown` becomes None, false. Buggy impl: `self.context_breakdown = note.breakdown` (overwrites with None). | UiState field read | 10m | pending | unit `state::breakdown_retains_last` |
| 5 | Under KAS the toolbar scalar `Context: N%` updates from context_usage frames (no kiro.dev/metadata under KAS). | Apply one context_usage notification; if `context_usage()`≠pct, false. Buggy impl: emits breakdown but never sets the scalar. | `TuiState::context_usage()` read | 5m | pending | unit `state::kas_context_usage_sets_scalar` |
| 6 | KAS-2a unperturbed: turn_end still → TurnCompleted; turn_completion still → None. | Run existing KAS-2a tests + turn_completion→None; if turn_end now → None/breakdown, false. Buggy impl: new arm matches all SessionInfoUpdate before the turn_end check. | existing `convert::kas::tests` | 5m | pending | existing `kas::*` tests |
| 7 | Toolbar renders 5 labeled categories (Context Files/Session Files/Tools/Prompts/Responses) with percents, aggregate-only. | Render via TestBackend with a known breakdown; if a label/percent missing or an item line present, false. Buggy impl: renders only scalar, or omits a category. | TestBackend buffer scan | 15m | pending | render test `toolbar::renders_breakdown_bar` |

**Cheapest falsifier (cost ~2m) — RUN, PASSED** (`.cyril-5et2/` jq on the real
2.10.0 frame): `all5_present:true, items_only_on_files:true,
flat_eq_nested_usage:true, every_bucket_has_tokens_percent:true`. Sample n=1
frame — the retain-last / breakdown-absent claims (C3,C4) are fenced by
constructed frames, not this capture (honest gap from prove-it).

## Negative space (what KAS-2b deliberately does NOT do)

1. **No per-file/per-tool items[] drill-in** — aggregate bar only; the file
   drill-in is **cyril-1116**.
2. **No change to the v2 context path** — `MetadataUpdated`/`UsageUpdated` scalar
   stays scalar; KAS-2b adds a parallel KAS-only breakdown.
3. **No history/persistence** — only the latest breakdown is held (retain-last).
4. **Does not touch the `/context` command surface** — KAS rejects it (-32603);
   reads the streaming notification only.

## Hard-gate checklist
- [x] Every production-reachable input shape has a claim (shapes 1–7; shape 3 noted out-of-scope-defensive, folded into C3)
- [x] Change classified (additive — sweep done, C6 fences the one adjacent path)
- [x] Every claim has a falsifier + independent oracle + a named buggy impl (non-vacuity)
- [x] Distinct per-claim outputs (each row a separate test)
- [x] Measurement-based claim (cheapest jq) has a CI regression fence (C1's fixture unit test)
- [x] Deferral cites a verified tracker ID (cyril-1116)
- [x] Cheapest falsifier run + passed
- [x] Negative space ≥ 3 (4 listed)

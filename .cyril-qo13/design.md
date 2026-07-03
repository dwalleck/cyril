# cyril-qo13 — Falsifiable design: exact-choice permission responses

**Status:** cheapest falsifier passed (C2); awaiting approval for `budgeted-plan`.
**Upstream:** probe + oracle agreement recorded in `probe-findings.md` (prove-it-prototype).

## Purpose

Cyril's `PermissionResponse` is kind-keyed (AllowOnce/AllowAlways/Reject/RejectAlways/
Cancel); the converter re-derives the wire optionId as the *first* option matching the
kind. KAS `user_input` questions carry N options all of kind `allow_once`, so every
user selection silently answers option 0. Fix: carry the **selected option's id**
end-to-end, so the wire reply names the exact choice.

This design deliberately narrows cyril-qo13's widened scope to the optionId fix.
The KAS consent-`_meta` half is tracked at **cyril-gn07** (probe proved the echo
optional for `allow_once`; only the `allow_always` scope question remains). The v2
trustOption echo-shape question is tracked at **cyril-sive**.

## Constraints inherited from the probe (may not be contradicted)

- Correct reply for picking option k is `{"outcome":{"outcome":"selected","optionId":
  options[k].optionId}}` — bare, no `_meta` (26/26 oracle agreement).
- `{"outcome":"cancelled"}` is legal for user_input; KAS re-asks (trace req 1→2).
- Distinct-kind approvals already answer correctly today (16/16) — must not change.
- acp 0.10.2 parses all observed requests losslessly, and hard-fails unknown option
  kinds upstream of cyril (`probe_qo13_unknown_option_kind_parse`).

## Proposed change

**`cyril-core` types (`types/event.rs`):**

```rust
pub struct PermissionOptionId(String);          // newtype, house rule

pub enum PermissionResponse {
    Selected {
        option_id: PermissionOptionId,          // the exact picked option
        trust_option: Option<String>,           // v2 phase-2 label, as today
    },
    Cancel,
}
```

`PermissionOption.id` becomes `PermissionOptionId`. `From<PermissionOptionKind> for
PermissionResponse` is deleted (compile-time flush of all constructors).

**`cyril-ui` (`state.rs`, `traits.rs`):**

- `ApprovalPhase::SelectTrust { chosen_option_id: PermissionOptionId }` — the phase-1
  pick is carried in the phase itself (illegal state "trust phase without a chosen
  option" becomes unrepresentable). `ApprovalPhase` loses `Copy`; readers match by ref.
- `approval_confirm`, phase SelectOption: send `Selected { option_id:
  options[selected].id, trust_option: None }` (AllowAlways-with-trust-options still
  transitions to phase 2, now carrying the id).
- `approval_confirm`, phase SelectTrust: send `Selected { option_id: <carried>,
  trust_option: Some(label) }`; still returns the chosen `TrustOption` for persistence.
- Out-of-bounds selection still resolves to `Cancel`.

**`cyril-core` converter (`protocol/convert/mod.rs`):**

- `from_permission_response`: `Selected` → `SelectedPermissionOutcome::new(<id>)`
  (+ `_meta.trustOption = label` when `trust_option` is `Some`, byte-identical to
  today's v2 echo); `Cancel` → `Cancelled`. **`find_option_id` is deleted** along
  with its first-match and fabricated-id fallbacks.
- No engine-conditional code lands in this change (consent passthrough: cyril-gn07).

## Input shapes (step 2)

| # | Shape | Source | Covered by |
|---|-------|--------|-----------|
| S1 | Multi-option, all same kind (2 and 3 options observed) | KAS user_input, trace ids 1–5, 10 | C1, C2, C8 |
| S2 | 4 options, distinct kinds | KAS tool_approval, trace ids 6–9 | C1, C3 |
| S3 | 3 options, distinct kinds (no reject_always) + trustOptions | v2 trace (1 request) | C1, C3, C4 |
| S4 | Single option | schema-legal, not in traces | C1 (synthetic fence fixture) |
| S5 | Empty options | not observed; confirm impossible (nothing selectable) | C6 (empty + oob fixture) |
| S6 | Option with unknown kind | **unreachable** — acp parse rejects the request | C7 (sentinel) |
| S7 | Duplicate option ids in one request | out of scope: ids observed unique per request (KAS `-option-N`, v2 fixed ids); no production evidence | — |
| S8 | Selection k = 0 / middle / last | UI | C1 |
| S9 | Selection out of bounds (state corruption) | UI | C6 |
| S10 | Esc / cancel (either phase; phase-2 Esc = back, as today) | UI | C5 |
| S11 | Responder dropped (App teardown) | out of scope: pre-existing `-32603` path in `client.rs:75`, untouched by this change | — |
| S12 | allow_always pick + non-empty trustOptions (phase 2) | v2 | C4 |
| S13 | KAS consent `_meta` on tool_approval request | replies carry no echo (probe-validated for allow_once); allow_always tracked at **cyril-gn07** | — |

## Removed-invariant sweep (step 2b)

The core move is subtractive: it deletes the kind→wire derivation (`find_option_id`)
and the re-derivability of a response from its kind.

| Removed constraint | What it silently guaranteed | Still holds? |
|---|---|---|
| Kind→first-option lookup | Reply optionId always ∈ request options | Yes, constructively — the id now comes *from* `options[k]` (C1) |
| Response re-derivable from kind at any time | Phase-2 trust confirm could recover its option later | Replaced by the carried `chosen_option_id` (C4) |
| `find_option_id` fallbacks (first option; fabricated `allow_once` id on empty) | A kind-based response always produced *some* Selected outcome | Deleted deliberately; the mismatch states become unrepresentable via UI (only ids that exist can be picked), and empty/oob resolve to Cancel (C6). No production consumer relied on the fallback — it only emitted `warn!` logs |
| `From<PermissionOptionKind> for PermissionResponse` | Kind-only construction | Compile-time flush; sole non-test caller is `approval_confirm` (rewritten); test-harness constructor `bridge.rs:2026` updates with an id from its scripted options |

## Claims and falsification

Claims are one sentence each; every falsifier names its independent oracle and a
specific buggy implementation it would catch (non-vacuity).

- **C1 (exact choice):** For every permission request in both committed 2.11.0 traces
  and every in-bounds pick k, the new pipeline's wire reply optionId equals
  `options[k].optionId`.
  *Buggy impl caught:* today's kind-keyed lookup (probe already shows it emitting
  option-0 on 11/16 user_input cases).
- **C2 (encoding):** The serialized reply for a non-first option is JSON-identical to
  the reference client's recorded reply (no extra fields, no `_meta`).
  *Buggy impl caught:* a serializer emitting `"_meta": null` or double-nesting the
  outcome object.
- **C3 (distinct-kind regression):** For trace requests 6–9 and the v2 request, each
  pick's reply is identical to today's recorded output (`probe-output.txt`).
  *Buggy impl caught:* replying with the selection *index* (`"1"`) instead of the id,
  or an off-by-one in option iteration.
- **C4 (trust-phase provenance):** Confirming a trust tier replies with the
  allow_always option id picked in phase 1 plus `_meta.trustOption = <label>`, even
  when allow_always is not `options[0]`.
  *Buggy impl caught:* reading `options[approval.selected]` at trust-confirm time
  (`selected` indexes `trust_options` in phase 2 → would emit `accept`).
- **C5 (cancel unchanged):** Cancelling the dialog emits
  `{"outcome":{"outcome":"cancelled"}}`, which KAS treats as a legal answer (re-asks).
  *Buggy impl caught:* Esc mapped to `Selected { options[0].id }`.
- **C6 (no fabricated ids):** A confirm with `selected >= options.len()` (including
  the empty-options case) sends `Cancel`, never an invented or clamped option id.
  *Buggy impl caught:* clamping the index to the last option; resurrecting the
  fabricated `allow_once` fallback.
- **C7 (unknown-kind unreachable):** Requests with unknown option kinds cannot reach
  cyril's selection pipeline because acp 0.10.2 rejects them at parse.
  *Buggy-world caught:* an acp upgrade adding a catch-all silently making the shape
  reachable (sentinel flips → revisit **cyril-p7kp**).
- **C8 (KAS acts on the choice):** KAS injects the specific selected option into the
  agent turn (not merely "an" answer).
  *Buggy impl caught:* sending the tool_call_id (or any non-option id) — the agent's
  design would not reflect the pick.

### Falsification table

| # | Claim | Falsifier | Oracle (independent) | Cost | Status | Regression fence |
|---|-------|-----------|----------------------|------|--------|------------------|
| C2 | Reply encoding byte-equal to reference | Serialize `SelectedPermissionOutcome::new(<req-3 option-1 id>)`, JSON-compare to the trace reply line | Reference client bytes in committed trace | 10m | **passed** (2026-07-02) | `probe_qo13_reply_shape_matches_reference_bytes` (deterministic, in CI) |
| C7 | Unknown kinds unreachable | Parse a request with a novel kind string | serde error from the upstream acp crate | 5m | **passed** (2026-07-02) | `probe_qo13_unknown_option_kind_parse` (sentinel, in CI) |
| C5 | Cancel unchanged / legal | Trace req 1: reference cancelled, KAS re-asked; plus existing cancel unit tests | Reference client + KAS behavior in trace | done | **passed** (trace) | existing `state.rs` cancel tests + convert `Cancel→Cancelled` test (kept green post-change) |
| C1 | Exact choice for every request × k | Replay both traces through the new pipeline; assert per request+k | `.cyril-qo13/oracle.py` output (raw-text extraction) | impl + 30m | pending | `probe_qo13_replay_trace_permissions` upgraded from printing to asserting; assert messages labeled `user_input`/`tool_approval` per request; + synthetic single-option fixture (S4) |
| C3 | Distinct-kind replies unchanged | Same replay; diff 6–9 + v2 request against recorded current output | `probe-output.txt` (recorded pre-change behavior, itself validated against reference replies) | impl + 15m | pending | same replay fence, distinct assert family (`tool_approval` label) |
| C4 | Trust-phase provenance | Fixture: options `[allow_once='accept', allow_always='always-accept']` + trustOptions; pick k=1, confirm tier | Hand-computed expectation from trace option-id vocabulary | impl + 30m | pending | new `cyril-ui` test `trust_confirm_replies_with_phase1_option_id` + convert test asserting `_meta.trustOption` label |
| C6 | No fabricated ids on oob/empty | Fixtures: `selected=5` on 3 options; empty options list | Hand-computed (Cancel) | impl + 15m | pending | existing out-of-bounds test kept + new empty-options fixture |
| C8 | KAS acts on the specific choice | Trace behavioral analysis (done: q3→option-1 produced the blob-append design); live cyril run answering the trace's 3 questions with non-first picks | Agent's generated text reflecting the pick | trace: done; live: ~1h | **passed by proxy** (trace) | cyril-side fenced by C1+C2 (deterministic); live smoke is a one-time build-phase validation, `manual` — requires user sign-off (see below) |

**C8 manual-fence note:** the server-side half of C8 (KAS's handling of a correct
reply) cannot be fenced deterministically in CI — it can only regress inside KAS
itself. Cyril's contribution (emitting the correct bytes) is fully fenced by C1+C2.
The one-time live validation during `checkpointed-build` satisfies the issue's
acceptance criteria ("verify against the trace's 3 questions"). Approving this design
includes accepting `manual` for the server-side half.

## Negative space (what this change deliberately does not do)

1. No special rendering for user_input questions — the `kind: Other` tool-call card
   stays filtered and `session_info_update pending_interaction` stays dropped
   (tracked at **cyril-0o7e**).
2. No consent `_meta` echo on any permission response — proven unnecessary for
   `allow_once`; the `allow_always` scope question is tracked at **cyril-gn07**.
3. No change to the v2 trustOption echo shape (label string, as today) — the
   label-vs-object question is tracked at **cyril-sive**.
4. No change to the dropped-responder path (`-32603 permission response dropped`).
5. No handling of unknown option kinds — unreachable through acp 0.10.2; watch item
   **cyril-p7kp**.

## Tracker references (all verified to exist)

- **cyril-gn07** — KAS allow_always consent-scope probe + optional passthrough.
- **cyril-sive** — v2 trustOption echo shape verification.
- **cyril-p7kp** — unknown-PermissionOptionKind parse hard-fail watch item.
- **cyril-0o7e** — KAS session_info_update kinds (incl. pending_interaction).

## Self-review record

Claims: 8. Falsifier independence: per-claim oracle named in table. Non-vacuity:
buggy implementation named per claim. Distinctness: separate tests or labeled assert
families per claim. Cost: only C8's server half is expensive → accepted as manual
with rationale. Removed invariants: all four rows map to C1/C4/C6 or compile-time.

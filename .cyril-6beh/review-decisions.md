# PR #68 — review-feedback decisions

Secondary review, 2026-07-26. 10 findings (5 Standards, 5 Spec). Every finding was
verified against the code/docs before any change was applied.

**Headline: the review was accurate.** All 10 bug claims reproduced. That is unusual
enough to state plainly — the normal expectation is 2–3 of 10 failing verification.
Three fixes are *modified* rather than accepted as written, for reasons below.

| # | Finding | Cat | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | [P1] live bearer token serialized into the capture | Bug | **Yes** — `send()` unconditionally `rec()`s; `rep()` routes `auth_reply()` (returns real `accessToken`) through it | **Accept** | Redact rather than suppress — the committed `kas-live-session-trace-2.11.0.jsonl` already stores `accessToken: "<redacted>"`, so a convention exists and this was a regression against it. Redacting keeps the frame shape, which is the point of a wire capture. Verified no committed capture currently leaks. |
| S2 | [P2] invalid token collapses to `{}` / null fields | Bug | **Yes** — `except Exception: return {}`; `d.get()` yields `None` | **Accept** | Validate once before spawn; exit non-zero with a credential-safe message. |
| S3 | [P2] temp dir, log handle, child process not cleaned up | Bug | **Yes** — `mkdtemp` never removed, `log` never closed, `terminate()` with no `wait`/`kill` | **Modify** | Took `try/finally` + bounded terminate→wait→kill + closed log. **Rejected the `TemporaryDirectory` half**: the workspace holds the files the orchestrated stages create (`alpha.txt`/`beta.txt`) — auto-deleting destroys the capture's own evidence. Path is printed instead. |
| S4 | [P2] cyril-6beh holds incompatible scopes | Design | **Yes** — title/description say "model the future protocol, don't render yet"; `design` prescribes rendering `agent-subtask` now; `notes` say to split | **Accept** | Split: cyril-6beh keeps the deferred protocol scope; near-term rendering filed separately. |
| S5 | [P2] rerun instructions incomplete + `auth_kv` contradiction | Bug | **Yes** — `Usage:` omits `[fresh-token.json]`; doc says `auth_kv` "not plaintext" while the probe comment gives a recipe reading it | **Modify** | Reviewer asked to "document one verified extraction path". Measured it — but see the correction below: the measurement was auth-method-specific and my first write-up overgeneralized it. |
| P1 | [P1] JSON-RPC errors reported as capture progress | Bug | **Yes** — `bool(init)` true for an error response; `sid=None` still prompts; prompt error → `stopReason: None` | **Modify** | Probe fixed as asked. **Did not "regenerate the summary"** — that needs a live authenticated run, which is the very thing that is blocked. Hand-editing captured output to insert an error that was never in it would fabricate evidence. Instead the dead workstation-local path is removed and a clearly-marked annotation records provenance. |
| P2 | [P1] workflow-progress detector misses the documented shape | Bug | **Yes** — audit ln 126–127 documents `_meta.kiro.notification.kind` (nested) and `messageId`/`notifyId` prefix `wf-progress-`; probe checks `_meta.kiro.kind` and `update.kind` | **Accept** | Both documented paths implemented. Note the reported `0` was uninformative regardless — the run produced zero tool frames of any kind — but the detector would have under-reported on a *successful* rerun, which is when it matters. |
| P3 | [P2] audit still calls parse-and-drop scaffolding a "renderer" | Bug | **Yes** — ln 107 says "full client-side parser + renderer"; the added correction says no consumer/renderer exists; tracker description repeats the wording | **Accept** | Corrected in the audit and in the tracker description. |
| P4 | [P2] 2.7.1 audit still says filtered calls render opaquely | Bug | **Yes** — `docs/kiro-2.7.1-wire-audit.md:262` says "They already render as opaque tool calls today; nested-crew UI is the only gap" | **Accept** | Corrected to state they are filtered by the `ToolKind::Other` rule and need the `_meta.kiro.kind` exception first. |
| P5 | [P2] capture terminates without draining trailing frames | Bug | **Yes** — `pump()` returns on the prompt response, then terminates immediately | **Accept** | Close stdin, drain to EOF or a bounded quiet period, then summarize and reap. |

## Deferred work — tracker IDs

The skill requires every deferral to name a tracker ID. Two of the decisions above
defer work; both are now filed.

| From | Deferred work | Tracker |
|---|---|---|
| P1 (Modify) | Regenerate the attempt summary from a real run — needs the live authenticated capture that is itself the blocked task | **cyril-ucii** |
| S1 (scope) | The same credential defect in two *other* probes, found by sweeping the directory after fixing this one | **cyril-hhgw** |

## Follow-on findings from the sweep (not in the review)

Checking whether the probe fixes implied changes elsewhere turned up three things
the review could not have seen from one file:

1. **The credential defect is not unique to this probe.** 49 probes in
   `experiments/conductor-spike` answer `getAccessToken`; two of them —
   `probe-kas-compact-summarization-2.9.0.py` and `probe-kas-orchestrate-wire-2.9.0.py`
   — persist the reply verbatim via the identical `rep→send→rec→file` chain. The other
   45 were checked and do not write the auth reply to a file. No committed capture
   currently leaks (swept; the only committed `accessToken` is the literal
   `<redacted>`). Filed **cyril-hhgw**.
2. **There was no written convention to violate.** `experiments/conductor-spike/README.md`
   documented layout and reproduction but said nothing about credentials or about
   failed probes reporting zeros. Both rules added there, with the `redact()`
   reference implementation, so the next probe author inherits them.
3. **The `auth_kv` measurement is load-bearing for an unrelated open issue.**
   `cyril-taba` (p2, auto-refresh the token before `getAccessToken` in wrapper mode)
   lists refresh candidates as a `kiro-cli whoami/profile` shell-out or KAS's own
   file-auth path, and its own notes call the shell-out "inherently fragile … relies on
   an undocumented side effect". The token is in fact readable directly from
   `auth_kv` as plaintext JSON with `profile_arn` included. Recorded on that issue as
   a third candidate — trading one fragility (undocumented side effect) for another
   (another program's DB schema and locking), so it needs the same falsifier, but it
   was absent from the candidate list entirely.

## Correction to S5 — the "verified path" was n=1

My first fix for S5 asserted flatly that `auth_kv` is plaintext, that the row is
`kirocli:social:token`, that it carries `profile_arn`, and therefore that **both** the
audit doc and the probe comment were wrong. The measurement was real, but it was taken
under **GitHub social auth** — and the repo owner had run those probes under IAM
Identity Center auth. Different login, different store shape.

Binary evidence that the paths genuinely diverge:

| string | implication |
|---|---|
| `social token has no profile ARN, treating as invalid` | social tokens **must** carry `profile_arn` in the row — which is why it was there to read |
| `Lazily resolved profileArn from list_available_profiles` | other methods resolve `profileArn` via an API call; it is **not** in the row |
| `Error getting builder id token from keychain` | Builder ID has an OS-keychain path distinct from the DB row |

So **the probe's original comment was most likely correct for the machine it was written
on**: `kirocli:odic:token` is the right key for an IdC login, and merging `profileArn`
from `kiro-auth-token-cli.json` is the right move when the row does not carry it. Calling
it "wrong" was my error, not the author's. The `auth_kv`-not-plaintext note may likewise
have been accurate for a Builder-ID setup where the token lives in the keychain.

Corrected in the audit doc, the probe docstring/`TOKEN_RECIPE`, the README, and on
`cyril-taba` — the last one materially, because that issue is a *product* feature: an
auto-refresh that assumes one store shape breaks for every user on another login, so its
falsifier needs an arm per auth method rather than one.

**Generalized lesson:** a measurement taken on one machine describes that machine. The
probe's comment and the audit doc did not contradict each other because one was wrong —
they described two different auth methods, and I collapsed them into a single claim
because I only had one environment to look at.

## Observed but deliberately not changed

- `probe-…-2.14.1.py` orchestrate-detection has a redundant clause:
  `"stages" in ri or "orchestrate" in name or "task" in ri and "stages" in ri` — the third
  disjunct is subsumed by the first (and its precedence reads misleadingly). Not a review
  finding, and altering detection semantics unasked would change what a rerun captures.
  Left as-is; flagged here so it is not lost.
- `[[wikilink]]` syntax appears in `docs/*.md` (4 pre-existing occurrences on main). It is
  an established habit in this repo's audit docs, so the one added by this branch was left
  alone. It was removed from `CLAUDE.md` only, where no such precedent exists.

## cyril-6beh design-review decisions — 2026-08-09

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| D1 | Enumerated input families lack case-labelled independent oracles and named fences | Design | Yes — design claim 3 collapses the optional, enum, numeric, path, and duplicate-path matrices into one replay row | **Accept** | Add independently expected case ids and explicit converter/state test names; no scope change. |
| D2 | `apply_event -> bool` cannot report atomic snapshot canonicalization failure | Design | Yes — `run_complete.finalState` reaches the tracker, where duplicate canonical paths can still fail | **Accept** | Return `Result<bool, WorkflowStateError>` from `apply_event`; App logs structured context and preserves state. |
| D3 | Signed full-capture double-replay determinism has no distinct claim/falsifier | Spec | Yes — merge/idempotence cases do not compare complete pass-one/pass-two state | **Accept** | Add a dedicated deterministically ordered projection and cardinality fence. |
| D4 | The ordinary-event O(1) claim is not mechanically observable | Design | Yes — result equality cannot distinguish keyed lookup from a full-map scan | **Modify** | Use private `#[cfg(test)]` lookup counters with a fixed per-event bound; do not add a public storage seam. |
| D5 | Required warnings can disappear while every state assertion still passes | Spec | Yes — malformed/unknown/post-terminal rows assert unchanged state but not structured warning context | **Accept** | Reuse the existing tracing-subscriber capture pattern and assert level plus stable context fields, not prose. |
| D6 | Repeat-wrapper metadata can disappear during `loop#N` → `iter-N` canonicalization | Spec | Yes — the signed contract keeps `nodeId` and iteration metadata as data, while the design only says the wrapper is omitted | **Accept** | Transfer wrapper metadata onto the canonical iteration entry and fence exact preservation. |
| D7 | App ownership is stated but exact-once workflow dispatch is not observable | Design | Yes — structural ownership and non-workflow regressions do not fail if App drops or forwards a workflow event | **Accept** | Add an App-seam test: one workflow event causes one tracker mutation and zero session/UI mutations. |
| D8 | Active-run conflicting `run_start` has no claim or fence | Spec | Yes — the state rule warns and ignores it, but no experiment distinguishes that behavior from overwrite, silent ignore, or duplicate creation | **Accept** | Add a case-labelled transition asserting unchanged state, cardinality, and structured warning. |
| D9 | Rejected C3 boundary cases do not require their warnings | Spec | Yes — C2 warning capture covers missing/wrong types only, so silent unknown-enum/overflow/forbidden-value rejection passes | **Accept** | Include stable warning fields in every rejected C3 manifest row and corresponding converter fences. |
| D10 | `run_complete.status`/`finalState.status` disagreement is promised but unfenced | Design | Yes — the adapter rejects the mismatch, while no generated row distinguishes rejection from accepting either status | **Accept** | Add case-labelled outer/inner mismatch rows; expected outcome is adapter drop, unchanged tracker state, and stable warning fields. |
| D11 | Numeric matrix omits the exact representable maximum | Design | Yes — observed maximum and overflow do not fence an off-by-one target bound | **Accept** | Use the existing core convention `u32`; require `u32::MAX` accepted and `u32::MAX + 1` rejected with warning for iteration, maximum-iteration, and plan-revision fields. |
| D12 | Sixteen claims exceed the design hard-gate limit | Process | Yes — exact dispatch and unchanged non-workflow handling are two sides of the same boundary and need not be separate claims | **Accept** | Consolidate C1/C13 while retaining labelled known/near-miss/unrelated cases and both targeted and existing-suite fences. |
| D13 | Live retry reopens a terminal same-id run, conflicting with absorbing-state design | Spec | Yes — the aborted 2.16.2 capture contains `run_start → aborted → run_start → aborted` for one id | **Modify** | The requester explicitly chose and re-signed new-incarnation semantics: post-terminal `run_start` atomically replaces current state; all other post-terminal events remain ignored. |
| D14 | Workspace-path matrix omits relative versus absolute forms | Design | Yes — no declared unreachable constraint or case-labelled result covers them | **Accept** | Preserve empty/relative/absolute/Unicode/spaced strings opaquely; only missing required fields reject. No filesystem conversion occurs. |
| D15 | Validated identifier newtypes have no standalone string rule | Design | Yes — path cases do not fence ids used outside node paths | **Accept** | Reject empty workflow/node ids; accept and byte-preserve every non-empty ASCII/Unicode/space/`#` form. |
| D16 | Identifier matrix omits separator-containing and large standalone ids | Design | Yes — “every non-empty value” is broader than the enumerated cases, so slash/backslash and size-limit bugs could pass C3 | **Accept** | Add labelled `run_start.workflowId` and descriptor `nodeId` rows for `/`, `\`, and large valid strings, all byte-preserved. |
| D17 | Duplicate raw keys and unknown extra fields are declared but absent from C3 | Design | Yes — normalized fixture cases cannot prove pre-conversion duplicate-key resolution or additive-field tolerance | **Accept** | Add raw JSON last-key-wins rows and top-level/descriptor/node-state unknown-extra rows with deterministic converter fences. |
| D18 | Opaque arbitrary JSON is confused with structured unknown fields and lacks a shape matrix | Design | Yes — keys inside recipe inputs/artifacts are data, not unknown schema fields | **Modify** | Ignore extras only in typed wire objects; byte-semantically preserve opaque null/bool/number/string/array/object values, including nested and duplicate array elements, behind `workflow_arbitrary_json_shape_matrix`. |
| D19 | Scalar-string empty rules are unspecified and unfenced | Design | Yes — “empty where forbidden” names no field or source constraint | **Accept** | Required scalar fields require presence and string type but accept/preserve empty; every non-ID scalar string is otherwise opaque. Add a deterministic scalar-string matrix. |
| D20 | Explicit snapshot reconciliation could bypass the signed sole-reset rule | Spec | Yes — terminal-to-nonterminal `apply_snapshot` had no transition | **Modify** | Reject it atomically with `WorkflowStateError::TerminalSnapshotConflict`; only `run_start` resets a terminal same-id run. Exact/equivalent terminal snapshots remain idempotent. |
| D21 | `loop#N` alone can misclassify a valid literal node id as an iteration wrapper | Design | Yes — ids accept `#`, while the rewrite lacked a metadata discriminator | **Accept** | Rewrite only a direct repeat child of type `sequence` whose `iteration` equals `N`; missing/mismatched metadata stays a literal node. Extend C05 controls. |
| D22 | Active-opening conflict fence omits inputs and parent session | Spec | Yes — equality that ignores either field passes the current descriptor/name-only cases | **Accept** | Vary workflow name, inputs, descriptor tree, and optional parent session independently; every active conflict warns and preserves state/cardinality. |
| D23 | “Incarnation” conflicts with the canonical workflow-run glossary | Domain | Yes — `CONTEXT.md` defines one workflow id as one execution, while live retry reuses it | **Accept** | Define a workflow run as the persisted same-id object and a run incarnation as one opening-to-terminal execution attempt; retain only the current incarnation. |
| D24 | Public `get` and `iter` methods lack explicit interface fences | Rust standard | Yes — indirect projections do not prove known/unknown or exact-size behavior | **Accept** | Add known/unknown/empty lookup and empty/multi/exact-size iteration tests; make oracle projections use these methods. |
| D25 | Repeat descriptor/control input variants are incomplete | Design | Yes — `onMaxIterations`, opaque stop fields, and both `stopConditionMet` boolean values are production inputs without fences | **Modify** | Type `onMaxIterations` as `pause`/`abort` with unknown rejection; preserve `stopCondition`/`stopWhen` as opaque JSON; fence false/true loop transitions. |
| D26 | Opaque JSON scalar boundaries are too coarse | Design | Yes — generic negative/zero/positive and non-empty strings cannot catch lossy `serde_json::Value` conversion | **Accept** | Add exact `i64::MIN`, `u64::MAX`, finite fraction, and ASCII/Unicode/spaced/large string/value-key rows to the arbitrary-JSON oracle. |
| D27 | Active-to-terminal snapshot reconciliation is internally inconsistent | Design | Yes — terminal `run_complete.finalState` must share the direct snapshot path, but the state rule allowed active/paused only | **Accept** | Allow an active run to reconcile any valid status, including becoming terminal; only terminal-to-different-status snapshots conflict. |
| D28 | State digests cannot detect workflow forwarding into no-op consumers | Design | Yes — SessionController/UiState invocation can occur without mutation | **Modify** | Add private `#[cfg(test)]` App dispatch counters/spies and assert tracker=1, session=0, UI=0 for workflow events. |
| D29 | Completion event status domains are contextually undefined | Design | Yes — `running` is a snapshot run status but has no signed `run_complete` behavior; node completion has no narrower published status type | **Modify** | Add `WorkflowCompletionStatus` (`paused` plus three terminals), reject `run_complete.running`, and explicitly accept every documented node status on `node_complete` because the wire contract provides no narrower subset. |
| D30 | Malformed matrix samples only one required and one typed field per method | Design | Yes — defaulting another field or treating present invalid/null optional data as absence could pass | **Accept** | Generate every required-field omission and every known-field wrong-type/invalid-null row, retaining warning, unchanged-state, and valid-successor checks. |
| D31 | Direct snapshots and terminal-event transition gates are conflated | Design | Yes — explicit persisted-state reconciliation may update a terminal run, while signed absorbing semantics require non-exact post-terminal events to warn/ignore | **Modify** | Fence entrypoint × prior status × incoming status: direct snapshots may seed/reconcile; unknown-run completion events do not seed; terminal event duplicates are unchanged and non-exact repeats warn/ignore; both use one canonicalizer behind their gates. |
| D32 | C3 lacks named fences for several generated families | Process | Yes — a single manifest test can silently omit descriptor/enum/duplicate/unknown/path/collection families | **Accept** | Name one deterministic regression test for every generated measurement family in C3. |
| D33 | Resolution-bearing non-empty queue frames are unspecified | Design | Yes — optional resolution and pending collection vary independently | **Modify** | Treat any resolution-bearing frame as acknowledgement-only: record outcome/reason and preserve current pending descriptors regardless of supplied array; fence all outcomes, reason presence, and empty/non-empty arrays. |
| D34 | Same-path changed node id is absent from the merge/index matrix | Design | Yes — the state rule promises latest-present replacement and index maintenance | **Accept** | Add changed-`nodeId` row proving the old index membership is removed and the new id resolves the path. |
| D35 | Changed-id index fence misses shared/occupied buckets and mixed types | Design | Yes — deleting a whole old bucket, overwriting a populated new bucket, or counting a same-id step as a repeat can pass | **Accept** | Add old/new bucket cardinality rows plus same-id step+repeat versus two-repeat resolution rows. |
| D36 | App error handling has no state-error fence | Design | Yes — valid conversion followed by invalid state can panic/suppress warning/lose successor while success-only C14 passes | **Accept** | Add duplicate-path error then valid App dispatch with warning, atomic state, exact counters, and successor application. |
| D37 | Completion metadata independence is tested only by absence | Design | Yes — status derived only when a signal is present can pass | **Accept** | Cross authoritative run/node statuses with contradictory signals and sources and assert status/liveness never changes. |
| D38 | `node_complete` partial merge has no state fence | Design | Yes — C6 covers only `node_start` | **Accept** | Add every optional completion-field absent/present matrix while proving unrelated prompt/session/runtime fields survive. |
| D39 | Snapshot-owned clearing versus stream-only preservation is unfenced | Design | Yes — C4 checks transition/error shape but not ownership | **Accept** | Seed snapshot-owned completion data plus stream-only prompt, reconcile an omitting snapshot, and assert owned fields clear while prompt remains. |
| D40 | Retry reset fixture does not populate all stale-state families | Design | Yes — live retry lacks queue/pause/progress/index/completion data | **Accept** | Add fully populated synthetic terminal incarnation and prove new opening removes every prior runtime/index/queue/progress/completion field. |
| D41 | Unknown-identity fence samples too few variants | Design | Yes — one dispatch arm could create placeholders | **Accept** | Generate unknown-run rows for all eight non-opening events and unknown-node rows for all four node-addressed updates. |
| D42 | Absorbing fence samples one post-terminal variant | Design | Yes — seven other event arms could mutate | **Accept** | Generate all eight non-opening post-terminal rows; only exact completion duplicate is idempotent without warning. |

The review verdict was “not approval-ready” before these corrections. No finding is deferred and no new tracker issue is required.

## cyril-6beh full-review decisions — 2026-08-09 (Standards ×10, Spec ×4 + 1 minor)

Two-axis review of the branch against `main`. Every finding verified before fixing;
all suites, both fence modes, and all four oracle modes green after.

| # | Finding | Verified? | Decision | Note |
|---|---|---|---|---|
| S1 | Step wire keys `model`/`effort` contradict the documented `modelId`/`effortLevel`; fixtures + manifest shared the mis-copy (correlated oracle) | **Yes** — audit :138 and the live recipe catalog in `terminal-aborted-2.16.2.jsonl` both spell `modelId`/`effortLevel`; zero bare `model` keys in any capture | **Accept** | Renamed wire structs, domain fields/accessors (`model_id()`/`effort_level()`), fixtures, manifest, and Python oracle. Added `descriptor_wire_spelling_matches_live_recipe_catalog` — pinned against the live catalog bytes, the one artifact not derived from our own fixtures — so the correlated-oracle hole cannot reopen. |
| S2 | C12 fence red: working tree gave the `*Parts` transfer structs `pub(crate)` fields | **Yes** — repo mode exited 1 | **Modify** | Chose *deliberate fence refinement* over revert: reverting would resurrect the positional `into_values()` tuples T5 flags. New rule — plain `pub` structs expose no field of any visibility (unchanged); `pub(crate)` transfer structs may carry `pub(crate)` (never plain `pub`) fields. Two new self-test mutations fence the refinement in both directions; design.md:79's private-fields sentence governs the exported domain types and still holds. |
| S3 | Adjudicated D4 lookup counters never built; manifest bounds unread | **Yes** — nothing read `ordinary_lookup_bound`/`pathless_lookup_bound` | **Accept + contract correction** | Built file-private `#[cfg(test)]` thread-local counters tallied at the actual map-read sites; scale fence asserts per-event bounds for ordinary `node_start` and (new coverage) pathless `loop_iteration`. Measurement falsified the frozen pathless bound: `node: 0` was speculative — the typed-repeat filter probe plus the addressed update are 2 node-map reads (still O(1)). Manifest corrected to `{run:1, node:2, id_bucket:1}`; fence mutation-tested (bound 2→1 fails with `(1,2,1)`). |
| S4 | Replay skipped `kas-custom-dag-2.16.0.jsonl` + three csig captures | **Yes** | **Accept** | All four folded into `REPLAY_SOURCES` (8 sources), `compare-oracles.sh replay`, and the regenerated expected doc; verified each contributes non-vacuous state. |
| S-minor | spec.md:54 “from its descriptors” implies declared nodes are addressable | **Yes** — only runtime nodes are | **Accept** | Spec clarified in place: addressability requires a snapshot or prior `node_start`; the declared plan alone does not create addressable nodes. |
| T1 | Unit tests `include_str!` fixtures from mutable pipeline dirs | **Yes** | **Accept** | Canonical home is now `crates/cyril-core/tests/fixtures/kas/workflow/` (moved contract/oracle artifacts, copied immutable captures, provenance README). Oracle scripts read the same bytes the tests embed — one copy per byte compared. |
| T2 | app.rs doc comment mangled | **Yes** | **Accept** | Original phrasing recovered from commit `0b26481` and restored. |
| T3 | CLAUDE.md still described two notification consumers | **Yes** | **Accept** | WorkflowTracker added as the third consumer in the layer bullets, Component Separation (with the exactly-once / no-second-consumer contract), the Data Flow diagram, and the refactoring checklist. |
| T4 | Nine `Wire*` serde enums + identity `From` impls duplicated `workflow_enum!` spellings; macro `TryFrom` had zero production consumers | **Yes** | **Accept** | Generic `WireEnum<T>` deserializes through the domain `TryFrom<&str>` — spellings single-sourced, rejection stays at serde time so `serde_path_to_error` keeps exact field paths; `classify_serde_error` recognizes the macro error's `unknown workflow …` prefix as `invalid_enum`. Deliberately out of scope: `WireNodeDescriptor`'s serde `tag` renames (structural tagged union, not a value vocabulary). |
| T5 | Positional `*Values`/`*Parts` tuples (same-typed adjacent `Option`s transpose silently) | **Yes** for the two `Values` tuples; the named `*Parts` tuples were distinct-typed but positional | **Modify** | All five named tuple aliases → named-field `pub(crate)` structs. `WorkflowNodeCompletedParts` deliberately drops `nodeId`/`parentSessionId` (hazard 6: completion merges on `nodePath` only) — the type now encodes what the `_` discards hid. The five small inline `into_parts` tuples with pairwise-distinct types were left: transposition cannot compile there. |
| T6 | `Option<Option<Notification>>` tri-state + pure-delegator wrapper | **Yes** | **Accept** | `WorkflowFrameOutcome { NotWorkflow, Dropped, Converted(Box<WorkflowEvent>) }` in `convert::kas` (fence forbids pub types in the adapter file); engine matches directly; wrapper deleted. `Converted` carries the boxed event, not `Notification` — wrapping is the engine's job and it keeps variant sizes flat. |
| T7 | `classify_serde_error` matches error strings | **Yes**, and mitigated | **Accept-modified** | Constraint documented at the function: serde erases error structure into a message before this layer; prefix-matching the known shapes is the only classification available, unknowns degrade to `invalid_value` with the raw message logged. No structural change possible without forking serde's error type. |
| T8 | `CaptureWriter`/`must_succeed` duplicated across test modules | **Yes** — and wider than reported: 3+2 `CaptureWriter`, 3 `must_succeed` (in different files than named) | **Accept** | Canonical copies in `cyril-core::test_support` (own file), cross-crate via a `test-support` cargo feature consumed only from `cyril`'s dev-dependencies (verified no feature leak into normal builds). All branch-introduced copies deduplicated. The two *pre-existing* `convert/mod.rs` copies predate the branch and were left (scope) — filed **cyril-74ly**. |
| T9 | Tight wall-clock asserts (50 ms engine, 100 ms state, +50 ms workflow scale found during fixing) | **Yes** — all three are perf smoke ceilings, none semantic | **Modify** | Raised to the repo's 5 s / 2 s CI-safe ceiling precedent. The single-call 64 KiB near-miss assert would have been vacuous at any safe ceiling, so it now loops ×100 — quadratic matching overshoots by ~4×, healthy paths run in microseconds. Logical asserts unchanged. |
| T10 | `apply_snapshot`/`get`/`iter` have no production caller | **Yes** | **Reject (no change)** | Not speculative: design.md:110 names cyril-jxfu (workflow-session routing) and cyril-0qe6 (control/snapshot replies) as the verified consumers; D24 added the interface fences. Staged API is design-signed. |

Deferred work with tracker IDs: **cyril-74ly** (pre-existing `convert/mod.rs` CaptureWriter dedup).

## PR 92 deep-review decisions — 2026-08-09 (five specialist lenses)

Five parallel review agents (general, tests, silent-failure, type-design, comments)
over the PR's Rust files, weighted at the seams the two-axis review under-covered.
Every finding verified before action.

| # | Finding (lens) | Verified? | Decision | Note |
|---|---|---|---|---|
| CR1 | `canonical_child_segment` strictly narrower than the vendor's own flattener; divergences fail closed and orphan repeat subtrees (general, **critical**) | **Yes** — `H1n` carved byte-exact from the 2.16.0 `kiro-cli-chat` binary: `iteration` wins outright, else suffix-only ASCII `#(\d+)$` verbatim; child type and parent id never consulted. The audit's own `review-loop` + `loop#0` naming is the reachable false negative | **Accept — SUPERSEDES D21's discriminator** | Vendor rule ported verbatim to all three implementations (Rust + both Python oracles, per the bundle-is-the-reference rule). `repeat_controls` redesigned: 9 explicit-segment rows covering every vendor decision point (suffix-only rewrite, iteration-wins conflict, type-ignored, leading-zeros-verbatim `iter-02`, foreign-prefix rewrite, non-repeat parent literal, non-digit/Unicode literals). D21's false-positive worry is answered by fidelity: whatever the vendor rewrites, we rewrite. |
| CR2 | `run_complete` with disagreeing outer/snapshot ids reads one run, writes another (general, important) | **Yes** — id check lived only in the adapter; public `apply_event` callers could seed a run from a completion, violating D31 | **Accept** | `WorkflowCompletionMismatchError` is now an enum (`Status`/`WorkflowId`); `WorkflowRunCompleted::new` rejects id disagreement, making the state unconstructible. Adapter error mapping distinguishes the variants; domain fence added. |
| CA1 | Engine trait doc promises `Err` on malformed-but-recognized frames — false for workflow warn-and-drop (comments, **critical**) | **Yes** | **Accept** | Trait doc now carves out the workflow exception explicitly. |
| T1 | `snapshot_node_fields` manifest oracle orphaned; malformed snapshot-node rows unfenced (tests, important) | **Yes** — zero consumers repo-wide | **Accept** | Matrix extended 205 → 277 rows: manifest-driven injection at the snapshot root AND a nested child (missing/wrong-type/unknown-enum/null per field shape); unclassified manifest names panic, so oracle growth forces rows. |
| T2 | Engine `Dropped → Ok(None)` arm unfenced (tests, important) | **Yes** | **Accept** | Engine-level fence added (`kas_engine_drops_malformed_workflow_frame_to_ok_none`). |
| T3 | Depth cap and its `Value::Null` artifact untested (tests, important) | **Yes** | **Accept** | `workflow_frames_survive_depth_extremes`: 120-deep descriptor + snapshot trees convert and canonicalize losslessly; whole-params `Null`/scalar warn-and-drop. |
| TD1 | Path–workflow binding validated then discarded; events can pair run B's id with run A's path (type-design) | **Yes** | **Accept-modified** | `debug_assert` guard in the four path-carrying constructors + `should_panic` fence, not a `Result` API break: the converter always builds paths from the event's own id, so the runtime path is safe; the guard catches future consumers during development. |
| TD2 | `WorkflowRun` Option-shaped where snapshot facts co-vary; hidden accessor pairing (type-design) | **Yes** | **Accept-modified** | Additive `plan()` → `WorkflowPlan::{Opening, Snapshot}` gives consumers the single honest seam; the interior field shape stays (restructuring six fields through the oracle projections buys no consumer-visible safety). |
| TD3 | `WorkflowNodeSnapshot::new` accepts leaf-with-children trees (type-design) | **Yes** | **Reject (no change)** | Wire leniency is the signed design (D18-family): snapshots preserve whatever tree the engine sent, and the internal runtime-children path already `debug_assert`s. Enforcing in the public constructor would make the converter reject representable wire shapes. |
| TD4 | Missing `Display`/`FromStr`/accessors (type-design, minor) | **Yes** | **Accept** | `Display for WorkflowNodePath` (slash-joined, documented non-parseable), `FromStr` via `workflow_enum!`, `WorkflowSnapshotData` accessors. |
| SF1 | Unknown `kiro/workflow/*` family member vanishes into generic `debug!` (silent-failure) | **Yes** | **Accept** | Family-scoped `warn!` before `NotWorkflow` — a tenth lifecycle kind now surfaces at default log level. |
| SF2 | Post-deserialize id rejections lose tree position (silent-failure) | **Yes** | **Defer** | **cyril-sinu** — diagnosability only; serde-time paths are exact. |
| SF3 | `Ok(_)` discards the changed flag; future renderer must wire redraw (silent-failure) | **Yes** | **Accept** | Constraint comment at the dispatch site. |
| SF4 | `unreachable!` on index desync would panic the TUI on a wire frame (silent-failure) | **Yes** | **Accept** | Degrades to `debug_assert` + `warn_ignored(index_desync)`. |
| CA2–4 | Lookup-telemetry doc overstates; `test_support` lock/feature docs imprecise (comments) | **Yes** | **Accept** | Docs scoped to the fenced events; lock documented as belt-and-braces over thread-scoped installers; feature additivity caveat stated. |
| CA5–7 | Hazard 1/2/D33 constraints undocumented at their enforcement sites (comments) | **Yes** | **Accept** | Docs added at `WorkflowCompletionStatus`, `is_terminal`, `apply_node_started`, `apply_steps_queued`. |
| CR3 | `from_snapshot` discards children via `..`; invariant unc checkable (general, suggestion) | **Yes** | **Accept** | Explicit binding + `debug_assert!(children.is_empty())`. |
| T4/T5 | Feature-exclusion assert; one-directional matrix tripwire (tests, minor) | **Yes** | **Defer** | **cyril-7sjs**. |

Deferred with tracker IDs: **cyril-sinu**, **cyril-7sjs**.

## PR 92 follow-up review decisions — 2026-08-10 (Standards ×9, Spec ×18)

Every finding verified before action (assessing-review-feedback discipline);
claims that failed evaluation are rejected with rationale, not silently dropped.

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | kas-feature rationale comment omits serde_path_to_error | Style | Yes | **Accept** | Clause added, matching comment style. |
| S2 | WorkflowId/WorkflowNodeId newtype duplication | Style (smell) | Yes — ~30 lines ×2 | **Reject** | Rule of three: two instances with distinct error wording don't buy a macro; the reviewer's own "partially justified" concedes it. |
| S3 | parent_session_id rides positional tuples then is discarded | Style | Yes | **Accept-modified** | Dropped from all five inline `into_parts` (routing metadata, not state) — same convention as the named `*Parts` structs — rather than structifying distinct-typed tuples. |
| S4 | `mod workflow;` placement between docs and uses | Style | Yes | **Accept** | Moved below the import block. |
| S5 | malformed_cases parses manifest inline despite workflow_manifest() | Style | Yes | **Accept** | Helper reused. |
| S6 | pipeline test re-runs four sibling #[test]s | Test design | Yes — zero added coverage, doubled runtime | **Modify** | Reviewer's implicit fix (delete) would break a plan-promised fence name. Reworked into real cross-frame coverage: one live pipeline receives all 297 malformed rows, state stays byte-stable, a valid successor still applies. |
| S7 | `[u32::MIN,u32::MAX] == [0,4_294_967_295]` tautology | Test design | Yes — cannot fail | **Accept** | Deleted; the real u32 boundaries are fenced in `workflow_numeric_and_path_boundaries`. |
| S8 | 5 s/2 s wall-clock ceilings remain nondeterministic | Test design | Yes, and adjudicated | **Reject** | Re-flag of the 2026-08-09 T9 decision: the ceilings are orders-of-magnitude smoke fences for quadratic regressions, which the deterministic asserts cannot catch. |
| S9 | probe.py reads argv[1] unguarded | Bug | Yes | **Accept** | Usage guard, exit 2. |
| SP1 | spec contradicts itself on snapshot seeding | Spec doc | **Yes — real contradiction** | **Accept** | Reject-section now covers only non-`run_start` lifecycle events; direct seeding stays with the seed section (D31/D20); dated clarification. |
| SP2 | D21 discriminator stale in design/plan/falsifier | Spec doc + oracle | **Yes** | **Accept** | Dated CR1/H1n supersession notes at each design/plan site; falsify-node-paths.py ported to the vendor rule with vendor-truth controls. |
| SP3 | replay oracle resets active runs unconditionally; vacuous | Bug (oracle) | **Yes** | **Accept** | Oracle rewritten to the signed active-duplicate/conflict/post-terminal rule with de-vacuating synthetic frames and regenerated expected; validated by the replay comparison mode (see the final gate run for this round). |
| SP4 | replay oracle overwrites duplicate canonical paths, keeps unknown fields | Bug (oracle) | **Yes** | **Accept** | Duplicate-path finalStates rejected atomically and node data filtered to the manifest's known fields, with a de-vacuating fixture; validated by the same gate run. |
| SP5 | family-warn (SF1) missing from spec | Spec doc | Yes | **Accept** | Dated amendment in the unrelated-extension section. |
| SP6 | design says kiro delegates workflow dispatch | Spec doc | Yes — engine owns it | **Accept** | Dated correction note. |
| SP7 | terminal gate ordered after canonicalization → Err instead of absorb | **Bug** | **Yes — reproduced by the new fence** | **Accept** | Gate reordered: while terminal, a snapshot that cannot canonicalize is warned+ignored (it cannot be the exact duplicate); fence `post_terminal_completion_with_invalid_snapshot_is_absorbed`. |
| SP8 | swap_remove breaks index-order equality | **Bug** | **Yes** | **Accept-modified** | Root fix is canonical bucket order, not just order-preserving removal: buckets kept sorted (`Ord` on WorkflowNodePath, binary-search insert), so equality is a function of contents; fence `node_index_buckets_stay_sorted_through_moves`. |
| SP9 | glossary "terminal run_complete" ambiguity | Domain doc | Yes | **Accept** | Incarnation entry now spells out the terminal set and paused-resumability. |
| SP10 | plan/findings point at pre-move artifact paths | Spec doc | Yes | **Accept** | Single dated relocation notes. |
| SP11 | plan still states 50/100 ms budgets | Spec doc | Yes | **Accept-modified** | One dated T9 note where budgets are introduced; original figures kept as sizing context. |
| SP12 | C8 queue fence covers one acknowledgement form | Test gap | Yes | **Accept** | Full outcome × reason × cardinality cross + resolution-free replacement rows. |
| SP13 | retry-reset fixture missing shared buckets + completion metadata | Test gap | Yes | **Accept** | Prior incarnation now seeds completion signal/source/failure and a two-path shared index bucket. |
| SP14 | malformed matrix omits finalState metadata | Test gap | Yes | **Accept** | 20 manifest-consistent rows (missing/wrong-type/unknown-enum/null); matrix 277 → 297. |
| SP15 | running/running completion pair unfenced | Test gap | Yes — `running` absent from the outer-status axis | **Accept** | Dedicated rows: outer `running` × all snapshot statuses → invalid_enum at `status`. |
| SP16 | scalar matrix omits slash/backslash + several fields | Test gap | Yes | **Accept** | Values +`path/with/slash`, +`back\slash`; fields +watch_poll.at, paused.pauseReason, ack reason, watch handler, finalState.createdAt. |
| SP17 | terminal oracle counts a union across captures | Oracle gap | Yes | **Accept-modified** | Per-file validation added; aggregate output shape kept byte-identical for the Rust comparison. |
| SP18 | wrapper falsifier checks nodeId+iteration only | Oracle gap | Yes | **Accept** | Metadata preservation checks widened (status + supplied runtime fields), folded into the SP2 falsifier port. |

No deferrals this round — every finding is applied, modified, or rejected with rationale.

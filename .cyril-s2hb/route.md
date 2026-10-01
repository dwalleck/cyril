# Route: cyril-s2hb

## Requester workflow override

> I'm giving explicit permission to override the standard gilfoyle workflow. The goal is to get a working, tested version of the crtool implemented. We can bypass the gilfoyle ceremony at this point

The immediate deliverable is working, tested crtool code. The Gilfoyle phase,
size-budget, mandatory mutation and reconstruction gates below are historical,
not prerequisites for completing this implementation. Preserve the functional
requirements, repository Rust quality checks, actual Linux/Windows execution,
and existing publication/CI conditions. Earlier results remain evidence for
their recorded source, not claims that the current implementation passed.

## Historical route

Change: native crtool gather/facts, cancellable diagnostics library, and resolved-shell executable prefix; the 2026-10-01 reset selects the git2 + Tokio direction and a bounded-drain capture contract.
Date: 2026-09-30; route tests rerun 2026-10-01 after the requester's git2/Tokio and capture-contract selection.
Source baseline: cc5eba260572e08fd20a58c8b0cdd1aaec782484; upstream discovered through origin/HEAD.
Invocation: gilfoyle cyril-s2hb, repository-local skill exposed by an uncommitted symlink.

Publication merge pins: three-way ancestor `422d16467aa48d8956d11dcc3b752fa91d53bd6f`; repaired PR150 normative source `64d9293685c1b7525c7b7dd42e48bb2f71cba7e0`; latest R17 implementation-source handoff is read-only. No source checkout is modified by this normative publication.

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The 2026-10-01 selection introduces unverified external-library premises: typed git2 operations over vendored libgit2 (diff with rename detection disabled, status, raw-path identity, patch content, feature/linking set) and Tokio-based concurrent capture with the one-second bounded final drain and explicit EOF/completeness tracking on Linux and native Windows. Existing subprocess-era fixtures and source inspection are not execution evidence for these mechanisms, so new library/native probes are required. | yes |
| 2 | Structural module shape | Create the cyril-review leaf with no `cyril-core`/ACP/UI coupling and no library-owned runtime; the requester's git2 + Tokio selection adds a typed Git dependency seam (vendored libgit2, unnecessary network features off; no speculative gix) and permits Tokio reuse of the App runtime. core owns the prefix, existing HostShell retains sole shell resolution ownership, and main gains only hidden early dispatch. Main and HostShell are protected parents. New crate/interface/dependency seams trigger module review independent of size. No aggregate commit-size gate is active; module/per-file tripwires remain independent and any other size blocker follows the measured-size/options/user-choice policy. Existing total-line signals and the reviewed growth ledger remain context, not a publication fit claim. | yes |
| 3 | Production-scale risk | Gather retains diffs through typed libgit2 operations and per-file work; facts scans usages per symbol; diagnostics concurrently captures potentially large output while enforcing timeout/cancellation and the bounded final drain. Pipe deadlock, descendant process lifetime, libgit2 diff/memory behavior, and data-volume behavior require independent fixtures and explicit measured bounds without silently narrowing Python behavior. | yes |
| 4 | Explicit behavior | All requested behavior is explicit: spec.md pins the requester-selected git2 + Tokio direction, the bounded-drain/explicit-incomplete capture contract, and the retained target/scope, raw path/content, ordering, and error obligations; cancellation result/tree semantics, deterministic controls, and POSIX/Windows diagnostics parsing carry their owning decisions. The remaining concrete library/native API, feature, linking, and schema-field naming choices are design-owned implementation selections for prove-it/design, not unresolved requester behavior. | yes |

Unknown tests: none. T1 contains identified unverified library/native premises; T4 has no unresolved observable behavior — the remaining API/feature/schema naming is design-owned implementation selection, not absent research.

## Selected route

Empirical — cross-platform process/output/cancellation premises require observed oracle evidence before design and implementation.

Scoped policy amendment (2026-10-01): the requester selected the git2 + Tokio direction (git2 as the primary typed Git API with vendored libgit2 and unnecessary network features off; no speculative gix; Tokio permitted with no library-created runtime) and a bounded-drain/explicit-incomplete capture contract, recorded once in spec.md's Requester selection section. This removes a categorical dependency prohibition and the mandatory synchronous-diagnostics wording; it does not waive the remaining functional or native-evidence obligations. The overall Empirical route remains: concrete library/native behavior still needs observed qualification (T1); the selected dependency/operation boundary must be recorded and reviewed with design and manifests (T2); capture/data-volume and concurrency risks remain (T3); T4 is yes — behavior is explicit, with concrete API/feature/schema naming left to design as an implementation selection. No production dependency or owner changes in this policy-only amendment; existing size and cycle limits are unchanged.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — pinned behavior source (T4 yes): the requester-selected git2/Tokio direction and bounded-drain/explicit-incomplete capture contract; concrete library/native API, feature, and schema-field naming is a design-owned implementation selection |
| evidence.md, probe.* | prove-it-prototype | required — discharge T1 library/native probes on available actual hosts |
| design.md | falsifiable-design | required — new crate, prefix seam, protected-parent, typed git2 boundary and capture ownership |
| plan.md | budgeted-plan | required — atomic verified slices and growth ledger |

Oracle checkpoint in checkpointed-build: required — Empirical route.

## Downstream sequence

interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Every empirical premise records PASS in prove-it-prototype; every later artifact satisfies its owning stage's completion criterion; checkpointed-build has no FAIL. Required cargo fmt --check, cargo test, cargo clippy -- -D warnings and actual acceptance smoke must pass. Final independently reviewed PR head must have zero unresolved findings and passing required CI before merge and issue closure. No gate is currently claimed PASS.

## Execution authorization and caps

The user approved routine gilfoyle signoffs/recommended decisions, with the explicit limit: “Only auto-approve decisions regarding the gilfoyle skill”. Scope expansion alone is not a stop; unavailable required hosts and destructive operations are stops. Do not use preapproval to waive evidence or silently change the issue's accepted architecture/behavior. Decisions made under this delegation are recorded as agent decisions authorized by the user, never as invented verbatim user decisions.

Windows prerequisite authorization (verbatim):
> And we can install missing dependencies on the windows VM if we need to

Inspect/reuse existing native tools first, distinguish missing PATH/vcvars setup from missing software, and install only prerequisites needed for the selected qualification. Record installations. This does not waive native execution or runtime packaging proof and does not authorize unrelated VM/network changes.

Historical route-stage counters: implementation attempts 0/5 and independent PR review/fix rounds 0/5. Routing and prerequisite inspection were not implementation attempts; parent cyril-5gb3 remained unchanged at route creation.

## Functional-equivalence amendment — 2026-09-30

Requester correction (verbatim):
> “It didn't need to be a 1 to 1 copy of the python script, just functionally the same”

Further requester clarification (verbatim):
> “The results and output don't have to be byte equivalent”

These latest corrections supersede the route's former byte-output/one-to-one parity framing. The selected Empirical route remains appropriate for native lifecycle, host, process, and functional-output evidence. Semantic fields/types, array/file/symbol/usage sequence, meaningful content, status/error/context, patch content, raw capture integrity, UTF-8 text, Unicode whitespace scope handling, and safety/lifecycle/native gates remain active. Presentation-only line endings are not a native LF-only gate. Old byte/text-wrapper evidence is historical. At PR150 opening, no implementation-success receipt existed; the sole A2 evidence owner, `.cyril-s2hb/checkpoint-A2.md`, now records current A2 qualification for C1–C4, C9 and applicable C10, with C1/C3/C10 subject to their B extension and C5–C7 pending the unimplemented B checkpoint. Final full-PR review, native qualification, required CI, merge and issue acceptance remain pending. Historical opening counters remain distinct from live PR consumption; no counter reset is implied.

R17 retains the Empirical route and all raw identity/framing obligations. The user's exact waiver is recorded verbatim: “Ignore the 4000 line commit limit”. It removes only the aggregate commit/publication-size gate and size-only fit claims; it does not waive acceptance, review, CI, native, lifecycle, safety, fixture, assertion, cap or raw-content requirements, and no future size-driven partition is planned. Module/per-file tripwires remain independent; if another size gate blocks progress, report measured size and options/tradeoffs and obtain the user's choice before restructuring, changing a gate, partitioning or seeking another waiver. No such blocker is known here.
> "Ignore the 4000 line commit limit"

The previously qualified flag-based fixture driver and complete CLI harness remain authoritative; no JSON stdin/from-reader transport change is introduced. A1 remains merged, while existing PR150 carries historical A2-C opening context followed by atomic A2 gather/facts+hiddenCLI and B diagnostics checkpoints. The sole A2 evidence owner, `.cyril-s2hb/checkpoint-A2.md`, records current A2 qualification for C1–C4, C9 and applicable C10; C1/C3/C10 remain subject to their B extension, while C5–C7 remain pending because B is unimplemented. Final full-PR review, native qualification, required CI, merge and issue acceptance remain pending; closure verifies A1's merge plus final PR150 merge and every criterion. No final PASS receipt is claimed.
The final A2 checkpoint retains the replaced rather than re-pinned `facts_pages_pack_unicode_characters_and_split_at_the_budget` heading/prose fence: `.cyril-s2hb/checkpoint-A2.md` records Unicode-scalar greedy `write_pages`/`body_characters` packing, unchanged meaningful input lines, and a chars-to-bytes mutation failure. All public/reference cap, oversize-line and content cases and the all-platform malformed-manifest matrix for both gather and facts are observed with unchanged artifacts. These A2 obligations are qualified; B diagnostics remains unimplemented, and no final PR/CI/merge/issue acceptance is claimed.

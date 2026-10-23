## Summary

```text
admitted session/new -> independent 90s off-loop response wait
  -> live ready / failure / manual OAuth notices
  -> typed active-attempt + actual-scope provenance
  -> timeout: known outcomes + explicit unknown complete inventory
  -> retire failed attempt; isolate late replies; tear down owned bridge

session/load: 30s | controls: 10s | prompt response: unbounded
```

**Draft — native acceptance, current CI and final combined review remain outstanding; not acceptance-complete or merge-ready.** Refs `cyril-caar`; the issue stays open.

Depends explicitly on [NK #196](https://github.com/dwalleck/cyril/pull/196), which depends on [Tqr #193](https://github.com/dwalleck/cyril/pull/193). This draft targets **main** for the repository's existing CI triggers; it does not replace the logical Tqr → NK → Caar delivery order. The cumulative main-targeted diff may include inherited changes; the [25-path Caar feature manifest](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/feature-files.json) distinguishes ownership.

The feature preserves the sole root-start admission owner, fatal fingerprint preflight, independent Load/control policy and qualified parent repairs. Early replacement progress is display-only until actual session correlation; a partial/pending inventory is never invented into a complete silent-server set. Manual sign-in delivery is not OAuth success.

## Evidence

- **Before:** Historical actual-request regressions expose the old 30-second New cutoff and replacement-progress/fingerprint failures. The original ordinary full review62 **FAIL** remains immutable.
  **After:** The repaired wording's **scoped PASS** is retained separately; it is not a new full-assembly approval. The [owning decision](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/review-decisions.md) preserves both reviews, original compiler/fixture failures and failed qualification attempts without relabeling them.
- **Current local qualification:** All **10 assigned commands PASS**: three test commands, three strict Clippy commands, two plain App binary builds and two formatter checks (no formatter application). Three-package KAS tests: **3006 passed / 0 failed / 12 ignored**; isolated App no-default: **380 / 0 / 1**; isolated Core no-default: **1067 / 0 / 4**. The aggregate **4453 passed / 0 failed / 17 ignored** counts overlapping executions, not globally unique cases. All **15 App startup cases freshly ran in both default and no-default lanes**. This is not whole-workspace, all-features, nextest, release or platform-CI evidence.
- **Qualified execution frame:** The following actual-input descriptors are not Git tree/commit identities. All 563 identified compile/runtime inputs and the held-parent scope were unchanged throughout the 10 commands; Main owns the final authoritative binding.

  ```text
  HEAD:                  62e313d4d27c5c76348ce7e6a58f794e56ff839b
  held NK parent:        1740221839f3c4455ed9c4536c1a23b1905fc531
  held Tqr parent:       2d1c93843834e21966ca8e816349d891ac97d5d9
  current529 descriptor: d4b06156946dc410b95bbde91e7157587bd5185d48713701c011472ebec09f2f
  identified563:         badeaff8ddbbe360cb5c357b2996abd8603c0726441f216bb88d00b05ba1852d
  default App SHA256:    0c339b4bbc0203dde6de2dadfbb2312b11425e0fb6872f7756f11ce7cabd5ca1
  ```

- **One owned actual-App same-ID smoke — PASS (10 checks):** The fresh default App above ran in owned **tmux/bwrap with an owned V2 peer**, not a native/provider acceptance lane. Root A's current toolbar remained present before, during and after Load A. The corrected qualifier **`(requested load; response pending)`** appeared once while the matching response was held and remained once in history after that actual request ID was released. Main read the pending capture. Ordinary `/quit` produced observed **App/bwrap exit 0**; separate launcher exit is **UNOBSERVED**, not inferred. No forced signals; the owned tmux server stopped and launcher pidfd closed.
- **Historical evidence stays historical:** The old529 original **22-command matrix** and `a5c6…` App/native-auth **FAIL** remain pinned to their original frames, not the current source or binary. Retained component/scoped source reviews do not establish fresh combined conformance approval.
- **Native acceptance remains outstanding:** The latest known authentication blocker is the retained **original-frame** `You are not logged in` failure before initialize returned or `session/new` was sent; no New budget began. It was **not probed on the current frame**. Earlier New/present-preview observations and both fixture-observation gaps remain separate. Current native startup/navigation/previews, natural inventory and held-response New80/New95 acceptance are not established. These gaps neither establish a product defect nor authorize a product repair. Deterministic actual-request tests support the existing >=90s New policy; natural inventory's independent 95s observation is separate. No new timing threshold, native timer-fire measurement or performance claim is made.

The [current proof index](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/index.json) owns evidence attribution. Planned portable publication: [assembled proof archive](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/p3-assembled-proof.zip) and [member index](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/p3-assembled-proof-members.json). These are selected evidence, not binaries, a full source tree or a self-contained native replay package. Main owns final binding/publication; this body performs no GitHub mutation and grants no acceptance or merge authorization.

## Merge Danger

**Door:** Two-way

No storage migration or authentication/configuration change. Reverting this feature would also remove its startup-policy and diagnostic behavior.

**Blast Radius:** MCP

Touches root startup, scope/provenance presentation and failure cleanup in the shared protocol/UI consumers. Keep this draft pending dependencies, required native acceptance, source-bound CI/platform status and final independent conformance/review. Publication is not an acceptance waiver, issue closure or merge authorization.

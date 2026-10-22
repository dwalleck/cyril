## Summary

```text
admitted session/new -> independent 90s off-loop response wait
  -> live ready / failure / manual OAuth notices
  -> typed active-attempt + actual-scope provenance
  -> timeout: known outcomes + explicit unknown complete inventory
  -> retire failed attempt; isolate late replies; tear down owned bridge

session/load: 30s | controls: 10s | prompt response: unbounded
```

**Draft — native authentication BLOCKED; not acceptance-complete or merge-ready.** Refs `cyril-caar`; the issue stays open.

Depends explicitly on [NK #196](https://github.com/dwalleck/cyril/pull/196), which depends on [Tqr #193](https://github.com/dwalleck/cyril/pull/193). This draft targets **main** for the repository's existing CI triggers; it does not replace the logical Tqr → NK → Caar delivery order. The cumulative main-targeted diff may include inherited changes; the [25-path Caar feature manifest](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/feature-files.json) distinguishes ownership.

The feature preserves the sole root-start admission owner, fatal fingerprint preflight, independent Load/control policy and qualified parent repairs. Early replacement progress is display-only until actual session correlation; a partial/pending inventory is never invented into a complete silent-server set. Manual sign-in delivery is not OAuth success.

## Evidence

- **Before:** Retained historical actual-request regressions expose the old 30-second New cutoff and replacement-progress/fingerprint failures. The original compiler/fixture failures and failed qualification attempts remain separately attributed in the [owning review record](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/review-decisions.md); they are not relabeled current-source runs.
  **After:** All **22 assigned current-source local gates PASS**; the workspace lane reports **3198 passed, 0 failed, 13 ignored**. Exact commands, feature lanes, rosters, ignored cases and log identities are in [qualification.json](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/qualification.json). These are not all-features, nextest, release or platform-CI claims.
- **Source binding:** Qualified 529-input descriptor `5c63c3965c6b653f5ff7cce0b00c83a79153c0e025f7929f2391e8be00a3a9ee`; private default App SHA256 `a5c6c8a7e2b7fc5bd853cc7222467217beaaca7e989d40411f18d4964dbab36a`. [Source pins](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/source-pins.json) preserve both the original build frame and Main's source-identical current held-NK boundary. The first fmt check failed at 92 hunks/10 paths; a separately authorized single normal formatter pass and fresh complete local matrix followed. No whitespace-only/token-identity claim is made.
- **Independent source review:** Two bounded integration-source reviews found no blocking source findings. Their original pre-format binding and explicit normal-formatter applicability are retained in [review-applicability.json](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/review-applicability.json), not upgraded into final conformance approval.
- **Native BLOCKED:** The latest pinned native CLI reports `error: You are not logged in, please log in with kiro-cli login` before initialize returns or `session/new` is sent. No New budget began. Latest startup-surface/navigation preconditions, previews, natural inventory and held-response New80/New95 acceptance are **not established**. Earlier actual New/present-preview observations remain preserved alongside both fixture-observation gaps; the corrected driver is reviewed but those corrected acceptance layers remain unexercised because of auth. These gaps do not establish a product defect or justify a product fix. [Native attempts](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/native-consumer.json) retain exact attempt, source, driver, predicate and owned-cleanup boundaries.

The [current proof index](https://github.com/dwalleck/cyril/blob/fix/cyril-caar-final/.cyril-caar/current-main-proof/index.json) separates published metadata projections from retained private originals; this is not a self-contained native replay archive. Deterministic actual-request tests establish the >=90s policy; live80/95 acceptance remains required. Natural inventory's independent95s observation is separate, and no independent native product timer-fire measurement or new UI timing threshold is claimed.

## Merge Danger

**Door:** Two-way

No storage migration or authentication/configuration change. Reverting this feature would also remove its startup-policy and diagnostic behavior.

**Blast Radius:** MCP

Touches root startup, scope/provenance presentation and failure cleanup in the shared protocol/UI consumers. Keep this draft pending dependencies, required native acceptance, source-bound CI/platform status and final independent conformance/review. Publication is not an acceptance waiver, issue closure or merge authorization.

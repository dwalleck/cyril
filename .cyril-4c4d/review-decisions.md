# cyril-4c4d — assembled review decisions

Active workflow: `route.md`, approved `design.md`, `plan.md` Slice 3. Main owns this decision surface.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Selected-package default/no-default checks run only on Ubuntu; run the existing lane on all supported native platforms. | ReviewAssembledKasDefault | Verified | Direct workflow inspection: `default-features` hardcodes Ubuntu, while macOS/Windows `test` uses workspace `--all-features`; those jobs cannot establish selected-package defaults or KAS exclusion. Approved D1/C6 requires isolated production-package feature policy and all-platform enforcement. | Accept | Slice 3: matrix the existing `.github/workflows/ci.yml` `default-features` job across Ubuntu, macOS and Windows, retaining its checks and the separate workspace matrix. | blocking — missing required native feature-policy proof, not a demonstrated runtime defect. |

## Compact repair record — F1

- Delivered behavior: Slice 3 now matrices the existing `default-features` job across Ubuntu, macOS and Windows with `fail-fast: false`; every prior check remains. The root cause was the single-OS owner of package-isolation checks. No product behavior, dependency, or new test machinery changed; D1/C6 and the other plan fields are unchanged.
- Performed checks/results: ReviewAssembledKasDefault independently re-reviewed the applied matrix, Cargo-only step shell compatibility and `CI Success` aggregation; no findings. A failed matrix leg still makes the required aggregate fail. Prior local source gates and consumer receipts remain valid because only runner coverage changed. Actual native execution is recorded with Slice 3's remote CI, not inferred from this static review.
- Unresolved failures: none for F1. Slice 3's successful assembled native qualification in `plan.md` records execution of every supported platform's default/no-default lane and the required aggregate; static review is no longer substituted for native proof.

## Review errors

None recorded. The empty-ID question was a source inquiry, not a defect: KAS 0.66.26 `koe` checks `!== undefined`, and `OB` matches a supplied empty ID and tags. The implementation preserves that behavior; no nonempty-ID validation is added.

## Independent review scope

ReviewAssembledKasDefault reviewed the working tree over `8f10bcfa` against `origin/main` `b9c33f02`, including the staged default change and integrated prerequisite repairs. C1–C5, C7, platform source/docs, ownership and caller migrations had no actionable findings. The subsequent scoped matrix review found no new defect and explicitly retained those conclusions for this CI-only repair.

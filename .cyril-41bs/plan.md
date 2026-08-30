# Budgeted plan: cyril-41bs

## Scope and ownership boundary

`cyril-41bs` is an architecture spike. Its issue and approved design explicitly forbid a production dependency cutover. This plan therefore has one checkpointed-build slice: the remaining route deliverable C14. C1–C13 are the approved production contract, not production work authorized in this issue.

The production contract is assigned exactly once to verified follow-on task `cyril-gl5s` (child of `cyril-1gfe`):

| Future implementation slice | Claim IDs | Pending checkpoint ownership |
|---|---|---|
| Transport ingress | C4, C6 | C4 → `.cyril-1gfe/checkpoints/C4.json` |
| Runtime API and actor topology | C1, C2, C3, C5, C7, C10, C11 | C10 → `.cyril-1gfe/checkpoints/C10.json` |
| App contract | C8 | C8 → `.cyril-1gfe/checkpoints/C8.json` |
| Memory and source contract | C9 | C9 → `.cyril-1gfe/checkpoints/C9.json` |
| Command and routing matrix | C13 | C13 → `.cyril-1gfe/checkpoints/C13.json` |
| Clean cutover | C12 | C12 → `.cyril-1gfe/checkpoints/C12.json` |

`rivets show cyril-gl5s` is the independent ownership check: its description carries this dependency order, the conductor-only clean-cutover boundary, and every C1–C13 acceptance family. `cyril-5g2o` remains the verified owner of a future bounded multi-client broadcaster; `cyril-1ixa` remains the verified pressure-risk trigger. None is a slice in this spike, and checkpointed-build must not begin their production work here.

## Partition arithmetic

The spike already contains required outputs from the route, prove-it-prototype, and falsifiable-design stages. They are included in the review-size projection even though those stages—not budgeted-plan—own them.

| Increment | Contents | Projected changed lines |
|---|---|---:|
| A — SDK component and process evidence | `route.md`; probe package manifest/lockfile; E1–E4 Rust probes and Python oracles | 3,065 |
| B — Topology, observer, memory, and version evidence | E5–E10 Rust probes and Python oracles, including live parity comparator | 2,281 |
| C — Approved decision and cleanup | `evidence.md`, `design.md`, this plan, ADR, C14 fence | 792 |
| **Slice/artifact sum** |  | **6,138** |
| **Churn margin** | **20%; ceiling of 1,227.6. Evidence is validated, but ADR/fence review may add cross-links and diagnostics.** | **1,228** |
| **Projected total** |  | **7,366** |

Because 7,366 exceeds the exact 4,000-line review-size gate, the work is partitioned into three dependency-ordered PR increments.

### Increment A — SDK component and process evidence

- **Slices:** N/A — artifacts already produced by the route/prove-it stages; no budgeted-plan slice reclaims their completion.
- **Mergeable definition:** the pinned probe package builds independently and E1–E4 plus their independent oracles pass without production-code changes.
- **Verification without later increments:** probe-workspace format, clippy, tests, and E1–E4 oracle commands; no design or ADR is required to execute them.

### Increment B — Topology, observer, memory, and version evidence

- **Slices:** N/A — artifacts already produced by prove-it-prototype; no budgeted-plan slice reclaims their completion.
- **Dependency:** Increment A supplies the pinned probe package.
- **Mergeable definition:** E5–E10, the six-cell live direct/conductor comparator, and independent oracles pass without production-code changes.
- **Verification without later increments:** probe-workspace format, clippy, tests, claim-local oracles, and authenticated live parity where credentials are available; the ADR is not required.

### Increment C — Approved decision and cleanup

- **Slices:** Slice 1, ADR and spike cleanup.
- **Dependency:** Increments A and B provide the evidence cited by the decision.
- **Mergeable definition:** the approved ADR, verified follow-on ownership, reproducible-only probe tree, and C14 fence land together. The increment contains no production migration.
- **Verification without future production work:** C14 checks the ADR link, follow-on IDs, retained artifacts, and absence of disposable/build output; the probe workspace remains green.

## Slice 1: ADR and spike cleanup

**Claim IDs:** C14

**Expected behavior:** An approved ADR selects conductor-first production topology and explicitly amends or supersedes ADR-0003; it records the `AgentProcess`/`ConnectTo<Client>` adapter, private `sdk_runtime` and `domain_mediator` placement, bounded enqueue-and-return handler discipline, stable wire v1, unchanged App/memory/source ownership, and no observer API. It records the upstream-proposal disposition: no blocking SDK gap remains because the retained `AgentProcess` adapter supplies required cwd/process semantics, so no upstream issue or PR is justified. It links verified follow-on owners `cyril-gl5s`, `cyril-5g2o`, and `cyril-1ixa`. Only reproducible probe sources, manifests, lockfile, and oracles remain; `target`, `__pycache__`, speculative production code, and disposable prototype output are absent.

**Oracle:** `route.md` T4, the approved `design.md`, primary-checkout `rivets show` results for the three cited issues, and an independent repository-path/content census in `probe.sdk2/oracles/c14.py`. The census requires an explicit upstream-proposal disposition rather than inferring one from absent proposal links.

**Stress fixture:** Two deliberate failures: remove the ADR's explicit ADR-0003 supersession/amendment link, then create `.cyril-41bs/probe.sdk2/target/stale.bin`. The fence must fail each run and report respectively the missing ADR link and exact stale path.

**Regression fence:** `.cyril-41bs/probe.sdk2/oracles/c14.py`, created in this slice. It emits `claim_ids: ["C14"]`, validates the ADR decision/link, upstream-proposal disposition, and follow-on citations; the separate primary-checkout `rivets show` command verifies those issue records. The fence requires all retained source/oracle paths, rejects build/cache/disposable paths, and reports the exact missing or stale path.

**Named mutation:** Omit the ADR-0003 supersession/amendment link; separately leave a disposable prototype/build artifact. `c14.py` must go red and name the missing/stale path for each mutation; restore each mutation before the green run.

**Complexity/production scale:** The new fence scans the explicit retained-artifact manifest plus production Rust/manifests once: $O(n)$ paths, with current scale 163 paths and an accepted maximum of 500 paths / 1 second on the repository workstation. The 500-path bound is over 3× current scale and catches accidental generated-tree traversal or an unexpectedly broad migration while remaining suitable for commit/CI use.

**Wall budget/phase:** N/A — reason: one-off checkpoint phase; no wall budget. The accepted 1-second census maximum remains a complexity bound, not an always-on wall budget.

**Files:**
- Create `docs/adr/0012-conductor-first-acp-sdk-2-runtime.md`.
- Modify `docs/adr/0003-defer-proxy-stack-for-host-callbacks.md` to mark it superseded by ADR-0012 and link forward.
- Create `.cyril-41bs/probe.sdk2/oracles/c14.py`.
- Retain `.cyril-41bs/{route.md,evidence.md,design.md,plan.md}` and reproducible `.cyril-41bs/probe.sdk2/{Cargo.toml,Cargo.lock,.gitignore,src/bin/*.rs,oracles/*.py}`.
- Remove `.cyril-41bs/probe.sdk2/target/`, `.cyril-41bs/probe.sdk2/oracles/__pycache__/`, and any other generated/disposable probe output.
- Produce checkpoint output `.cyril-41bs/checkpoints/C14.json` during checkpointed-build.
- N/A — no production Rust module, workspace manifest, ROADMAP, App/UI/memory file, or route-owned artifact is modified by this slice.

**Estimate:** 2 hours.

**Diff estimate:** 540 changed lines for the ADR, plan, C14 oracle, links, and checkpoint record. Prior-stage artifacts are budgeted separately in the partition table.

**PR increment:** Increment C — Approved decision and cleanup.

**Commands and expected results:**
- `python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → emits `claim_ids: ["C14"]`, every decision/follow-on/retained-path cell is true, no stale path is reported, and exits zero.
- Remove the ADR-0003 supersession/amendment text, run `python3 .cyril-41bs/probe.sdk2/oracles/c14.py`, then restore it → exits nonzero and reports the missing ADR link; the restored run exits zero.
- Create `.cyril-41bs/probe.sdk2/target/stale.bin`, run `python3 .cyril-41bs/probe.sdk2/oracles/c14.py`, then remove it → exits nonzero and reports that exact path; the restored run exits zero.
- `rivets show cyril-gl5s && rivets show cyril-5g2o && rivets show cyril-1ixa` from the authoritative primary checkout → each ID exists; `cyril-gl5s` owns C1–C13 conductor cutover, `cyril-5g2o` owns bounded observer fan-out, and `cyril-1ixa` owns the pressure trigger.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-cargo-target cargo fmt --all -- --check && CARGO_TARGET_DIR=/tmp/cyril-41bs-cargo-target cargo clippy --all-targets -- -D warnings && CARGO_TARGET_DIR=/tmp/cyril-41bs-cargo-target cargo test --all-targets` from `.cyril-41bs/probe.sdk2` → the retained probe package is formatted, warning-free, and all probe suites pass without creating an in-tree `target/`.
- `PYTHONPYCACHEPREFIX=/tmp/cyril-41bs-pycache python3 -m compileall -q .cyril-41bs/probe.sdk2/oracles` from the repository root → every retained oracle parses without creating in-tree `__pycache__`.
- `python3 .cyril-41bs/probe.sdk2/oracles/c14.py` after all verification commands → emits `claim_ids: ["C14"]`, reports no stale path, and exits zero, proving the final tree—not only the pre-verification tree—passes the census.

## Tracker taxonomy

- **Permanent non-goals:** no production cutover in `cyril-41bs`; no draft wire v2; no App/UI/domain rewrite; no placeholder stage crate or public runtime trait; no claim that conductor is a broadcaster. These are scope boundaries with rationale in the approved design, not deferred work.
- **Intended future work:** `cyril-gl5s` (verified child of `cyril-1gfe`) owns the conductor-first SDK 2 cutover and every C1–C13 production checkpoint. `cyril-5g2o` owns bounded multi-client observer topology. `cyril-1ixa` owns the current unbounded-pressure trigger. No uncited follow-up remains.

## Self-review

1. **Claim assignment:** PASS. C14 is assigned exactly once to the only current-change slice. C1–C13 are assigned exactly once to the named future implementation slices in verified `cyril-gl5s`; their production PENDING checkpoints are not silently claimed or started by this spike.
2. **Mandatory fields:** PASS. Slice 1 records all thirteen fields; conditional file scope uses an explicit `N/A — reason`.
3. **Fence and mutation:** PASS. C14's fence is created in Slice 1, and both named mutations are scheduled red/restore/green in that slice.
4. **Complexity and wall budget:** PASS. The only new loop is bounded by path count with an explicit 500-path/1-second maximum; the phase is one-off and has no production runtime cost.
5. **Partition:** PASS. 6,138 + 1,228 = 7,366, so three dependency-ordered increments are required; each has a mergeable definition and independent verification.
6. **Tracker taxonomy:** PASS. Every future-work item cites a verified Rivets owner; permanent non-goals remain rationale-backed.
7. **Completion ownership:** PASS. This plan declares no slice complete. Checkpointed-build exclusively judges Slice 1 and writes C14's checkpoint.

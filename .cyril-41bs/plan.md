# Budgeted plan: cyril-41bs

## Scope and ownership boundary

`cyril-41bs` is an architecture spike. Its issue and approved design explicitly forbid a production dependency cutover. The original build plan therefore gave C14 one checkpointed slice. After that slice was committed, review findings F2–F14 required appended C14 review-fix Slice 2; F1 was refuted. Final two-axis review then verified F15–F29 against already-committed evidence; the C14 findings extend Slice 2 and the probe findings append Slices 3–5 in a fourth PR increment. C1–C13 remain the approved production contract, not production work authorized in this issue.

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
| C — Approved decision, cleanup, and C14 review fix | `evidence.md`, `design.md`, this plan, review decisions, ADR, C14 fence/checkpoints | 1,090 |
| D — Final evidence review corrections | E1/E3/E4/E5/E6 probe corrections, shared live support, three review checkpoints | 650 |
| **Slice/artifact sum** |  | **7,086** |
| **Churn margin** | **20%; ceiling of 1,417.2. Live parity correction may add diagnostics but no production code.** | **1,418** |
| **Projected total** |  | **8,504** |

Because 8,504 exceeds the exact 4,000-line review-size gate, the work is partitioned into four dependency-ordered PR increments.

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

- **Slices:** Slice 1, ADR and spike cleanup; Slice 2, harden the C14 decision fence after review.
- **Dependency:** Increments A and B provide the evidence cited by the decision.
- **Mergeable definition:** the approved ADR, verified follow-on ownership, reproducible-only probe tree, and C14 fence land together. The increment contains no production migration.
- **Verification without future production work:** C14 checks the ADR link, follow-on IDs, retained artifacts, and absence of disposable/build output; the probe workspace remains green.

### Increment D — Final evidence review corrections

- **Slices:** Slice 3, harden E1/E3/E4 probe gates; Slice 4, preserve structured E6 error/cancellation/wire evidence; Slice 5, complete the composed E5/E6 live parity matrix.
- **Dependency:** Increments A and B supply the reviewed probes; Increment C supplies the review decision log and portable C14 census extended by these slices.
- **Mergeable definition:** all accepted F15–F29 corrections land as probe-only code/checkpoints, the complete direct/conductor evidence composition is explicit, and no production manifest/source changes remain.
- **Verification without future production work:** focused probe binaries/oracles and mutation fences pass; then the full probe workspace and all authenticated/offline oracles pass with external build/cache directories.

## Slice 1: ADR and spike cleanup

**Claim IDs:** C14

**Expected behavior:** An approved ADR selects conductor-first production topology and explicitly amends or supersedes ADR-0003; it records the `AgentProcess`/`ConnectTo<Client>` adapter, private `sdk_runtime` and `domain_mediator` placement, bounded enqueue-and-return handler discipline, stable wire v1, unchanged App/memory/source ownership, and no observer API. It records the upstream-proposal disposition: no blocking SDK gap remains because the retained `AgentProcess` adapter supplies required cwd/process semantics, so no upstream issue or PR is justified. It links verified follow-on owners `cyril-gl5s`, `cyril-5g2o`, and `cyril-1ixa`. Only reproducible probe sources, manifests, lockfile, and oracles remain; `target`, `__pycache__`, speculative production code, and disposable prototype output are absent.

**Oracle:** `route.md` T4, the approved `design.md`, primary-checkout `rivets show` results for the three cited issues, and an independent repository-path/content census in `probe.sdk2/oracles/c14.py`. The census requires an explicit upstream-proposal disposition rather than inferring one from absent proposal links.

**Stress fixture:** Two deliberate failures: remove the ADR's explicit ADR-0003 supersession/amendment link, then create `.cyril-41bs/probe.sdk2/target/stale.bin`. The fence must fail each run and report respectively the missing ADR link and exact stale path.

**Regression fence:** `.cyril-41bs/probe.sdk2/oracles/c14.py`, created in this slice. It emits `claim_ids: ["C14"]`, validates the ADR decision/link, upstream-proposal disposition, and follow-on citations; the separate primary-checkout `rivets show` command verifies those issue records. The fence requires all retained source/oracle paths, rejects build/cache/disposable paths, and reports the exact missing or stale path.

**Named mutation:** Omit the ADR-0003 supersession/amendment link; separately leave a disposable prototype/build artifact. `c14.py` must go red and name the missing/stale path for each mutation; restore each mutation before the green run.

**Complexity/production scale:** The new fence scans the explicit retained-artifact manifest plus production Rust/manifests once: $O(n)$ paths. Its original checkpoint visited 163 paths; final review hardening visits 169, below the accepted maximum of 500 paths / 1 second on the repository workstation. The bound catches accidental generated-tree traversal or an unexpectedly broad migration while remaining suitable for commit/CI use.

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

## Slice 2: Harden the C14 decision fence after review

**Claim IDs:** C14

**Purpose:** Resolve verified C14 review findings F2–F14 and F25/F27–F29 after Slice 1 was committed; record refuted F1 without changing behavior for it.

**Expected behavior:** C14's design lifecycle is discharged everywhere, not left PENDING. The permanent fence parses each exact P1–P10 verdict; validates exactly one complete F1–F29 review-decision row with a closed evidence state and decision; rejects any unlisted file anywhere under `.cyril-41bs`; rejects SDK2 conductor/runtime markers or any ACP dependency not inherited source-free from the pinned workspace 0.10 line across normal/dev/build, patch, and replace tables; and runs without Git-history assumptions. ADR-0012 explicitly supersedes ADR-0003's move-memory-on-proxy-activation promise.

**Oracle:** Direct comparison with the review reproductions: exact evidence and design-table cells; closed artifact and review-decision manifests; a full production Rust/manifest marker census; recursively parsed Cargo dependency tables, including target and multiline table forms; the ADR-0003 memory clause; and execution from the working source tree without a Git subprocess or baseline revision.

**Stress fixture:** Independently: flip only P1's decisive verdict to FAIL while leaving comparison-table PASS rows; add `.cyril-41bs/prototype.tmp`; add a production Rust `ConductorImpl`; remove F13's review-decision row; add a conductor dependency marker; add multiline, exact-version, path, custom-registry, and crates.io-patch ACP dependencies; change C14's status to PENDING; remove ADR-0012's memory-promise supersession; and restore one stale “pending C14” design phrase. Each mutation must make C14 red with its exact failed premise/path/marker/predicate, then restoration must return green.

**Regression fence:** The existing `.cyril-41bs/probe.sdk2/oracles/c14.py`, hardened in this slice to check exact premise and C14 lifecycle rows, a repository-artifact allowlist, complete F1–F29 review decisions, parsed production source/manifest markers, every normal/dev/build/patch/replace ACP dependency spec and source selector, the memory-promise supersession, and no Git-history dependency.

**Named mutation:** Retain C14's design mutations—remove the ADR supersession link and leave `target/stale.bin`—and all thirteen review reproductions from the Stress fixture. Every isolated mutation must make the hardened fence red; every restoration must return green.

**Complexity/production scale:** One $O(n)$ census over artifact files, production Rust sources, and member/root manifests: 169 final paths, maximum 500 paths, maximum accepted cost 1 second. The bound is almost 3× final scale and catches generated-tree traversal or unexpectedly broad migration scope.

**Wall budget/phase:** N/A — reason: one-off review-fix checkpoint phase; no wall budget. The 1-second census maximum is a complexity bound.

**Files:**
- Create `.cyril-41bs/review-decisions.md`.
- Modify `.cyril-41bs/probe.sdk2/oracles/c14.py`.
- Modify `.cyril-41bs/design.md`, `.cyril-41bs/plan.md`, and `.cyril-41bs/checkpoints/C14.json`.
- Modify `docs/adr/0012-conductor-first-acp-sdk-2-runtime.md`.
- Create `.cyril-41bs/checkpoints/C14-review-fix.json` during checkpointed-build.
- N/A — no production Rust code, manifest, App/UI/memory code, route, or empirical evidence changes.

**Estimate:** 1 hour.
**Diff estimate:** 298 changed lines.

**PR increment:** Increment C — Approved decision, cleanup, and review fix.

**Commands and expected results:**
- `python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exact P1–P10 and C14 statuses pass; F1–F29 decisions are complete; artifact and production censuses are empty; only inherited/pinned ACP 0.10 remains; C14 exits zero.
- `cp docs/adr/0012-conductor-first-acp-sdk-2-runtime.md /tmp/c14-adr.md && python3 -c 'from pathlib import Path; p=Path("docs/adr/0012-conductor-first-acp-sdk-2-runtime.md"); p.write_text(p.read_text().replace("Supersedes: [ADR-0003]", "Related: [ADR-0003]", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with `supersedes_adr_0003: false`; `mv /tmp/c14-adr.md docs/adr/0012-conductor-first-acp-sdk-2-runtime.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `mkdir -p .cyril-41bs/probe.sdk2/target && touch .cyril-41bs/probe.sdk2/target/stale.bin && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `probe.sdk2/target/stale.bin`; `rm -rf .cyril-41bs/probe.sdk2/target && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp .cyril-41bs/evidence.md /tmp/c14-evidence.md && python3 -c 'from pathlib import Path; p=Path(".cyril-41bs/evidence.md"); p.write_text(p.read_text().replace("| PASS |", "| FAIL |", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with `P1: FAIL`; `mv /tmp/c14-evidence.md .cyril-41bs/evidence.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `python3 -c 'from pathlib import Path; Path(".cyril-41bs/prototype.tmp").write_text("stale\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `prototype.tmp`; `rm .cyril-41bs/prototype.tmp && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `python3 -c 'from pathlib import Path; Path("crates/cyril-core/src/protocol/prototype_migration.rs").write_text("pub(crate) struct ConductorImpl;\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `prototype_migration.rs:ConductorImpl`; `rm crates/cyril-core/src/protocol/prototype_migration.rs && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp .cyril-41bs/review-decisions.md /tmp/c14-review-decisions.md && python3 -c 'from pathlib import Path; p=Path(".cyril-41bs/review-decisions.md"); p.write_text("\n".join(line for line in p.read_text().splitlines() if not line.startswith("| F13 |")) + "\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with `review_decisions_complete: false`; `mv /tmp/c14-review-decisions.md .cyril-41bs/review-decisions.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-core/Cargo.toml /tmp/c14-core-Cargo.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-core/Cargo.toml"); p.write_text(p.read_text() + "\nagent-client-protocol-conductor = \"0.1\"\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming the manifest and conductor marker/dependency; `mv /tmp/c14-core-Cargo.toml crates/cyril-core/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-ui/Cargo.toml /tmp/c14-ui-Cargo.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-ui/Cargo.toml"); p.write_text(p.read_text() + "\n[target.'\"'\"'cfg(any())'\"'\"'.dependencies.agent-client-protocol]\nversion = \"2.0.0\"\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `crates/cyril-ui/Cargo.toml:agent-client-protocol=2.0.0`; `mv /tmp/c14-ui-Cargo.toml crates/cyril-ui/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-ui/Cargo.toml /tmp/c14-ui-exact.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-ui/Cargo.toml"); p.write_text(p.read_text().replace("[dependencies]\n", "[dependencies]\nagent-client-protocol = { version = \"=2.0.0\" }\n", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `agent-client-protocol==2.0.0`; `mv /tmp/c14-ui-exact.toml crates/cyril-ui/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-ui/Cargo.toml /tmp/c14-ui-path.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-ui/Cargo.toml"); p.write_text(p.read_text().replace("[dependencies]\n", "[dependencies]\nagent-client-protocol = { path = \"../../../sdk2-source\" }\n", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `agent-client-protocol=path:../../../sdk2-source`; `mv /tmp/c14-ui-path.toml crates/cyril-ui/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-ui/Cargo.toml /tmp/c14-ui-registry.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-ui/Cargo.toml"); p.write_text(p.read_text().replace("[dependencies]\n", "[dependencies]\nagent-client-protocol = { version = \"0.10\", registry = \"private-registry\" }\n", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming the member manifest and ACP dependency; `mv /tmp/c14-ui-registry.toml crates/cyril-ui/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp crates/cyril-ui/Cargo.toml /tmp/c14-ui-patch.toml && python3 -c 'from pathlib import Path; p=Path("crates/cyril-ui/Cargo.toml"); p.write_text(p.read_text() + "\n[patch.crates-io]\nagent-client-protocol = { git = \"https://example.invalid/sdk2.git\" }\n")' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `agent-client-protocol=git:https://example.invalid/sdk2.git`; `mv /tmp/c14-ui-patch.toml crates/cyril-ui/Cargo.toml && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp .cyril-41bs/design.md /tmp/c14-design.md && python3 -c 'from pathlib import Path; p=Path(".cyril-41bs/design.md"); p.write_text(p.read_text().replace("| <5 seconds | PASS —", "| <5 seconds | PENDING —", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with a PENDING `c14_design_status`; `mv /tmp/c14-design.md .cyril-41bs/design.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp docs/adr/0012-conductor-first-acp-sdk-2-runtime.md /tmp/c14-adr-memory.md && python3 -c 'from pathlib import Path; p=Path("docs/adr/0012-conductor-first-acp-sdk-2-runtime.md"); p.write_text(p.read_text().replace("promise to move persistent-memory adapters", "historical note about persistent-memory adapters", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with `supersedes_memory_move_promise: false`; `mv /tmp/c14-adr-memory.md docs/adr/0012-conductor-first-acp-sdk-2-runtime.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `cp .cyril-41bs/design.md /tmp/c14-design-pending.md && python3 -c 'from pathlib import Path; p=Path(".cyril-41bs/design.md"); p.write_text(p.read_text().replace("C14 is discharged", "C14 owns the pending ADR/cleanup checkpoint and is not discharged", 1))' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero with `design_c14_discharged: false`; `mv /tmp/c14-design-pending.md .cyril-41bs/design.md && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.
- `PYTHONPYCACHEPREFIX=/tmp/cyril-41bs-pycache python3 -m compileall -q .cyril-41bs/probe.sdk2/oracles` → all oracles parse without in-tree cache output.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e6_live_parity.py && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → all six authenticated live direct/conductor cells match and the final tree remains clean.

## Slice 3: Harden E1, E3, and E4 probe gates

**Claim IDs:** C1, C5, C6

**Purpose:** Resolve verified findings F15, F19, F22, and F23 in the already-committed component/process evidence.

**Expected behavior:** E1's positive actor handoff returns runtime errors instead of panicking while the `Rc` negative control still fails to compile. E3 requires untyped-first containment, requires both reversed/removed mutations to drop the unknown update, treats only an elapsed responsiveness timeout as the intended slow-handler result, and returns an error for a closed/unexpected event channel. E4 returns a contextual error when the nominal nonzero helper unexpectedly succeeds.

**Oracle:** `oracles/e1.py` compiles/runs the successful bounded handoff, compile-failing `Rc` move, and a closed-receiver runtime-failure control; the last must return contextual `Result` diagnostics without panic. `oracles/e3.py` executes E3, independently validates every `CaseResult`/responsiveness field, and runs wrong-expectation/closed-channel/unexpected-event modes. `oracles/e4.py` executes E4, independently validates process fields, and runs unexpected-success mode.

**Stress fixture:** E1's runtime control drops the receiver before `send` and must exit nonzero with channel context but no panic. E3 mode `wrong-containment-expectation` fails the exact event predicate; `closed-channel-control` and `unexpected-event-control` must return distinct infrastructure diagnostics. E4 mode `unexpected-success-control` runs the nominal nonzero helper with exit 0 and must return unexpected-success error.

**Regression fence:** `oracles/e1.py`, strengthened `e3` plus `oracles/e3.py`, and strengthened `e4` plus `oracles/e4.py`; Python owns expected fields/negative diagnostics rather than trusting probe booleans.

**Named mutation:** N/A — the design-row production mutations (capture `Rc<dyn Engine>` in future `protocol/sdk_runtime.rs`, await domain/host work in future SDK handlers, replace `AgentProcess`/omit production `.current_dir(cwd)`) are forbidden by this spike and remain assigned to verified `cyril-gl5s`. This review overlay runs exact controls: C1 compile-failing `Rc` move plus closed-receiver runtime error; C5 `typed-first-mutation`, `untyped-removed-mutation`, `wrong-containment-expectation`, `closed-channel-control`, and `unexpected-event-control`; C6 `unexpected-success-control`.

**Complexity/production scale:** N/A — probe-only bounded channels/processes. E3 uses one 100 ms negative-control timeout; E4 retains its existing bounded helper deadlines.

**Wall budget/phase:** N/A — one-off review-fix checkpoint; no production phase budget.

**Files:**
- Modify `.cyril-41bs/probe.sdk2/oracles/e1.py`, `oracles/e3.py`, and `oracles/e4.py`.
- Modify `.cyril-41bs/probe.sdk2/src/bin/e3.rs` and `src/bin/e4.rs`.
- Create `.cyril-41bs/checkpoints/C1-C5-C6-review-fix.json`.
- Modify `.cyril-41bs/probe.sdk2/oracles/c14.py`, `.cyril-41bs/review-decisions.md`, and this plan for retained-artifact/review traceability.
- N/A — no production file or manifest changes.

**Estimate:** 1 hour.

**Diff estimate:** 120 changed lines.

**PR increment:** Increment D — Final evidence review corrections.

**Commands and expected results:**
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e1.py` → positive `Result` control compiles/runs; exact `Rc` mutation fails with Send evidence; closed-receiver control exits nonzero with contextual channel error and no panic.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e3` → exact contained/reversed/removed/slow-handler contract exits zero.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e3 -- wrong-containment-expectation` → exits nonzero naming containment mismatch.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e3 -- closed-channel-control` → exits nonzero naming closed responsiveness channel.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e3 -- unexpected-event-control` → exits nonzero naming the unexpected responsiveness event.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e3.py` → independently validates the green output and all three negative modes.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e4` → process parity fields pass without panic.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e4 -- unexpected-success-control` → exits nonzero naming unexpected helper success.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e4.py` → independently validates green output and the negative mode.

## Slice 4: Preserve structured E6 error evidence

**Claim IDs:** C2, C5

**Purpose:** Resolve verified findings F18, F20, and F21 in the already-committed offline conductor probe.

**Expected behavior:** Every conductor case requires the exact cancellation event; timeout and channel closure are contextual errors. Every direction-matching wire-log entry parses successfully before request/response identity lookup. Terminal failure preservation compares `ErrorCode::InternalError` and exact JSON data, never debug wording.

**Oracle:** SDK `Error` public `code`/`data`; fake-agent cancellation sender; every inspected wire entry; and E6 zero/no-op/transform/distinct/repeated/failure output. `oracles/e6.py` executes E6, independently parses these fields, runs six negative-control modes, then runs pinned upstream conductor tests.

**Stress fixture:** Exact E6 modes `wrong-cancellation-event`, `cancellation-timeout`, `cancellation-channel-closed`, `malformed-wire-entry`, `wrong-terminal-data`, and `wrong-response-id` must return nonzero with their named event/timeout/closure/parse/typed-data/identity diagnostics.

**Regression fence:** The strengthened `e6` binary plus `oracles/e6.py`; Python independently owns expected structured fields and all failure diagnostics.

**Named mutation:** N/A — design-row production mutations (add a direct zero-stage bypass/reverse future stage vector; await domain/host work in future SDK handlers) are forbidden here and remain assigned to `cyril-gl5s`. This review overlay runs exact evidence modes: C2 `wrong-response-id`; C5 `wrong-cancellation-event`, `cancellation-timeout`, `cancellation-channel-closed`, and `malformed-wire-entry`; F18 `wrong-terminal-data`; default distinct/repeated stages retain ordered forwarding.

**Complexity/production scale:** $O(n)$ parse over at most 1,000 wire entries with maximum accepted local parse cost 100 ms, plus a 2-second cancellation wait. Current cases emit tens of entries; the >25× entry headroom catches accidental unbounded capture while keeping mutation feedback local.

**Wall budget/phase:** N/A — one-off probe checkpoint; no production phase budget.

**Files:**
- Modify `.cyril-41bs/probe.sdk2/src/bin/e6.rs`.
- Modify `.cyril-41bs/probe.sdk2/oracles/e6.py`.
- Create `.cyril-41bs/checkpoints/C2-C5-review-fix.json`.
- Modify `.cyril-41bs/probe.sdk2/oracles/c14.py`, `.cyril-41bs/review-decisions.md`, and this plan for retained-artifact/review traceability.
- N/A — no production file or manifest changes.

**Estimate:** 1 hour.

**Diff estimate:** 120 changed lines.

**PR increment:** Increment D — Final evidence review corrections.

**Commands and expected results:**
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6` → exact cancellation/identity/failure/ordering fields pass.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- wrong-cancellation-event` → exits nonzero naming the unexpected event.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- cancellation-timeout` → exits nonzero naming the elapsed cancellation wait.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- cancellation-channel-closed` → exits nonzero naming the closed cancellation channel.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- malformed-wire-entry` → exits nonzero with direction/index parse context.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- wrong-terminal-data` → exits nonzero naming structured error mismatch.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6 -- wrong-response-id` → exits nonzero naming response identity.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e6.py` → independently validates default output, all six negative modes, and five pinned upstream tests.

**Claim IDs:** C2, C7

**Purpose:** Resolve verified findings F16, F17, F24, and F26 without claiming nondeterministic or capture-backed callbacks were authenticated-live.

**Expected behavior:** One private probe module owns event logging, credential loading, callback results, engine capability advertisement, the independent 18-request/2-notification host matrix, and normalization. E5 emits two distinct evidence layers: authenticated stable-v1 lifecycle evidence for v2/KAS, and an exhaustive deterministic direct SDK callback matrix. KAS's authenticated prompt must exercise auth/filesystem/terminal/permission/hooks, permission before terminal creation, and structural `turn_end` before the prompt response; v2 advertises no KAS host adapters and records unelicited host callbacks as a named N/A. Authenticated cells explicitly name typed-error, outer-response-ID, and cancellation evidence as not exercised. E6 emits the same authenticated layer for all six engine × zero/no-op/transform cells and runs the exhaustive callback matrix through direct plus all three conductor topologies. The Python comparator independently owns expected methods and exact cell cardinality, validates actual observed wire response IDs and typed error data, requires declared transform markers, and exact-compares the first occurrence of stable lifecycle milestones; complete event streams retain nondeterministic backend/model progress and repeated tool executions without misattributing them to conductor topology.

**Oracle:** Authenticated `kiro-cli` v2/KAS runs; deterministic SDK `Client`/dynamic callback agent in direct and conductor topologies; independent Python method/family tables; `oracles/e7.py` as the approved independent C7 proxy-leverage oracle; current captures only for versioned extension references, never as callback parity substitutes. Stable v1, nonempty session, stop reason, KAS host-family/permission/`turn_end` ordering, exact typed errors, EOF/crash disposition, actual wire response identity, one-hop cancellation, and declared transform markers remain distinct evidence cells rather than plausible live defaults.

**Stress fixture:** Comparator self-tests remove one callback from one topology, reorder terminal events, remove every agent-message chunk, alter typed error data, alter outer identity, alter cancellation count, and remove/add a transform marker. Each must produce a named divergence and nonzero result. A source census must find shared event/auth/callback helpers only in `src/live_support.rs`.

**Regression fence:** New shared `src/live_support.rs`; focused E5/E6 matrix modes; `oracles/e5.py`; approved independent `oracles/e7.py`; `oracles/e6_live_parity.py --self-test`; authenticated `oracles/e6_live_parity.py`; and C14 retained/source census.

**Named mutation:** N/A — design-row production mutations (add a direct bypass/reverse future stages; move auth/terminal effects into a production proxy/drop a production callback) are forbidden here and remain assigned to `cyril-gl5s`. This review overlay runs `oracles/e6_live_parity.py --self-test` fixtures `missing_callback`, `terminal_order`, `missing_agent_message`, `typed_error_data`, `outer_response_id`, `cancellation_count`, `transform_marker`, and `transform_marker_extra`; F24 exact source mutation prepends `struct Events;` to `src/bin/e5.rs` and C14 must report `src/bin/e5.rs:Events`.

**Complexity/production scale:** Deterministic callback matrices are fixed at 18 requests + 2 notifications across direct and three conductor topologies because that is the route/covenant host contract. Authenticated work is fixed at 2 direct + 6 conductor sessions, each bounded to 60 seconds. Comparator work is $O(e+c)$ over at most 1,000 events per cell—over 20× current traces, chosen to expose runaway capture without constraining valid streaming.

**Wall budget/phase:** 10 minutes — one-off authenticated external phase. Eight 60-second session bounds leave 2 minutes for process startup/comparison; on breach, record exact engine/topology and last terminal event, stop, and fail without silent skip.

**Files:**
- Create `.cyril-41bs/probe.sdk2/src/live_support.rs`.
- Modify `src/bin/e5.rs`, `src/bin/e6.rs`, and `src/bin/e6_live.rs`.
- Modify `oracles/e5.py`, `oracles/e6_live_parity.py`, and `oracles/c14.py`.
- Create `.cyril-41bs/checkpoints/C2-C7-live-review-fix.json`.
- Modify `.cyril-41bs/review-decisions.md` and this plan.
- N/A — no production file, workspace manifest, App, UI, memory, or source-observer changes.

**Estimate:** 3 hours.

**Diff estimate:** 410 changed lines.

**PR increment:** Increment D — Final evidence review corrections.

**Commands and expected results:**
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e5 -- matrix` → exactly 18 callback requests and 2 notifications cross the direct SDK client with all request IDs answered.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target cargo run --quiet --manifest-path .cyril-41bs/probe.sdk2/Cargo.toml --bin e6_live -- matrix` → the same 20 callbacks cross zero/no-op/transform conductor topologies with exact equality and only declared transform markers.
- `python3 .cyril-41bs/probe.sdk2/oracles/e5.py` → independent method tables/captures are valid and evidence layers are explicitly classified.
- `python3 .cyril-41bs/probe.sdk2/oracles/e6_live_parity.py --self-test` → all eight corrupted contract fixtures are rejected with exact divergences; exits zero only because every negative control was caught.
- `python3 .cyril-41bs/probe.sdk2/oracles/e7.py` → independent bidirectional transformation/order/identity/ownership checks pass.
- `CARGO_TARGET_DIR=/tmp/cyril-41bs-final-target python3 .cyril-41bs/probe.sdk2/oracles/e6_live_parity.py` → v2/KAS direct baselines and all six conductor cells complete; composed callback/lifecycle/order/error/identity/cancellation contracts match or only predeclared engine-live N/A divergences remain.
- `cp .cyril-41bs/probe.sdk2/src/bin/e5.rs /tmp/cyril-e5.rs && python3 -c 'from pathlib import Path; p=Path(".cyril-41bs/probe.sdk2/src/bin/e5.rs"); p.write_text("struct Events;\\n" + p.read_text())' && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits nonzero naming `src/bin/e5.rs:Events`; `mv /tmp/cyril-e5.rs .cyril-41bs/probe.sdk2/src/bin/e5.rs && python3 .cyril-41bs/probe.sdk2/oracles/c14.py` → exits zero.

## Tracker taxonomy

- **Permanent non-goals:** no production cutover in `cyril-41bs`; no draft wire v2; no App/UI/domain rewrite; no placeholder stage crate or public runtime trait; no claim that conductor is a broadcaster. These are scope boundaries with rationale in the approved design, not deferred work.
- **Intended future work:** `cyril-gl5s` (verified child of `cyril-1gfe`) owns the conductor-first SDK 2 cutover and every C1–C13 production checkpoint. `cyril-5g2o` owns bounded multi-client observer topology. `cyril-1ixa` owns the current unbounded-pressure trigger. No uncited follow-up remains.

## Self-review

1. **Claim assignment:** PASS. C1–C13 remain assigned exactly once to future production slices in verified `cyril-gl5s`; Slices 3–5 repeat C1/C2/C5/C6/C7 only as required review-fix overlays on already-committed spike evidence and claim no production completion. C14 belongs to original Slice 1 with Slice 2 as its committed review fix.
2. **Mandatory fields:** PASS. Slices 1–5 each record all thirteen mandatory fields; conditional scope uses explicit `N/A — reason`.
3. **Fence and mutation:** PASS. Slices 1–2 own the C14 mutations; Slice 3 fences actor/handler/process failure modes; Slice 4 fences structured conductor evidence; Slice 5 independently corrupts every composed parity dimension and the shared-helper census.
4. **Complexity and wall budget:** PASS. Final C14 census is projected at 169 paths under 500/1 second; Slice 4 caps 1,000 wire entries; Slice 5 fixes matrix/event sizes and has a 10-minute breach policy. No production runtime cost is introduced.
5. **Partition:** PASS. 7,086 + 1,418 = 8,504, so four dependency-ordered increments are required; each has a mergeable definition and independent verification.
6. **Tracker taxonomy:** PASS. Every future-work item cites a verified Rivets owner; permanent non-goals remain rationale-backed.
7. **Completion ownership:** PASS. This plan does not judge Slices 3–5 complete; checkpointed-build exclusively records their gates.

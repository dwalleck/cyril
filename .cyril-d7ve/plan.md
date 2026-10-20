# cyril-d7ve — atomic shared-resolver migration

Design approval: design.md §Approval. One independently reviewable delivery increment and one atomic production slice; no feature budget or hard size limit supplied. cyril-0ssb remains an independent issue/commit/PR with disjoint production ownership in discovery.rs.

## Slice 1: validated profile root across every consumer

**Outcome:** design C1–C5. Settings (including workflow/goal gates), global agent trust persistence, host hook registration and sidecar root agree with Kiro's validated KIRO_HOME policy, including symlink aliases of home/root, without reading or mutating real user state in verification.

**Changes:** Deepen the existing kiro_agent_config resolver owner with an explicit-input crate-private seam and private lexical/nearest-existing-ancestor validation. Retain ambient kiro_home_dir wrapper, migrate global_agents_dir and protected-parent mediator wiring, delete settings' duplicate policy and feed selected environment inputs into the shared resolver. Keep sidecar's existing wrapper caller. Preserve workspace precedence, missing-home behavior, Replacement isolation and all data/store roots. Add colocated temporary-filesystem resolver regressions, selected-environment capabilities regressions and a reentered-child actual trust/mediator regression with an assertion receipt. No exported external interface or dependency changes. LSP references were attempted first but returned an unexplained empty set even for known sidecar caller; the discrepancy was reported to tool QA and explicit source callers enumerated in design.md are the migration basis.

**Verification:**
- Parent focused resolver/settings/mediator behavior tests → valid trimmed root selected; relative/root/home/ancestor/symlink aliases rejected with reasons; nearest-existing-ancestor and lexical accepted-spelling semantics preserved; no environment mutation.
- Parent controlled actual-defect RED/GREEN → restore pre-fix settings/global-agent/hook callsites under safe restoration and require the discriminating consumer tests to fail before restoring passing code. Source helper tests additionally exercise warning positive controls and relevant Windows native-path boundaries.
- Parent current-source consumer and real CLI environment smoke in owned temporary HOME/data/runtime/cwd → agreed settings/trust/hook roots, no real credentials or state changes. Source-bound audit facts remain linked in design.md and 0ssb evidence.md; distinguish native runs from cross-host fixtures.
- Parent required Rust feature-matrix tests/clippy/format/builds and independent code review → no unresolved defect; existing README behavior updated only after smoke; dedicated d7ve commit/PR.

**Dependencies:** No unresolved design/premise remains. Worker is prohibited from build/test/lint/formatter and changed-source smoke execution; parent owns serialized checkpoint evidence and publication. The worker hands off edits, not a fabricated PASS.

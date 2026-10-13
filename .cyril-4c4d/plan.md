# cyril-4c4d — implementation plan

Consumes approved `design.md` D1–D5 and `spec.md`. Main owns integration/checkpoint judgments and records evidence here; implementation agents do not run concurrent build/test/lint/formatter commands. No feature size budget was supplied. Upstream is `origin/main`.

## Slice 1: enforce Host hook tag selectors — cyril-5nep

**Outcome:** C3: requested tag-selected hooks are served/executed, ID matching remains compatible, malformed tag input is diagnosed rather than weakened.

**Changes:** Independent `fix/cyril-5nep-hook-tags` worktree. Modify the existing core Host callback parser/dispatch and registry only, plus every affected caller/test. Follow design D3 and the placement ledger; compile selectors at load time with existing regex, preserve unsupported matchers as nonmatching, do not change execution authority. After smoke, update existing hook documentation and the repair's tracker evidence on this branch.

**Verification:**
- First add a baseline-compatible callback/registry reproducer using toolId execute_bash and tags shell/@builtin; Main runs it before production edits and observes the omitted hook (red).
- Run the corrected reproducer plus design C3's boundary matrix and existing hook lifecycle tests.
- Run a live tag hook in an owned temporary workspace: observed marker execution and exit-2 denial preventing a disposable write.
- Run cargo test, clippy with warnings denied, fmt check and doctests at applicable feature scope; independent review before the atomic commit.

## Slice 2: dispatch advertised KAS commands — cyril-oieu

**Outcome:** C4–C5: supported advertised commands produce a real ACP prompt turn, preserve raw invocation and local context policy, and cannot fall into V2 extension RPCs.

**Changes:** Independent `fix/cyril-oieu-kas-commands` worktree. Core engine conversion classifies supported custom-agent metadata into a typed route carried by CommandInfo/registry; typed prompt intent reaches App's existing prompt lifecycle. Atomically migrate changed constructors/matches/callers. Keep builtin collisions, exact workflow suppression, V2 options/execute and unknown slash behavior. D4's command prompts carry no automatic lesson/file text and leave local first-prompt lessons pending. No PromptEnvelope resource-format redesign. After smoke, document command/context behavior in existing docs and update this repair's tracker evidence.

**Verification:**
- Add a baseline-compatible App/conversion reproducer and observe wrong outbound command before repair; if the new typed interface prevents a baseline-compatible test, Main uses the real pre-fix TUI invocation instead, not compilation failure as red evidence.
- Cover C4/C5 inputs and positive controls through the real App-to-bridge seam, with existing context lifecycle regression coverage retained.
- Invoke a real currently advertised non-workflow KAS command in Cyril and observe a completed prompt turn, not only an advertisement or mock echo.
- Run cargo test and clippy default and kas feature scopes, fmt and doctests; independent review before the atomic commit.

## Slice 3: integrate prerequisites and make KAS default — cyril-4c4d

**Outcome:** C1–C2, C6–C7: normal production build/config drives KAS, V2/no-default remains usable, first-run errors stay actionable, native-memory defaults stay unchanged, docs/CI accurately describe the policy.

**Changes:** `feat/cyril-4c4d-default-kas`. Integrate accepted platform changes from PR #167 and the two independently-qualified repairs; do not take over or close another session's work. Enable the binary kas default, change the existing AgentEngine default feature-aware, migrate V2-specific fixture/example assumptions, and correct relevant diagnostics. Extend existing CI default-features lane to selected-package default/no-default builds/tests. After consumer proof, amend ADR-0002 and current README, agent guidance, CONTEXT/default-engine references and ROADMAP/parity documentation. This plan owns assembled evidence; branch summaries reference it rather than duplicating changing results.

**Verification:**
- Selected-package default and no-default builds plus focused config/CLI precedence tests.
- Real TUI default KAS response, explicit CLI V2 response, config V2 with CLI override, and no-default V2 response.
- Real isolated missing-bundle TUI message with searched path and extraction/override remedy.
- Preserve C3–C5 coverage after integration and run workspace repository gates at their required scope, plus isolated no-default tests/lints.
- Retain only provenance-valid PR #167 platform evidence; obtain affected native CI evidence where required. Independent assembled review covers approved behavior, placement, security and feature isolation.

**Dependencies:** Slices 1 and 2; externally-owned cyril-igrx/PR #167 for completion and target claim. Pure default-policy code preparation does not imply that prerequisite acceptance is complete. Publication/merge requires scoped user authorization and is not inferred from design approval.

## Checkpoint records

### Slice 1

- Delivered behavior: D3/C3 implemented in `fed31186`, updated to upstream `b9c33f02` in `f3d19b24`, and integrated by `bafa552a`. Host parser/dispatch/registry preserve authority and non-tool triggers. No design deviation.
- Performed checks/results: the cyril-5nep tracker note dated 2026-10-13 owns the baseline-red, corrected local gates, real tag-hook allow/exit-2-deny proof and independent review checkpoint. Post-upstream `cargo test -p cyril-core -p cyril --features kas` and warnings-denied Clippy passed. [PR #168](https://github.com/dwalleck/cyril/pull/168) [CI run 38382652431](https://github.com/dwalleck/cyril/actions/runs/38382652431) passed, including Linux/macOS/Windows tests, Linux/Windows release builds and the required aggregate.
- Unresolved failures: none for this slice. Final default-policy assembly is judged separately below.

### Slice 2

- Delivered behavior: D4/C4–C5 implemented in `6a9037bd`, updated to upstream in `409bd938`, and integrated by `8f10bcfa`, with atomic route/constructor migrations and the approved local-context exception. No design deviation.
- Performed checks/results: the cyril-oieu tracker note dated 2026-10-13 owns before/after real-TUI command proof, feature/no-feature local gates and independent review. Fresh post-upstream compilation passed 1425 tests and warnings-denied Clippy. [PR #169](https://github.com/dwalleck/cyril/pull/169) [CI run 38383707626](https://github.com/dwalleck/cyril/actions/runs/38383707626) passed, including all native test platforms and the required aggregate.
- Unresolved failures: none for this slice. Final default-policy assembly is judged separately below.

### Slice 3 / final assembled state

- Delivered behavior: binary-default KAS with feature-aware core defaults; explicit CLI/config V2 and isolated no-default V2 retained; existing actionable Free-spawn errors retained; both qualified repairs and PR #167 integrated. Current docs/ADR/parity guidance describe credential custody, all-platform roots and unchanged native-memory policy. Review finding F1 extends selected-package CI coverage to every supported native platform; its decision and scoped re-review are owned by `review-decisions.md`.
- Performed checks/results: Linux/Rust 1.94.0, source over `8f10bcfa` plus the default-policy changes in the behavior-owning commit. `cargo fmt --all --check`, selected-package default/no-default binary builds, `cargo test --workspace`, workspace default Clippy, `cargo nextest run --workspace --all-features` (2307 passed, 13 existing skips), all-feature doctests (2 passed), all-feature Clippy, selected core/binary no-default nextest (1092 passed, 5 existing skips), and selected core/binary no-default Clippy all passed with warnings denied. Both the advertised-command and hook-selector regression names were observed in the fresh assembled run. Ordinary independent assembled review found only F1; the applied CI repair received a clean scoped re-review, retaining the clean behavior/ownership/interface/security conclusions. Upstream was fetched again before publication and remained `b9c33f02`.
- Unresolved failures: no local product, consumer, lint/test or review defect remains. The assembled draft's native CI—including all three isolated default/no-default legs added for F1—must pass before completion. Prerequisite CI is not substituted for that proof. Draft publication is authorized; merge and issue closure are not.

#### Assembled consumer receipts

Real Cyril TUI in Hex1b terminal `term-777a6bf`, Linux, installed kiro-cli 2.29.1. Successful prompts requested uppercase forms of lowercase sentinels, so displayed uppercase responses were not input echoes. Config isolation used disposable `HOME` directories; `XDG_DATA_HOME` pointed at the existing Kiro data root except in the missing-bundle case. The no-config case's `config.toml` absence was checked.

| Scenario | Observed result |
|---|---|
| Default binary, no engine flag or config | KAS session `sess_a4542705-0977-4dee-af32-27b811b44ab1`; response `CYRIL_FINAL_DEFAULT_OK`, then idle input. |
| Advertised `/general-task-execution` in that session | Response `CYRIL_FINAL_COMMAND_OK`, not a V2 command error; idle input returned. |
| Host `shell` tag policy in that session | `assembled-shell-tag-proof` ran and wrote its marker. Exit 0 plus explicit Allow produced `assembled-allowed.txt` containing `CYRIL_FINAL_ALLOWED`; the next hook exited 2, UI reported `CYRIL_ASSEMBLED_POLICY_DENIED`, and `assembled-denied.txt` was absent. |
| Default binary with `--agent-engine v2` | V2 session `9d6386fb-a72d-40bc-950a-d352e06652b2`; response `CYRIL_FINAL_EXPLICIT_V2_OK`. `/powers` advised selecting KAS rather than rebuilding an already KAS-capable binary. |
| Default binary with `[agent] engine = "v2"` | V2 session `160a1216-b9a7-4727-a042-3eba7c0dfa78`; response `CYRIL_FINAL_CONFIG_V2_OK`. |
| Same V2 config plus `--agent-engine v3` | KAS session `sess_6ac1dd07-3183-4c9d-9f4d-24d6bf2c2d30`; response `CYRIL_FINAL_OVERRIDE_KAS_OK`, proving CLI precedence and the alias. |
| Isolated `--no-default-features` binary, no selector/config | V2 session `4ca39b4e-721d-4008-993d-ef6c16c3d617`; response `CYRIL_FINAL_NO_DEFAULT_V2_OK`. |
| Default binary, empty data root, server-path override unset | No session; UI named the missing `$XDG_DATA_HOME/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js`, prescribed `kiro-cli acp --agent-engine v3` or `KIRO_KAS_SERVER_PATH`, and did not fall back. |

The exercised default binary SHA256 was `c573baab9be2af32225386e905f3eda52e90e0bf59bbd941fb40cf1007414366`; the no-default binary was `27da1aefe62ad9543832ee25e738d706a99a432452ccc004c847f7f805a959d6`. Subsequent F1 changes affect CI coverage only, not either binary's production source.

#### Evidence validity corrections

- An initial `XDG_CONFIG_HOME` fixture did not isolate Cyril config: Cyril reads `$HOME/.config/cyril`. That run is not config-selection proof. Final config/no-config receipts above use the correct boundary.
- Shared Cargo targets reused hook-worktree test binaries during two attempted command-worktree requalifications despite correct cwd/manifest selection. Those results were discarded. Clearing the workspace-crate artifacts forced fresh command compilation, then artifacts were cleared again before assembled compilation; expected command and hook regression identities were observed. No stale green result is used above.
- Native CI qualifies compilation/tests and the existing platform probes, not a live authenticated macOS/Windows KAS turn. PR #167's retained Apple path evidence and the existing Windows correction remain the platform-discovery provenance.


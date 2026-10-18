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
- Initial native qualification: [PR #170 CI run 38388968155](https://github.com/dwalleck/cyril/actions/runs/38388968155), behavior commit `1a718f36`, passed Linux/macOS tests and isolated feature lanes but exposed Windows platform-guard lint failures and four standalone-memory readiness failures. The required aggregate correctly failed. The repair and successful qualification checkpoints below supersede that failed gate without discarding its evidence.

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

#### Native qualification repair round

- Windows lint repair: gate only Unix-owned imports, executable fixtures and recording-process helpers; retain the portable recording-reader test on Windows. The host-file builder uses mutable shadowing only on Unix, preserving permission and persist behavior. Linux affected core/memory tests and workspace all-feature/all-target warnings-denied Clippy passed. Independent scoped review found no defect or coverage reduction.
- Readiness attribution: the same standalone companion tests passed the selected-binary Windows nextest step in the failed run; neither KAS selection nor either prerequisite changes the companion/IPC implementation. The failing helper discarded all connect/health errors and child state. It now records phase counts, elapsed time and the last observation, and reports exited children or terminal Failed health explicitly. Ready-only success, the five-second startup deadline, 250 ms attempts and 20 ms polling remain unchanged. All eight Linux real-process memory-runtime tests and all-feature Clippy passed; independent scoped re-review was clean. This is diagnostic improvement, **not** a claimed readiness fix.
- Native diagnostic access: the local Windows VM is running, but the existing WinRM credentials were rejected with HTTP 401 using both Basic and NTLM. No account or host-policy changes were attempted. Qualification instead used published native CI; no local Windows execution is claimed.

#### Successful assembled native qualification

- Qualified revision: `a813eed99d92a8dbd2e2c81f4391b0d535edca7c`, tested as PR merge `94778f46e2b072d7806f6509102f10c9a074bc39` against `b9c33f02`. [CI run 38393249904](https://github.com/dwalleck/cyril/actions/runs/38393249904) passed every job and the required `CI Success` aggregate.
- Linux/macOS/Windows workspace tests and default-feature lanes passed. The Windows feature lane executed every default and no-KAS build/test/Clippy step successfully, rather than skipping later steps. Linux/Windows release builds, KAS checks, formatting, lint, commit validation, worktree tooling and all native code-review-tooling lanes also passed.
- Windows all-feature nextest ran 2189 tests successfully with 13 existing skips. All seven standalone-memory integration tests passed, including the four that previously missed readiness; the portable `c4_recording_reader_preserves_segmented_batch_malformed_and_numeric_bytes` test also ran and passed.
- Residual observation: the earlier Windows readiness failures did not recur with unchanged deadlines and unchanged production memory/IPC code. Their cause is not established or claimed fixed; the improved helper retains actionable failure context if they recur. This is separate from the now-repaired cfg lint failures and completed default-engine qualification.
- All required consumer, repository/native and independent-review gates pass for this source. A subsequent record-only commit does not change production, tests or workflow; this pinned source evidence remains valid. Delivery is three authorized draft PRs, not merges or issue closures.


## Update 2026-10-18 — merge main with landed #172–#176

Main at `d399a340` now carries every remaining default-switch prerequisite: #172 (cyril-k8wc, `shell_type` = executed shell's name; `host_shell.rs` rewrite), #173 (cyril-zbu7, KAS token via `kiro-cli chat _ get-kas-token`; `kas/auth.rs`, `kas/stored_token.rs`, `kas/discovery.rs`, `bridge.rs`, `kas/version.rs`, `domain_mediator/host.rs`), #174 (cyril-auxt, always-allow consent echo), #175/#176 (cyril-q0kg / cyril-rcub, approval overlay) and ADR-0013 (docs).

Method (same as the earlier merge `1e66d802`): result tree = `origin/main` + this branch's net own diff (`1e66d802^2..1e66d802`, which contained only `1a718f36`/`a813eed9`/`b4e793f6` content — no foreign files). Main's reviewed prerequisite code wins; this branch's intent is re-expressed on top.

| File | Main (prerequisite) did | This branch did | Resolution |
|---|---|---|---|
| `crates/cyril-core/Cargo.toml` (`kas` feature comment; textual conflict) | #173 reworded the gate to "KAS host code, including the `getAccessToken` responder that relays `get-kas-token`"; dropped `rusqlite` from the feature comment | Reworded to "opt-in for this library; production `cyril` enables it by default" | Combined: opt-in library / default-on binary (4c4d) + main's responder wording (zbu7); main's dependency list kept. |
| `crates/cyril-core/src/protocol/kas/host_shell.rs` (two textual conflicts) + `terminal_io.rs` (applied cleanly) | #172 rewrote `ShellKind` (`Posix(PosixShell)`), made `test_posix_at` classify by basename (new doc comment), added `PosixShell` to the test imports, and made the **fish** fixture portable: the new ungated test `shell_type_reply_names_the_registry_shell_not_its_family` calls `test_fish_registry_at`, which #172 left ungated. `test_posix_at`'s only caller (`test_registry_at`) and `SystemHost`'s only test use stay `#[cfg(unix)]` | `a813eed9` gated `test_posix_at` / `test_fish_at` to `cfg(all(test, unix))`, `test_fish_registry_at` to `cfg(unix)` and `SystemHost` to `cfg(unix)` in tests, because on 2026-10-13 all their callers were unix-only (dead code under warnings-denied Windows Clippy) | Re-derived per helper. Still needed: `cfg(all(test, unix))` on `test_posix_at` and `#[cfg(unix)] use super::SystemHost` (callers still unix-only) — kept #172's body/doc comment and `PosixShell` import. **Superseded by #172:** the fish gating — the first assembled push (`e288326c`) re-applied it and broke the Windows test build (`test_fish_registry_at` not found, VM + CI); follow-up commit restores #172's portable `test_fish_at` / `test_fish_registry_at` (`terminal_io.rs` now byte-identical to main). |
| `docs/ROADMAP.md` Open Tension #7 (textual conflict) | #173 appended "Update 2026-10-18 (cyril-zbu7): custodian role shrank — kiro-cli renews; on-disk store only as fallback" | Rewrote #7 for the default-on policy (production builds include the surface; isolated no-default build is the exclusion) | Merged both: main's zbu7 update verbatim + this branch's default-build policy and ADR-0002 link. |
| `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/ROADMAP.md`, `docs/adr/0002-kas-cargo-feature-gate.md` (applied cleanly, semantically stale) | #173 changed the auth source | Said Free spawn's callback auth is "supplied by Cyril from Kiro's credential store" and that the amended ADR's production binaries "use the existing custodian" | Re-expressed: token from `kiro-cli chat _ get-kas-token` on every callback, store read only as a never-renewed fallback; ROADMAP parity table gains rows for zbu7/k8wc/auxt/q0kg+rcub; ADR-0002 gains a dated zbu7 update and the landed-prerequisite list. |
| `.rivets/issues.jsonl` | Main updated cyril-4c4d (2.32.0 flip-blocker note, `updated_at`) plus many other issues | Added cyril-4c4d qualification notes | Per-issue/per-field three-way merge (base `7c7b0d1d`): only cyril-4c4d changed on both sides — `notes` unioned by `created_at` (6 notes), `updated_at` = max (main's). No other field changed on both sides. |

ADR-0013 (supersedes ADR-0011) checked against this branch's text: no 4c4d-added text restates ADR-0011's gate-off policy; the design's "four workflow exclusions" suppress KAS's gate-advertised workflow slash commands in favour of cyril's own `/workflow` commands, consistent with ADR-0013's retained cyril control plane (the ADR-0011 citations in `convert/kas.rs` are main's cyril-oieu code, not this branch's).

### Gate (2026-10-18, Linux, Rust 1.94.0, source `db8ae821`)

Every cargo lane in this branch's `ci.yml` passed, run as `env -u CARGO_TARGET_DIR cargo …`: fmt; workspace all-feature Clippy, nextest and doctests; `review_prefix` (bash/posix); KAS Feature build/Clippy/nextest (`-p cyril -p cyril-core --features kas`); Default Features — binary default build/nextest, workspace default Clippy/nextest, binary `--no-default-features` build/Clippy/nextest, core `--no-default-features` Clippy/nextest; worktree tooling; crtool self-test; review-asset sync; `cargo test -p cyril-review`; `cargo test -p cyril --test crtool_cli`; `cargo build --release --all-features`. No flakes needed re-running.

The Linux gate could not see the Windows-only fixture regression above (unix cfg is true there); the Windows VM and Windows CI caught it on `e288326c`, and `5f35832d` repairs it.

### Live check (2026-10-18) — default binary, no `--agent-engine`, no config file

Receipts: `receipts/live-2026-10-18/`. Default-feature `cargo build -p cyril --bins` (sha256 in `binary.sha256`), kiro-cli 2.32.0, private `HOME`, `XDG_DATA_HOME=~/.local/share`, private `XDG_CONFIG_HOME`, `SHELL=/bin/bash`, driven through hex1b. Two separate cyril processes (two fresh sessions).

| Check | Result |
|---|---|
| KAS selected by default | Session `sess_c04c9d66-…` (KAS `sess_` id) created; log binds `/usr/bin/node` (Free spawn); `getAccessToken served … origin: Helper` — the token came from `kiro-cli chat _ get-kas-token` (cyril-zbu7), no auth failure. |
| Autocomplete | `/pro` offered `/probe-skill` and `/probe-prompt` (both sessions; `s2-autocomplete.txt`). |
| `/probe-skill omega4` | `SKILLRAN OMEGA4` (agent uppercased the argument). |
| `/probe-prompt gamma delta` | `PROMPTRAN GAMMA DELTA`. |
| `/general-task-execution …agentran_q2 in uppercase…` | Sub-agent ran; `AGENTRAN_Q2`. |
| Shell approval offers "Always allow" (`SHELL=/bin/bash`) | `echo alwaysprobe_k7 \| tr a-z A-Z` → overlay `Allow / Always allow / Deny / Always deny` (`s1-approval.txt`). KAS asks once per pipeline segment: "Always allow" → scope page → `s` to workspace → exact `echo alwaysprobe_k7`; KAS then asked for the same call again for the uncovered `tr a-z A-Z`, granted the same way. Output `ALWAYSPROBE_K7`. |
| Grant persists | `~/.kiro/workspace-roots/<hash>/permissions.yaml` gained `shell allow [echo alwaysprobe_k7, tr a-z A-Z]` (`permissions.yaml`). |
| Fresh session runs it without asking | Second cyril process, session `sess_308b6d10-…`: same prompt ran `ALWAYSPROBE_K7` with no Permission Required dialog (`s2-shell.txt`; no approval-preview log line after session 1). |

The shell approvals showed "Preview unavailable" in the body (the command is in the dialog title; KAS `raw_input` has no text/content field, logged once per request per cyril-q0kg). That is main's existing approval rendering, unchanged by this branch; it is noted, not filed.

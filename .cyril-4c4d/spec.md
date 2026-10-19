# cyril-4c4d — default-engine specification and parity audit

Status: approved for implementation; see design Approval for the final command-context decision. Date: 2026-10-12.

## Request (verbatim)

“Claim and implement cyril-4c4d”

## What this is / Behavior

The requested release makes KAS/v3 the normal Cyril engine while preserving explicitly selected Classic/v2 until Kiro removes it. The ticket authorizes an actionable missing-bundle error as an alternative to automatic bootstrap. It requires platform correctness, CI default/no-default coverage, an ADR amendment, documentation, and a parity list identifying blockers.

Behavior for the default switch:

- The production `cyril` package enables its existing `kas` feature by default. Keep the compile-time opt-out; do not remove the core feature gate.
- Missing engine configuration selects KAS in a KAS-capable build and V2 in a selected-package no-default build. Explicit CLI engine wins over config. Explicit `v2`, `kas`, and `v3` parsing/serialization retain their current contracts.
- Keep Free spawn. A missing bundle continues to report its searched path and instruct the user to run `kiro-cli acp --agent-engine v3` once or supply `KIRO_KAS_SERVER_PATH`. Missing Node and unusable credentials retain their distinct actionable errors. No silent V2/Wrapper fallback.
- The ADR explicitly states that normal builds now contain credential-reading code; an isolated `cargo build -p cyril --no-default-features` remains the compile-time opt-out. Runtime V2 selection is not represented as the same security guarantee.
- Integrate the macOS data-root correction from PR #167 rather than narrowing to Linux or duplicating another session's work.
- Repair KAS advertised-command routing and Host hook tag matching before enabling the new default.
- Preserve upstream KAS native-memory defaults. Do not introduce a memory veto or imply that Cyril's `/memory` controls upstream native memory.

## Success criteria

1. Build the selected production package with default features, launch its real TUI with absent/default config, and complete a KAS prompt. Existing explicit-engine bridge smokes alone do not prove default selection.
2. Exercise explicit V2 CLI/config selection and precedence; build and exercise the isolated no-default V2 binary. Pin fixtures intending a fake/V2 transport instead of relying on the changed engine default.
3. Launch the production entry point with an isolated missing-bundle environment and observe the repair instruction in the TUI. Preserve error propagation and no implicit engine/auth fallback.
4. Obtain the platform prerequisite's accepted evidence and run applicable repository/platform checks after integration. An open PR is not completion.
5. CI checks selected-package default and no-default builds, not just workspace feature unions. `cyril-workbench` already enables core KAS, so workspace-only default coverage cannot establish the binary's feature policy.
6. Amend ADR-0002 and current README, agent guidance, glossary/default-engine references, and ROADMAP. Keep historical audit reports historical. Record the parity dispositions below, with no claim of complete KAS/V2 feature parity.
7. Complete repository tests/lints/formatting, real consumer checks, and independent review before declaring implementation complete.

## Related issues / parity evidence

Source basis: production files at `99ed7ce5`; tracker records read from primary main; 2.28.0/2.29.1 audit records on primary main. This is a source/tracker audit, not a new live parity qualification. Installed `kiro-cli --version` returned `2.29.1`.

| Area | Baseline evidence / issue | Disposition |
|---|---|---|
| Native data paths | `cyril-igrx`; merged PR [#167](https://github.com/dwalleck/cyril/pull/167), `b9c33f02`. Native Windows correction was already landed; this PR supplies the macOS correction. | Formal prerequisite satisfied. Its source is integrated; final assembled/native CI evidence remains an implementation obligation. |
| Credential custody | ADR-0002:7-20 rejects shipping credential readers in normal builds. | Required ADR/security amendment; preserve compile-time opt-out. |
| KAS-advertised slash commands | `commands/mod.rs:509-541` dispatched registered commands through `ExecuteCommand`; `domain_mediator/commands/extensions.rs` constructed V2 `kiro.dev/commands/*` requests. KAS has no equivalent endpoint (`cyril-lnxg` and covenant). | Functional prerequisite repaired under `cyril-oieu`, approved design D4. No claim that every slash command was broken; exact custom-agent routing and local-context boundaries are recorded in the design. |
| Host hook tag matchers | `cyril-5nep`; baseline `kas/hooks.rs:587-630` read only toolId and matched only that string; callbacks did not retain toolTags. | Safety prerequisite repaired under approved D3, preserving Host enforcement authority rather than switching hook ownership. |
| Native KAS memory | `cyril-0na7`, `cyril-s21o`; corrected 2.28.0 init-settings probe reports control `read_write`/reflection true and honors explicit disabled. The 2.29.1 audit reports the handshake unchanged. Cyril sends no native-memory policy. | N/A — approved risk: preserve upstream defaults, accepting possible overlap/costs. See Approval. No actual duplicate writes were demonstrated by the cited probes. Full controls/status remain separate. |
| V2 model fallback | `cyril-ci3j`, `cyril-lnxg`; the latter records the user's 2026-10-08 deferral because V2 is retiring. | Not a flip blocker; honor the prior deferral, retain explicit V2 selection, do not claim fallback parity. |
| KAS model fallback | `cyril-aoby`; new notification/capability not consumed; served fallback chain not provoked in the audit. | Follow-up, not an observed startup/default-selection blocker. |
| Settings drift | `cyril-53qx`; current Cyril sends todoList and subagentOrchestration; 0.66.33 logs both as unknown. Older binding guidance intentionally retained orchestration on 0.66.26. | Follow-up with version-policy correction, not permission to silently rewrite the older approved design. Do not use the stale orchestration smoke as proof of the new default. |
| Native memory controls/status | `cyril-0na7`; generic memoryReflection/memoryConfig surfaces incomplete. | Full UI remains separate; the default policy is resolved under Approval. |
| Telemetry policy | `cyril-zjm5`; Cyril does not send an explicit telemetryEnabled opt-out. Cited 2.29.1 probes observed zero telemetry endpoint dials in control and opt-out arms. | Follow-up; absence of observed traffic is not a privacy guarantee. No new telemetry policy is silently introduced here. |
| Terminal input/background execution | `cyril-7p7n`, `cyril-1rpv`; no terminal/write responder, stdin is null, background mode not enabled in current settings. | Not a flip blocker while interactive/background terminal support remains unsupported; do not advertise it. |
| Bundled steering commands | `cyril-2nz7`, `cyril-79ox`; current KAS stopped advertising three steering commands; command metadata also lost in conversion. | Follow-up discovery/control limitation, distinct from dispatching an advertised command to the wrong RPC. |

## Decisions

### Existing requested decision: KAS default with V2 escape

Source: cyril-4c4d records the user's 2026-10-08 decision. Do not ask whether to switch engines again. Retaining the compile-time gate, Free spawn and actionable first-run error is the conservative implementation of the ticket's permitted outcomes.

### Parity prerequisite scope

Repair advertised KAS command routing and `cyril-5nep` hook-tag enforcement in this effort, tracked separately where appropriate. The default flip waits for those repairs and cyril-igrx. This is authorization to expand prerequisite work, not to close incomplete records or skip design/verification.

### Native-memory policy

Preserve upstream native-memory defaults, including possible overlap and costs. Do not add a native-memory veto or controls in this change. Document that Cyril's `/memory` does not govern both systems. Keep `cyril-0na7`/`cyril-s21o` as separate work; do not mark their acceptance criteria delivered.

### Local context on advertised command invocations

Source inspection found that KAS joins text blocks before slash parsing. Automatic local lesson prepending prevents recognition; appending context changes arguments. Approved behavior: a recognized KAS advertised command sends exactly its original text, without automatic local lesson/file expansion; first-prompt lessons remain pending for the next ordinary chat prompt. This preserves the current registered-command boundary (no automatic context) while repairing dispatch. Ordinary chat/context and unregistered slash fallback remain unchanged. The approval is recorded once in `design.md` Approval, which covers D4.

## Out of scope

Unless the requester expands it: no automatic wrapper/auth-owner migration (`cyril-ny8z`, `cyril-zbu7`); no general memory panel, interactive terminal, fallback-model, steering-discovery, or settings-schema implementation. These are tracked limitations, not declarations of parity. No takeover of PR #167, no primary-main history rewrite, and no publication/merge authorization is inferred.

## Approval

2026-10-12 requester answers:

- Parity scope: **“Repair before flipping”**, selecting the option to authorize prerequisite repairs in this effort and keep the flip gated on those repairs plus cyril-igrx.
- Native memory: **“Preserve upstream defaults”**, selecting the option to leave native KAS memory uncontrolled by Cyril, explicitly accepting possible overlap and costs and the fact that Cyril's `/memory` does not govern both systems.

These answers resolve prerequisite scope and native-memory policy. Native-memory overlap is the only accepted risk recorded here. The later local command-context decision is approved under `design.md` Approval. Implementation proof is owned by `plan.md`; the externally-owned cyril-igrx prerequisite has since landed.

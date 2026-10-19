# KAS retains a compile-time gate; the production binary enables it by default

Status: accepted (2026-06-16); amended for cyril-4c4d (2026-10-12). Final acceptance and qualification pending.

## Original context (2026-06-16)

Driving the KAS engine requires cyril to read kiro's bearer token from its on-disk auth store and hand it to the KAS subprocess via the `_kiro/auth/getAccessToken` responder — making cyril a custodian of a credential it does not own (Open Tension #7). KAS is also alpha, pulls in an ~801 MB self-extracting engine at runtime, and is not on the critical path for the stable v2 experience. Engine selection happens at runtime (`AgentEngine` enum, default v2), but a runtime gate alone still *ships* the credential-reading code in every binary.

## Original decision (default-off policy superseded)

KAS impls — especially the credential-reading `AuthResponder` — live behind a **default-off `kas` cargo feature**, in addition to the runtime `AgentEngine` selection. A default `cargo build` produces a binary that **cannot** read the kiro token, because that code is not compiled in. KAS is an opt-in build. The feature is established (empty) in KAS-0 with a CI lane that builds, lints, and tests `--features kas` so it cannot bitrot under the workspace's "warnings are errors" rule.

## Considered options

- **Runtime gate only** — rejected as the *sole* gate: it ships the credential-custodian code in every binary, so a security-conscious user or org build cannot obtain a cyril that is incapable of touching the credential. It only defers Open Tension #7 rather than discharging it.

## Original consequences (2026-06-16)

- "The default build cannot read your token" becomes a checkable, compile-time fact — a stronger posture than "it can, but only if you pass a flag."
- v2 users get a smaller, simpler binary that excludes KAS's surface.
- This is the project's first cargo feature: it introduces a CI build-matrix obligation and the only `#[cfg(feature = …)]` conditional compilation in an otherwise flag-free codebase.

## Amended decision (2026-10-12)

The approved cyril-4c4d policy makes KAS the normal engine while retaining explicit v2 selection until upstream removes it. The production `cyril` package enables `kas` by default; `cyril-core` keeps the feature opt-in. Normal production binaries therefore contain the credential-reading code and use the existing custodian when running KAS (superseded for token sourcing by the 2026-10-18 update below: tokens now come from `kiro-cli chat _ get-kas-token`). The original custody concern remains valid; the original **default-build exclusion** claim no longer applies.

An isolated `cargo build -p cyril --no-default-features` produces a V2-only binary without the KAS credential reader. Runtime `--agent-engine v2` changes engine selection, not compiled capability. A workspace build is not evidence of exclusion: `cyril-workbench` enables core KAS through Cargo feature unification. CI must cover the selected production package with and without defaults.

Keep Free spawn: discovered Node.js plus the extracted bundle, with callback authentication. Missing prerequisites report actionable errors, including `kiro-cli acp --agent-engine v3` for extraction or `KIRO_KAS_SERVER_PATH` for an explicit bundle; there is no silent Wrapper or v2 fallback.

The requester also accepted preserving upstream KAS native-memory defaults, including possible overlap and costs. Cyril's `/memory` controls Cyril's local memory only, not native KAS memory. Full native-memory controls/status remain separate work.

**Update 2026-10-18 (cyril-zbu7, PR #173):** the custody surface narrowed. On every `_kiro/auth/getAccessToken` callback cyril now runs `kiro-cli chat _ get-kas-token` (`--force-refresh` iff KAS asks) and relays the result, so kiro-cli alone owns and renews the refresh token. The compiled-in credential-store reader survives only as a fallback for a kiro-cli lacking that hidden subcommand; it never renews. The compile-time exclusion above still applies to that reader and to the responder as a whole.

The platform correction from cyril-igrx / PR #167 and the qualified hook-tag/advertised-command repairs (cyril-5nep / cyril-oieu) are integrated. So are the remaining landed default-switch prerequisites (2026-10-18): cyril-zbu7 (PR #173, kiro-cli token source), cyril-k8wc (PR #172, `shell_type`), cyril-auxt (PR #174, always-allow consent echo), cyril-q0kg (PR #175) and cyril-rcub (PR #176) (approval overlay). The repairs passed local checks, live proofs and independent review; overall default-switch acceptance and platform evidence review remain pending. No live-native qualification is claimed. See [ROADMAP](../ROADMAP.md#current-default-policy-and-parity-cyril-4c4d).

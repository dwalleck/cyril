# KAS is gated by a default-off `kas` cargo feature, not only a runtime flag

Status: accepted (2026-06-16)

## Context

Driving the KAS engine requires cyril to read kiro's bearer token from its on-disk auth store and hand it to the KAS subprocess via the `_kiro/auth/getAccessToken` responder — making cyril a custodian of a credential it does not own (Open Tension #7). KAS is also alpha, pulls in an ~801 MB self-extracting engine at runtime, and is not on the critical path for the stable v2 experience. Engine selection happens at runtime (`AgentEngine` enum, default v2), but a runtime gate alone still *ships* the credential-reading code in every binary.

## Decision

KAS impls — especially the credential-reading `AuthResponder` — live behind a **default-off `kas` cargo feature**, in addition to the runtime `AgentEngine` selection. A default `cargo build` produces a binary that **cannot** read the kiro token, because that code is not compiled in. KAS is an opt-in build. The feature is established (empty) in KAS-0 with a CI lane that builds, lints, and tests `--features kas` so it cannot bitrot under the workspace's "warnings are errors" rule.

## Considered options

- **Runtime gate only** — rejected as the *sole* gate: it ships the credential-custodian code in every binary, so a security-conscious user or org build cannot obtain a cyril that is incapable of touching the credential. It only defers Open Tension #7 rather than discharging it.

## Consequences

- "The default build cannot read your token" becomes a checkable, compile-time fact — a stronger posture than "it can, but only if you pass a flag."
- v2 users get a smaller, simpler binary that excludes KAS's surface.
- This is the project's first cargo feature: it introduces a CI build-matrix obligation and the only `#[cfg(feature = …)]` conditional compilation in an otherwise flag-free codebase.

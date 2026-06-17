# Kiro engines (v2 and KAS) sit behind a Kiro-scoped `Engine` trait, not an enum or the vendor seam

Status: accepted (2026-06-16)

## Context

Kiro ships two agent engines reachable over the same `kiro-cli` binary: **v2** (Rust, `kiro.dev/*` dialect, today's default) and **KAS** (TypeScript/LangGraph, `_kiro/*` dialect, `--agent-engine kas`). Cyril must drive both. The engines are *asymmetric* — KAS adds whole surfaces v2 lacks (host-supplied auth, fs/terminal callbacks, hooks host, org governance) — and the maintainer expects Kiro's backend to keep growing new wire surfaces.

## Decision

Engine is bound at **agent-subprocess spawn** — the bridge runs one `kiro-cli acp [--agent-engine kas]` process and holds one engine for its life, so it is immutable for that subprocess and for every session on it. In v1, selection is **startup-only** (`--agent-engine` / config); switching engines means restarting the subprocess (a live `/engine`-as-respawn is a deferred nicety). The two engines live behind a **small, Kiro-scoped `Engine` trait** (convert wire notification → internal `Notification`; declare `client_capabilities`; detect turn-end) plus **optional capability sub-traits** (`AuthResponder`, `HostIo`, `HooksHost`, `GovernanceSource`, …) that KAS implements and v2 does not. Engine nests *under* the Kiro vendor; it is **not** the same mechanism as the vendor seam (Phase 1/4) — Claude and other vendors do not implement `Engine`.

## Considered options

- **Enum + targeted `match`** — rejected: the backend is expected to keep growing new wire surfaces, and an enum makes each new surface a scattered edit across match sites rather than an additive trait.
- **One fat `Engine` trait with default no-op methods** — rejected: every new KAS surface would edit the shared trait and re-touch v2. Sub-traits keep v2 untouched as KAS grows (open/closed).
- **Vendor-agnostic engine trait (Claude implements it too)** — rejected: v2 and KAS share the `kiro-cli` binary, `~/.kiro` auth/session storage, and Kiro slash-command/mode heritage that Claude does not; the vendor seam belongs one level up.

## Consequences

- The first KAS milestone (KAS-0) is larger than "add an arg": it must define the core trait and port today's working v2 conversion into a `V2Engine` impl behind it — a pure refactor of load-bearing code whose acceptance criterion is strict v2 behavioral parity, sized and tested on its own before any KAS turn renders.
- New Kiro backend surfaces become new capability sub-traits — additive and v2-safe.
- Because the binding is per-subprocess (not per-session), the bridge holds a single `Box<dyn Engine>` chosen once at spawn and used for all its notifications — no per-session engine lookup, and no need to carry engine on `RoutedNotification`. Concurrent mixed engines in one cyril instance would require multiple subprocesses (deferred).

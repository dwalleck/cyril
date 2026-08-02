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
- ~~**Capability sub-trait stubs land with their first consumer, not in KAS-0.**~~ **[Superseded by the 2026-07-30 amendment below — this prediction was falsified: `AuthResponder` never landed, auth shipped as a static feature-gated handler, and the `as_*`-accessor pattern is withdrawn. Retained for the record.]** The original plan was to stub the first sub-trait (`AuthResponder`) in KAS-0; checkpointed-build found that a defaulted `as_*` accessor + empty sub-trait with no caller is dead code under the workspace's `-D warnings`, which forbids `#[allow(dead_code)]`. So the accessor pattern is introduced in **KAS-1** (cyril-evwh), where `AuthResponder` gets a real implementation and consumer. KAS-0 ships the core trait (convert + `client_capabilities`) + `V2Engine` only.
- Because the binding is per-subprocess (not per-session), the bridge holds a single `Box<dyn Engine>` chosen once at spawn and used for all its notifications — no per-session engine lookup, and no need to carry engine on `RoutedNotification`. Concurrent mixed engines in one cyril instance would require multiple subprocesses (deferred).

## Amendment: host-callback capability adapters (2026-07-30)

### Context

KAS subsequently shipped authentication, file I/O, terminal, and hooks host
callbacks, but their concrete execution stayed in `KiroClient`, gated by the
`kas` **cargo feature** rather than by the bound engine — so a `--features kas`
build running `V2Engine` still answers `_kiro/auth/getAccessToken`, `fs/*`, and
`terminal/*` despite advertising none of them (cyril-dn91). Capability
advertisement remained in `Engine`, and the optional capability adapters
anticipated above never landed. That split made “advertised” and “executable”
separate facts and let callback execution bypass the bridge mediator.

Sourced from the 2026-07-30 recent-hotspot architecture review, Candidate 01.
Its deletion test is the decision driver: removing the mediator hop discards
mostly plumbing, while removing `Engine` branching spreads conversion and
capability decisions across the codebase. **Depth belongs on the Engine module,
not on the mediator.**

### Decision

- The deepened host-callback seam remains **Kiro-scoped**. Do not introduce a
  vendor-neutral adapter until a second vendor supplies host callbacks.
- Engines select optional capability adapters grouped by real concern:
  **Auth**, **Host I/O** (file plus terminal), and **Hooks**. Permission remains
  the standard ACP human-decision path, not an engine capability adapter.
- **The adapter set is data the engine returns, and inbound advertisement is
  derived from it** — not a second method an engine must remember to keep in
  step. Shape sketch, not a pinned signature:

  ```text
  Engine::adapters() -> Adapters { auth, host_io, hooks }   // each optional
  Engine::client_capabilities()                             // derived from adapters()
  ```

  The `as_*`-accessor phrasing of the original decision is **withdrawn**: a
  defaulted downcast-style accessor leaves advertisement and execution as two
  facts a future engine can desynchronize, which is precisely the defect this
  amendment exists to remove. Deriving one from the other makes the invariant
  mechanical instead of conventional.
- **An absent inbound adapter cannot be advertised as an inbound host-callback
  capability**, and an engine with no adapter for a callback family **refuses**
  those callbacks rather than answering the protocol default. The default is a
  `null` result, which the agent reads as *success with an empty result*
  (`unhandled_ext_response`) — the silent-failure shape this codebase forbids.
  This is what closes cyril-dn91: `V2Engine` installs no adapters, so a
  `--features kas` build running v2 refuses auth, fs, and terminal.
- **The invariant is per direction.** [ADR-0010](0010-kas-hook-registry-direction.md)
  established that `_kiro/hooks/list` is bidirectional: under
  `kas_hooks = "host"` cyril *serves* it inbound; under `"kas"` cyril *sends* it
  outbound as a `BridgeCommand` and the agent executes hooks itself. So `"kas"`
  mode legitimately advertises hooks with **no** inbound adapter. That
  advertisement is an outbound-client declaration, not a host-callback
  advertisement. It must **not** be reconciled by installing an empty registry
  as a stand-in — an empty adapter standing for "absent" is a sentinel.
- One adapter set is bound for one bridge/agent-subprocess lifetime, matching
  the existing immutable Engine binding. Callback scope inside that lifetime is
  explicit: bridge-global, session-scoped, or operation-scoped.
- Concrete Rust method shapes remain an implementation decision. The
  architectural requirement is a small interface per capability concern, not
  one fat callback adapter and not one adapter per callback method.

### Consequences

- Adding a Kiro host capability is additive: install its adapter, derive its
  advertisement from that installation, and add its typed callback variants.
- v2 keeps the standard permission path and no KAS capability adapters; KAS
  installs only the adapters its selected modes actually support.
- Capability tests must prove both advertisement and execution reachability;
  separate tests of those facts are not sufficient. With the adapter set as
  data, one test walks it — replacing N paired per-capability tests, and
  covering capabilities that do not exist yet.
- Hooks is the one family where cyril is both server and client of the same
  method name. A Hooks adapter must hold both directions, or it will fit `host`
  mode and silently exclude `kas` mode.
- Supersedes the struck consequence above. `AuthResponder` never existed; the
  accessor pattern it promised is replaced by the derived adapter set.
- This realizes the original optional-capability decision rather than changing
  the Engine/vendor seam.

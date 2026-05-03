# Cyril Roadmap

> *Cyril is the polished TUI for the Agent Client Protocol ecosystem.*

This document captures the phased path from cyril's current Kiro-focused implementation toward the vendor-neutral platform vision. It is the source of truth for the project's direction; every phase below should produce concrete commits, and finishing one phase unblocks the next.

## Mission

Cyril is a polished terminal interface for the Agent Client Protocol ecosystem. Run any of 37+ registered agents — Claude, Cursor, Codex, Cline, Goose, Kiro, and more — through a single interface. Beneath the TUI, composable proxy stages add behaviors no agent ships natively: skill systems, transcript audit, organizational permission policies, persistent memory across sessions, multi-client observers. Vendor neutrality is a feature, not a roadmap; stages are how cyril compounds value over time.

## Why this direction

The original mission ("cross-platform TUI client for Kiro CLI, with WSL bridging on Windows") is obsolete: Kiro CLI now ships a native Windows binary, eliminating the WSL gymnastics that originally motivated the project. At the same time, the broader [ACP ecosystem](https://github.com/agentclientprotocol) has matured into a multi-vendor standard with formal governance, language SDKs, and a curated agent registry. Cyril's strategic position shifts from "Kiro client" to "polished TUI for the ACP ecosystem with composable proxy-stage behaviors."

The competitive landscape has a real gap to fill:

| Tool | Vendor strategy | Agentic features | Composability |
|---|---|---|---|
| aider, cline, opencode | OpenAI-API-compat across providers | Reimplements its own tool-call/file-edit | None |
| Claude Code, Kiro CLI native, Cursor | Vendor-locked | Native | Vendor-controlled plugins only |
| `acpr` (registry runner) | Vendor-neutral | Native via ACP | None — just a process spawner |
| **cyril (envisioned)** | **Vendor-neutral via ACP registry** | **Native via ACP** | **Stages composable across all vendors** |

That last row is currently empty. The polished-TUI-for-the-ACP-registry niche has no incumbent.

## Phases

### Phase 0 — Re-anchor mission ✅

**Status:** complete (PR #8).

Update `README.md` and `CLAUDE.md` to use the canonical tagline and elaboration paragraph above. Update the GitHub repository description to match. Establishes the framing every subsequent decision defers to.

### Phase 1 — Transport refactor + Kiro extension boundary

**Estimate:** 1–2 weeks.
**Depends on:** Phase 0.

- Refactor `crates/cyril-core/src/protocol/transport.rs` to take a `Vec<String>` for the agent command instead of hardcoding `<binary> acp` as the second arg. The conductor passthrough spike already identified this as the gating wart for any non-Kiro agent integration.
- Move Kiro-specific extension parsing (the `_kiro.dev/*` handling currently in generic `convert.rs`) into a clearly named module. A module boundary is sufficient; a separate crate can wait for Phase 3+. The TUI side starts treating Kiro as one supported vendor, not the universe.
- Update README's Prerequisites section: drop the WSL-only line now that Kiro ships a native Windows binary.

### Phase 2 — First proxy stage (stages-first)

**Estimate:** 2–4 weeks.
**Depends on:** Phase 1.

Build `transcript-recorder` as cyril's first proxy stage in a new workspace crate (`crates/cyril-stages/`). Use the `sacp-proxy` framework. Replaces the existing `experiments/kiro-proxy-rs/` POC with a properly architected stage. Wire conductor invocation through cyril's bridge, gated on a feature flag (default = direct spawn, today's behavior).

**Why stages-first** (over registry-first): ships immediate value to today's Kiro user base, lower risk because the user-facing UX doesn't shift much, and validates the stage architecture before committing to building several more stages. The vendor-neutral story is real but doesn't require registry integration to validate empirically — Phase 4 can use a hardcoded `claude-acp` invocation if needed.

### Phase 3 — Registry-aware agent selection

**Estimate:** 2–4 weeks.
**Depends on:** Phase 1.

- Fetch `https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`. Cache locally with sensible refresh (the registry's own auto-update cadence is hourly).
- Present an agent picker at startup or via `/agent <name>` command.
- Use `acpr` as a runtime helper (cyril spawns it instead of the agent binary directly, getting npm/PyPI/binary handling for free).
- Per-vendor auth flow handling (each registered agent has its own auth method per `AUTHENTICATION.md`); design the UX surface for redirect/login flows.

### Phase 4 — First non-Kiro agent end-to-end

**Estimate:** 1–2 weeks.
**Depends on:** Phase 3 (or Phase 2 if you prefer to validate vendor-neutrality with hardcoded invocation first).

Pick `claude-acp` as the first non-Kiro target (largest user base, well-supported, ~1,800 stars on the wrapper). Verify cyril's TUI works end-to-end. Generalize Kiro-specific UI affordances (mode picker, `_meta.welcomeMessage` rendering, `kiro.dev/*` commands) as capability-gated rather than always-on. This is the moment "vendor-neutral cyril" becomes demonstrable rather than aspirational.

### Phase 5+ — Stages catalog growth

**Estimate:** ongoing.
**Depends on:** Phase 2.

Each new stage is reusable across all supported vendors. Candidates in rough priority order:

- **Skill resolver** — supplement whatever skill system the underlying agent has (or doesn't have)
- **Context injector** — auto-attach project context, steering files, environment metadata per turn
- **Auto-approval policy** — bypass permission prompts for whitelisted tools according to org rules
- **Persistent memory** — cross-session memory synthesis the underlying agent doesn't ship natively
- **Fan-out observer** — broadcast notifications to additional ACP clients for editor integrations (e.g. nvim plugin reading what cyril sees)

Each stage is its own subprocess, written in any language that speaks `sacp-proxy`'s protocol. Stages can be authored by third parties and dropped into a user's chain config.

## Open tensions

1. **Kiro is conspicuously absent from the ACP registry.** Cyril's most-tested agent is the one outside the curated ecosystem. Either ignore (use Kiro by direct path, others via registry) or advocate to AWS for registry membership.
2. **Some current cyril UI is Kiro-specific** (mode picker, welcomeMessage rendering, `kiro.dev/*` command parsing). Vendor-neutrality means generalizing, gating on detected capabilities, or accepting they only show for Kiro.
3. **Auth flows differ per vendor.** Each registered agent has its own auth method. Real UX work to handle showing/redirecting auth flows per agent.
4. **Stages framework is ahead of the curve.** No standardized stage registry exists yet; `sacp-conductor` and `sacp-proxy` are one project's framework. If that stack doesn't become canonical, cyril's stages story may need to migrate.
5. **Single-maintainer risk on the Rust tooling tier.** `sacp`, `sacp-tokio`, `sacp-conductor`, `sacp-proxy`, and `acpr` are all published by Niko Matsakis. The protocol itself is multi-stakeholder; the Rust tooling is one-person-led. Mitigated by clean exit ramps (all MIT/Apache), but worth knowing.
6. **Mission drift.** "Vendor-neutral platform" is more ambitious than "Kiro client." Need to keep shipping a strictly-better-than-status-quo Kiro experience while building the platform underneath.

## Reference / further reading

- [Agent Client Protocol](https://agentclientprotocol.com) — protocol spec
- [agentclientprotocol/registry](https://github.com/agentclientprotocol/registry) — curated agent registry (37+ agents as of 2026-05-03)
- [agentclientprotocol/acpr](https://github.com/agentclientprotocol/acpr) — registry runner CLI
- [agentclientprotocol/symposium-acp](https://github.com/agentclientprotocol/symposium-acp) — `sacp` / `sacp-conductor` / `sacp-proxy` / `sacp-tokio`
- [agentclientprotocol/claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp) — Claude Code as an ACP server
- [`experiments/conductor-spike/`](../experiments/conductor-spike/README.md) — the 2026-05-03 spike that empirically validated conductor passthrough and produced the binary-isolation findings

## How to update this document

When a phase is completed, mark its status and add a link to the merge commit / PR that finished it. When tensions are resolved or new ones surface, update the Open Tensions section with dated notes. This document supersedes itself; older versions live in git history.

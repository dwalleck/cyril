# Domain Docs

How the engineering skills should consume this repo's domain documentation when
exploring the codebase. **cyril is a single-context repo.**

## Before exploring, read these

- **`CONTEXT.md`** at the repo root — the project's domain glossary and
  language. This exists today.
- **`docs/adr/`** — Architecture Decision Records that touch the area you're
  about to work in. This directory does not exist yet; it's the conventional
  place ADRs will land when decisions get recorded (see `/grill-with-docs`).

If `docs/adr/` (or any of these) doesn't exist, **proceed silently**. Don't flag
its absence; don't suggest creating it upfront. The producer skill
(`/grill-with-docs`) creates ADRs lazily when decisions actually get resolved.

## File structure

Single-context repo (this repo):

```
/
├── CONTEXT.md          ← exists
├── docs/adr/           ← conventional location for ADRs (not yet created)
│   ├── 0001-....md
│   └── 0002-....md
└── crates/             ← cyril-core, cyril-ui, cyril
```

> A multi-context layout (a root `CONTEXT-MAP.md` pointing at per-context
> `CONTEXT.md` files under `src/<context>/`) is *not* in use here. If cyril ever
> splits into independently-documented contexts, switch this doc to that model.

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal,
a hypothesis, a test name), use the term as defined in `CONTEXT.md`. Don't drift
to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal — either
you're inventing language the project doesn't use (reconsider) or there's a real
gap (note it for `/grill-with-docs`).

> Note: cyril also carries a large body of protocol/architecture documentation
> in `CLAUDE.md` and `docs/` (ACP wire audits, the ROADMAP, KAS covenant). Treat
> those as authoritative for protocol behavior; `CONTEXT.md` is the domain
> glossary.

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than
silently overriding:

> _Contradicts ADR-0007 (...) — but worth reopening because…_

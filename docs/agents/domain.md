# Domain Docs

How the engineering skills should consume this repo's domain documentation when
exploring the codebase. Cyril uses a **single-context** layout.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root — the canonical domain glossary.
- **`docs/adr/`** — read ADRs that touch the area about to be changed.

If either location does not exist, proceed silently. Do not suggest creating it
upfront. The `/domain-modeling` skill, reached through `/grill-with-docs` and
`/improve-codebase-architecture`, creates domain records lazily when terms or
decisions are resolved.

## File structure

```text
/
├── CONTEXT.md
├── docs/
│   └── adr/
│       ├── 0001-kiro-engine-trait.md
│       └── ...
└── crates/
    ├── cyril/
    ├── cyril-core/
    ├── cyril-ui/
    └── cyril-voice/
```

A multi-context layout using a root `CONTEXT-MAP.md` and per-context
`CONTEXT.md` files is not in use.

## Use the glossary's vocabulary

When output names a domain concept—in an issue title, refactor proposal,
hypothesis, or test name—use the term defined in `CONTEXT.md`. Do not drift to
synonyms the glossary explicitly avoids.

If a needed concept is absent, reconsider whether the term belongs to this
domain. If the gap is real, note it for `/domain-modeling`.

Cyril also carries protocol and architecture documentation in `CLAUDE.md` and
`docs/`. Treat those files as authoritative for protocol behavior;
`CONTEXT.md` remains the canonical domain glossary.

## Flag ADR conflicts

If proposed work contradicts an existing ADR, surface the conflict explicitly
rather than silently overriding it:

> _Contradicts ADR-0007 (...) — but worth reopening because…_

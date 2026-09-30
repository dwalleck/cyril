# KAS workflow-recipe schema export

Regenerates the JSON Schema (draft 2020-12) for **Kiro KAS workflow recipes** committed at
`docs/kiro-kas-workflow-recipe-schema-<ver>.json`. The schema validates
`.kiro/workflows/*.workflow.json` and, once parsed, `*.workflow.yaml` / `*.workflow.yml`
(yaml-language-server and check-jsonschema consume it directly).

`generate.js` is a **hand-maintained reconstruction** of the recipe zod schemas carved from
the `@kiro/agent` bundle (`kas-carves/<ver>/node_modules/@kiro/agent/dist/server/acp-server.js`
in the research archive), run through real zod v4 `z.toJSONSchema` (draft 2020-12, input
mode). The bundle is **minified** from 2.20.1 on, so the transcription works from minified
zod; `generate.js` lists the minified binding names (`uy`, `NH`, `Fue`, `wHt`, `OKi`, `VKi`, ...)
so a refresh can re-find each definition.

zod refinements do not survive `z.toJSONSchema`. Every one the runtime enforces is re-added
as a JSON Schema keyword in `generate.js`'s `REFINEMENTS` table, citing the runtime error it
mirrors: StopCondition needs one field, fileCheck requires `value` and rejects an empty array,
repeat allows at most one of `stopCondition`/`stopWhen`, plus the `github-pr` and `command`
watch-handler config rules (checked at run creation, `_kiro/workflow/new`).

## Oracles

- `validate.js`: offline, ajv 2020. Must pass: the official docs' complete recipes
  (`fixtures/docs-*`, copied from kiro.dev/docs/workflows/{authoring,patterns} on 2026-09-30),
  this repo's own `.kiro/workflows/*.workflow.json`, and inline cases; one failing case per
  constraint. Also runs `engine-cases/` and expects the engine's verdict.
- `engine_diff.py`: live, **zero credits**. Registers each `engine-cases/*.workflow.json`
  with `_kiro/workflow/new` (never `invoke`) via
  `experiments/code-review-workflow/run_review.py --validate-only --recipe`, one at a time,
  and checks that the engine's verdict matches the file name. It prints each rejection's
  engine message, so you can confirm it failed for the intended reason. Needs a logged-in
  `kiro-cli`. **Never parallelize it:** concurrent token renewals log the user out.

Mutation check (done for 2.26.0): with `REFINEMENTS` disabled, exactly the 13
refinement-backed failing cases flip to MISMATCH.

## Deliberate tolerances (runtime-faithful, not lint-strict)

- Unknown keys pass everywhere except `fileCheck`. KAS's `z.object` (no `.strict()`)
  silently strips them. Set `additionalProperties: false` locally for typo-linting.
- An unknown watch `handler` or agent name passes. Those are registry lookups the engine makes
  at run creation (`engine-cases/e13-SCHEMA-PASS-unknown-handler`: "Watch handler 'crux-cr' is
  not registered").
- Not expressible in JSON Schema, enforced at load/creation time instead (the
  kiro-workflow-authoring skill's `validate_workflow.py` checks the static ones): at most 50
  static step nodes and 8 nesting levels; unique node ids; `stopWhen` grammar and watch-id refs;
  template happens-before (`{{<id>.output}}`, `{{artifacts.x}}`, no `{{previous.output}}` in a
  container's first node or inside a parallel branch); no `completion` step inside `parallel`;
  fileCheck/prRef paths inside the workspace roots.

## Per-release refresh

1. Carve the new KAS bundle; find `uy=A.object({name:` (top level) and the node union
   (`A.discriminatedUnion("type",[A.object({type:A.literal("step")`), plus the watch handler
   `configSchema`s (search `minPollIntervalSec:`). Diff them against `generate.js`.
2. Update `generate.js`, bump the version in the output filename (`package.json` scripts) and
   the `VERSION`/`KAS` constants.
3. `bun install` (or `npm install`), then `node generate.js ...` and `node validate.js ...` via
   the package scripts: all green. Then `python3 engine_diff.py`: ALL AGREE.

If the system `node` is broken, `~/.local/share/kiro-cli/node` (bundled with kiro-cli) runs
both scripts.

## Recipe-schema timeline (from extracted bundles and live probes)

| change | where seen | note |
|---|---|---|
| step cap 20 → 50 | ≤ KAS 0.66.15 | 20 on 2.16.0 / KAS 0.27.8 |
| repeat with neither stop form: rejected → allowed | ≤ KAS 0.66.15 | the engine's embedded model reference still says "exactly one" |
| step `input` removed (now rejected) | ≤ KAS 0.66.15 | on 2.16.0 it took precedence over `prompt` |
| `crux-cr` handler removed, `command` added | ≤ KAS 0.66.15 | |
| `injectOriginalUserRequest`, `planRevision` | ≤ KAS 0.66.15 | undocumented on kiro.dev |

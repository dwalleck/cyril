# KAS agent-file schema export

Regenerates the JSON Schemas for **KAS (v3) engine agent files** committed at:

- `docs/kiro-kas-agent-file-schema-<ver>.json` — root validates `.kiro/agents/*.json` agents
- `docs/kiro-kas-agent-frontmatter-schema-<ver>.yaml` — root validates the YAML front matter of `.kiro/agents/*.md` agents (same JSON Schema, serialized as YAML; YAML has no schema language of its own)

`generate.js` is a **hand-maintained reconstruction** of the zod schemas carved from the
`@kiro/agent` bundle (`~/.local/share/kiro-cli/kas/<ver>-<sha>/node_modules/@kiro/agent/dist/server/acp-server.js`,
modules `src/services/custom-agents/types.ts`, `src/hooks/schema.ts`, `src/agent.ts`,
`src/mcp/server-config.ts`). It runs the reconstruction through real zod v4's
`z.toJSONSchema` (draft 2020-12, input mode) so the emitted schema is generated, not
hand-written — only the zod source is transcribed.

`validate.js` is the oracle: pass/fail fixtures for every constraint (enum values,
`file://` knowledge sources, hook shapes, the JSON-vs-frontmatter `tools` difference)
plus a **tolerance mirror** — a v2-format agent file (with `allowedTools`/`toolsSettings`)
must PASS, because KAS's runtime `z.object` (no `.strict()`) silently strips unknown keys.
Set `additionalProperties: false` locally if you want typo-linting instead of
runtime-faithful validation.

## Per-release refresh

1. Extract or locate the new KAS bundle; diff the four source modules against the
   reconstruction in `generate.js` (grep `CustomAgentFileFrontMatterSchema =` and read the
   surrounding `// src/services/custom-agents/types.ts` module — esbuild keeps doc comments).
2. Update `generate.js`, bump the version in the output filenames + `package.json` scripts.
3. `npm install && npm run generate && npm run validate` — all cases must be green.

## Field timeline (from extracted bundles)

| field | landed | note |
|---|---|---|
| `welcomeMessage` | ≤ 2.10.0 | v2 has had it since 2.1.1 |
| `dispatchKind` | 2.11.0 | enum `sub-agent \| custom-agent \| spec` (KAS 0.3.299→0.8.0 renumber release) |
| `hooks` (inline in agent file) | 2.12.3 | per-agent hooks, `HookDocument` shape |

The official kiro.dev v3 agent-config docs (as of 2026-07-17) document only description,
model, tools, mcpServers, resources, permissions, welcomeMessage — `name`, `excludedTools`,
`includeMcpJson`, `includePowers`, `dispatchKind`, and `hooks` are undocumented.

// Reconstructs the KAS (v3) agent-file zod schemas carved from the
// @kiro/agent bundle (kiro-cli 2.13.0 / KAS 0.18.2) and emits JSON Schema
// (draft 2020-12) via zod v4's native z.toJSONSchema.
//
// Source modules in the bundle:
//   src/services/custom-agents/types.ts  — CustomAgentFileFrontMatterSchema, JsonAgentFileSchema, PermissionsPolicySchema
//   src/hooks/schema.ts                  — hookDocumentSchema + action/confirm sub-schemas
//   src/agent.ts                         — AgentResourceSchema, KnowledgeBaseResourceSchema
//   src/mcp/server-config.ts             — McpServerWireRecordSchema
//
// Two deliberate stand-ins for constructs JSON Schema cannot express:
//   * McpServerWireRecordSchema is z.preprocess(non-map -> undefined, record) at
//     runtime; represented here as the inner record. (Runtime tolerance noted in
//     the description.)
//   * KnowledgeBaseResourceSchema.source uses .refine(startsWith('file://') &&
//     non-empty path); represented as a regex pattern with identical semantics.
const { z } = require('zod');
const fs = require('fs');
const yaml = require('js-yaml');

// ---- src/hooks/schema.ts ---------------------------------------------------
const commandActionSchema = z.object({
  type: z.literal('command'),
  command: z.string().min(1).describe('Shell command to run.'),
}).describe('Run a shell command.');

const agentActionSchema = z.object({
  type: z.literal('agent'),
  prompt: z.string().min(1).describe('Prompt injected for the agent to act on.'),
}).describe('Prompt the agent.');

const hookActionSchema = z
  .discriminatedUnion('type', [commandActionSchema, agentActionSchema])
  .meta({ id: 'HookAction' });

const hookConfirmOptionSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  run: z.boolean(),
  continueReason: z.string().optional(),
});

const hookConfirmSchema = z.object({
  question: z.string().min(1),
  options: z.array(hookConfirmOptionSchema).min(1),
}).meta({ id: 'HookConfirm' });

const hookDocumentSchema = z.object({
  name: z.string().min(1),
  description: z.string().optional(),
  trigger: z.string().min(1).describe(
    'Hook trigger. Known triggers in KAS 0.18.2: preToolUse, postToolUse, promptSubmit, agentStop, preTaskExecution, postTaskExecution, sessionStart.'),
  matcher: z.string().optional(),
  action: hookActionSchema,
  timeout: z.number().int().nonnegative().optional().describe('Timeout in seconds (>= 0).'),
  enabled: z.boolean().optional(),
  confirm: hookConfirmSchema.optional(),
}).meta({ id: 'HookDocument' })
  .describe('A hook embedded inline in the agent profile, scoped to the active agent.');

// ---- src/mcp/server-config.ts ----------------------------------------------
// Runtime: z.preprocess(value => isPlainObject(value) ? value : undefined, z.record(z.unknown()).optional())
const mcpServersSchema = z.record(z.string(), z.unknown())
  .meta({ id: 'McpServerWireRecord' })
  .describe(
    'MCP servers to initialize for this agent, keyed by server name. Runtime is tolerant: ' +
    'a non-map value falls back to empty; entries are kept raw and classified per-entry later, ' +
    'so a malformed entry surfaces as a failed server rather than dropping the profile.');

// ---- src/agent.ts ----------------------------------------------------------
// Runtime: source is z.string().refine(s => s.startsWith('file://') && s.slice(7).trim().length > 0)
const knowledgeBaseResourceSchema = z.object({
  type: z.literal('knowledgeBase'),
  source: z.string().regex(/^file:\/\/.*\S/, 'knowledge base source must be a file:// URI with a non-empty path')
    .describe('file:// URI with a non-empty path.'),
  name: z.string().optional(),
  description: z.string().optional(),
  indexType: z.enum(['fast', 'best']).optional(),
  include: z.array(z.string()).optional(),
  exclude: z.array(z.string()).optional(),
  autoUpdate: z.boolean().optional(),
}).meta({ id: 'KnowledgeBaseResource' });

const agentResourceSchema = z.union([
  z.string().describe('Context-file (`file://`) or skill (`skill://`) URI.'),
  knowledgeBaseResourceSchema,
]).meta({ id: 'AgentResource' });

// ---- src/services/custom-agents/types.ts -----------------------------------
const permissionsPolicySchema = z.object({
  rules: z.array(z.object({
    capability: z.string(),
    match: z.array(z.string()).optional(),
    exclude: z.array(z.string()).optional(),
    effect: z.enum(['allow', 'deny', 'ask']),
  })),
  policies: z.array(z.string()).optional()
    .describe('References to named policy bundles, expanded inline into scoped rules.'),
}).meta({ id: 'PermissionsPolicy' })
  .describe('Policy rules that scope down what this agent can do.');

const dispatchKindSchema = z.enum(['sub-agent', 'custom-agent', 'spec'])
  .describe('Controls which dispatch adapter handles this agent\'s execution.');

const COMMON = {
  name: z.string().min(1, 'Name must not be empty').optional()
    .describe('Explicit agent name that overrides filename-based ID.'),
  description: z.string().optional()
    .describe('Human-readable description (defaults to empty string).'),
  excludedTools: z.array(z.string()).optional()
    .describe('Tools to exclude (applied after allowedTools matching).'),
  model: z.string().optional().describe('Model override.'),
  includeMcpJson: z.boolean().default(false).describe(
    'Whether to automatically include MCP tools in the agent\'s available tools. ' +
    'When true, all MCP tools are included. When false (default), MCP tools are only ' +
    'included if explicitly matched by `tools` patterns.'),
  includePowers: z.boolean().default(false).describe(
    'Whether to automatically include Powers tools in the agent\'s available tools. ' +
    'When true, the kiroPowers tool is included. When false (default), Powers tools are ' +
    'only included if explicitly matched by `tools` patterns.'),
  mcpServers: mcpServersSchema.optional(),
  resources: z.array(agentResourceSchema).optional()
    .describe('Context-file (`file://`), skill (`skill://`), and knowledge-base resources.'),
  permissions: permissionsPolicySchema.optional(),
  welcomeMessage: z.string().optional()
    .describe('Message displayed when switching to this agent.'),
  dispatchKind: dispatchKindSchema.optional(),
  hooks: z.array(hookDocumentSchema).optional()
    .describe('Hooks embedded inline in the agent profile, scoped to the active agent.'),
};

const frontMatterSchema = z.object({
  ...COMMON,
  tools: z.union([z.string(), z.array(z.string())]).optional().describe(
    'Comma-separated tool IDs, array of tool IDs, or "*" for all tools. If omitted, agent has no tools.'),
}).meta({ id: 'MarkdownAgentFrontMatter' }).describe(
  'YAML front matter of a Markdown agent file (`.kiro/agents/*.md`). The Markdown body is the system prompt.');

const jsonAgentFileSchema = z.object({
  ...COMMON,
  prompt: z.string().nullish().describe(
    'Prompt content (inline string or file:// URI, defaults to empty string).'),
  tools: z.union([z.literal('*'), z.array(z.string())]).optional()
    .describe('Array of tool IDs or "*" for all tools.'),
}).meta({ id: 'JsonAgentFile' }).describe(
  'A JSON agent file (`.kiro/agents/*.json`) for the KAS (v3) engine.');

// ---- emit -------------------------------------------------------------------
const PROVENANCE =
  'Reconstructed 2026-07-17 from the @kiro/agent (KAS 0.18.2) bundle shipped inside kiro-cli 2.13.0 ' +
  '(src/services/custom-agents/types.ts: CustomAgentFileFrontMatterSchema / JsonAgentFileSchema). ' +
  'NOT an official Kiro artifact. Runtime parsing is TOLERANT: zod z.object without .strict() ' +
  'silently strips unknown keys, so additionalProperties is intentionally left permissive here to ' +
  'mirror runtime; set "additionalProperties": false locally for typo-linting. ' +
  'v2-format marker fields the KAS loader checks for (CLI_ONLY_FIELDS): allowedTools, toolsSettings; ' +
  'KAS_MARKER_FIELDS: permissions. The v2 (Rust) engine agent schema is a DIFFERENT format ' +
  '(see docs/kiro-agent-schema-2.1.1.json) and, unlike this one, rejects unknown fields.';

function emit(schema, opts) {
  const js = z.toJSONSchema(schema, { target: 'draft-2020-12', io: 'input' });
  return {
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    $id: opts.id,
    title: opts.title,
    $comment: PROVENANCE,
    ...js,
  };
}

const jsonDoc = emit(jsonAgentFileSchema, {
  id: 'https://github.com/dwalleck/cyril/docs/kiro-kas-agent-file-schema-2.13.0.json',
  title: 'Kiro KAS (v3) agent file — JSON format',
});
const fmDoc = emit(frontMatterSchema, {
  id: 'https://github.com/dwalleck/cyril/docs/kiro-kas-agent-frontmatter-schema-2.13.0.yaml',
  title: 'Kiro KAS (v3) agent file — Markdown YAML front matter',
});

fs.writeFileSync(process.argv[2], JSON.stringify(jsonDoc, null, 2) + '\n');
const yamlHeader =
  '# Kiro KAS (v3) agent file — Markdown YAML front matter (JSON Schema, draft 2020-12, serialized as YAML)\n' +
  '# Validates the YAML front matter of `.kiro/agents/*.md` agent files for the KAS engine.\n' +
  '# For `.kiro/agents/*.json` agents use the sibling kiro-kas-agent-file-schema-2.13.0.json.\n' +
  '# YAML has no schema language of its own — validators (check-jsonschema, yaml-language-server) consume JSON Schema.\n';
fs.writeFileSync(process.argv[3], yamlHeader + yaml.dump(fmDoc, { lineWidth: 110, noRefs: true }));
console.log('wrote', process.argv[2], 'and', process.argv[3]);

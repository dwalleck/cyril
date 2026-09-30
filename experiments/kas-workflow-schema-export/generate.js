// Reconstructs the KAS workflow-recipe zod schemas carved from the @kiro/agent
// bundle (kiro-cli 2.26.0 / KAS 0.66.15) and emits JSON Schema (draft 2020-12)
// via zod v4's native z.toJSONSchema.
//
// Source (acp-server.js is MINIFIED from 2.20.1 on, so these are transcribed from
// minified zod; the minified binding names are given so a refresh can re-find them):
//   uy   top-level WorkflowDefinition     NH   node discriminatedUnion("type")
//   t0s  step        r0s sequence          n0s  repeat      i0s parallel     o0s watch
//   Fue  StopCondition                     wHt  fileCheck (.strict())
//   vHt  onMaxIterations enum              yHt  joinPolicy enum              nT = 1000
//   Dvn  step `input` tombstone (refine: must be undefined)
//   I9i  common watch config (passthrough)
//   OKi  github-pr handler configSchema    VKi  command handler configSchema
//
// zod refinements do not survive z.toJSONSchema. Each one the runtime enforces is
// re-added below as a JSON Schema keyword in REFINEMENTS, keyed by $defs id, with
// the runtime error text it mirrors; validate.js has a failing fixture for each.
//
// Deliberate stand-in: the step `input` tombstone is z.unknown().refine(v => v ===
// undefined) at runtime; it is represented as z.never().optional() (JSON Schema
// `not: {}`), which has identical semantics for any present value.
const { z } = require('zod');
const fs = require('fs');

const VERSION = '2.26.0';
const KAS = '0.66.15';

// ---- leaf enums ---------------------------------------------------------------
const onMaxIterations = z.enum(['abort', 'continue', 'pause']).describe(
  'What a repeat does when it reaches maxIterations without its stop condition matching: ' +
  '`abort` aborts the repeat AND the run (status `aborted`); `continue` completes the repeat and ' +
  'moves to its next sibling; `pause` parks the run for a human decision. A plain resume of a ' +
  'cap-paused repeat pauses again immediately; `_kiro/workflow/resume` with `extendRepeat ' +
  '{nodeId, additionalIterations}` (KAS 0.66.15+) grants more iterations.');

const joinPolicy = z.enum(['all', 'allSettled', 'any']).describe(
  'When the parallel node settles and whether one branch failing cancels its siblings: `all` = ' +
  'every branch must succeed, the first failure aborts the rest; `allSettled` = wait for every ' +
  'branch regardless of failures; `any` = the first completion wins and the rest are aborted. A ' +
  'PAUSED branch never cancels its siblings.');

const completionSignal = z.enum(['success', 'need_input', 'error']).describe(
  'The step signal recorded from the step agent\'s `send_message` call: severity `success` -> ' +
  '`success`, `warning` -> `need_input`, `error` -> `error` (`info` records nothing).');

// ---- stop conditions ----------------------------------------------------------
const fileCheck = z.object({
  path: z.string().describe(
    'JSON file to read. Relative paths resolve from the primary workspace root; `{{...}}` ' +
    'templates resolve to bare values. A path outside every allowed workspace root FAILS the node.'),
  jsonPath: z.string().describe('Dot path into the parsed JSON (e.g. `verdict`, `result.complete`).'),
  value: z.unknown().describe(
    'Deep-equality target. An ARRAY is an any-of list of candidates; to match a literal array, ' +
    'wrap it: `[["ready", "reviewed"]]` (and `[[]]` for a literal empty array). Required.'),
}).strict().meta({ id: 'FileCheck' }).describe(
  'Matches when the JSON value at `jsonPath` in `path` deep-equals `value`. A missing or unreadable ' +
  'file, invalid JSON, missing path, or mismatch does not match (the repeat continues). STRICT: ' +
  'keys other than path, jsonPath, value are rejected.');

const stopCondition = z.object({
  containsText: z.string().optional().describe('Matches when the latest captured output contains this text.'),
  fileCheck: fileCheck.optional(),
  completionSignal: completionSignal.optional().describe(
    'Matches when the latest step signal equals this value.'),
}).meta({ id: 'StopCondition' }).describe(
  'At least one field is required; when several are present, ANY single match stops the loop ' +
  '(or completes the step).');

// ---- nodes (recursive via getters) --------------------------------------------
const nodeId = z.string().describe(
  'Node id. Must be unique across the whole static recipe (load-time check, not expressible here).');
const modelId = z.string().describe(
  'Model override. `auto` or omission inherits (step -> workflow -> parent session captured at run ' +
  'creation -> the step agent\'s default). An unknown id passes validation with only a warning and ' +
  'then fails at the step\'s first model call with no fallback.');
const effortLevel = z.string().describe(
  'Reasoning-effort override (model-dependent, e.g. low/medium/high/xhigh/max). An unsupported value ' +
  'is reconciled to the model\'s default when the step session is created.');

const stepNode = z.object({
  type: z.literal('step'),
  id: nodeId,
  agent: z.string().describe(
    'Exact, case-sensitive name of a workflow-capable agent: a bundled `wf-*` agent (wf-planner, ' +
    'wf-coder, wf-design, wf-design-reviewer, wf-review-aggregator, wf-pr-submitter, wf-pr-responder, ' +
    'wf-auto-researcher), `semantic_reviewer`, or a custom agent from `.kiro/agents/`. ' +
    '`wf-workflow-creator` is orchestrator-only. A built-in sub-agent is rejected. Custom-agent ' +
    'existence is checked at run creation, not by validate_workflow.'),
  prompt: z.string().describe(
    'The step session\'s user message. `{{...}}` templates interpolate: `{{input}}`, `{{<id>.output}}`, ' +
    '`{{steps.<id>.output}}` (legacy alias), `{{previous.output}}`, `{{artifacts.<name>}}`. Captured ' +
    'outputs are injected wrapped in tamper-resistant delimiters; make the deliverable the step\'s ' +
    'final response.'),
  artifacts: z.record(z.string(), z.string()).optional().describe(
    'Maps logical names to files this step produces; later steps reference `{{artifacts.<name>}}`. A ' +
    'path registry only: the engine never reads or writes the files. Relative paths resolve from the ' +
    'primary workspace root; values are templated.'),
  captureOutput: z.boolean().optional().describe(
    'Capture the step\'s final assistant message for `{{<id>.output}}`. Defaults to true.'),
  completion: stopCondition.optional().describe(
    'Makes the step INTERACTIVE: after each turn the condition is evaluated and, while false, the step ' +
    'waits for another message in its session. Not allowed inside `parallel` (load-time check).'),
  modelId: modelId.optional(),
  effortLevel: effortLevel.optional(),
  input: z.never().optional().describe(
    'REMOVED. Runtime error: "The step-level `input` field was removed; embed the data in `prompt` via ' +
    '{{...}} template references". (On 2.16.0 it was an alternative to prompt that took precedence.)'),
}).meta({ id: 'StepNode' }).describe(
  'Runs one named agent in its own session with fresh context. The agent signals its outcome with ' +
  '`send_message` (success completes, warning pauses for user input, error fails); a turn that ends ' +
  'with no signal and no unmet `completion` completes the step.');

const sequenceNode = z.object({
  type: z.literal('sequence'),
  id: nodeId,
  get steps() { return z.array(workflowNode).describe('Child nodes, run in order.'); },
}).meta({ id: 'SequenceNode' }).describe(
  'Runs child nodes in order. The root `steps` array is already an implicit sequence.');

const repeatNode = z.object({
  type: z.literal('repeat'),
  id: nodeId,
  get steps() { return z.array(workflowNode).describe('The loop body. Always runs at least once.'); },
  maxIterations: z.number().int().positive().max(1000).describe(
    'Iteration cap, 1-1000. Repeat iterations do not consume extra static step slots.'),
  stopCondition: stopCondition.optional().describe(
    'Evaluated AFTER each iteration; a match completes the repeat. At most one of stopCondition / ' +
    'stopWhen; with neither, the repeat runs to maxIterations.'),
  stopWhen: z.string().optional().describe(
    'Sugar form: `"<watchId>.terminal"` (a watch inside the loop latched a terminal outcome) or ' +
    '`"{{expr}} contains <text>"`. The watch id must exist (load-time check).'),
  onMaxIterations,
}).meta({ id: 'RepeatNode' }).describe(
  'Runs child nodes until a stop condition matches or maxIterations is reached. Each iteration gets ' +
  'fresh node state, so a static id appears once per iteration at runtime.');

const parallelNode = z.object({
  type: z.literal('parallel'),
  id: nodeId,
  get branches() {
    return z.array(workflowNode).describe(
      'Independent branches, started concurrently, each with its own context. A branch cannot ' +
      'reference another branch\'s outputs or artifacts, and `{{previous.output}}` is invalid inside one.');
  },
  joinPolicy,
}).meta({ id: 'ParallelNode' }).describe(
  'Schedules independent branches and joins their results. Use it for isolation and join semantics, ' +
  'not assumed speed.');

const watchNode = z.object({
  type: z.literal('watch'),
  id: nodeId,
  handler: z.string().describe(
    'Watch handler id. Built in (KAS 0.66.15): `github-pr`, `command`. (`crux-cr` existed on 2.16.0 ' +
    'and is gone.) Handler availability is checked at run creation, not by validate_workflow.'),
  config: z.record(z.string(), z.unknown()).optional().describe(
    'Handler configuration, validated at run creation. Common keys: `pollIntervalSec` (positive; ' +
    'default 60; below the handler minimum is rejected) and `commandTimeoutSec` (positive). Top-level ' +
    'string values are templated when the watch is entered (except `command`).'),
  idleTimeoutSec: z.number().positive().optional().describe(
    'On the NODE, not in config. After this many idle seconds the watch ends with a terminal outcome ' +
    'and an EMPTY output. The idle clock restarts each time a repeat re-enters the watch. Unset = wait ' +
    'forever.'),
}).meta({ id: 'WatchNode' }).describe(
  'Polls an external system through a handler without spending model turns while nothing changes. ' +
  'On new activity its JSON payload becomes `{{<id>.output}}`; a terminal result latches for ' +
  '`stopWhen: "<id>.terminal"`. Delivery is at-least-once across interruptions.');

const workflowNode = z.discriminatedUnion('type',
  [stepNode, sequenceNode, repeatNode, parallelNode, watchNode]).meta({ id: 'WorkflowNode' });

// ---- top level ------------------------------------------------------------------
const workflowDefinition = z.object({
  name: z.string().describe('Stable recipe name. Name precedence: project > user > synced account > bundled.'),
  description: z.string().optional().describe('Purpose shown by recipe selectors (listRecipes).'),
  inputs: z.record(z.string(), z.string()).optional().default({}).describe(
    'Launch inputs: name -> free-form type hint (`prompt`, `file`, `string`, ...). Hints only guide ' +
    'launch UIs; values reach the recipe as text. Every bare `{{name}}` must be declared here and ' +
    'supplied at launch: an undeclared one is only a WARNING and stays literal in the prompt.'),
  steps: z.array(workflowNode).describe('The workflow tree. Top-level nodes run in order (an implicit sequence).'),
  modelId: modelId.optional().describe('Default model for every step (see StepNode.modelId).'),
  effortLevel: effortLevel.optional().describe('Default reasoning effort for every step.'),
  injectOriginalUserRequest: z.boolean().optional().describe(
    'Undocumented on kiro.dev. Default TRUE: every step\'s first prompt opens with an ' +
    '`<original_user_request>` block quoting the PARENT session\'s user messages verbatim, so steps ' +
    'check their work against the user\'s own words. Set false when the parent conversation is ' +
    'unrelated to the run or private.'),
  planRevision: z.number().int().nonnegative().optional().describe(
    'Runtime bookkeeping for plan revisions (copied into run state). Accepted, but do not author it.'),
});

// ---- refinements re-added after emission ------------------------------------------
const knownHandler = (id) => ({ properties: { handler: { const: id } }, required: ['handler'] });
const REFINEMENTS = {
  FileCheck: (d) => {
    // wHt superRefine: value required; empty array rejected.
    d.required = [...new Set([...(d.required ?? []), 'path', 'jsonPath', 'value'])];
    d.properties.value = { ...d.properties.value, not: { type: 'array', maxItems: 0 } };
  },
  StopCondition: (d) => {
    // Fue refine: "StopCondition requires at least one of containsText, fileCheck, or completionSignal".
    d.anyOf = [{ required: ['containsText'] }, { required: ['fileCheck'] }, { required: ['completionSignal'] }];
  },
  RepeatNode: (d) => {
    // n0s refine: "RepeatNode allows at most one of stopCondition or stopWhen".
    d.not = { required: ['stopCondition', 'stopWhen'] };
  },
  WatchNode: (d) => {
    const positive = { type: 'number', exclusiveMinimum: 0 };
    d.allOf = [
      // I9i: common keys, every handler.
      { properties: { config: { properties: { pollIntervalSec: positive, commandTimeoutSec: positive } } } },
      {
        // OKi refine: "github-pr config requires either `prRef` or `url`."; minPollIntervalSec 30.
        if: knownHandler('github-pr'),
        then: {
          required: ['config'],
          properties: { config: {
            anyOf: [{ required: ['prRef'] }, { required: ['url'] }],
            properties: {
              prRef: { type: 'string', description: 'Workspace JSON file with a top-level `url` (confined to the workspace roots).' },
              url: { type: 'string', description: 'PR URL `https://<host>/<owner>/<repo>/pull/<n>`, or a bare PR number (checked when polling).' },
              pollIntervalSec: { minimum: 30 },
              includeOwnActivity: { type: 'boolean', description: 'Let the authenticated gh identity\'s comments wake the watch. Default false.' },
              ignoreAuthors: { type: 'array', items: { type: 'string' }, description: 'Logins whose activity never wakes the watch.' },
            },
          } },
        },
      },
      {
        // VKi: command min(1); superRefine rejects `{{` in command and any `args` key; minPollIntervalSec 10.
        if: knownHandler('command'),
        then: {
          required: ['config'],
          properties: { config: {
            required: ['command'],
            properties: {
              command: {
                type: 'string', minLength: 1, not: { pattern: '\\{\\{' },
                description: 'One static command line, run by the session\'s default shell ($SHELL; PowerShell on Windows) in the workspace. No `{{...}}` templates.',
              },
              args: false,
              pollIntervalSec: { minimum: 10 },
            },
          } },
        },
      },
    ];
  },
};

// ---- emit -----------------------------------------------------------------------------
const PROVENANCE =
  `Reconstructed 2026-09-30 from the @kiro/agent (KAS ${KAS}) bundle shipped inside kiro-cli ${VERSION} ` +
  '(workflow definition zod + watch-handler configSchemas), cross-checked against the official docs ' +
  '(kiro.dev/docs/workflows/authoring). NOT an official Kiro artifact. Recipe files: ' +
  '`.kiro/workflows/*.workflow.{json,yaml,yml}` (project) and `~/.kiro/workflows/` (user). Runtime ' +
  'parsing is TOLERANT: z.object without .strict() silently strips unknown keys, so additionalProperties ' +
  'stays permissive to mirror runtime (only fileCheck is strict); set it to false locally for ' +
  'typo-linting. NOT expressible here and enforced at load/creation time instead: at most 50 static ' +
  'step nodes and 8 nesting levels; node ids unique across the tree; stopWhen grammar and watch-id ' +
  'references; template happens-before (`{{<id>.output}}` / `{{artifacts.x}}` must come from an ' +
  'earlier producer, no `{{previous.output}}` in a container\'s first node or inside a parallel ' +
  'branch); `completion` steps not inside parallel; fileCheck paths inside the workspace roots; agent ' +
  'and handler registry lookups. The kiro-workflow-authoring skill\'s validate_workflow.py checks the ' +
  'static ones.';

const body = z.toJSONSchema(workflowDefinition, { target: 'draft-2020-12', io: 'input' });
for (const [id, apply] of Object.entries(REFINEMENTS)) {
  const def = body.$defs?.[id];
  if (!def) throw new Error(`refinement target $defs.${id} missing from emitted schema`);
  apply(def);
}

const doc = {
  $schema: 'https://json-schema.org/draft/2020-12/schema',
  $id: `https://github.com/dwalleck/cyril/docs/kiro-kas-workflow-recipe-schema-${VERSION}.json`,
  title: 'Kiro KAS workflow recipe (.workflow.json / .workflow.yaml)',
  description: 'A declarative multi-agent workflow: a tree of step, sequence, repeat, parallel and watch nodes.',
  $comment: PROVENANCE,
  ...body,
};
delete doc.$schema; // re-insert first so key order stays readable
fs.writeFileSync(process.argv[2], JSON.stringify(
  { $schema: 'https://json-schema.org/draft/2020-12/schema', ...doc }, null, 2) + '\n');
console.log('wrote', process.argv[2]);

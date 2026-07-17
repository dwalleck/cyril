// Oracle for the generated KAS agent-file schemas: fixtures that must pass,
// fixtures that must fail, and a v2-format file that must pass (tolerance mirror).
const Ajv2020 = require('ajv/dist/2020');
const fs = require('fs');
const yaml = require('js-yaml');

const jsonSchema = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const fmYamlText = fs.readFileSync(process.argv[3], 'utf8');
const fmSchema = yaml.load(fmYamlText);

const ajv = new Ajv2020({ strict: false, allErrors: true });
const vJson = ajv.compile(jsonSchema);
const vFm = ajv.compile(fmSchema);

const cases = [
  ['PASS json: full valid agent', vJson, {
    name: 'reviewer', description: 'second-opinion reviewer',
    prompt: 'file://prompts/reviewer.md',
    tools: ['read', 'grep'], excludedTools: ['shell'],
    model: 'claude-sonnet-4.6', includeMcpJson: false, includePowers: true,
    mcpServers: { probe: { command: 'probe-mcp', args: [] } },
    resources: ['file://README.md', 'skill://review',
      { type: 'knowledgeBase', source: 'file://kb/', indexType: 'best', autoUpdate: true }],
    permissions: { rules: [{ capability: 'fs', match: ['src/**'], effect: 'allow' }], policies: ['readonly'] },
    welcomeMessage: 'hi', dispatchKind: 'custom-agent',
    hooks: [{ name: 'lint', trigger: 'postToolUse', matcher: 'fs_write',
      action: { type: 'command', command: 'cargo clippy' }, timeout: 30, enabled: true,
      confirm: { question: 'run?', options: [{ id: 'y', label: 'yes', run: true }] } }],
  }, true],
  ['PASS json: minimal empty agent', vJson, {}, true],
  ['PASS json: tools star literal', vJson, { tools: '*' }, true],
  ['FAIL json: tools as csv string (md-only form)', vJson, { tools: 'read,grep' }, false],
  ['FAIL json: bad dispatchKind', vJson, { dispatchKind: 'subagent' }, false],
  ['FAIL json: bad permission effect', vJson, { permissions: { rules: [{ capability: 'fs', effect: 'allow-all' }] } }, false],
  ['FAIL json: hook missing action', vJson, { hooks: [{ name: 'x', trigger: 'agentStop' }] }, false],
  ['FAIL json: kb source not file://', vJson, { resources: [{ type: 'knowledgeBase', source: 'kb/' }] }, false],
  ['FAIL json: kb source empty path', vJson, { resources: [{ type: 'knowledgeBase', source: 'file://  ' }] }, false],
  ['FAIL json: empty name', vJson, { name: '' }, false],
  ['FAIL json: negative hook timeout', vJson, { hooks: [{ name: 'x', trigger: 't', action: { type: 'agent', prompt: 'p' }, timeout: -1 }] }, false],
  ['PASS fm: csv tools string', vFm, { name: 'a', tools: 'read,grep' }, true],
  ['PASS fm: arbitrary tools string', vFm, { tools: 'anything goes here' }, true],
  ['FAIL fm: prompt not a front-matter field (strict-lint only)', vFm, { prompt: 'x' }, true /* tolerant: unknown keys allowed */],
];

let bad = 0;
for (const [label, v, data, expect] of cases) {
  const ok = v(data);
  const verdict = ok === expect ? 'OK ' : 'MISMATCH';
  if (ok !== expect) bad++;
  console.log(`${verdict} ${label}${ok !== expect ? ' — errors: ' + JSON.stringify(v.errors?.slice(0, 2)) : ''}`);
}

// Tolerance mirror: a v2-format agent file (allowedTools/toolsSettings) must PASS,
// because KAS's runtime z.object silently strips unknown keys.
const v2file = JSON.parse(fs.readFileSync(process.argv[4] ?? (process.env.HOME + '/.kiro/agents/dg-dinesh.json'), 'utf8'));
const okV2 = vJson(v2file);
console.log(`${okV2 ? 'OK ' : 'MISMATCH'} PASS json: v2-format file (dg-dinesh.json) accepted via tolerance${okV2 ? '' : ' — ' + JSON.stringify(vJson.errors?.slice(0, 3))}`);
if (!okV2) bad++;

console.log(bad === 0 ? 'ALL GREEN' : `${bad} MISMATCHES`);
process.exit(bad === 0 ? 0 : 1);

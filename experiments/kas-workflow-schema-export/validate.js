// Oracle for the generated KAS workflow-recipe schema: real recipes that must pass
// (the official docs' examples in fixtures/, and this repo's own .kiro/workflows/),
// then one inline case per constraint, including every refinement generate.js
// re-adds, and a tolerance mirror (unknown keys pass, as the runtime strips them).
const Ajv2020 = require('ajv/dist/2020');
const fs = require('fs');
const path = require('path');
const yaml = require('js-yaml');

const schema = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const ajv = new Ajv2020({ strict: false, allErrors: true });
const validate = ajv.compile(schema);

const load = (file) => (/\.ya?ml$/.test(file)
  ? yaml.load(fs.readFileSync(file, 'utf8'))
  : JSON.parse(fs.readFileSync(file, 'utf8')));

const step = (id, extra = {}) => ({ type: 'step', id, agent: 'wf-coder', prompt: 'p', ...extra });
const recipe = (steps, extra = {}) => ({ name: 'r', steps, ...extra });
const repeat = (extra = {}) => ({
  type: 'repeat', id: 'loop', maxIterations: 3, onMaxIterations: 'continue', steps: [step('b')], ...extra,
});
const watch = (handler, config, extra = {}) => ({ type: 'watch', id: 'w', handler, config, ...extra });
const fc = (fileCheck) => repeat({ stopCondition: { fileCheck } });

const cases = [];
// ---- real recipes -------------------------------------------------------------------
const fixtureDir = path.join(__dirname, 'fixtures');
for (const f of fs.readdirSync(fixtureDir).filter((f) => /\.workflow\.(json|ya?ml)$/.test(f)).sort()) {
  cases.push([`PASS fixture: ${f}`, load(path.join(fixtureDir, f)), true]);
}
// engine-cases/ are also run against the LIVE engine by engine_diff.py; the schema
// must give the same verdict, except -SCHEMA-PASS- (a registry lookup the schema
// deliberately leaves to run creation).
const engineDir = path.join(__dirname, 'engine-cases');
for (const f of fs.readdirSync(engineDir).filter((f) => f.endsWith('.workflow.json')).sort()) {
  cases.push([`engine case: ${f}`, load(path.join(engineDir, f)), !f.includes('-FAIL-')]);
}
const repoRecipes = path.join(__dirname, '..', '..', '.kiro', 'workflows');
for (const f of fs.readdirSync(repoRecipes).filter((f) => /\.workflow\.(json|ya?ml)$/.test(f)).sort()) {
  cases.push([`PASS repo recipe: .kiro/workflows/${f}`, load(path.join(repoRecipes, f)), true]);
}

// ---- top level -------------------------------------------------------------------------
cases.push(
  ['PASS minimal recipe', recipe([step('a')]), true],
  ['PASS every top-level field', recipe([step('a')], {
    description: 'd', inputs: { task: 'prompt', dir: 'file' }, modelId: 'auto', effortLevel: 'high',
    injectOriginalUserRequest: false, planRevision: 0 }), true],
  ['PASS tolerance: unknown top-level key (runtime strips it)', recipe([step('a')], { _comment: 'x', $schema: 'y' }), true],
  ['FAIL missing name', { steps: [step('a')] }, false],
  ['FAIL missing steps', { name: 'r' }, false],
  ['FAIL non-string input type hint', recipe([step('a')], { inputs: { n: 3 } }), false],
  ['FAIL injectOriginalUserRequest not boolean', recipe([step('a')], { injectOriginalUserRequest: 'no' }), false],
  ['FAIL negative planRevision', recipe([step('a')], { planRevision: -1 }), false],
  ['FAIL unknown node type', recipe([{ type: 'loop', id: 'x', steps: [] }]), false],
);

// ---- step ---------------------------------------------------------------------------------
cases.push(
  ['PASS step with every optional field', recipe([step('a', {
    artifacts: { plan: '{{dir}}/plan.md' }, captureOutput: false, completion: { completionSignal: 'success' },
    modelId: 'claude-opus-5.5', effortLevel: 'max' })]), true],
  ['PASS tolerance: unknown step key', recipe([step('a', { notes: 'x' })]), true],
  ['FAIL step missing prompt', recipe([{ type: 'step', id: 'a', agent: 'wf-coder' }]), false],
  ['FAIL step missing agent', recipe([{ type: 'step', id: 'a', prompt: 'p' }]), false],
  ['FAIL step-level input (removed field)', recipe([step('a', { input: '{{w.output}}' })]), false],
  ['FAIL artifact path not a string', recipe([step('a', { artifacts: { x: 1 } })]), false],
);

// ---- stop conditions -------------------------------------------------------------------------
cases.push(
  ['PASS stopCondition containsText only', recipe([repeat({ stopCondition: { containsText: 'DONE' } })]), true],
  ['PASS stopCondition with several fields', recipe([repeat({ stopCondition: {
    containsText: 'DONE', completionSignal: 'success' } })]), true],
  ['FAIL empty stopCondition (needs one field)', recipe([repeat({ stopCondition: {} })]), false],
  ['FAIL bad completionSignal', recipe([repeat({ stopCondition: { completionSignal: 'warning' } })]), false],
  ['PASS fileCheck scalar value', recipe([fc({ path: 's.json', jsonPath: 'done', value: true })]), true],
  ['PASS fileCheck any-of array', recipe([fc({ path: 's.json', jsonPath: 'v', value: ['PASS', 'OK'] })]), true],
  ['PASS fileCheck wrapped literal array', recipe([fc({ path: 's.json', jsonPath: 'l', value: [['a', 'b']] })]), true],
  ['PASS fileCheck wrapped literal empty array', recipe([fc({ path: 's.json', jsonPath: 'l', value: [[]] })]), true],
  ['PASS fileCheck null value (present)', recipe([fc({ path: 's.json', jsonPath: 'x', value: null })]), true],
  ['FAIL fileCheck missing value', recipe([fc({ path: 's.json', jsonPath: 'done' })]), false],
  ['FAIL fileCheck empty-array value', recipe([fc({ path: 's.json', jsonPath: 'l', value: [] })]), false],
  ['FAIL fileCheck unknown key (strict)', recipe([fc({ path: 's.json', jsonPath: 'x', value: 1, expected: 1 })]), false],
  ['FAIL fileCheck missing jsonPath', recipe([fc({ path: 's.json', value: 1 })]), false],
);

// ---- repeat / sequence / parallel -------------------------------------------------------------
cases.push(
  ['PASS repeat with neither stop form (runs to cap)', recipe([repeat()]), true],
  ['PASS repeat stopWhen', recipe([repeat({ stopWhen: 'w.terminal' })]), true],
  ['PASS repeat maxIterations 1000', recipe([repeat({ maxIterations: 1000 })]), true],
  ['FAIL repeat with both stopCondition and stopWhen', recipe([repeat({
    stopWhen: 'w.terminal', stopCondition: { containsText: 'x' } })]), false],
  ['FAIL repeat maxIterations 0', recipe([repeat({ maxIterations: 0 })]), false],
  ['FAIL repeat maxIterations 1001', recipe([repeat({ maxIterations: 1001 })]), false],
  ['FAIL repeat maxIterations not integer', recipe([repeat({ maxIterations: 2.5 })]), false],
  ['FAIL repeat missing onMaxIterations', recipe([repeat({ onMaxIterations: undefined })]), false],
  ['FAIL repeat bad onMaxIterations', recipe([repeat({ onMaxIterations: 'stop' })]), false],
  ['PASS nested sequence in parallel in repeat', recipe([repeat({ steps: [{
    type: 'parallel', id: 'p', joinPolicy: 'any', branches: [
      { type: 'sequence', id: 's', steps: [step('x'), step('y')] }, step('z')] }] })]), true],
  ['FAIL parallel bad joinPolicy', recipe([{ type: 'parallel', id: 'p', joinPolicy: 'race', branches: [step('x')] }]), false],
  ['FAIL parallel missing branches', recipe([{ type: 'parallel', id: 'p', joinPolicy: 'all' }]), false],
  ['FAIL invalid node deep in the tree', recipe([repeat({ steps: [{
    type: 'sequence', id: 's', steps: [{ type: 'step', id: 'bad', agent: 'wf-coder' }] }] })]), false],
);

// ---- watch ----------------------------------------------------------------------------------------
cases.push(
  ['PASS github-pr with prRef', recipe([watch('github-pr', { prRef: '{{run_dir}}/pr.json' }, { idleTimeoutSec: 3600 })]), true],
  ['PASS github-pr with url + every option', recipe([watch('github-pr', {
    url: 'https://github.com/o/r/pull/1', pollIntervalSec: 30, includeOwnActivity: true,
    ignoreAuthors: ['bot'], commandTimeoutSec: 20 })]), true],
  ['FAIL github-pr without config', recipe([{ type: 'watch', id: 'w', handler: 'github-pr' }]), false],
  ['FAIL github-pr without prRef or url', recipe([watch('github-pr', { pollIntervalSec: 60 })]), false],
  ['FAIL github-pr poll below 30', recipe([watch('github-pr', { url: 'u', pollIntervalSec: 10 })]), false],
  ['FAIL github-pr ignoreAuthors not an array', recipe([watch('github-pr', { url: 'u', ignoreAuthors: 'bot' })]), false],
  ['PASS command with own keys', recipe([watch('command', {
    command: 'node watch.mjs', reviewFile: '{{f}}', pollIntervalSec: 10, commandTimeoutSec: 5 })]), true],
  ['FAIL command missing command', recipe([watch('command', { pollIntervalSec: 60 })]), false],
  ['FAIL command empty command', recipe([watch('command', { command: '' })]), false],
  ['FAIL command with template in command', recipe([watch('command', { command: 'node {{script}}' })]), false],
  ['FAIL command with args key', recipe([watch('command', { command: 'node w.mjs', args: ['--once'] })]), false],
  ['FAIL command poll below 10', recipe([watch('command', { command: 'x', pollIntervalSec: 5 })]), false],
  ['FAIL commandTimeoutSec zero', recipe([watch('command', { command: 'x', commandTimeoutSec: 0 })]), false],
  ['FAIL idleTimeoutSec zero', recipe([watch('command', { command: 'x' }, { idleTimeoutSec: 0 })]), false],
  ['PASS tolerance: unknown handler (registry is checked at run creation)', recipe([watch('crux-cr', { crId: '1' })]), true],
  ['FAIL watch missing handler', recipe([{ type: 'watch', id: 'w', config: {} }]), false],
);

let bad = 0;
for (const [label, data, expect] of cases) {
  const ok = validate(data);
  if (ok !== expect) bad++;
  console.log(`${ok === expect ? 'OK      ' : 'MISMATCH'} ${label}` +
    (ok !== expect && !ok ? ` — ${JSON.stringify(validate.errors.slice(0, 2))}` : ''));
}
console.log(bad === 0 ? `ALL GREEN (${cases.length} cases)` : `${bad} MISMATCHES of ${cases.length}`);
process.exit(bad === 0 ? 0 : 1);

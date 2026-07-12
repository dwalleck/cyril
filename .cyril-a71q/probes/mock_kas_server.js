#!/usr/bin/env node
const fs = require('fs');
const readline = require('readline');
const scenario = process.env.PROBE_SCENARIO;
const trace = process.env.MOCK_TRACE;
let promptCount = 0;
let firstPromptId;
function log(s) { fs.appendFileSync(trace, s + '\n'); }
function send(x) { process.stdout.write(JSON.stringify(x) + '\n'); }
function result(id, value) { send({ jsonrpc: '2.0', id, result: value }); }
function turnEnd(session, owner) {
  log(`emit turn_end session=${session} owner=${owner}`);
  send({ jsonrpc: '2.0', method: 'session/update', params: {
    sessionId: session, update: { sessionUpdate: 'session_info_update',
      _meta: { kiro: { kind: 'turn_end', stopReason: 'end_turn',
        turnEnd: { stopReason: 'end_turn' } } } } } });
}
function prompt(msg) {
  promptCount += 1;
  const id = msg.id;
  const names = scenario === 'same' ? ['A', 'B', 'C']
    : scenario === 'cross' ? ['B', 'C', 'D'] : ['R1', 'R2', 'R3'];
  const name = names[promptCount - 1];
  log(`recv prompt name=${name} id=${id}`);
  if (scenario === 'response_only' && promptCount <= 2) {
    log(`respond prompt owner=${name}`);
    result(id, { stopReason: 'end_turn' });
  } else if (scenario === 'same' && promptCount === 1) {
    firstPromptId = id;
    turnEnd('sess_main', 'A-scoped');
  } else if (scenario === 'same' && promptCount === 2) {
    log('respond late prompt owner=A');
    result(firstPromptId, { stopReason: 'end_turn' });
    setTimeout(() => turnEnd('sess_main', 'B-owned'), 250);
  } else if (scenario === 'cross' && promptCount === 1) {
    turnEnd('sess_foreign', 'X-foreign');
    setTimeout(() => turnEnd('sess_main', 'B-owned'), 250);
  }
}
readline.createInterface({ input: process.stdin }).on('line', line => {
  let msg;
  try { msg = JSON.parse(line); }
  catch (error) { log(`invalid json=${error.message}`); return; }
  log(`recv method=${msg.method || 'response'} id=${msg.id ?? '-'}`);
  if (msg.method === 'initialize') result(msg.id, { protocolVersion: 1,
    agentCapabilities: { loadSession: true, _meta: { kiro: {} } } });
  else if (msg.method === 'session/new') result(msg.id, { sessionId: 'sess_main' });
  else if (msg.method === 'session/prompt') prompt(msg);
  else if (msg.id !== undefined) result(msg.id, {});
});
log(`start scenario=${scenario} argv=${process.argv.slice(2).join('|')}`);

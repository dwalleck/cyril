# Evidence: cyril-a5wo

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | Current KAS ACP frames cannot carry absent or partial `rawInput`. | In the extracted `@kiro/agent` 0.38.7 source, do all production `tool_call` and `tool_call_update` emission paths require a complete `rawInput` value? | PASS |
| N1 | The six committed cancel captures expose one same-identifier `pending` → `in_progress` → `failed` lifecycle. | N/A — current applicable evidence already covers it: `.cyril-a5wo/findings.md` lines 19–35 and 56–76 plus the six immutable captures. | N/A — already evidenced |
| N2 | Cyril must preserve one committed call, guarded fields, and terminal liveness when replaying that lifecycle. | N/A — this is requested behavior from `route.md` T4, not a claim about an existing external system. | N/A — behavior to build |

## Data

- Source: production-shaped data — the actual self-extracted kiro-cli 2.18.1 KAS package at `~/.local/share/kiro-cli/kas/2.18.1-b23bc10526e6c0197a381b39bc956c5edf18c5a041a765d7597a68c97e191d90/node_modules/@kiro/agent/`.
- Shape: `@kiro/agent` 0.38.7 (`package.json` lines 1–5); `dist/server/acp-server.js` is the shipped production server, 521,553 LF-delimited lines, SHA-256 `965ae084945a48eb73fe2049feed7e3deb6fb8d8a9cf49aa4713b172ed3fb70a`.
- Safety: both probe and oracle read the local extracted package without launching KAS or mutating package, session, credential, or production state.

## Probe

- File: `probe-wire-raw-input.py`
- Mechanism: a pinned Python lexical scanner verifies package identity and source SHA, enumerates every `this.toolCallEmitter.toolCall(...)` and `.toolCallUpdate(...)` call by balanced parentheses, classifies each call's `rawInput` member, and checks the emitter boundary plus three explicit path-only streaming producers.
- Run:

  ```sh
  ./.cyril-a5wo/probe-wire-raw-input.py \
    ~/.local/share/kiro-cli/kas/2.18.1-b23bc10526e6c0197a381b39bc956c5edf18c5a041a765d7597a68c97e191d90/node_modules/@kiro/agent/dist/server/acp-server.js \
    ~/.local/share/kiro-cli/kas/2.18.1-b23bc10526e6c0197a381b39bc956c5edf18c5a041a765d7597a68c97e191d90/node_modules/@kiro/agent/package.json
  ```

- Output: 4 initial `tool_call` sites and 15 `tool_call_update` sites. Initial calls: 3 carry `rawInput`, line 488444 omits it. Updates: 5 carry `rawInput`; lines 488489, 488499, 488544, 488550, 488558, 488578, 488659, 488667, 488727, and 488837 omit it. Explicit partial producers: append line 444399, write line 444615, replace line 445884. Final output: `verdict=REPRESENTABLE absent=true partial=true`.

## Oracle

- Mechanism: the independent oracle parses the JavaScript AST with `ast-grep`, rather than using the probe's textual parenthesis scanner. Three structural queries enumerate `$OBJ.toolCall($ARG)`, `$OBJ.toolCallUpdate($ARG)`, and `this.emitStreamingAction($$$ARGS)`; each returned argument object was then hand-counted against the cited source. This differs from both the probe's lexical mechanism and Cyril's Rust ACP conversion.
- Run: invoke OMP `xd://ast_grep` on the pinned `acp-server.js` with each of these patterns:

  ```text
  $OBJ.toolCall($ARG)
  $OBJ.toolCallUpdate($ARG)
  this.emitStreamingAction($$$ARGS)
  ```

- Output: the AST oracle independently found the same 4 initial sites (488290, 488444, 488823, 488917) and 15 update sites (487977, 488299, 488315, 488489, 488499, 488544, 488550, 488558, 488578, 488659, 488667, 488727, 488837, 488934, 488959). It found eight streaming partial-input producers at 444394, 444406, 444610, 444621, 445879, 445893, 465258, and 465550; the path-only argument subsets are visible at 444399, 444615, and 445884.

## Source trace

- `ToolCallEmitter` makes initial `rawInput` conditional (`acp-server.js` 459232–459247) and passes update patches through unchanged (459249–459250). The wire therefore permits omission at both boundaries.
- `SyncTool::emitStreamingAction` explicitly documents that it emits `AgentExecutionAction` with partial `rawInput` when only a subset of streamed parameters has arrived (429385–429396).
- Append, write, and replace tools emit path-only initial subsets and later expanding argument objects (444394–444411, 444610–444627, 445879–445899). The second implementation set does the same with conditional `text`, `oldStr`, and `newStr` members (465258–465270, 465550–465563).
- `ACPEventAdapter::handleToolAction` forwards `event.rawInput ?? event.input` to the initial frame and subsequent updates without completeness validation (488864–488941), and forwards the same value on terminal updates (488943–488971).
- Absence is not theoretical: the `user_input` initial `tool_call` omits `rawInput` (488440–488450), while status-only completion/failure updates omit it at the ten update sites listed above.

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | 4 initial sites / 15 update sites; absent at 1 initial and 10 updates; three pinned path-only streaming producers; `REPRESENTABLE absent=true partial=true`. | Same 4 / 15 sites from parsed AST; same absent-member classification; eight AST-visible streaming partial producers including the same three path-only subsets. | PASS |

## Validated / learned

- P1: new learning — six live subagent-cancel attempts correctly showed complete `rawInput` for that `agent-subtask` path, but they did not establish the whole ACP emission contract. Current production source proves absent `rawInput` is emitted for user-input and status-only paths, while partial `rawInput` is emitted by streaming append/write/replace tools and forwarded unchanged to ACP. Revised acceptance criterion 3 is therefore reachable and requires a source-derived fixture plus conversion/display fences.

## Related issues

- Consulted: `cyril-a5wo`; `cyril-2yn8` covers trusted v2 tool identity and the `rawInput` camelCase echo quirk but not KAS partial-input representability; `cyril-ebqu` relies on complete KAS `agent-subtask` initial input for crew rendering but does not cover interruption or streaming partial input. No prior issue covers P1 or the required fences.
- Filed: none — representability is expected shipped behavior, not an upstream defect or deferred feature.

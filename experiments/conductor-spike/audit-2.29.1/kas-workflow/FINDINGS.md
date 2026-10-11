# kas-workflow lane — kiro-cli 2.28.0 → 2.29.1 workflow audit

Scope: `_kiro/workflow/*`, lifecycle events, snapshot/state fields, `recipes_changed`, `load` vs `inspect`,
`resume`/`extendRepeat`, auto-wake, the step-session `consentRound` loop, watch handlers, the recipe schema,
bundled recipes/agents, `run_workflow`/`save_workflow_definition`, `_kiro/session/notify`, gate-off reachability.
OLD = KAS 0.66.26 (`kas-carves/2.28.0/…/@kiro/agent/dist/server/acp-server.js`, 12,295,918 B).
NEW = KAS 0.66.33 (`kas-carves/2.29.1/…/@kiro/agent/dist/server/acp-server.js`, 12,669,464 B).
Every add/remove claim carries per-token occurrence counts in BOTH bundles + quoted context (Trap 2/3).
**Evidence class:** rows are STATIC string-literal / zod-schema / diagnostic-message diffs, plus **body lexeme
comparisons** for the workflow handlers and auto-wake bodies (the complete token stream — identifiers, numbers,
strings, keywords, operators, punctuation — aligned with a learned consistent rename map). Equal token counts alone
prove *vocabulary, schema and message* identity; the lexeme comparisons additionally prove *control-flow equivalence
modulo identifier renames* for the functions named in Method notes, and that `_kiro/workflow/new` changed only by
include machinery.
Scripts (written fresh, in this dir): `static-kas-workflow-{vocab,counts,region,anchors,recipezod,include,include-wiring,include2,resolver,tools,tools2,tools3,docs,rows,final,surface2,wake,last,last2,advisory,advisory2,loadinspect,lex}-2.29.1.py`.
Raw dumps: `raw-*.txt` + `raw-vocab-diff.json` + `raw-lex-*.json` + `raw-lex-<label>-{old,new}.txt` (see Method notes).

## Summary

1. The workflow **control plane vocabulary/schema is unchanged**: all 25 quoted `_kiro/workflow/*` literals
   (14 methods + 11 lifecycle events), the 9-event lifecycle map, the 3 watch-handler ids, the 9 `wf-*` agent ids,
   the 5-node recipe union, the `_kiro/session/notify` payload, `recipes_changed`, `resume`/`extendRepeat` messages,
   the `run_workflow`/`save_workflow_definition` schemas, the `extensionMethods` method array, the persistence
   classification, and the auto-wake literals — **0 added / 0 removed tokens, matching counts** in every set.
2. What IS new in 0.66.33: a **sixth recipe node type `include`** (recipe composition — absent from feed.json and
   from cyril's pinned recipe schema) and the wire field **`includedFrom {name,path}`** on expanded `sequence`
   descriptors. Both are new (`includedFrom` 0→8, `includeResolver` 0→13, `inForce` 0→34).
3. **`consentRound` was already on the wire in 0.66.26** on `session/request_permission._meta.kiro`
   (raw 1 occurrence). What is new is its *propagation*: the `session_info_update` kinds
   `pending_interaction`/`interaction_resolved` now carry it, a persisted `interaction_resolved` schema carries
   it, and round-aware interaction dedup was added (`consentRound` 1→17). **NEW only** (the propagation + dedup).
4. cyril **DROPS** the two genuinely new wire items (`includedFrom`, `consentRound` propagation): unknown inbound
   fields are silently ignored, and neither `pending_interaction` nor `interaction_resolved` is dispatched.
5. **Gate:** `_kiro/workflow/*` ext methods are advertised unconditionally (`extensionMethods:[…,…v9r,…]`);
   `settings.workflows.enabled` gates only the **agent tool pool** (`run_workflow`/`inspect_workflow`/…), not the
   ext methods. So `_kiro/workflow/new` is reachable gate-OFF; gate ON is needed only for the model to call
   `run_workflow` itself. `recipes_changed`/recipe-catalog state are gate-ON-only.
6. **No changelog line maps to a workflow delta** — every 2.29.0/2.29.1 item and every 2.28.0 baseline item is
   outside this lane. The two workflow deltas are unannounced.
7. **Body-level identity was then checked rigorously** by comparing the **complete lexical token stream** (identifiers,
   numbers, string/template chunks, keywords, operators, punctuation) of each extracted function with a
   difflib alignment and a learned rename map; a difference is accepted only if it is a single consistent bijection
   on identifiers with **every non-identifier lexeme byte-identical**. Result: `_kiro/workflow/{load,inspect,resume}`
   and the auto-wake bodies are **control-flow equivalent modulo renames** — load 384/384 tokens (8 renames),
   inspect 109/109 (4), resume 716/716 (20), `sendParentNote` 105/105 (6), `reserveWakePrompt` 46/46 (0),
   `startInfoWake` 162/162 (11), **0 non-identifier changes in each**. `_kiro/workflow/new` 1073→1131 tokens with
   15 insert/delete hunks, **all of them the include machinery** (`if(g1t(c.workflow)…)`, `sxo(e.host,…)` resolver,
   `recipeInForce`) — no operator/number/string/keyword change. Full OLD/NEW function texts: `raw-lex-<label>-{old,new}.txt`.

## Deltas

| item | version | gate/default | direction | STATIC/LIVE | cyril verdict (file:line) | proposed issue? |
|---|---|---|---|---|---|---|
| `_kiro/workflow/*` method set (25 distinct literals; 14 methods) + the `extensionMethods` array (`v$r`≡`v9r`, 14 workflow methods) | 2.29.1 | advertised unconditionally in `initialize` result (`…,…v9r,…`); NOT gated by `settings.workflows` | **unchanged** (all counts equal: `/new` 16→16, `/resume` 10→10, `/listRecipes` 10→10, `/inspect` 8→8; 0 added / 0 removed) | STATIC | HANDLED | no |
| Lifecycle events `node_start/node_complete/node_paused/run_start/run_complete/loop_iteration/watch_poll/paused/steps_queued` (map `hBc`→`aou`, 9 key/value literals) | 2.29.1 | gate-independent | **unchanged**: map identical; `node_paused` 20→20, `run_complete` 13→13, `run_start` 4→4 | STATIC (2.28.0 LIVE: same ordering, auto-wake both gate states) | HANDLED | no |
| Recipe node union `{step,sequence,repeat,parallel,watch}` + enums `completionSignal`/`onMaxIterations`/`joinPolicy` | 2.29.1 | — | **unchanged** (5 types both; `k.enum(["success","need_input","error"])` 1→1; zod-key raw counts shift ±1 — `onMaxIterations` 28→27, `joinPolicy` 13→14, `idleTimeoutSec` 8→9, `captureOutput` 11→12 — bundler/format only; union text differs solely by `includedFrom` on `sequence`) | STATIC | HANDLED | no |
| Watch handlers `github-pr`/`command`/`background-process`; `_kiro/workflow/listWatchHandlers` | 2.29.1 | handler registered at run creation | **unchanged** (`background-process` 1→1, `github-pr` 6→6, `listWatchHandlers` 6→6) | STATIC (2.28.0 LIVE) | HANDLED | no |
| Bundled `wf-*` agent ids (9) + bundled recipe registry (7: autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph, semantic-review-multi-model) | 2.29.1 | — | **unchanged sets** (quoted counts shift, e.g. `"wf-planner"` 5→12 raw 14→14, `"wf-coder"` 12→15 — embedding/rename only) | STATIC | N/A (KAS-side resources) | no |
| **NEW recipe node `include`**: `oKc=k.object({type:k.literal("include"),id:k.string().regex(/^[A-Za-z0-9_-]+$/),path:k.string().min(1),inputs:k.record(k.string()).optional().default({}),includedFrom:wfe.optional()}).strict()` | 0.66.33 (absent in 0.66.26: `"include"` 3→6, `includedFrom` 0) | none (fully wired: `includeResolver` 0→13; bundled cap `Gho=50`; included recipes may not include) | **added** | STATIC | N/A at runtime (expanded to a `sequence` before the wire) — but cyril's pinned schema doc lacks it: `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` `$defs.WorkflowNode` has 5 variants | **yes** — refresh recipe schema to 0.66.33 |
| **NEW `includedFrom:{name,path}`** on `sequence` node descriptor: `case"sequence":return{nodeId:e.id,type:e.type,steps:e.steps.map(…),…e.includedFrom!==void 0&&{includedFrom:e.includedFrom}}` | 0.66.33 | none | **added** (0→8) | STATIC | **DROPPED**: `crates/cyril-core/src/protocol/convert/kas/workflow.rs:340-346` (`WireNodeDescriptor::Sequence{node_id,steps}`) and re-projection at `:1410` never emit it; `WorkflowNodeType` has no `include` (`crates/cyril-core/src/types/workflow.rs:172-178`) | yes — 1-line descriptor field |
| **`consentRound` propagation** (NEW): wire `session_info_update` kinds `pending_interaction`/`interaction_resolved` carry it, persisted `interaction_resolved` zod carries it, round-aware dedup `uPt(e,t){return e.toolCallId===t.toolCallId&&e.consentRound===t.consentRound}` | 0.66.33 | none (per-approval) | **added** (`consentRound` raw 1→17; zod-key 0→2; `vce(e){return e.consentRound===void 0?{}:{consentRound:e.consentRound}}`) | STATIC | **DROPPED**: `crates/cyril-core/src/protocol/convert/kas.rs:525` (`_ => None` — neither kind is dispatched); no `consentRound` in `crates/` | yes — attach to cyril-w55l |
| `session/request_permission._meta.kiro.consentRound` | — | — | **NOT new — baseline in 0.66.26** (`…d&&{consent:d},…p!==void 0&&{consentRound:p}`); unchanged | STATIC | (cyril kas requestPermission path; kas-core lane) | no |
| `agent_note` payload `+noteUserRequests?:string[]`; summary payloads `+summaryUserRequests/summaryNoteTag/summaryNoteKind/summaryKeys` | 0.66.33 | — | **added** (`noteUserRequests` 0→5, `summaryUserRequests` 0→9) | STATIC | DROPPED (session-message payloads; no field in `crates/`) — **cross-ref kas-core** | note for kas-core |
| `_kiro/workflow/new` recipe-source gate: `vau={prefix:"_kiro/workflow/new: ",field:"workflowPath",notInForce:e=>"the workflow recipe at '<e>' is not available in this session's configuration."}`; `recipeInForce`/`inForce` 0→7/0→34; include-path gate `Fau` | 0.66.33 | session recipe scope/admission | **added** (error strings 0→2) | STATIC | NOT-SENT (cyril never resolves recipes; server-side text only) | no (informational) |
| `run_workflow` schema (`Itu`≡OLD `t4c`) and `save_workflow_definition` schema (`Xtu`≡OLD `_4c`) | 2.29.1 | tool pool gated by `settings.workflows` | **unchanged** (`workflow:{…},name:…` identical; `save_workflow_definition` 10→10, `run_workflow` 45→45) | STATIC | HANDLED | no |
| `_kiro/session/notify` payload (`cAt`≡OLD `v0t`) | 2.29.1 | — | **unchanged** (`{sessionId,callerSessionId?,message,severity,sender,workflowId?,nodeId?,agentName?}`; literal 2→2) | STATIC | DROPPED (kas-core lane; unchanged) | no |
| `recipes_changed {sessionId,workspacePaths,recipes[]}` + recipe-catalog `stateRecipes` | 2.29.1 | **gate ON only** (`settings.value(Di.workflows)==="on"`; 2.28.0 LIVE confirmed 2× vs 0×) | **unchanged** (2→2) | STATIC | HANDLED | no |
| Auto-wake (`sendParentNote`, `parent_note`, `network-give-up`/`network-waiting`, `wakePolicy`, `pendingWakePrompts`, `deliverRuntimeNoteToParent`) | 2.29.1 | gate ON and OFF (2.28.0 LIVE) | **unchanged** — literals 3→3, 3→3, 1→1/1→1, 4→4, 6→6; **full-lexeme**: `sendParentNote` 105/105 tokens (6 renames), `reserveWakePrompt` 46/46 (0), `startInfoWake` 162/162 (11), **0 non-identifier changes** | STATIC (body lexeme-compared) | HANDLED | no |
| `_kiro/workflow/load` vs `/inspect` (subscription) | 2.29.1 | — | **unchanged** — classification groups identical (OLD `11559637`/NEW `11888711`); **full-lexeme body comparison**: load 384/384 tokens, 8 renames, **0 non-identifier changes** (registers the subscription via `notificationBridgeUnsubscribe` + `sessionTracker.list`); inspect 109/109 tokens, 4 renames, 0 non-identifier changes, read-only (registry→`pxo`, else disk), **no subscription** | STATIC (body lexeme-compared) | HANDLED | no |
| `_kiro/workflow/new` handler body | 2.29.1 | — | **changed only by includes** — **full-lexeme**: 1073→1131 tokens, 15 insert/delete hunks, all include machinery (`if(g1t(c.workflow)…)` inline-include refusal, `sxo(e.host,…)` resolver, `recipeInForce`); 92 consistent identifier renames, **0 operator/number/string/keyword changes** | STATIC (body lexeme-compared) | HANDLED (no cyril change) | no |

## Unreleased / dark candidates

1. **Recipe includes (recipe composition).** KAS 0.66.33 adds a sixth node type `include`
   (`{type:"include", id:<slug>, path:<file>, inputs?:{…}}`) that splices another recipe's steps in place of the
   node, expanding it to a `sequence` whose id is the include id and prefixing every included node id / artifact
   name with `<id>/` (`prefix:`${i.id}${Jho}``). Machinery: schema `oKc` (`.strict()`), cycle/limit validation
   (`include cycle`; "included recipes cannot contain include nodes"), `includeCount`, cap `Gho=50` includes, three
   resolver kinds (`bundled://<name>`, `generated://<id>`, file path), and the default resolver
   `Ube={resolve(){return Promise.resolve({ok:!1,error:"include paths cannot be resolved here"})}}` overridden by
   `includeResolverFor`. Evidence: `0.66.26`: `includedFrom` 0, `includeResolver` 0, `inForce` 0;
   `0.66.33`: `includedFrom` 8, `includeResolver` 13, `inForce` 34, `recipeInForce` 7.
   **Gate/default:** no engine gate (`includeResolverFor()` is passed to the recipe catalog, the
   `save_workflow_definition` tool and `_kiro/workflow/new`); ON wherever workflows are on. **Status:** unannounced
   in `feed-*.json`, absent from cyril's pinned schema, but *documented in the bundled agent prompt*:
   Quote: `` ### 6. include\n\nPuts another recipe\'s steps into this workflow in place of the node. Required: type, id, path. Optional: inputs. ``
   Quote: `` the include becomes a `sequence` whose id is the include id, and every included node id and artifact name gets the prefix `<include id>/` ``
2. **`consentRound` propagation.** The persisted pending/interaction pair gains a round counter so a client can
   correlate the Nth re-prompt of the same tool call. NEW zod:
   `` …consentRound:k.number().optional() `` (on both `pending_interaction` and `interaction_resolved`), and the
   round-aware dedup `` uPt(e,t){return e.toolCallId===t.toolCallId&&e.consentRound===t.consentRound} ``. OLD carried
   `consentRound` only in the `request_permission` meta (baseline) — never on the `session_info_update` kinds and
   never in a schema. Unannounced. Directly relevant to **cyril-w55l** ("Always allow" on a step approval loops
   past round 20). Note the `maxRounds` literal (`AGn(e){return{autoFulfill:…,maxRounds:…}}`, 6→6) is **not** a
   consent cap — it belongs to the MCP multi-round-trip path ("Multi-round-trip request '<e>' still required input
   after <t> rounds (inputRequired.maxRounds)") and is identical; the `allow_always` option builder
   (`t0o`≡OLD `ouo`) and its enum are identical too. No KAS consent-round cap constant exists in either bundle, so
   the observed 20-round loop is client-side — `consentRound` is the field that lets a client correlate/terminate it.
3. **Recipe source "in force" gating.** NEW refuses a `_kiro/workflow/new` recipe whose source is not in the
   session's configuration ("not available in this session's configuration", 0→2) and validates
   `injectOriginalUserRequest` homogeneously across includes. Pure host-side policy; no new wire field.

## Leads for the live lane

Gate note for all: `_kiro/workflow/*` ext methods are advertised unconditionally, so every lead below is
**gate-independent** unless marked; `settings.workflows {enabled:true}` (`initialize.clientCapabilities._meta.kiro.settings.workflows`)
is needed only when the **model** must call `run_workflow`/`save_workflow_definition`/`inspect_workflow`.
cyril's own sender (`crates/cyril-core/src/protocol/domain_mediator/commands/kas.rs:354-363`) always sends
`{workflowPath, inputs, parentSessionId, workspacePaths}` for `_kiro/workflow/new` and
`{workflowId, parentSessionId, workspacePaths}` for `_kiro/workflow/load`; mirror those fields.

- **L1 — recipe `include` expansion (ext method; gate-independent).** Workspace recipes
  `<ws>/.kiro/workflows/inc.workflow.json` = `{"name":"inc","steps":[{"type":"step","id":"s","agent":"wf-coder","prompt":"say OK"}]}`
  and `<ws>/.kiro/workflows/parent.workflow.json` =
  `{"name":"parent","steps":[{"type":"include","id":"review","path":"<abs>/inc.workflow.json","inputs":{}},{"type":"step","id":"after","agent":"wf-coder","prompt":"read {{review/s.output}}"}]}`.
  Call `_kiro/workflow/new {workflowPath:"<abs>/parent.workflow.json", inputs:{}, parentSessionId, workspacePaths}`.
  Expected signal: run starts; the `_kiro/workflow/inspect`/state descriptor's root children include a `sequence`
  node `{nodeId:"review",type:"sequence",steps:[{nodeId:"review/s",…}],includedFrom:{name:"inc",path:"<abs>/inc.workflow.json"}}`.
  Discriminator = presence of `includedFrom` (cyril currently drops it). Control = gate OFF (still works).
- **L2 — include negative arms (ext method).** (a) an *inline* `workflow` object passed to the `run_workflow` tool
  (gate ON) containing an include node → `Invalid workflow shape: an inline workflow cannot contain include nodes; launch the recipe by workflowPath.`;
  (b) an included recipe that itself contains an include → `_kiro/workflow/new: '<path>' has invalid include nodes: … include cycle …`;
  (c) a `_kiro/workflow/new` `workflowPath` under an unadmitted root → `the workflow recipe at '<path>' is not available in this session's configuration.`
- **L3 — `consentRound` propagation.** Drive a workflow step that issues an approval (e.g. `execute_bash`) and reply
  `allow_always` repeatedly, recording: (i) `session/request_permission._meta.kiro.consentRound` (present on both
  versions — baseline), (ii) `session_info_update` kinds `pending_interaction` and `interaction_resolved` each
  carrying `consentRound` (**NEW on 0.66.33; absent on 0.66.26**). Expected signal = the round integer appearing on
  those two kinds; control = 0.66.26 engine (no field). This is the wire evidence behind cyril-w55l.

## Changelog reconciliation

`feed-2.29.1.json` (2.29.1): "MCP OAuth sign-in in classic and Rust V2 now works with servers that require an
issuer" → not in my lane (MCP/auth). 2.29.0: (1) `[V3] /memories` → not in my lane (memory/settings, kas-core);
(2) V2 TUI hook matchers `read` widen → not in my lane (V2 TUI/hooks); (3) V2 TUI tool hooks receive `tool_tags` →
not in my lane (V2 TUI); (4) `[V3] kiro-cli acp` uses CLI login unless `--auth-method=client` → not in my lane
(auth); (5) `[V3]` custom agents use tool settings for `mcp.json` servers → not in my lane (MCP); (6) `[V3]` stdio
MCP servers inherit full agent env → not in my lane (MCP); (7) Classic `fs_write` symlink → not in my lane (v2 fs);
(8) model fallback applies without restart → not in my lane (model fallback); (9) MCP OAuth URL shown → not in my
lane; (10) `[V3]` non-interactive runs respect `telemetry.enabled`/`KIRO_DISABLE_TELEMETRY` → not in my lane;
(11) `[V3] /upgrade-agent` rewrites legacy hook matchers → not in my lane (hooks; added `hook_output` 0→5 = kas-core);
(12) knowledge search limits across bases → not in my lane; (13) `[V3]` sessions no longer close under load → not in
my lane (connections); (14) `[V3]` custom agents as subagents load `mcpServers` → not in my lane; (15) `[V3]` effort
selector at session start → not in my lane (session start); (16) `[V3]` images resized → not in my lane (new
`@silvia-odwyer/photon-node` dep); (17) `[V3]` `KIRO_API_KEY` home-region auth → not in my lane (auth,
`api-key-region-probe.d.ts`); (18) `[V3]` readable non-JSON error page → not in my lane (http_4xx/5xx literals);
(19) `[V3]` resumed sessions keep `UserPromptSubmit` hook output → not in my lane (hooks); (20) `[V3]` Stop cancels a
prompt during MCP start → not in my lane; (21) `[V3]` prompt sent at turn end joins history → not in my lane (turn
history); (22) `[V3]` manual steering files load mid-prompt → not in my lane (steering).

`feed-2.28.1.json` (2.28.1 + the 2.28.0 baseline block — OLD=2.28.0, so these 2.28.0 lines are already in the
baseline bundle and are listed for completeness):
- 2.28.1 "`kiro-cli agent upgrade <dir>` upgrades a directory of agent configs for V2 and V3" → not in my lane (host lane, per RUN-STATE).
- 2.28.0 added "`/settings` Display > Full Screen … Copy on select for terminal/tmux" → not in my lane (TUI/settings).
- 2.28.0 added "Classic sessions offer to switch you to 3.0 and upgrade your agent configs" → not in my lane (v2/host).
- 2.28.0 added "[V3] Hook matchers accept tool tags such as `shell`, `read`, `@builtin`, `@mcp`" → not in my lane (hooks; kas-core).
- 2.28.0 changed "`/model` and `/effort` pickers open mid-turn, selections applied after the turn" → not in my lane (TUI/model).
- 2.28.0 fixed "Conversations no longer stop responding after a sub-agent finishes in V2 TUI sessions" → not in my lane (V2 TUI).
- 2.28.0 fixed "Expanded activity trays no longer limit task lists to six rows" → not in my lane (TUI).
- 2.28.0 fixed "Windows V2 stdio MCP servers preserve `|`, `&`, `%`, spaces…" → not in my lane (v2/MCP/Windows).
- 2.28.0 fixed "Voice recording hints point to `kiro-cli settings voice.*`" → not in my lane (voice/TUI).
- 2.28.0 fixed "Returning from the agent monitor or session dashboard no longer misplaces the chat view" → not in my lane (TUI).
- 2.28.0 fixed "[V3] Interrupted tool calls report accurate outcomes, live and after reload" → not in my lane (tool calls; kas-core, 2.28.0 B5).
- 2.28.0 fixed "[V3] Reduced memory growth during very long agent sessions" → not in my lane (memory/session; kas-core).

**No changelog line — 2.29.1, 2.29.0, 2.28.1 or the 2.28.0 baseline — maps to a workflow-lane delta. The two
workflow deltas (`include`, `consentRound` propagation) are unannounced.**

## Method notes

- Technique: minified-bundle **string-literal set diff** with broad patterns (`_kiro/workflow/[A-Za-z_/]+`,
  snake_case lifecycle literals, zod keys, `wf-*`, watch-handler ids), then per-token **occurrence counts in BOTH
  bundles** and <=200-char context — the kas-core OLD-vs-NEW technique (Trap 3 / Trap 2). `acp-server.js` is one
  minified file, so "count" is substring occurrences, never `grep -c` lines.
- The 25-distinct `_kiro/workflow/*` quoted literals all had equal counts; a raw-substring count of
  `_kiro/workflow/` read 189→186, which a diff of the non-exact occurrences proved to be minifier-identifier
  renaming inside error strings (`_kiro/workflow/new: …`), **not** a removed literal (Trap 2 closed explicitly).
- Minified zod identifiers are renamed every build (`XBe`-family, `Ude`↔`vfe`, `t1n`↔`DFn`, `hBc`↔`aou`,
  `cAt`↔`v0t`, `Itu`↔`t4c`, `Xtu`↔`_4c`); all comparisons are on *string literals / zod keys*, which the minifier
  does not rename.
- The recipe zod is built twice (a `superRefine` union copy and a duplicate `.refine` per-node copy) — both were
  checked so the `includedFrom` addition is not a one-of-two artefact.
- **Body comparison (advisory-driven, rigorous).** Handler bodies were extracted by brace-matching from the handler
  object and compared on their **complete lexical token stream** (identifiers, numbers, string/template chunks,
  keywords, operators, punctuation) with a difflib alignment plus a learned rename map; a difference is accepted
  only when it is a single consistent, injective rename of identifiers and **every other lexeme is byte-identical**
  (operators, numbers, strings and keywords are all compared, so `x<20` vs `x>20` or a swapped helper would be
  caught). Result: `load` 384/384 tokens (8 renames), `inspect` 109/109 (4), `resume` 716/716 (20),
  `sendParentNote` 105/105 (6), `reserveWakePrompt` 46/46 (0), `startInfoWake` 162/162 (11) — **0 non-identifier
  changes in each** ⇒ control-flow equivalent modulo renames. `_kiro/workflow/new` 1073→1131 with 15 hunks, all the
  include machinery (inline-include refusal + resolver wiring), 92 consistent renames, 0 non-identifier changes.
  Full OLD/NEW function texts: `raw-lex-<label>-{old,new}.txt`; script `static-kas-workflow-lex-2.29.1.py`; raw `raw-lex.txt`, `raw-lex-*.json`.
- Advisory-driven corrections applied: (a) `consentRound` on `request_permission._meta.kiro` is a 0.66.26 baseline,
  not a new field — only the `session_info_update`/persisted-schema propagation and round-aware dedup are new;
  (b) `_kiro/workflow/*` ext methods are advertised unconditionally and are not gated by `settings.workflows`
  (only the agent tool pool is) — leads state gate-independence accordingly; (c) L1 params include
  `parentSessionId`+`workspacePaths` as cyril actually sends them; (d) the feed-2.28.1 2.28.0 baseline block is
  reconciled. **(e)** An earlier "structural signature" body comparison (ordered string literals + `.property` +
  `label:` only) was **superseded** by the full-lexeme comparison above after review flagged that it discarded
  operators, numbers, keywords and bare callees — signature equality alone would not have caught `x<20` vs `x>20`.
  The load-vs-inspect subscription observation itself was unaffected (it is a literal/method-call fact).
- Not analysed (out of lane, cross-referenced only): the `session_info_update` kind union as a whole and message
  payloads (`noteUserRequests`/`summary*`) → kas-core; `_kiro/configuration/getState` (new in `extensionMethods`)
  and the `"served"/"notEnabled"` enum growth → kas-core; TUI/host bundles.
- Artifacts: `raw-vocab-diff.json`, `raw-vocab-diff.txt`, `raw-counts.txt`, `raw-region.txt`, `raw-anchors.txt`,
  `raw-recipezod.txt`, `raw-include.txt`, `raw-include-region.txt`, `raw-include-wiring.txt`, `raw-include2.txt`,
  `raw-resolver.txt`, `raw-tools*.txt`, `raw-docs.txt`, `raw-rows.txt`, `raw-final.txt`, `raw-surface2.txt`,
  `raw-wake.txt`, `raw-last.txt`, `raw-last2.txt`, `raw-advisory*.txt`, `raw-loadinspect.txt`, `raw-bodysig*.txt`
  (superseded), `raw-lex.txt`, `raw-lex-*.json`, `raw-lex-<label>-{old,new}.txt` (full OLD/NEW function texts).

## Explicit answer — does 0.66.33 change cyril's workflow control plane or the recipe schema cyril pins?

**Control plane — no static evidence of change.** Start/stop/approve/resume: the 25 `_kiro/workflow/*` literals
are the same set with the same counts (0 added / 0 removed), the 9-event lifecycle map is identical, the 3 watch
handlers are identical, `resume`/`extendRepeat` messages are unchanged, `run_workflow` (`Itu`) and
`save_workflow_definition` (`Xtu`) schemas are textually identical to `t4c`/`_4c`, `_kiro/session/notify`
(`cAt`≡`v0t`) and `recipes_changed` are unchanged, the `extensionMethods` array (`v$r`≡`v9r`) and the persistence
classification groups are identical, and the auto-wake literals are unchanged. **Body check (advisory-driven,
rigorous):** the handler bodies were compared on their complete lexical token stream (operators/numbers/strings/
keywords included) with a learned rename map — `load` 384/384 tokens, `inspect` 109/109, `resume` 716/716,
`sendParentNote` 105/105, `reserveWakePrompt` 46/46, `startInfoWake` 162/162, all with **0 non-identifier changes**
⇒ control-flow equivalent modulo renames; `_kiro/workflow/new` (1073→1131) differs by 15 hunks that are **all the
include machinery**, with 0 non-identifier changes. No live re-run is required to close the body question. (The
earlier weaker "structural signature" pass was superseded after a review noted it discarded operators/numbers.)
Evidence quote (identical in both): `` var hBc={run_start:"_kiro/workflow/run_start",node_start:"_kiro/workflow/node_start",node_complete:"_kiro/workflow/node_complete",node_paused:"_kiro/workflow/node_paused",loop_iteration:"_kiro/workflow/loop_iteration",watch_poll:"_kiro/workflow/watch_poll",paused:"_kiro/workflow/paused",run_complete:"_kiro/workflow/run_complete",steps_queued:"_kiro/workflow/steps_queued"} `` (NEW names the var `aou`).

**Recipe schema — yes, cyril's pinned schema is incomplete.** `docs/kiro-kas-workflow-recipe-schema-2.26.0.json`
documents 5 node types and a `sequence` without `includedFrom`; 0.66.33 accepts a sixth node type `include` and
projects `includedFrom:{name,path}` on expanded sequences. cyril's *runtime* parser (`WireNodeDescriptor`,
`WireNodeSnapshot`) is a tolerant serde struct, so the new field is **DROPPED** (received, ignored) rather than a
hard failure — but the authoring-facing schema and any include-aware validation are missing it.
Evidence quote (NEW): `` sequence:{type:"literal",id:"node-id",steps:"children",includedFrom:"literal"} `` (OLD: absent).

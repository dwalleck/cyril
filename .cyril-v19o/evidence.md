# Evidence: cyril-v19o

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | The installed `kiro-cli` (2.21.2) emits `_kiro/powers/items_changed` in cyril's spawn shape (`kiro-cli acp --agent-engine kas`, direct spawn) carrying the documented item schema | What are the exact params, and how many items, when the real powers tree is present? | PASS |
| P1b | That frame is **unprompted** — it arrives with no client request after `session/new` | Does `items_changed` arrive while the client issues nothing at all? | PASS |
| P2 | `_kiro/powers/refresh` is declared but not implemented (the design must not wire an affordance to it) | What does the agent answer when a client calls it, and does a sibling method in the same session still work? | PASS |
| P3 | The v2 engine has no powers surface at all: no advertised `powers` command and no `_kiro/powers/*` frame | Which commands does `kiro-cli acp` advertise, and is any powers method ever emitted? | PASS |
| P4 | A `_`-prefixed extension notification reaches cyril's converter with no allowlist in between | Is there any method filter between the SDK notification handler and `Engine::convert_ext_notification`? | `N/A — code-level premise already covered by current repository evidence` (`client.rs:136-145` enqueues every `_`-prefixed method; `inbound.rs:170` calls the converter; the engine fence `kas.rs:1399-1402` asserts unknown `_kiro/*` frames drop to `Ok(None)`). Its live traversal through cyril's own binary is SC1, executed in `checkpointed-build` |
| P5 | Powers are a per-user install (`~/.kiro/powers`), not session state, so one process-global catalog is correct | Where does the catalog come from on disk, and does any item field carry a session/tenant key? | PASS |
| P6 | The catalog is small enough that no scale work is needed | How many powers exist in the real install, and what does one item weigh? | `N/A — spec decision` (max-scale row: viewport clamped, list unbounded; spec "Max scale (many powers)"). Observed 3 items / ~7 fields each in this install |

Valid evidence reused rather than re-probed: the committed captures `experiments/conductor-spike/kas-powers-2.20.1.jsonl` (populated), `kas-powers-2.21.0-2.21.1.jsonl`, `kas-powers-2.21.1-2.21.1.jsonl` (empty), and `crates/cyril-core/tests/fixtures/kas/workflow/kas-csig-2.16.0-neutral.jsonl:5` (empty). They agree with this run's shape and stay valid for the *shape* claim; they were not valid for the *installed version* claim, which is what P1/P2 re-probed.

## Data

- **Source**: production-shaped. The probe spawns the real `kiro-cli 2.21.2` with `HOME` pointed at a throwaway directory seeded with a **copy** of the user's real powers tree (`~/.kiro/powers` → registry `installed.json` + the three installed bundles with their real `mcp.json`/`steering/` contents). The v2 probe runs with an unseeded throwaway HOME.
- **Shape**: exactly the shape the feature consumes — the real registry file, the real bundle layout (`installed/<name>/{POWER.md,mcp.json,steering/}`), and the real item payload (3 items, 7 fields + `_meta`).
- **Safety**: no production state is written. `HOME`, `XDG_RUNTIME_DIR`, and `TMPDIR` are per-run temp directories, so kiro-cli's logs, sessions, and session-index land outside `~/.kiro`; the probes only *read* the real `~/.local/share/kiro-cli/data.sqlite3` for the OAuth token and the real powers tree. Process groups are reaped on every exit (SIGTERM then SIGKILL). Auth values are redacted before persistence.
- **Approval**: the requester approved live probing in this run — "Log in now, verify live" — and performed the `kiro-cli login` device flow themselves (2026-09-10).

## Probe

- Files: `probe-kas-powers-2.21.2.py` (payload + refresh trap), `probe-kas-powers-unprompted-2.21.2.py` (P1b), `probe-v2-command-advertisement.py` (P3)
- Mechanism: each script spawns the real agent as a direct child over stdio — `kiro-cli acp --agent-engine kas` for the KAS legs, bare `kiro-cli acp` for the v2 leg — answers the two server→client requests the agent makes (`_kiro/auth/getAccessToken` from the real token DB, `_kiro/terminal/shell_type` → `bash`), performs `initialize` + `session/new`, and records every frame in order with timestamps.
- Runs:
  - `SEED_POWERS=1 PROBE_OUT=.cyril-v19o LABEL=2.21.2 python3 .cyril-v19o/probe-kas-powers-2.21.2.py`
  - `SEED_POWERS=1 PROBE_OUT=.cyril-v19o LABEL=2.21.2 IDLE_SECONDS=12 python3 .cyril-v19o/probe-kas-powers-unprompted-2.21.2.py`
  - `PROBE_OUT=.cyril-v19o LABEL=2.21.2 python3 .cyril-v19o/probe-v2-command-advertisement.py`

Recorded output (not re-quoted in full; see the named artifacts):

| Artifact | Key output |
|---|---|
| `kas-powers-2.21.2-2.21.2-verdict.json` / `.jsonl` | 1 push, 3 powers, item key set `{_meta, description, displayName, hasSteeringFiles, isAgentPlugin, keywords, mcpServerNames, name}`; `list` returns the same 3; `refresh` → `-32603`; `powersAdvertised: false` |
| `kas-powers-unprompted-2.21.2-verdict.json` / `.jsonl` | client issued only `initialize` and `session/new`; the agent pushed `items_changed` **+0.018 s after the `session/new` reply** with 3 powers, then idled 12 s |
| `v2-commands-2.21.2-verdict.json` / `.jsonl` | 25 advertised commands, `/powers` absent; zero powers methods emitted |

## Oracle

- **Oracle A — filesystem ground truth (P1, P5)**: the catalog is computed with `jq` straight off the registry and bundle files, with no ACP and no agent process involved: `jq -r '.installedPowers[].name' ~/.kiro/powers/installed.json`, plus `.mcpServers | keys` from each `installed/<name>/mcp.json`, plus an `find <name>/steering -type f | wc -l` file count. Different mechanism: the wire could emit stale, cached, or invented items and this would catch it.
- **Oracle B — sibling handler in the same capture (P1, P2)**: `_kiro/powers/list` is a *different agent-side handler* from the notification emitter, extracted from the same trace with `jq` rather than the probe's Python parser. Its payload must equal the pushed payload, and its success is the control that makes the `refresh` error specific rather than a symptom of a broken session.
- **Oracle C — jq over the traces (P1b, P3)**: frame order and method sets recomputed from the raw JSONL with `jq`/`grep`, independent of the probe's in-process bookkeeping. For P3 the same single mechanism answers both halves: the advertised command list (from `_kiro.dev/commands/available`) and the absence of any `powers` method in the same trace.
- **Oracle D — historical committed captures (P2)**: `experiments/conductor-spike/kas-powers-2.{20.1,21.0-2.21.1,21.1-2.21.1}-verdict.json` each record the same `-32603 Unknown ext method: _kiro/powers/refresh`, from three different binaries recorded weeks before this run. Independent of today's probe *execution* and of the feature's implementation.
- Oracle mechanisms differ from the probe (filesystem/jq/other binaries vs an in-process Python ACP client) and from the production implementation (no cyril converter code is involved in any oracle).

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | 3 items: `aws-infrastructure-as-code` / `datadog` / `markdownlint`; mcp `awslabs.aws-iac-mcp-server` / `datadog` / `markdownlint`; `hasSteeringFiles` false/true/true; `status: "success"`; item keys as above | A: names `aws-infrastructure-as-code, datadog, markdownlint`; mcp keys identical; steering **file counts** 0 / 1 / 2. B: `list` returns the same 3 items with identical key sets and `errors: []` | PASS |
| P1b | Push arrives +0.018 s after the `session/new` reply; client issued only `initialize` + `session/new` | C: `jq` over the same trace shows exactly two `client->agent` method frames (no `resp`/notification rows), and one `items_changed` frame | PASS |
| P2 | `refresh` → `-32603`, `data.details "Unknown ext method: _kiro/powers/refresh"` | B: sibling `list` in the same session returns a successful result (the session and routing work); D: three prior committed verdicts on other binaries, same error | PASS |
| P3 | 25 commands, `powers` absent; no powers methods seen | C: `jq` extraction gives the same 25 names with `/powers` absent and zero powers methods in the trace | PASS |
| P5 | Items carry `_meta.kiro.resource = {resourceType: "power", source: {origin: "user"}}`; no session/tenant field | A: the sole catalog source is the user-level `~/.kiro/powers/installed.json` registry; the throwaway-HOME copy reproduces it exactly | PASS |

## Validated / learned

- **P1 — validated prior understanding (with one corrected reading)**: probe and oracles agree that the 2.21.2 push carries the three real powers with the documented item schema, and that `list` returns the identical payload. The design's reliance on `displayName`, `name`, `mcpServerNames`, and `hasSteeringFiles` is sound.
- **P1 — learning (`hasSteeringFiles` is a file count, not a directory test)**: the first oracle run checked `test -d <name>/steering` and reported `true` for all three, disagreeing with the wire's `false` for `aws-infrastructure-as-code`. Investigation showed all three bundles ship a `steering/` directory, but that one is **empty** (0 files) while `datadog` has 1 and `markdownlint` has 2. The oracle was at fault, was corrected to count files, and now agrees exactly. Design consequence: the panel's `steering` marker means "this power ships steering documents", and the field must be read verbatim from the wire — deriving it from a directory listing would be wrong.
- **P1b — learning (the first probe's ordering could not prove "unprompted")**: `probe-kas-powers-2.21.2.py` emits `_kiro/powers/list` 1 ms after the `session/new` reply, because its drain helper returned early on the first notification, so its trace shows the push *after* the client's list request. That made "pushed unprompted at session creation" unproven rather than disproven. The single-purpose unprompted probe removes the race: with the client issuing nothing, the push still arrives, 18 ms after the session was created. The premise holds; the first probe's *timing* leg was the defect, not the system.
- **P2 — validated prior understanding**: the declared-but-unimplemented trap reproduces on 2.21.2, and the control method proves it is specific to `refresh`. Cyril must not call it, and this run is the fourth independent confirmation (2.20.1, 2.21.0, 2.21.1, 2.21.2).
- **P3 — validated prior understanding**: the v2 engine advertises 25 commands — including `/code` and `/hooks` — with no `powers` among them, and emits no powers method. cyril's `/powers` builtin therefore cannot shadow an agent-advertised command on either engine, so no command-source seam is needed.
- **P5 — validated prior understanding**: the catalog is user-level and session-independent; a process-global cache populated by the latest push is the correct model.

## Related issues

- Consulted (copied from `spec.md`; no upstream search was repeated): **cyril-v19o** (this feature), **cyril-q159** (open Agent Plugin format research — owns the strategic consume/supply question, deliberately out of scope here), **cyril-nk4o** (the sibling KAS panel scope for `_kiro/mcp/*`), **cyril-58uv** (in_progress; instrumentation of the same silent-drop boundary — the design keeps its converter in a new KAS-side module to avoid colliding with its edits in `convert/kiro.rs`), **cyril-oiyt** (KS hooks-panel extension; source of the never-auto-open panel discipline), **cyril-7q8u** (prior live capture of `_kiro/powers/items_changed`, agreeing with this run), and `docs/kiro-2.20.1-wire-audit.md` §3 (the wire contract).
- Filed: none. No premise failed, so no underlying-system defect or intended future work surfaced from this stage.

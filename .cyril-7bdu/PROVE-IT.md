# KAS-5 (cyril-7bdu) — prove-it-prototype

**Goal:** before designing KAS-5 (fs + terminal host-callback responders), confirm
on the **2.10.0** binary that the server→client host-callback wire contract cyril
must implement has not drifted from the 2.7.1/2.8.1 baseline, and capture genuine
wire bytes as fixtures.

**Probe:** `experiments/conductor-spike/probe-kas-fs-terminal-host-2.10.0.py`
(live KAS turn, `kiro-cli-chat 2.10.0 acp --agent-engine v3`). Advertises
`clientCapabilities {fs:{readTextFile,writeTextFile}, terminal}`, implements real
responders, records every inbound request, and diffs methods+param-keys vs the
documented baseline. Token self-sourced; only **requests** recorded (token rides
our reply, never recorded) → fixtures are safe to commit.

**Oracle:** the covenant host-callback signatures + the 2.7.1 probe's documented
method set. Agreement (no new/unknown methods, only expected param keys) ⇒ no drift.

## Result: NO WIRE DRIFT at 2.10.0 ✅

One clean turn drove 19 host callbacks. Methods + param keys observed:

| method | params | notes |
|---|---|---|
| `_kiro/auth/getAccessToken` | `{}` | session setup; reply `{accessToken,expiresAt,profileArn}` |
| `_kiro/terminal/shell_type` | `{sessionId}` | session setup; reply `{shellType}` |
| `fs/read_text_file` | `{sessionId, path, line}` | **bare ACP**; `path` ABSOLUTE; `line:0`; auto-allowed (no permission) |
| `fs/write_text_file` | `{sessionId, path, content}` | **bare ACP**; ABSOLUTE path; gated by `request_permission` |
| `terminal/create` | `{sessionId, command, args, cwd}` | **bare ACP**; ABSOLUTE `cwd`; gated by permission |
| `terminal/output` | `{sessionId, terminalId}` | reply `{output,truncated,exitStatus:{exitCode}}` |
| `terminal/wait_for_exit` | `{sessionId, terminalId}` | reply `{exitStatus:{exitCode,signal}}` |
| `terminal/release` | `{sessionId, terminalId}` | reply `{}` |
| `session/request_permission` | `{sessionId, toolCall, options, _meta.kiro.consent}` | KAS-specific consent block |

**Baseline diff:** **no NEW/unknown methods.** Every "extra" param key vs the
2.7.1 baseline is either standard ACP (`line`, `cwd`, `_meta`, `toolCall`) or the
always-present **`sessionId`** (the baseline probe simply hadn't recorded keys).

### Findings that shape the design
1. **KAS uses the BARE ACP host methods, not the `_kiro/fs/*` extras.** The
   `_kiro/fs/{read_file,write_file,read_directory,stat,delete}` variants did **not**
   fire — KAS routed *list directory* and *delete* through `terminal/create`
   (shell `ls`/`rm`). First-cut `HostIo` only needs bare-ACP `fs/{read,write}_text_file`
   + `terminal/{create,output,wait_for_exit,release}` (+ `kill` defensively). Treat
   `_kiro/fs/*` as not-yet-observed (don't build responders for them speculatively).
2. **Every callback carries `sessionId`; fs paths + terminal `cwd` are ABSOLUTE.**
   → `HostIo` responders route on `sessionId` (ADR-0004 loop), and `platform/path.rs`
   translation applies at the fs/terminal boundary (Windows/WSL).
3. **fs READ is auto-allowed; WRITE + terminal go through `session/request_permission`**
   with a KAS-only `_meta.kiro.consent {capability, resource, askType, workspaceRoot}`
   + `consentRound`. cyril already has an approval overlay; the KAS consent metadata
   is extra context to thread through (or ignore for the first cut).
4. **Per ADR-0004 invariant:** the bridge loop forwards these requests and must NOT
   await resolution — slow fs read / shell exec spawns OFF-LOOP (like the turn prompt,
   cyril-84ca). Re-introducing a blocking await is the regression to guard against.

## Artifacts
- `host_callbacks_2.10.0.json` — full ordered list of all 19 captured requests
- `fixtures/<method>.json` — one raw example envelope per method (all from this run)
- `initialize_result_2.10.0.json` — agentCapabilities advertised by KAS 2.10.0

## Probe-harness lesson (carried forward)
The IdC OIDC token (`kirocli:odic:token`) no longer carries `profile_arn`; kiro-cli
persists the active profile in `state['api.codewhisperer.profile'] = {arn, profile_name}`.
A probe hardcoding `profile_arn` from the token sends `null` → backend rejects with
*"profileArn is required for this request"* (turn ends with an error, only the 2 setup
callbacks fire). Source `profile_arn` from the `state` table. (Also bit the KAS-2b probe.)

# cyril-tqr8 — supported KAS OAuth evidence

Historical investigation date: 2026-10-19; inspected production base `8e5c325e`. The delivery checkpoint below owns the current assembled local qualification, source/binary pins, owned-consumer result and explicit native/CI limits. Historical failures and partial checkpoints retain their original identities and results; the new current PASS does not rewrite them.

## Premises

| ID | Question / dependent decision | Conclusion |
|---|---|---|
| P1 | Does shipped KAS have a real URL host callback independently of secret storage? | Yes. `_kiro/openExternalUrl {url:string}` is an agent-to-client request returning `{success:boolean}`. `clientCapabilities._meta.kiro.openExternalUrl === true` and `secretStorage` are resolved in independent branches. A real URL adapter can ship without a secret backend. |
| P2 | Must handling this callback launch a browser or mean OAuth completed? | No. Current producer awaits the RPC and discards its returned value. The first-party TUI attempts clipboard copy, displays the URL even when copy fails, then acknowledges `{success:true}` without launching a browser. Cyril may truthfully acknowledge visible URL surfacing; authorization must still be derived from MCP status. |
| P3 | Will advertising URL handling surface initial passive startup authorization? | Not by itself. The provider saves the authorization URL but calls the host only outside `passive`, in `lazy` or `explicit-reauth`. The passive failed-authorization URL is in `_kiro/mcp/status`; `nk4o` must retain and show it and initiate `resetServer {serverName,startOAuth:true}`. |
| P4 | Is `_kiro/mcp/oauthCompleted` a supported KAS completion mechanism? | No in the inspected 0.66.43 source. This is a dark TUI path, armed by fields current KAS does not send. Do not require or implement speculative producer support. |
| P5 | Does absence of a client elicitation advertisement guarantee no elicitation callback? | No. Current KAS's resolver has no established elicitation opt-in flag, its MCP client advertises both form and URL modes, and its ACP tool adapter installs `_kiro/mcp/elicitation` unconditionally. `nk4o` needs an explicit valid resolution policy, not a claim that an absent capability prevents calls. |

These facts settle design-changing protocol premises. They do not by themselves establish integrated Cyril behavior/timing; the current delivery checkpoint distinguishes the later observed outcomes and remaining failures.

## Primary sources and applicability

The read-only `McpOAuthEvidence` investigation inspected these executable sources:

- KAS: `/home/dwalleck/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js`; package metadata identifies `@kiro/agent` **0.66.43**. Capability resolver at line 11041; `Lh` extension request helper at 16221; OAuth provider `P8e` at 16459; MCP client at 16540; elicitation bridge `Xp` at 16928; MCP installation, callback and failure producer at 17463; MCP management dispatch at 17467. These are source line anchors, not byte offsets.
- TUI: `/home/dwalleck/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js`, URL handler `Osr`/`D9t` at line 899, dark completion handler at 541.
- Source/package availability was observed; installed-versus-carve byte identity was not independently re-established in this task. The existing release record is `experiments/conductor-spike/audit-2.32.0/RUN-STATE.md:14-17` and `docs/kiro-2.32.0-wire-audit.md:43-46`.
- Covenant inputs/results: `docs/kiro-kas-acp-covenant.md:23-38,81-104`. That document dates its covenant package to 2.7.1; current executable source, not stale capability-gating prose, governs P5.
- Detailed prior OAuth analysis: `experiments/conductor-spike/mcp-remote-2.31.1/kas-static.md:372-415,429-453`; consolidated lifecycle `docs/kiro-mcp-remote-servers.md:104-152`. Current source confirms independent storage/browser gates and non-passive callback behavior, so those conclusions remain applicable.
- Dark-path corroboration: `docs/kiro-2.32.0-wire-audit.md:190-210`.

### Observed protocol meaning

1. The shipped URL producer sends **only `{url}`**, no session ID, server name, or completion mode. Do not synthesize those as wire requirements or claim a per-server callback identity absent an independent correlation.
2. `{success:false}` is currently ignored by the producer; an RPC error is instead logged/rethrown. Neither success acknowledgement nor `resetServer` acknowledgement proves a connected/authorized server.
3. No secret backend means MCP credentials remain in process memory. This does not disable URL callback or protocol authorization. `secretStorage` remains unadvertised; no `_kiro/secret/*` handler/store is introduced by this issue.
4. The current MCP failure producer emits local-session `session_info_update` metadata `{kind:"display_error",message,errorType:"mcp_connection_error"}`. Auth copy names the server and directs the user to the MCP panel; other failures name the server and original connection error. The ordinary envelope is `session/update` with `update.sessionUpdate:"session_info_update"`, `_meta.kiro` on the update.
5. The old MCP-specific display-error envelope in `docs/kiro-2.22.0-deserialization-audit.md:454-473` is explicitly a probe reconstruction, not a retained raw frame. `experiments/conductor-spike/audit-2.32.0/live/kas-live-sweep-2.32.0.jsonl:167` (replay:264) retains the flat/mirrored envelope for `InvalidModelError`, not MCP failure. MCP failure's current source producer is primary evidence; a new live MCP frame is still required for assembled acceptance.
6. Current reset/getPrompt/getResource dispatch exists even though the retained initialize `extensionMethods` list omits these methods. Do not reject supported calls solely because this incomplete list does not enumerate them. The typed handler and bound KAS engine govern their availability.

## Inspected Cyril baseline (`8e5c325e`)

- `protocol/engine.rs:22-56,91-135`: adapter presence determines both execution and advertisement. Baseline has no URL or secret adapter.
- `protocol/client.rs:275-290` and `protocol/domain_mediator/host.rs:45-190`: extension requests enter the separate host drain; unsupported requests can reach the generic `null` fallback. New supported URL/elicitation requests must not use that fallback.
- `protocol/convert/kas.rs:583-756`: KAS session-info conversion lacks `display_error`; the final catch-all returns `None`.
- `protocol/domain_mediator/inbound.rs:148-215`: current unsupported extension families are debug-dropped, not consumer-visible.
- `crates/cyril/src/app.rs:1640-1649`: v2 already displays a manual sign-in URL. Reuse that consumer behavior through typed events; no browser abstraction is necessary.

Paths without a crate prefix above are relative to `crates/cyril-core/src/`.

## Data and safe acceptance fixture

Existing isolation precedent is `.cyril-jlxx/evidence.md:63,88-100` and `probe-mcp.summary.json:1-82`: inherited MCPs can start before `SessionCreated` and before profile selection; `includeMcpJson:false` is insufficient. Fresh HOME/KIRO_HOME/workspace roots, with untrusted captured config only in inert subdirectories, prevented inherited server launches. This justifies clean-root fixture construction; it does not prove this issue's new behavior.

`fixture.py` is a test-owned loopback MCP + stub authorization server. Its generated settings and runtime directories stay beneath a freshly minted `cyril-mcp-fixture-*` root. OAuth registration/codes/tokens/verifiers are synthetic and in memory; logs retain lifecycle metadata, not headers/query/body. It never starts Cyril/KAS, launches a browser, copies clipboard contents, reads credentials or changes ambient account settings. Parent subsequently qualified the current `df5d9437` fixture and authorized the earlier native runs recorded below. The fixture manifest is neither launch authorization nor product acceptance. The delivery worker did not execute the fixture.

Static compatibility review against KAS0.66.43 found and corrected a concrete first-registration mismatch without executing the fixture: `acp-server.js:16459` `P8e.clientMetadata` requests `grant_types:["authorization_code","refresh_token"]`; bundled SDK normalization/registration is at `:2578`. Discovery advertising code-only does not remove that requested metadata. The fixture now accepts that request but grants/returns only `["authorization_code"]`, issues no refresh token and still rejects refresh exchange. No offline-access scope, token persistence or refresh implementation is added. KAS's runtime refresh branch requires an actual stored refresh token. This establishes source compatibility of the branch, **not** observed end-to-end success; parent qualification remains required. Detailed source handoff: `agent://McpLifecycle.McpOAuthEvidence`.

Original live acceptance protocol after implementation (historical recipe, not authorization to run now):

- Record selected bundle/package version and actual consumer source revision. Use clean HOME/KIRO_HOME/cwd/config/cache/temp roots and hooks off. Keep any required native host authentication read-only/in-memory and separate from synthetic MCP OAuth; do not copy login stores into artifacts.
- Never send `session/prompt`: initialize/session-new/status/reset exercise OAuth without a billed model turn.
- Run URL-capability absent/present arms, both without `secretStorage`. Initial passive status must contain auth-required URL; no callback is assumed there. Explicit `resetServer startOAuth:true` in the present arm must yield the actual `{url}` request and a visible Cyril URL, then `{success:true}` only after successful delivery into the consumer path.
- Drive the synthetic authorize URL with a local HTTP client, preserving redirect/state/PKCE. Observe KAS connected status; do not synthesize connection success from the callback or reset acknowledgement. Use the failed endpoint for a real `mcp_connection_error` display-error frame.
- Record monotonic order of notifications, new-session response, URL callback/reply, reset reply and connected status. Retain redacted frame envelopes and exact fixture-control inputs.
- Stop the fixture and the exact owned consumer/KAS descendants; confirm teardown. No production organization settings, external MCP accounts, browser or clipboard are needed.

No new empirical probe was necessary before design. Later qualification is recorded below; the first subsequent preview attempt failed the native authentication prerequisite before initialize, so this record authorizes no native retry or auth/config/account change.

## Executable parent-owned live recipe

**Historical parent-owned recipe, not permission to execute.** The later native authentication stop supersedes this recipe: do not run it, build, log in, change accounts/configuration or inspect native token stores without fresh Main authorization. The current standalone consumer evidence below uses a separately qualified current binary and one authorized isolated owned ACP peer, not real native KAS.

Source anchors: `cyril/src/main.rs:20-41,52-72,303-306` supplies `--cwd`, `--agent-engine kas`, trailing multivalue `--agent-command`, and `$HOME/.config/cyril/config.toml`. `types/config.rs:61-65` requires `kas_spawn:"wrapper"` so the supplied agent command is actually used; default Free mode does not honor it for direct KAS spawn. `protocol/kas/version.rs:93-154,338-362` binds that same executable for version/token helpers and appends `--agent-engine v3`. Hooks off is `kas_hooks:"off"`. Do not invent `--agent`, `--provider`, `--hooks` or `--config` flags.

```bash
WT="$PWD"
RUN=$(mktemp -d /tmp/cyril-mcp-consumer-XXXXXX)
CARGO_TARGET_DIR="$RUN/build" cargo build \
  --manifest-path "$WT/Cargo.toml" -p cyril --features kas
CYRIL_BIN="$RUN/build/debug/cyril"
python3 "$WT/.cyril-tqr8/fixture.py" --silent --slow-ready 35 \
  >"$RUN/manifest.json" 2>"$RUN/fixture.stderr" &
FIXTURE_PID=$!
trap 'kill "$FIXTURE_PID" 2>/dev/null; wait "$FIXTURE_PID" 2>/dev/null' EXIT

python3 -c '
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time

manifest_path = Path(sys.argv[1])
binary = Path(sys.argv[2]).resolve(strict=True)
native_data = Path(
    os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local/share")
).resolve(strict=True)
actual_cli_name = shutil.which("kiro-cli")
if actual_cli_name is None:
    raise SystemExit("Installed kiro-cli is unavailable")
actual_cli = Path(actual_cli_name).resolve(strict=True)
bwrap = shutil.which("bwrap")
if bwrap is None:
    raise SystemExit("bubblewrap is required for read-only native authentication")

deadline = time.monotonic() + 10
while True:
    try:
        manifest = json.loads(manifest_path.read_text())
        break
    except (FileNotFoundError, json.JSONDecodeError):
        if time.monotonic() >= deadline:
            raise SystemExit("Fixture did not publish its manifest; inspect fixture.stderr")
        time.sleep(0.05)

root = Path(manifest["root"]).resolve(strict=True)
workspace = Path(manifest["workspace"]).resolve(strict=True)
home = Path(manifest["environment"]["HOME"])
config_dir = home / ".config/cyril"
config_dir.mkdir(parents=True, exist_ok=True)
(config_dir / "config.toml").write_text(
    "[agent]\nengine = \"kas\"\nkas_spawn = \"wrapper\"\n"
    "kas_hooks = \"off\"\n[memory]\nenabled = false\n"
)
shim_dir = root / "bin"
shim_dir.mkdir(mode=0o700)
shim = shim_dir / "kiro-cli"
shim.write_text(
    "#!/bin/sh\nexport XDG_DATA_HOME=" + shlex.quote(str(native_data))
    + "\nexec " + shlex.quote(str(actual_cli)) + " \"$@\"\n"
)
shim.chmod(0o700)
allowed = (
    "PATH", "LANG", "LC_ALL", "TERM", "COLORTERM",
    "SSL_CERT_FILE", "SSL_CERT_DIR"
)
environment = {key: os.environ[key] for key in allowed if key in os.environ}
environment.update(manifest["environment"])
environment["PATH"] = str(shim_dir) + os.pathsep + environment.get("PATH", os.defpath)
environment.setdefault("TERM", "xterm-256color")
environment.setdefault("LANG", "C.UTF-8")
command = [
    bwrap, "--ro-bind", "/", "/",
    "--bind", str(root), str(root),
    "--proc", "/proc", "--dev-bind", "/dev", "/dev",
    "--die-with-parent", "--unshare-pid",
    "--chdir", str(workspace), "--",
    str(binary), "--agent-engine", "kas", "--cwd", str(workspace),
    "--agent-command", str(shim), "acp",
]
raise SystemExit(subprocess.call(command, env=environment, cwd=workspace))
' "$RUN/manifest.json" "$CYRIL_BIN"
```

The launcher inherits terminal stdin rather than consuming it through a Python heredoc. Cyril's own XDG data remains isolated/writable. Only the owned agent shim gives native data access to the selected CLI and its token/version helpers; bubblewrap keeps that native data read-only. No credential files are copied or auth-store names guessed. No provider-token/proxy/KAS-override variables or mock chat environment are forwarded. An expired native credential can require a refresh/write that this sandbox correctly refuses: retain that failure, do not weaken the mount or run login as an acceptance workaround. The old raw v2 probe did **not** qualify this new KAS wrapper/token-helper path.

After nk4o is assembled, source-known panel flow is `/mcp` Enter, arrows to the named row, `a` authenticate or `r` reset. Require passive URL → explicit callback/manual notice → authoritative connected status, not ACK completion. Tab twice from Servers selects Prompts; select `synthetic_prompt`, enter required `marker=READ_ONLY_CANARY`, Enter, expect `SYNTHETIC_MCP_CANARY::ready::READ_ONLY_CANARY`. `Ctrl+E` explicitly enters an empty string; `Ctrl+U` unsets it. Esc back to catalog, Resources, select exact `fixture://ready/canary`, Enter, expect read-only `SYNTHETIC_MCP_CANARY::ready`. Preview scroll/back/closed-panel no-reopen, approval/review priority and actual small geometry must also hold. Quit with Ctrl+Q and stop/wait only the owned fixture/consumer descendants.

Drive the displayed synthetic authorization URL with a local HTTP client preserving redirect/state/PKCE, not a browser or shell-history URL literal. Request-log path is `manifest["request_log"]`; it intentionally contains only sanitized lifecycle metadata. Ready/failure/OAuth/slow/silent endpoints and both read markers must be observed, with no `session/prompt`, tool invocation or file/browser/clipboard effect.

**Timing boundary:** fixture declarations deliberately have `waitForReady:false`; `--slow-ready 35` delays MCP initialize, not necessarily `session/new`. It cannot by itself prove caar's >30s new-session wait. Use caar's actual SDK gated-new regression and separately qualify any real delayed-new fixture before claiming that timing case.


## Delivery checkpoint 2026-10-20

### Source, ownership and publication

The qualified assembled Git **tree**, not commit, is
`a09f1dfb63b17d380f4661823de431bfb5d72160`. It combines original HEAD
`7a0108786a0cc52c01c87c14b42d5d449017328b`, landed main
`26b6dcb9346dbecf16c8c79f7abb86271ddd26bd`, and released-rendering prerequisite
`3a5b88b66b1267844ea764c4ff58e48294c93141`, plus exactly 19 feature paths.
The prerequisite-only index tree is
`699256c2b0dadf4000f72f6beb178b15ae8d1b90`. The portable
`current-publication-evidence/integration/assembled-source-allowlist.json`
records all **40 affected source paths, including deletion**; its integration
receipt records both incoming refs in the open prospective three-parent merge,
retained auth6c/EOF3730 fixtures, 17 unchanged feature postimages and exact README24/state18+77-line
reconciliation. Landed root/output-first tracking and released width helpers
are part of this qualified assembly, not inferred from HEAD alone.

The preserved current default-KAS executable is
`/tmp/cyril-tqr8-current-tested-20261020`, SHA-256
`1d7948e61584937f6355973f987280a3d1974a5b2adc86c751919c1122d69df0`.
**Current local qualification: all 12 requested Cargo lanes PASS.**
The independently sourced current source review and one owned B consumer also
PASS within their scopes below. Main owns staging, merge/feature commits,
tracker closure and the substantive PR; none is claimed performed here.

The source tree includes the README bytes qualified at SHA-256
`080fc6dba24454b8ff877ebbd9ac7d65ad6d376268cf820fa7da9bac897ea8e3`.
Main subsequently authorized only a README qualification-status correction.
That **postqualification documentation delta is not the same a09 whole-tree
identity**: before/after hashes and the exact patch are in
`current-publication-evidence/integration/README-documentation-delta.json`.
The qualified README bytes are retained as `qualified-source-README.txt`;
the other 18 feature source/test postimages are unchanged. Publication hashes
in `publication-manifest.json` identify current documentation separately from
the qualified source inventory.

Historical binaries6e99/source-before-gutter and2954/tree851c remain historical
only. Their original workspace failure and partial B/C consumer PASS are
retained below and in the unchanged `tqr8-publication-evidence.tar.gz`.
The original 116 pre-publication evidence-file identities are preserved by the
historical archive/snapshots; current workflow/index updates are explicitly
documentation changes, not relabelled original receipts.

`publication-index.json`, `publication-manifest.json` and
`current-publication-evidence/index.json` map every selected portable path and
hash to its original source. The new
`tqr8-current-publication-evidence.tar.gz` is separate from the historical
archive. No compiled binary, Git/index bytes, private HOME, credentials,
private journal or Cyril runtime log is packaged. Historical native IDs remain
publicly redacted; the current wire/screens/ANSI use only the owned
`sess_tqr_owned_synthetic` identity. Owned ANSI is an evidence download, not an
instruction to replay active terminal controls.

### Repository, sensitivity and independent review

**Current assembled qualification — PASS:** the exact copied parent receipt is
`current-publication-evidence/qualification/canonical-handoff.json`, SHA-256
`e5ed71d712d514ff04c9f5df11167c1780e08384251ad0f6ce812850d65b3104`.
It records treea09 unchanged before/after all requested gates, zero retries,
zero unexpected failures, original commands/environment and raw log hashes.
The retained command timestamps are UTC2026-10-21; the delivery heading/run
directory retain their original2026-10-20 naming.

| Lane | Exact command | Observed result |
|---|---|---|
| 01 | `cargo fmt --all` | PASS; no paths changed |
| 02 | `cargo test --workspace` | **2,869 passed / 0 failed / 13 ignored** |
| 03 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| 04 | `cargo test -p cyril --no-default-features` | **278 / 0 / 1** |
| 05 | `cargo clippy -p cyril --no-default-features --all-targets -- -D warnings` | PASS |
| 06 | `cargo build -p cyril --no-default-features --bins` | PASS |
| 07 | `cargo test -p cyril-core --no-default-features` | **1,035 / 0 / 4** |
| 08 | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | PASS |
| 09 | `cargo test -p cyril -p cyril-core --no-default-features --features cyril/kas,cyril-core/kas` | **1,816 / 0 / 10** |
| 10 | `cargo clippy -p cyril -p cyril-core --no-default-features --features cyril/kas,cyril-core/kas --all-targets -- -D warnings` | PASS |
| 11 | `cargo build -p cyril --bins` | PASS; preserved binary1d79 |
| 12 | `cargo fmt --all --check` | PASS |

Ignored means **not executed**, not passed. The four test lanes overlap and
must not be added as unique tests; direct-target count receipts exclude nested
subprocess summaries. Four all-target Clippy lanes and both build variants
passed. These are local Linux default/no-default/explicit-KAS results, not
CI-all-features, native KAS, live ignored-test, or host/platform qualification.

**Fresh current source review — no findings, confidence0.96:** ReviewCurrentTqr
inspected the exact 19-path feature delta699→a09 and source context, including
adapter/capability refusal, bounded delivery ACK, opaque escaping/nonterminal
notices, callback cleanup, stable gutter/four tests and both-sides integration.
`current-publication-evidence/review/current-source-review.txt` retains its
scope: read-only source inspection, not empirical consumer/Cargo proof, binary
rehashing, common-mechanism re-audit or review of the later README status edit.

The following pre-repair evidence remains historical, with its original
counts, source identities and scope:

- **Historical pre-repair PASS — parent bg121/artifact818:** 2,839 tests passed
  and 13 ignored, which did not execute. All-target Clippy, default build and
  formatting passed and output pins pre-repair binary6e99. The later viewport
  source change invalidates reusing this as the new source's assembled gate.
  `publication-evidence/tqr8-final-bg121-artifact818.log` preserves the authorized
  output. Main recovered the exact assistant bash tool call from the retained
  parent transcript; command, cwd, timeout and provenance are now in the index.
  That command executes workspace tests before Clippy.
- **Historical bug sensitivity:** artifact444 executed two example-render
  tests: apostrophe-positive failed because the old whole-string escaper
  changed `a'b` to `a\\'b`; the terminal-control positive passed. Its retained
  raw output is `publication-evidence/url-printable-apostrophe-red444.log`.
  An earlier focused-green checkpoint was parent-reported without a recovered
  original source/command receipt; its unverified count is omitted. This is
  historical context, not a reconstructed current PASS. Pre-repair feature tests
  execute in artifact818. Parent's later gutter sensitivity and qualification
  receipts below own the post-repair observations.
- **Feature review:** ReviewMcpUrl's original substantive review covered
  adapter/capability gates, dispatch, bounded responder handling, session/UI
  paths, tests and safe fixture; only F1 printable-URL fidelity was actionable.
  Fresh scoped F1 repair review concluded “F1: resolved. No new findings from
  the scoped repair review”; correctness **correct for the reviewed repair**,
  confidence **0.98**. This is fresh scoped repair review plus retained
  unaffected substantive review, not a newly rerun full-tree review.
  Receipts: initial diff artifact284, repair diff artifact447 (reviewed
  `lib.rs` postimage `e4835975`, example postimage `ac2bf6d2`), and
  `history://ReviewMcpUrl`. Reviewer did not compute a whole-feature tree/SHA
  pin or run empirical checks. Actual `mcp_notice.rs` and caller migration
  were read in that review.
- **Common test-fixture repair:** the separate auth-retarget review is
  confidence **0.97**, not 0.98; see `auth-retarget-proof-index.json`.
  Parent bg118 performed the deterministic six cases, including expected
  held-writer diagnostic RED, repaired GREEN, two typed overlap REDs and
  restored GREENs. These test-only receipts do not establish feature consumer
  behavior or the identity of the historical writable-descriptor holder.

### Historical first standalone consumer: immutable partial results and FAIL

Explicitly authorized command:

```sh
uv run --offline --script /tmp/cyril-tqr8-manual-owned-20261020.py \
  --output /tmp/cyril-tqr8-manual-owned-20261020-a
```

This historical run used the pre-gutter qualified binary6e99, a fresh isolated
HOME/PATH/XDG/KIRO_HOME/workspace, hooks off, local memory disabled, immutable
fake CLI and an explicit Node launcher for the owned Python ACP peer. No native
agent, network/OAuth endpoint, account, model or permission-grant request was
used. The consumer exited0. Its preassigned root-removal flag is not independent
teardown confirmation; the current independent observation is recorded below.
Private Cyril logs were neither read nor packaged.

The native source material is the explicitly authorized safe terminal and
wire projection from VerifyMcpConsumer's earlier NK run. The full synthetic
localhost URL is recovered from the visible terminal and hash-checked as
`92961ea663eec802f2dc1a32a3893165cf170cfa2869f8588647d1c7e9346c5a`.
The observed callback ID is 10; the supported payload is only `{url}`.
The wire file is a **nonsecret projection**, not raw ACP. Consequently this
peer uses source-shaped reconstructed envelopes: no exact raw-native-frame
replay, native response, sourceURL field or missing envelope field is claimed.
The native session identity was hash-matched privately to the terminal header;
published A captures redact it. B/C instead use their own explicitly synthetic
identities. The actual failure message is the
hash-matched native literal (`47dfeb004cfae952d3391165032110914ceefb4c6d0acdc8955f45af868d92a7`);
its MCP display-error envelope is source-shaped reconstruction.

The original receipt remains **FAIL**, not an all-cases PASS:

| Observation | Result and exact portable capture |
|---|---|
| Full opaque native URL at 140×40, callback during the new-session wait, actual `{success:true}` delivery ACK, no connected/authorized claim | PASS — `controlled-peer-a-01-native-url-manual-ack-not-auth.txt` and client wire |
| Exact native error text visible through actual KAS conversion and actual consumer | PASS — `controlled-peer-a-02-durable-native-shaped-error.txt` |
| Explicitly synthetic URL: printable apostrophe, quote, backslash and café exact; ESC/newline/CR/tab/bidi shown as visible escapes | PASS — `controlled-peer-a-03-escaped-controls-printable-exact.txt` |
| Unsubmitted draft survives unrelated subsequent content | PASS — `controlled-peer-a-04-later-content-before-navigation.txt` |
| Full opaque URL reachable intact after PageUp | **FAIL** — `controlled-peer-a-FAILED-final-screen.txt`: main-conversation browse scrollbar paints over the last wrapped URL characters at the right edge |
| PageDown return and final all-case assertions | NOT REACHED after the real full-URL assertion failed |

All filenames in that table are beneath `publication-evidence/`. The original
private result/screens are retained unchanged in
`/tmp/cyril-tqr8-manual-owned-20261020-a/`; published versions are clearly
labelled privacy projections. The failure is inside the approved full durable
manual-URL acceptance, not waived as unrelated work. Main first requested the
bounded source diagnosis coordinated with C6, then DG-authorized the repair
below. Main later supplied explicit owned-consumer GO for the exact partial
candidate2954; this did not waive its failed workspace gate.

### Historical gutter repair — observed sensitivity and source review

Only `crates/cyril-ui/src/widgets/chat.rs` changed in production. Main chat now
always reserves the rightmost column when outer width>1 and uses the remaining
rectangle for message rendering, live Markdown preparation, wrapped line count
and paragraph paint. The scrollbar paints only in the separate one-column
gutter. Widths0/1 retain their available content and omit the indicator.
Geometry does not depend on overflow, scroll offset or follow/browse state.
There is no URL byte, formatter, protocol, ACK or auth change, allocation,
vendor/dependency change, or focused-subagent expansion. The separate C6 owner
is applying the same stable-width contract to its retained renderer.

Tests were written before the production correction, without execution.
The four new real-renderer regression tests are:

- `manual_url_bytes_survive_scrollbar_in_follow_and_browse`: actual
  `ExternalUrlRequested` → `UiState` message → renderer, complete opaque URL
  with café/wide Unicode, printable quotes/backslash and explicit control/bidi
  escapes, with actual overflow and scrollbar chrome;
- `chat_gutter_width_is_stable_without_overflow`: the same content width
  before scrolling/overflow;
- `live_text_uses_the_same_reserved_content_width`: complete streaming text
  survives separate chrome and consistent pre-wrap/paint widths;
- `zero_and_one_column_chat_keep_available_content_without_scrollbar`:
  full available tiny content in follow and browse, with no indicator. After
  reviewer P3 identified the two-row zero-track trap, this uses a four-row
  overflowing viewport and asserts all18 backend cells; only this test changed.

The retained patch inputs are **pre-format proof artifacts**, not the final
publication postimages: `scrollbar-regression-tests.patch` adds only the four
tests; `scrollbar-layout-only.patch` reverses only production geometry while
all feature APIs/tests remain; `scrollbar-source-and-tests.patch` assembles that
pre-format source. Parent proved all four unchanged tests behaviorally **RED**
on old layout and **GREEN** after exact restoration in attempt2. The earlier
compiler-setup failure was not behavioral RED and is not counted.

The exact `scrollbar-width-one-guard-mutation.patch`, SHA256
`69af8d72462d3ace0b60b8d0ee7b6b906686745a64c6b74443144f5c3807a689`,
lowers only the indicator guard after content paint from width>1 to width>0.
Its separate content-width rule and row geometry remain unchanged; width0
stays safe, while width1's arrows/track overwrite expected existing cells.
Parent observed that full-cell assertion **RED**, then unchanged-test restored
**GREEN**, with exact source restoration. This is not a vacuous zero-track or
reserved-away-content failure.

These five sensitivity pairs ran on source tree
`b7b9d2546a945d2278a5aec359ed276bce335699`, chat SHA256
`e750b9765c6843b9d649b9897f407992e4385cdfee81188e0c03d12465896b1c`.
Fresh source-only gutter review was correct0.97, P3 resolved by inspection,
reviewed Git blob `7535e86096b0a32c8204c55667bd981396772e10`
(`artifact://1156:2`); see `publication-evidence/fresh-gutter-ReviewMcpUrl.log`.
That source review itself did not execute the proofs.
Parent then ran its authorized formatter: only chat and the callback test file
changed between the pre-format proof tree and recorded post-format tree851c.
The publication chat postimage is SHA256
`76065d35db2febc4b3c0627a6f02edeccb4673baf9979887f5789d766c3874c5`.
Source/format boundaries, command logs and exact restoration are preserved in
`publication-evidence/gutter-attempt2-canonical-handoff.json`.
Current qualification retains these **five historical** RED/restored-GREEN
pairs; it did not rerun them under the released ratatui pair. The copied
`qualification/retained-gutter-mutation-applicability.json` records the
unchanged production geometry, three byte-identical test bodies, and only
rustfmt folding in the width0/1 test with unchanged assertion tokens. The URL
fixture is mixed Unicode, not ASCII-only; helper-width applicability is a
fixture-specific inference, not general grapheme proof. Fresh current GREEN
execution of all four tests and the current consumer are separate evidence.
The original consumer FAIL remains unchanged, not relabelled or waived.

### Parent-applied URL callback fixture cleanup — changed-path checks PASS

Main applied only `review-callback-cleanup-final.patch`, SHA256
`f44db50c711041216ab6edc31669479dc4b179ebf49f912a484c5edc62e814c2`,
to `crates/cyril-core/src/protocol/domain_mediator/tests/external_url_wire.rs`.
Its pre-format application SHA256 was
`2b4e4eb80bff700ac0da74fbf5493b18a7756699c5a90d412490c2411a6a3599`;
the post-format publication SHA256 is
`01fccc081c3f9343b02e9b789b03de3f291a7ce130853989c7ab41100c1cf674`.
source owner supplied old/new Git blobs
`7a25c20e584eb2e9a012b8444d8a8c4bb7ea5b81` →
`2ab281893c88998154d3fe5e8bfad021f3e65110`.

The test helper's cancellation-only cleanup assertion could reject a normal
host finish after SDK shutdown when KAS is absent. The existing five-second
fixture allowance now bounds the whole shutdown/abort/join sequence; it accepts
normal unit completion or cancellation and still rejects panic/other errors.
Exact initialization/callback assertions and success/unsupported/closed-delivery
predicates are unchanged. This is test-only: no URL product, chat, auth,
dependency or product-deadline change. The separate NK `mcp_wire.rs` cleanup
does not belong to Tqr and is neither applied nor packaged here.

Provenance and canonical safe source path are in
`publication-evidence/parent-url-fixture-cleanup.log`. Main retains the original
no-default Caar cleanup FAIL; the separate Tqr no-default callback-focused
case later passed1/1 and the full no-default core lane passed1,031 with
4ignored (not executed). The overlapping no-default binary-package lane
passed278 with1ignored (not executed). These are separate lanes, not a unique
aggregate count or a replacement for failed workspace acceptance.
The worker did not reapply source or run Cargo/format/checks.

### Historical partial qualification — superseded current gate

Attempt2 preserved four old-layout/restored pairs and the named indicator-only
pair, then passed workspace all-target Clippy. Its full workspace test command
failed in `cyril-workbench`, `tests/reviewer.rs`,
`runtime_drop_retains_evidence_until_core_completion`, with
`Error: "missing EOF window"`. Observed completed top-level suites totalled
**2,834passed,1failed,13ignored (not executed)** before Cargo stopped; nested
filtered subprocess summaries are excluded. This is not full workspace or
doc-test completion, a flake classification, baseline waiver or accepted risk.
The exact raw failure is
`publication-evidence/gutter-attempt2-13-full-workspace-tests.log`.

Main explicitly authorized continued changed-path qualification without
repeating the failed workspace run or sensitivity proofs. All seven continued
commands passed: default build, final fmt check, no-default binary build,
no-default binary all-target Clippy, no-default binary package tests,
exact no-default core URL-callback case, and full no-default core tests.
Commands, cwd, timings, source pins, explicit shared-target environment,
entrypoint-touch receipts and individual log hashes remain in the two portable
canonical handoffs. Their original absolute log paths map to listed
`gutter-attempt2-*` and `gutter-continued1-*` archive files.
The continued receipt is
`publication-evidence/gutter-continued1-canonical-handoff.json`.
Candidate2954 was copied from the default build before no-default builds;
its SHA was confirmed by the qualifier afterward.
**WORKSPACEBLOCKED was that historical candidate's result.** The fresh12-lane assembled-source PASS above supersedes the current blocker; the original failed receipt is neither waived nor rewritten.

### Historical post-repair consumer — scoped PASS on partial candidate

Main's explicit GO retained the same140×40 viewport, exact full-URL comparison,
no padding/masking/reinsertion, immutable fake CLI, isolated roots and controlled
peer only. It expressly approved reading the actual139 declared content cells
while retaining all140 raw screen columns and independently checking the
reserved scrollbar column. This is the product's repaired content rectangle,
not an oracle that deletes URI bytes: the old captured continuation lost `l`
before `ZHeh`, whereas the repaired screen actually contains `lZHeh`.
The four executed renderer REDs independently distinguish the old geometry.

```sh
uv run --offline --script /tmp/cyril-tqr8-manual-owned-20261020.py \
  --output /tmp/cyril-tqr8-manual-owned-partial-20261020-b
uv run --offline --script /tmp/cyril-tqr8-manual-owned-20261020.py \
  --output /tmp/cyril-tqr8-manual-owned-partial-20261020-c
```

Both actual owned runs **PASS** on binary2954/source851c. B's session
`sess_tqr_owned_synthetic` already satisfied the admitted `sess_` KAS shape.
C is an additional owned fixture variant, not a new requirement: the preparer
misinterpreted UUID-suffix wording and it finished before Main clarified that
no UUID suffix was needed. B remains a valid PASS and its executed harness,
result and reconstructed inputs are retained unchanged. C uses only the owned
synthetic `sess_5f9db173-329e-4c0f-8e5b-43eec1a2858f`; neither uses an original
native identity. No later run followed the clarification.

C's exact executed harness SHA256 is
`0865210da51546f142acc674b90286d5daec6dc05fbe68b1863439cff2a59b8a`.
Its full raw ANSI,140-column text screens, actual-content-cell records, wires,
synthetic inputs and result are explicitly listed as `controlled-peer-c-*`.
The retained observed native URL/error values are the same safe synthetic
localhost observations as A; ACP envelopes/session/request IDs are reconstructed
or owned synthetic. This is **not raw native-frame replay**, a fresh native
producer run, a sourceURL claim, OAuth completion, NK-panel or caar proof.

| Actual consumer observation | Result / C portable capture |
|---|---|
| Entire opaque observed URL during pending session/new, exact display-delivery ACK, no authorization-success claim | PASS — `controlled-peer-c-01-native-url-manual-ack-not-auth.txt`, client wire |
| Observed native error literal through actual KAS conversion/consumer | PASS — `controlled-peer-c-02-durable-native-shaped-error.txt` |
| Printable apostrophe/backslash/quote/café preserved; ESC/newline/CR/tab/bidi visibly escaped | PASS — `controlled-peer-c-03-escaped-controls-printable-exact.txt` |
| Draft survives unrelated later content | PASS — `controlled-peer-c-04-later-content-before-navigation.txt` |
| PageUp retains complete exact native-value and synthetic escaped URLs, durable error and draft with separate scrollbar chrome | PASS — `controlled-peer-c-05-pageup-durable-full-urls-and-error.txt` and content-cell record |
| PageDown returns to latest row49; no prompt/reset/permission, openExternalUrl present and secretStorage absent | PASS — `controlled-peer-c-06-pagedown-return-to-latest.txt`, wires and result |

Consumer exit0 and **owned temporary-root removal observed after context exit**
are recorded in C; no complete-descendant/handle census is claimed.
The independent read-only owned-proof review found no findings, correct within
scope0.94, including the C delta and correction that B needed no UUID suffix.
It inspected B wires/screens and only the five named C delta artifacts; other
C wire outcomes are receipt-backed, not independently reread.
See `publication-evidence/owned-partial-consumer-review.log`.
It did not execute checks, recompute binary hashes, review production source
or establish native/host/full-gate qualification.

The original A FAIL, raw workspace FAIL, historical native31/32 FAIL and their
scope/adjudications remain separate and immutable. B/C established partial
candidate2954 navigation proof only. The fresh assembled local gates and
current owned B checkpoint now own current status; Main still owns
commit/tracker/own-PR publication. No blanket all-claims/native/CI PASS follows.


### Current owned B consumer — one invocation PASS

Main authorized exactly one current owned B scenario:

```sh
uv run --offline --script /tmp/cyril-tqr8-current-owned-setup-20261020/controlled-peer-current-harness.py \
  --output /tmp/cyril-tqr8-current-owned-20261020-b
```

Harness and consumer both exited0. No retry, C duplicate, viewport enlargement,
padding, masking, reinsertion or product-oracle change occurred. The executed
harness SHA-256 is
`a9b194233c78c6970c2fd01408f7eae75d22e6e515a659c9c3b207dd4c7daa45`;
the copied `consumer/result.json` SHA-256 is
`e6bddc068c68ed2dd31603f88b310ef3b5e8b376e6a18d4692d7345ce0d34367`.
Its `fully_qualified:false` / `pr_ready:false` fields correctly limit that
consumer-only receipt; they are not a competing overall Cargo/publication
judgment. Original receipt bytes are unchanged.

All six captures passed at **140×40 outer / 139 content columns**, with actual
gutter chrome retained: full published fixture URL and delivery ACK during
session/new wait without authorization claims; durable error; printable opaque
URL characters exact with terminal/bidi controls visibly escaped; draft after
later content; PageUp recovery of both complete URLs/error/draft; PageDown
return to latest content with draft intact. Screens and matching cell records
are in `current-publication-evidence/consumer/`. Client wire has initialize,
session/new and two delivery ACKs only: no prompt/reset/permission; URL
capability present and secretStorage absent.

The published historical native SID is the explicit redaction placeholder,
not the owned `sess_tqr_owned_synthetic` ID. The retained fixture URL hash stays
`92961ea663eec802f2dc1a32a3893165cf170cfa2869f8588647d1c7e9346c5a`.
Current ACP envelopes/session/request IDs are synthetic; no raw native-frame
replay, original native SID or missing sourceURL field is invented. Setup
receipts retain offline/public-input hashes and root-safety provenance.

**Initial preflight METHOD FAILURE, not product failure:** the inapplicable
tree-versus-index `git diff` showed apparent deletion of five known untracked
files. Its original1041-line observation is retained under
`current-publication-evidence/method-failure/`; there were **zero product
invocations before correction**. Main authorized exact blob-byte and100644
mode comparisons for precisely those five files; the corrected read-only
preflight passed before the one invocation. The failed command was not rerun.
No index-equals-tree or documentation/tracker equality claim is made.

**Independent cleanup and limit:** the owned temporary root was absent after
exit; a937-entry post-exit `/proc` symlink census found zero exact owned-cwd
marker/deleted-cwd or tested-binary matches. No observer cleanup signals were
sent. Live observation captured only uv, not a complete descendant PID/PGID
tree. This proves root absence plus a bounded post-exit owned-marker census,
**not captured-complete historical process-group teardown**.

This is owned fake ACP-peer/PTy evidence only. No real Kiro/auth/provider/
network/model/grant/browser/clipboard operation, fresh native KAS/host/
terminal acceptance, current NK-panel/Caar acceptance or CI result is claimed.
The documentation worker copied/hashes evidence and corrected status only; it
ran no new consumer, Cargo command, formatter, check or native operation.

### Earlier native integration and strict stop boundary

Earlier real-KAS evidence belongs to the separately qualified NK binary
`04eb4da4cc068c08e00e8ec6bd7bc6d8d70329ecaf01fdaa6db6e65beb7cb96f`,
Git **tree** (not commit)
`19e2d205b717f828fd7d18e46153b0553bbb22e9`, fixture `df5d9437…9b7`.
It is not relabelled as a fresh standalone Tqr native run.

- Passive **17/17 PASS**: real auth-required status, full visible opaque URL,
  real MCP display-error and truthful unreported policy.
- Active original **31/32 FAIL remains immutable**. The required full visible
  URL → explicit local synthetic OAuth callback → authoritative connected
  leg was observed. The sole extra failed premise required normal no-OAuth
  reset to stay connected despite no secretStorage. Main accepted the separate
  `normal-reset-adjudication-df5d9437.json`: omitted `startOAuth`, authoritative
  auth-required status and truthful ACK presentation satisfy the approved
  normal-reset contract. This is source/evidence adjudication, not a rerun,
  blanket active PASS, credential-persistence claim or changed acceptance.
- Projected native active wire lines 29–30 record callback and delivery ACK;
  line 31 is authoritative connected after the local OAuth leg. Lines 35,
  39 and 42 separately show ordinary reset without `startOAuth`, auth-required
  status, then ACK. Neither ACK is authorization.
- The first later preview attempt failed **before initialize completed** with
  native stderr `error: You are not logged in, please log in with kiro-cli login`.
  No session/new, preview or MCP HTTP request was reached. Its cause remains
  unclassified; no native retry, login, account/config change, token-store
  inspection or copy is permitted. NK previews/natural inventory and caar's
  deadline arms remain unqualified, and are not represented as Tqr features.

The native evidence handoff, scenario receipts, safe wire projections, selected
terminal projections and separate reset adjudication are explicitly listed in
`publication-index.json`. Native sourceURL was not recorded: a known fixture
endpoint must not be invented as that field's original value. Linux decoded
PTY/text evidence is not native Ghostty/Windows/macOS terminal qualification.


## Related issues

Full records read: `cyril-tqr8`, `cyril-nk4o`, `cyril-caar`; `cyril-j16p` prerequisite is closed. `nk4o` owns passive status/panel/reset and the elicitation decision; `caar` owns startup deadline and diagnostics. `cyril-0o7e` owns broader governance observability and is not silently claimed by this stack. No issue is closed or tracker record changed.

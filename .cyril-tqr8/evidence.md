# cyril-tqr8 — supported KAS OAuth evidence

Historical investigation date: 2026-10-19; inspected production base `8e5c325e`. The [held-main integration 2026-10-22 addendum](#held-main-integration-2026-10-22) owns the current actual503-input identity, fresh32-gate qualification, scoped static integration review and one fresh authorized eleven-capture owned-consumer PASS. The [standalone PR193 repair checkpoint](#current-pr193-repair-checkpoint-2026-10-21), [prior a09 delivery checkpoint](#delivery-checkpoint-2026-10-20), earlier failures and partial checkpoints retain their original identities/results/scopes. Held-source qualification is not a completed merge, current-head CI, native acceptance or final integration acceptance.

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


## Held-main integration 2026-10-22

**Current held-source checkpoint — fresh qualification and owned consumer PASS,
not all-claims acceptance.** Main's source integration of Tqr HEAD
`09e41ddd0306567b9430ea3216902f895fc96370` with held MERGE_HEAD
`03838a501dad8be140acd78dc3fff4cae35fafef` is qualified in its held state.
Completed merge/ancestry, commit/push and current PR-head CI/review remain
Main-owned and are not claimed here. This addendum supersedes the standalone
checkpoint's active source pointer and not-yet-integrated chronology, without
rewriting any prior receipt, archive, empirical result or approval.

### Current source and private executable binding

The **503-row bounded actual-input SHA-256 descriptor** is
`1305934e10aa4e1124a22eb0ae308e8549f7da6619a3a33a0dca13e77669d66d`.
It is **not a Git tree, completed-merge identity or whole-repository identity**.
The authoritative503 manifest uses SHA-256 of the qualified merged extended
input-row JSON sorted by path, a different descriptor convention from the
standalone498/d265 record. Neither the descriptor algorithms nor their source
scopes are interchangeable; HEAD alone does not identify the held inputs.

The fresh canonical qualification owner is
`/home/dwalleck/.claude/tmp/cyril-batch3-qualification-20261020-ipe924db/tqr8-current-main-integration-green-attempt1/canonical-handoff.json`,
883,702 bytes, SHA-256
`db4c63cd2b65e06b752eed1b2d69ac97e4052da332f081b8af665be27885649e`.
The archive below contains that exact canonical and the exact111,646-byte
503-input manifest, rather than only an editorial summary.

The qualified private default executable actually used by the fresh consumer
is `/tmp/cyril-tqr8-held-main-default-qualified-20261022T015043431767Z`,
102,118,680 bytes, mode0755, SHA-256
`d3a754b3d5176b102a154c12c74e3080edcfbef126195dcee765519450258142`.
Its ready receipt is631 bytes, SHA-256
`41ee2cfee52d775bde45e12abc606dd41eb6b359d4ac7fd74b3f2b0656ef2ccc`.
The private binary is externally pinned, not embedded in the publication
archive. Canonical/ready/preparation records retain their then-unexecuted
consumer/native fields; the distinct subsequent GO and actual result own the
later consumer execution, not retroactive edits to those records.

### Fresh held-source Cargo qualification

The canonical receipt records **32 actual authorized Cargo commands, all32
exit0: two fmt,22 test, six Clippy and two build commands**. These are fresh
held503 executions, not promotion of standalone09/d265 or Main-only gates.
The default workspace reports **3,058 PASS /0 FAIL /13 ignored**.

| Fresh isolated test lane | PASS | FAIL | Ignored |
|---|---:|---:|---:|
| App no-default | 354 | 0 | 1 |
| UI no-default | 967 | 0 | 2 |
| Core no-default | 1,035 | 0 | 4 |
| App-only explicit KAS, no-default | 370 | 0 | 1 |
| Both App/Core explicit KAS | 1,892 | 0 | 10 |

Across all22 test commands, **7,720 passing /0 failing /31 ignored
observations include focused/workspace/feature overlap**, not7,720 unique
tests; ignored tests did not execute. The canonical's exact commands and
target summaries own the individual gate scope. Fresh Core lanes are needed
because incoming Core test support and manifests invalidate the earlier
identical-Core-input retention premise.

### Fresh owned consumer — one invocation, eleven captures PASS

The [fresh outer consumer result](current-publication-evidence/held-main-integration-20261022/tqr8-current-main-consumer-result.json)
records **exactly one Main-authorized invocation, zero retries,2.19seconds,
consumer exit0 and all11 capture cases PASS**, bound to503/130593 and the
private d3 executable above. All captures are140×40 outer /139 content
columns with the existing one-column gutter. The actual raw wire contains
**client5 /peer67, three URL callbacks with three success delivery ACKs,
four MCP display errors and zero real native Kiro sessions**. Client requests
are initialize1 and session/new1 only; ACK IDs are10010,20,30. No prompt,
reset, permission, model or tool-management request is inferred.

All original six case bodies and appended five case bodies remain unchanged:
the original sequence reaches the complete fixture URL while session/new
awaits delivery ACK, manual ACK-not-auth, durable native-shaped error,
printable-exact opaque URL/control escaping, later content and PageUp
URL/error recovery followed by PageDown return. Original stage6's draft
field remains recorded-only; the appended cases independently assert exact
draft and block-caret preservation.

The appended sequence actually establishes prior browse, current URL reveal
of recent main/follow state with ACK30, prior browse before errors, exact
main/optimistic-child presentations once with safely escaped actual-session
attribution and the entire opaque child payload, then one late workflow
claim/error presentation with the entire opaque workflow payload and no
duplicate old child presentation. Visible chronology, foreign child-text
isolation, exact draft and independent caret are preserved.
**Delivery ACK is not connected/authorized status or OAuth completion.**

Hidden canonical child/workflow storage, adoption, pending replay, internal
authority and focused rendering/reveal remain source/API-test evidence,
not visible-main screen proof or an end-user production focus-entry path.
No focus binding/view was added. Synthetic lifecycle ingress with the
unchanged `workflows=false` advertisement does not prove natural native
gate-off workflow origination.

**Independent cleanup and visibility limits:** the actual owned root
`/tmp/cyril-tqr8-owned-private-6kgc1jvy` was observed absent, with zero exact
owned-marker matches in **readable** cwd/exe links. The968-entry global
census read455 links of each kind and retained **482 EACCES +31 ENOENT per
link**; the same-UID supplement retained **five EACCES +31 ENOENT per
link**. Unreadable/missing entries are not declared inspected, attributed or
absent. No observer signals were sent. Frozen launch sites and one-session
wire evidence do not constitute a complete historical/live/cross-owner
PID/PGID ancestry capture or universal process-absence claim; runner
pre-exit flags are not substituted for the independent observation.

### Portable48-member archive and distinct receipt identities

The exact three-file package is under
`current-publication-evidence/held-main-integration-20261022/`:

| Published file | Bytes | SHA-256 |
|---|---:|---|
| [Consumer archive](current-publication-evidence/held-main-integration-20261022/tqr8-current-main-consumer-evidence-20261022.tar.gz) | 256,279 | `94ef9abf2e9004b4268263b13525f552c35d102f62d24e24851e13ba32e3599d` |
| [Archive index](current-publication-evidence/held-main-integration-20261022/tqr8-current-main-consumer-archive-index.json) | 17,445 | `bd4ba70328f00b90f2086a4456209aeaf6b9aaf7ce72efbf1555513c68c5322b` |
| [Outer consumer result](current-publication-evidence/held-main-integration-20261022/tqr8-current-main-consumer-result.json) | 30,124 | `c77fea4452a365a5d0ac31b6567bbe766a0d199924c6a9b190847ad7fa847f9d` |

The archive has **48 explicitly selected regular-file members**, including
all raw ANSI/wires/results, eleven text/content-cell capture pairs, frozen
runner/adapter/binding/authorization provenance and the qualification
metadata. Its internal manifest SHA-256 is
`5a53cd00693c29894eb0b9e56df3163be8dcc27ac6cc054c19bb5f56d370aa8b`.
The index owns the complete member inventory; selected decisive member pins
are:

| Archive-relative member | Bytes | SHA-256 |
|---|---:|---|
| `evidence/canonical-qualification.json` | 883,702 | `db4c63cd2b65e06b752eed1b2d69ac97e4052da332f081b8af665be27885649e` |
| `evidence/current503-source-manifest.json` | 111,646 | `d85c23d8ff4b2f5edc68946282808be9ff395e46b69294b1063ea7c348d05e0e` |
| `evidence/current-main-qualified-handoff.json` | 4,102 | `5279e480a999212bdd2914266ec4b25ac3b852f88f19f75cc19e790859d069c5` |
| `evidence/ready-private-default-binary.json` | 631 | `41ee2cfee52d775bde45e12abc606dd41eb6b359d4ac7fd74b3f2b0656ef2ccc` |
| `runtime/result.json` | 6,972 | `fe2ac79dbabb2c8a8f1855ba4bc0b5b5302ac7c2b6337199d415ebe1a90104d4` |
| `runtime/independent-cleanup.json` | 1,529 | `3cdc90f78ccfd7e9b361a96f550bb81fed198775d783f0f92f97daca58fd75c0` |
| `evidence/current-main-consumer-result-core.json` | 19,288 | `1b988ec67ca8fa580662e83d84be2de623b34c4e8bbd6f5724f5b636479d423b` |

The internal core result deliberately precedes archive metadata to avoid
circular hashing. Its19,288-byte/1b988 identity is **not** the
30,124-byte/c77 outer result or the6,972-byte/fe2 runtime result.
No product binary, full workspace source copy, private Kiro/credential root
or unrelated log collection is included. Prior standalone49-member/
270,484-byte archive,691 executable and all raw receipts remain byte-for-byte
historical evidence; none is replaced by these48-member/256,279-byte/d3 facts.

### Review layers and retained-proof applicability

The canonical's `both_parent_static_review_scope_only` selects
`agent://IntegrateTqrPrerequisite.ReviewHeldMainTqrIntegration`: **no actionable
source-integration findings; correct for scoped static integration review,
confidence0.96**. It found both parents' relevant routing/reveal, canonical
ownership/dedup, shared gutters, incoming theme/cache/diff/selected-fit,
CLI formatter and Peaq fixture behavior preserved. This is static integration
review, **not** qualification PASS, a fresh whole-merge review or isolated
production reconstruction. The fresh32 gates and subsequent eleven-capture
consumer separately establish only their exercised empirical layers.

The standalone09/d265 ordinary repair review remains correct/no findings,
confidence0.97 **within its original scope**. Changed UI production inputs,
App tests, manifests/lockfile, Core test support and five incoming inputs
prevent blanket promotion of its App/UI/workspace/Core gates or consumer.
Unchanged formatter/example mechanism preserves that reviewed mechanism,
not whole-production byte identity or fresh merged CLI execution. The
separate historical CLI RED/GREEN remains historical.

The retained consumer recipe behavior review
`b1e891f5befdbbacebf4e192ce037d4f0f082bbd9b2a036ba19f686deb65a1d0`
applies because all11 case bodies, actors, fixtures, adapter and cleanup
are unchanged. Main separately reviewed the binding-only diff
`11aab470590bc7c32edb227c9aa699cebd4a795f617eb1359a614184d048a870`
before authorizing this one fresh invocation. Neither review is expanded
into new native/whole-feature acceptance. Canonical pre-consumer/HOLD/ready
fields retain their original scope and are not rewritten to claim the later
consumer run.

### Current remaining boundaries and Main ownership

- **Main038 source integration is held and qualified, not a completed merge
  or push.** Main owns ancestry completion, staging/commit/push, current
  PR-head CI/review and tracker/PR/stack disposition. PR193's retained CI
  result remains old RED, not current-head/platform CI PASS.
- **Peaq PR194 is resolved and landed** at main038 after refreshed
  head991b16f9/base7a0716dc and GREEN CI run39690453316. Its
  [publication record](https://github.com/dwalleck/cyril/pull/194#issuecomment-6329881410)
  is not current Tqr CI. Separate **`u62f` Windows attribution remains open**,
  not waived by green Windows CI.
- No fresh native KAS/auth/bootstrap/model/provider/browser/clipboard/network,
  native-host, Windows or macOS acceptance is claimed. The native-auth stop
  still forbids retries, login, account/config or token-store actions without
  fresh Main authorization.
- Main-only cache/performance/C4/benchmark evidence is **not automatically
  held-composition proof**; its source applicability remains unresolved
  wherever relied upon. Fresh Cargo gates do not supply benchmark acceptance.
- Downstream NK propagation and final `nk4o`/C4
  reset→callback→authoritative-connected, `caar` and C6
  retained-history/live/gutter acceptance remain unresolved/unclaimed.
  NK local `ef46c43813d12b63f45b264e427abda84d299f46` is not proof of
  propagation into Tqr or final NK/native/C4 acceptance.
- Original RED/oracle/fmt/CLI/A/EOF/native failures, partial candidate2954 B/C,
  historical818, a09/twelve-lane/six-capture and old mutation receipts keep
  their exact source/time/reach. No historical mutation is relabelled a fresh
  held-main sensitivity run. This addendum is no issue-closure, shipping,
  merge/publication authorization or acceptance waiver.

This docs/evidence-only amendment changes the four assigned existing owning
documents and hashes those documents only. It runs no checks, tests, builds,
linters, formatters, product/native runtime, Git/index/ref, tracker or PR action.

## Current PR193 repair checkpoint 2026-10-21

**Historical standalone scope (2026-10-21, later retained at09/d265):** all
“current” labels and figures in this checkpoint describe that standalone
source/time, not held-main503. The [held-main addendum](#held-main-integration-2026-10-22)
now owns the active identity, qualification/consumer and remaining boundaries,
including the superseding held-source integration chronology. Original
standalone receipts, archives, pins and empirical records below are unchanged.

**Current approved repair scope — PASS, not all-claims acceptance.** All four
PR193 findings are resolved within the approved scope. The
[portable repair index](current-publication-evidence/pr193-repair/index.json)
links the selected [qualification](current-publication-evidence/pr193-repair/qualification.json),
[repair review](current-publication-evidence/pr193-repair/repair-review.json),
[owned consumer](current-publication-evidence/pr193-repair/owned-consumer.json)
and [history/CLI](current-publication-evidence/pr193-repair/history-and-cli.json)
records. These are the current pointers; existing a09/native/A/B/C receipts and
archives below remain historical, not overwritten or relabelled.

### Current source identity and bounded review

The current **498-row bounded actual-input SHA-256 descriptor**, **not a Git
tree or commit**, is
`d265f295d35ba2eabe692d6000c3127771ded51f30865361b55cb86406bb7117`.
Published base HEAD `b75010427c16b1dac13b7cf00c595ce2492f391f` alone does not
identify the repaired working source. The [source-input metadata](current-publication-evidence/pr193-repair/source-inputs.json)
preserves the qualifier-owned manifest provenance. The final canonical receipt
is 798,044 bytes, SHA-256
`2ba47cce19d4f09deb62759d4d298b91b656eaa579496311000847207c9cbad0`;
the portable qualification is a selected record, not an assertion that it has
the full canonical's bytes/hash.

The separately retained postformat default executable
`/tmp/cyril-tqr8-current-postfmt-tested-20261021` has SHA-256
`691519208093d6b682fea20ce0cb366e01e15d39f739218c7b98d61e979a81f6`.
The qualifier recorded stable bounded source and HEAD/ref/merge pins but
unknown-cause raw-index-only drift. Neither original logical-index identity,
restoration nor a whole-repository identity is claimed. This documentation
update is not an index/staging/commit action.

Independent bounded repair review concluded **no findings; correct,
confidence 0.97**:

| PR193 finding | Current scoped disposition |
|---|---|
| F-URL / 4332636456 | One global chronological main notice, MCP-only reveal/follow reset; no invented session/server attribution or duplicate URL path. |
| F-GUTTER / 4332636462 | One allocation-free content/gutter split reused by both existing render branches; content preparation, line counts, scrolling and paint share width; widths0/1 retain content without chrome. |
| F-WORKFLOW / 4332636476 | Scoped canonical storage remains with its owner; one escaped actual-session main presentation, without main lifecycle-authority migration or repeated presentation on adoption. |
| F-CLI / 4332636469 | Actual initial drain reuses the unchanged terminal-safe formatter for MCP errors; printable opaque text is preserved and terminal/bidi controls visibly escaped. |

This is ordinary independent bounded source review, not isolated reconstruction,
whole-feature/native/platform/CI review. The review's earlier optional-field
comma-removal claim was retracted: the preserved preformat fixture had no such
field comma. Exact token identity **including punctuation** stands; no extra
fence/rerun or risk waiver is implied.

### Current qualification, retained RED and oracle corrections

The immutable original test-only run actually executed **21 runnable cases**:
17 new/replacement cases comprised **14 intended behavioral RED + three new
GREEN controls**, with **four retained prior App GREEN controls**; zero ignored
and zero unexpected compile/setup/wrong-assertion failures. This is not a claim
that all later postconditions or all19 declared input iterations were reached:
18 iterations entered; the second UI-state input and later assertions after
first failures remained unreached. [History/CLI](current-publication-evidence/pr193-repair/history-and-cli.json)
retains those reach limits and original failure pins.

Two subsequent repair-attempt failures remain immutable:

- **Attempt1 focused-error oracle FAIL:** actual locked widgets0.3.2 wrapping
  consumes separator whitespace at the prefix/overlong-word boundary. The
  approved correction asserts the entire306-byte canonical message including
  separator, ordered rendered prefix and the entire unchanged287-byte
  opaque/escaped tail plus final marker contiguously. The retained old-gutter
  capture loses an underscore inside that required tail; the repaired capture
  preserves it. No product/input/geometry/helper or whitespace-policy change,
  normalization or reinsertion was made. Corrected Rust execution actually
  passed in attempt2 and later suites; the old capture comparison is not a
  retroactive corrected-test execution.
- **Attempt2 App projection FAIL:** the old physical-cell-to-text helper
  inserted a phantom blank after wide `日`. The approved helper correction
  uses the existing width-advancing pattern and already available `CellWidth`,
  skipping only covered continuation cells while retaining real spaces and
  row newlines. All14 case bodies/later helpers (23,444 bytes) were unchanged
  **at that correction**; later rustfmt changes raw bytes, not their tokens.
  Fresh whole-App execution in attempt3 actually passed14/14, including the
  URL case's later preservation/authority assertions. No corrected-helper
  old-production run or generic original-RED mutation rerun is claimed.

The current [qualification](current-publication-evidence/pr193-repair/qualification.json)
records these **11 actual successful preformat gates**:

| Gate | Actual preformat result |
|---|---|
| 01 — whole App `mcp_oauth` module | 14 passed / 0 failed / 0 ignored |
| 02 — full workspace tests | 2,885 / 0 / 13 |
| 03 — workspace all-target Clippy | PASS |
| 04 — default all-bins build | PASS |
| 05 — isolated App no-default tests | 287 / 0 / 1 |
| 06 — isolated App no-default all-target Clippy | PASS |
| 07 — isolated App no-default all-bins build | PASS |
| 08 — isolated UI no-default tests | 862 / 0 / 2 |
| 09 — isolated UI no-default all-target Clippy | PASS |
| 10 — isolated App explicit-KAS/no-default tests | 303 / 0 / 1 |
| 11 — isolated App explicit-KAS/no-default all-target Clippy | PASS |

All21 scoped cases passed within the fresh preformat workspace run. Isolated
App no-default executed13 MCP cases; explicit-KAS App executed14. The seven
earlier exact UI selector passes are retained, not fresh individual reruns;
all seven also passed in fresh workspace/isolated UI suites. UI's dev Core-KAS
dependency does **not** establish isolated no-KAS Core proof. The unchanged
199 Core inputs and their separately retained feature-matrix obligations are
not relabelled as a fresh combined App/Core command.

**Gate12 `cargo fmt --all --check` actually FAILed and remains immutable.**
Main then authorized exactly one formatter correction (gate13, exit0), limited
to four test-only reflows in `app/tests/mcp_oauth.rs`, UI `state.rs`,
`subagent_ui.rs` and `widgets/chat.rs`. The
[format applicability receipt](current-publication-evidence/pr193-repair/format-applicability.json)
records exact full Rust tokens **including punctuation**, literals, comments,
operators, assertions/cases/inputs and production-prefix byte identity.
That explicit identity retains the eleven successful gates and unaffected
review; it is not eleven postformat reruns. Actual postformat gate14
`cargo fmt --all --check` and gate15 `cargo build -p cyril --bins` both PASS.
No full postformat tests or variant rebuilds are claimed.

Totals: **15 Cargo commands, 14 exit0 plus the preserved fmt FAIL**;
**4,351 passed / 0 failed / 17 ignored test executions**, including suite
overlap, **not unique cases**. Ignored tests did not execute. The prior
no-default binary is not a fresh postformat rebuild; equal old/new default
binary hashes do not make the old preformat source binding current.

### Retained actual CLI proof

The [history/CLI record](current-publication-evidence/pr193-repair/history-and-cli.json)
retains the actual finite-peer initial-drain RED and GREEN three-exact-row
proof. The probe and formatter are unchanged; the current example source is
SHA-256 `42d991b5008f86c583bf65503310afd0c381c8eb2736511efb242def13f43c12`.
Printable quotes/backslashes/Unicode/combining/ZWJ and literal escape text remain
exact while actual controls/bidi are visibly escaped. This is retained actual
CLI evidence, not a new run, replay, full-example, native, network or CI claim.

### Current owned consumer — one invocation, eleven captures PASS

The [current owned-consumer record](current-publication-evidence/pr193-repair/owned-consumer.json)
owns **one Main-authorized actual invocation, zero retries, consumer exit0,
11 capture points, client5 / peer67 and zero native Kiro sessions**. Its final
outer source receipt has SHA-256
`4a6f0d2b89cb5c20eb9002b526fe607d2606cea1ff5cf03516d6901ac5dd17c2`;
it is distinct from the runtime result and archive-internal core result.
The executable/source binding is the postformat path/d265 descriptor above,
not the historical six-capture binary.

At140×40 outer /139 content columns with actual gutter chrome, the retained
six-stage sequence passed full published fixture URL, durable error, exact
printable/control-safe URL, later content/draft, PageUp recovery and PageDown
return. Original stage6's draft field remains recorded-only; appended stages
independently assert exact draft plus block caret.

The appended five captures actually establish browse-before-current-URL,
the complete escaped current URL/ACK30 and visible main-follow restoration;
browse-before-errors; exact main/optimistic-child presentations once with
escaped actual session attribution and **entire opaque child payload**;
then a late workflow claim with the old child presentation still once and
**entire opaque workflow payload** once. Visible chronology, foreign child-text
isolation, exact draft and independent caret are preserved. Client wire is
initialize1/session-new1/three success delivery ACKs only; peer wire contains
three URL callbacks/four display errors. No prompt/reset/permission/tool
management or connected/authorized claim follows: **delivery ACK is not
authorization**.

Hidden canonical child/workflow storage, adoption, pending/internal authority
and focused-renderer/reveal remain source/API-test proof, **not** visible-main
screen proof or production focused entry. No production focus-entry caller,
new binding/view/backdoor was added. Incoming synthetic workflow lifecycle
events with the client's existing `workflows=false` advertisement do not prove
that natural native workflows originate with that gate off.

The [consumer archive index](current-publication-evidence/pr193-repair/owned-consumer-archive-index.json)
identifies the unchanged [49-member evidence archive](current-publication-evidence/pr193-repair/owned-consumer-evidence.tar.gz),
270,484 bytes, SHA-256
`56aa494a80e05a5abf414447a5ff51847d82cdf0214e7c22472cd05496067231`.
The binary is not embedded. Prior preparation/HOLD and historical receipts
keep their original unexecuted/pre-run state; the separate authorization and
actual result own this one execution.

**Observed cleanup, with explicit visibility limits:** the actual owned root
was absent; zero owned markers were found in **readable** cwd/exe links.
The962-entry global census had475 permission/other errors **per link**;
the494-entry same-UID supplement had **five EACCES +29 ENOENT per link**.
Opaque entries are not claimed inspected, absent or attributed. No observer
signals were sent. This is direct root absence plus a bounded readable-marker
census, not a complete historical/live/PGID tree or universal process absence.

### Current remaining boundaries and parent ownership

- **PR193 CI is still old RED**, not current-head/platform CI PASS. Current
  Tqr b750+repairs has not yet integrated main
  `03838a501dad8be140acd78dc3fff4cae35fafef`; Main owns ancestry reconciliation
  and current CI after this package.
- **Peaq PR194 refresh/review/CI blocker is resolved and landed** at that main
  merge, following refreshed head `991b16f9f902ff225c18ec528df7ee999f914e36`,
  base `7a0716dc7f28035244dd7ba451a4e2293e887a0c` and GREEN CI run39690453316.
  The [Peaq publication record](https://github.com/dwalleck/cyril/pull/194#issuecomment-6329881410)
  belongs to Peaq, not Tqr's current-head CI. Separate **`u62f` Windows
  attribution remains open**, not waived by green Windows CI; older Peaq
  failures retain their historical scope.
- No fresh native KAS/auth/bootstrap/model/provider/browser/clipboard/network,
  native-host, Windows or macOS acceptance is claimed. The earlier native-auth
  stop remains in force; this package authorizes no retry/login/account/config
  or token-store action.
- Final `nk4o`/C4 reset→callback→authoritative-connected status, `caar` and C6
  retained-history/live/gutter integration are not claimed complete. NK local
  checkpoint `ef46c43813d12b63f45b264e427abda84d299f46` exists, but does not
  establish propagation into Tqr or final NK/native/C4 acceptance.
- The original native31/32 FAIL, owned A FAIL, workspace EOF FAIL, partial
  candidate2954 B/C, historical818 and prior a09/12-lane/0.96-review/six-capture
  records retain their original scope. No historical mutation is presented as
  a fresh current sensitivity run.
- Main owns staging/commit/PR/tracker and stack disposition. This Tqr record
  does not claim issue closure, shipping, merge, PR publication or publication GO.

This documentation worker edited the four assigned existing documents and
hashed them only; it ran no checks/builds/formatters, source changes, runtime,
Git/index/ref, tracker or PR/publication action.

<a id="delivery-checkpoint-2026-10-20"></a>
## Historical delivery checkpoint 2026-10-20 — prior a09 package

**Historical scope notice:** all a09/12-lane/0.96-review/six-capture results and
source pins in this checkpoint retain their original dates and scope.
“Current” in retained receipt names or descriptions below means current at
that prior checkpoint, not the PR193 repair. The
[current repair checkpoint](#current-pr193-repair-checkpoint-2026-10-21)
supersedes this section as the current evidence ledger without rewriting it.

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

The preserved prior a09 default-KAS executable is
`/tmp/cyril-tqr8-current-tested-20261020`, SHA-256
`1d7948e61584937f6355973f987280a3d1974a5b2adc86c751919c1122d69df0`.
**Prior a09 local qualification: all 12 requested Cargo lanes PASS.**
The independently sourced then-current source review and one owned B consumer also
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

**Prior a09 assembled qualification — PASS:** the exact copied parent receipt is
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

**Prior a09 source review — no findings, confidence0.96:** ReviewCurrentTqr
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
The prior a09 qualification retains these **five historical** RED/restored-GREEN
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

### Historical partial qualification — superseded by a09 gate

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
**WORKSPACEBLOCKED was that historical candidate's result.** The prior12-lane a09 PASS superseded that candidate's workspace blocker; it is not the current PR193 repair qualification. The original failed receipt is neither waived nor rewritten.

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
candidate2954 navigation proof only. The a09 gates and six-capture owned B proof
below are also prior evidence; the [current PR193 repair checkpoint](#current-pr193-repair-checkpoint-2026-10-21)
owns current scoped status. No blanket all-claims/native/CI PASS follows.


<a id="current-owned-b-consumer--one-invocation-pass"></a>
### Historical a09 owned B consumer — prior six-capture invocation PASS

This prior proof is retained unchanged in scope; the
[current eleven-capture consumer](#current-owned-consumer--one-invocation-eleven-captures-pass)
above owns the current repaired-source result.

Main authorized exactly one then-current a09 owned B scenario:

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

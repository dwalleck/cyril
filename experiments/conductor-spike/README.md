# conductor-spike

Captures from the 2026-05-03 spike that established two things:

1. **`sacp-conductor` is a clean drop-in for `kiro-cli acp`** — passes all Kiro extension methods (`_kiro.dev/*`) through with no field corruption, no `Method not found` rejections, and microseconds of overhead.
2. **kiro-cli-chat 2.1.0 and 2.2.0 produce field-level identical ACP wire output** when hitting the same backend on the same day. ACP wire-format changes between April 2026 and May 2026 (notably `meteringUsage[]` and `turnDurationMs` on `_kiro.dev/metadata`) are AWS backend rollouts, not binary changes.

See memory entries for full context: `reference_sacp_conductor_spike.md`, `reference_kiro_wire_audit_methodology.md`, `reference_kiro_2_2_0_diff.md`.

## Layout

| File | What |
|---|---|
| `conductor-wrapper-2.2.0.sh` | Wrapper that points conductor at the system-installed kiro-cli (whatever version is on `$PATH`). Filename refers to when the wrapper was authored, not the binary it captures. |
| `conductor-wrapper-2.1.0.sh` | Wrapper that points conductor at `~/.local/share/kiro-research/binaries/2.1.0/kiro-cli-chat acp` directly, bypassing the kiro-cli router so the 2.1.0 binary is exercised regardless of `$PATH`. See CLAUDE.md "Research archive" for the archive layout. |
| `diff_fields.py` | Structural field-path differ. Parses two conductor debug logs and prints field additions/removals per JSON-RPC method. |
| `logs/conductor-2.2.0.log` | Conductor's debug log of the 2.2.0 binary capture — every JSON-RPC line in both directions, with `C →`/`C ←`/`0 →`/`0 ←` direction markers. |
| `logs/conductor-2.1.0.log` | Conductor's debug log of the 2.1.0 binary capture. |
| `test_bridge-2.2.0.out` | Harness-side output (cyril's notifications, step results) from the 2.2.0 run. |
| `test_bridge-2.1.0.out` | Same for the 2.1.0 run. |

## Reproducing

```sh
# Install conductor (sandboxed prefix)
cargo install sacp-conductor --root ~/.local/cargo-spike

# Run against current system kiro-cli
cargo run --example test_bridge -- \
    --agent-command bash experiments/conductor-spike/conductor-wrapper-2.2.0.sh

# Run against archived 2.1.0 binary
# (requires ~/.local/share/kiro-research/binaries/2.1.0/kiro-cli-chat — see CLAUDE.md "Research archive")
cargo run --example test_bridge -- \
    --agent-command bash experiments/conductor-spike/conductor-wrapper-2.1.0.sh

# Diff a fresh capture against the in-repo 2.1.0 reference
python3 experiments/conductor-spike/diff_fields.py \
    docs/kiro-acp-capture-2.1.0.json \
    /tmp/conductor-spike/logs/<latest>.log \
    --label-ref 2.1.0 --label-cap 2.2.2
```

(Wrapper scripts write to `/tmp/conductor-spike/logs*/` since `/tmp` is fine for runtime artifacts. The captures saved here are the post-run copies.)

## Probe conventions

Rules for any new `probe-*.py` in this directory. Both exist because they were
violated and the violation was caught in review, not by a tool.

**1. Never persist a credential.** A probe that answers `_kiro/auth/getAccessToken`
holds a live bearer token. If it also logs frames to a capture file, the token lands
in that file — and captures get committed. Redact on the way to the log *only*, so
the agent still receives the real token and the frame shape survives:

```python
SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer"}

def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k])) for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj
```

`<redacted>` is the established marker — `kas-live-session-trace-2.11.0.jsonl` already
uses it. Reference implementation: `probe-kas-orchestrate-capture-2.14.1.py`. Two
probes still violate this (cyril-hhgw).

**2. A failed probe must not report zeros.** `pump()`-style helpers return JSON-RPC
*error* responses as readily as results, so `bool(resp)` is true for a failure,
`resp["result"]["x"]` silently yields `None`, and the run prints "0 frames" — which
reads as evidence about the agent when it is really evidence about your credentials.
Check `error` after every request and abort non-zero. A zero is only meaningful from a
run that is known to have reached the code path being measured.

**Auth for KAS probes — check your login first.**

```sh
kiro-cli user whoami --format json    # -> {"accountType": "SocialGitHub", "email": …}
```

`accountType` tells you which credential layout you have, and it changes everything below.
(`kiro-cli user profile` erroring with *"only available for IAM Identity Center or External
IdP users"* is a second, cheaper discriminator.)

`~/.aws/sso/cache/kiro-auth-token*.json` goes stale on every method, because kiro-cli
refreshes into its SQLite `auth_kv` — that is the usual cause of `-32000 TokenInvalidError`.
Under **GitHub social auth** (measured) the fresh token is in
`~/.local/share/kiro-cli/data.sqlite3`, row key `kirocli:social:token`, as plaintext JSON
already carrying `profile_arn`.

Do not assume that shape on a different login. The key is `kirocli:odic:token` for
IdC/Builder ID and `kirocli:external-idp:token` for external IdP, `profile_arn` is only
guaranteed in the row for social (others resolve it via `list_available_profiles`, so it
must be merged from the JSON cache), and Builder ID has an OS-keychain path distinct from
the DB row. **A probe that worked for one contributor can fail for another purely on auth
method** — that is the likeliest explanation when a probe suddenly cannot authenticate.
Measure your own row and record it. Full detail: `docs/kiro-2.14.1-wire-audit.md`.

## Why this matters for cyril

The conductor passthrough result unblocks the integration plan in `project_cyril_conductor_integration.md`: cyril's bridge can swap `kiro-cli acp` for `sacp-conductor agent "kiro-cli acp"` with zero protocol changes, opening the door to pluggable proxy stages (skill resolver, transcript recorder, auto-approval, etc.) without rewriting cyril as a proxy.

## Per-release audit checklist additions

- **clientInfo recognition set (cyril-0wyn / ADR-0006):** each kiro-cli
  release, re-carve `resolveAgentContext` + `resolveRemoteToolAllowlist` from
  the new KAS bundle and re-run `.cyril-0wyn/probe-b-name-ab.py` (update its
  glob to the new version). This is the approved manual regression fence for
  the claim that unrecognized names (cyril's honest identity) fall back to
  kiro-ide silently and `kiro-cli` is accepted. If the recognition set, the
  fallback target, the warn text, or the allowlist branches move, update
  ADR-0006 and re-triage cyril-jrl1/cyril-ctnv.

## Per-release fence: KAS hooks host models (cyril-jiyn / KAS-7)

Each kiro-cli release, re-run `.cyril-jiyn/probe-hooks-ab-2.13.0.py` (ARM=host
and ARM=v2; update its 2.13.0 glob to the new version). It is the manual
regression fence for the claim that the `enabled` (host-callback) and `v2`
(agent-side standalone loader) hook models **do not compose** — the v2 arm
must drive ZERO `_kiro/hooks/*` host callbacks and execute the on-disk hook
agent-side (marker file created), while the host arm drives `list`+`executeHook`
and leaves the disk hook inert. If `buildSessionHooks` selection changes (both
models active, or a different gate), update the decided default in
docs/ROADMAP.md KAS-7 and re-triage the kas_hooks knob.

Harness caveat (fixed 2026-07-23, cyril-tpfd): until then the probe's
`token()` sent the `api.codewhisperer.profile` row — a JSON object — verbatim
as `profileArn`, so every turn died with KRS 400 REQUEST_BODY_INVALID and the
2026-07-19 A/B recorded `prompt_completed: false` in both arms. The A/B's
LIST/EXEC/marker conclusions are unaffected (those callbacks fire before the
model call). Expect `prompt_completed: true` from fixed runs; a false there is
signal again.

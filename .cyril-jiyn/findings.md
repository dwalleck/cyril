# cyril-jiyn — prove-it-prototype findings

Date: 2026-07-19. Target: kiro-cli 2.13.0 on PATH, `acp --agent-engine kas`
(wrapper spawn; KAS bundle `@kiro/agent` 0.18.2). Live turns authenticated
via the odic token + profileArn (see "environment changed" below).

## Q1 — Do the two hook models compose per session?

- **Oracle (static)**: `oracle-buildSessionHooks.txt` — the carved 2.13.0
  selection logic is winner-take-all:
  `if (v2HooksCache && workspacePaths.length > 0) return <v2 binding>;
   return v1SessionHooks ?? disabled`. With `v2:true`, the v1 ACP (host)
  provider is unreachable for any session that has a workspace.
- **Probe (live A/B)**: `probe-hooks-ab-2.13.0.py`, constant workspace disk
  hook (`.kiro/hooks/probe.json`, UserPromptSubmit command hook touching an
  absolute marker) + one `echo hi` turn; only the `_meta.kiro.hooks` flag
  differs. Expectations pre-registered in the probe header.
- **Result**: both arms MATCH.
  - `{enabled:true}` (host arm): `_kiro/hooks/list` fired
    (trigger `promptSubmit`), host `executeHook` ran (exit 0),
    `_kiro/hooks/sessionStart` called; **marker NOT created** — the disk
    hook file is inert; cyril's registry is the only hook source.
  - `{enabled:true, v2:true}` (v2 arm): **zero** host hook callbacks (no
    list, no execute, no sessionStart); **marker CREATED** — KAS's
    standalone loader executed the disk hook inside the agent process
    (`NodeProcessRunner`, `oracle-v2-gate.txt`).
- **Answer: NO composition.** `v2:true` moves hook ownership wholesale to
  KAS for workspace sessions; the host model and the standalone loader are
  mutually exclusive per session, selected at `buildSessionHooks`.

## Q2 — Wire vocabulary differs between the two models

The host model queries triggers in **camelCase** on the wire
(`promptSubmit`, and per the 2.7.1 end-to-end capture also `preToolUse`,
`postToolUse`); disk hook files use **PascalCase** (`UserPromptSubmit`, per
the hooksBlock/kasHookFileSchema). A cyril hook registry loading user
`.kiro/hooks/*.json` files MUST map PascalCase file triggers to the wire's
camelCase query values.

## Environment changed since the June/July probes

The kiro auth store no longer has `kirocli:social:token` — login is now IdC:
`kirocli:odic:token` (with refresh_token; `kiro-cli whoami` refreshes a
stale one) + profileArn at `state/api.codewhisperer.profile`. Both existing
hook probes (`probe-kas-hooks-host-2.7.1.py`,
`probe-kas-hook-confirm-2.13.0.py`) would fail today on the missing social
key. The getAccessToken responder shape that works live (this probe):
`{accessToken, expiresAt, profileArn}` from the odic token + profile state.

## Honest caveats

1. `prompt_completed=false` in BOTH arms — the model turn did not finish
   inside the 170s window (auth reached the hook-callback stage; the
   promptSubmit lifecycle point is where all arm observables fired, so the
   arms are comparable). Consequence: `preToolUse`/`postToolUse` were NOT
   re-exercised on 2.13.0 in this run; the exit-2 preToolUse block rests on
   the 2.7.1 end-to-end capture (2026-06-16, HOOK_BLOCK mode) plus source
   continuity (the 2.13.0 carve shows the same
   `_kiro/hooks/list(preToolUse)` + executor call sites). The build phase
   fences preToolUse blocking at cyril's executor level regardless.
2. The v2 arm's zero-callbacks finding is for turn/session-driven paths.
   The carve shows `_kiro/hooks/triggerHook` (client→agent) can still cause
   an agent→client `executeHook` callback for runCommand hooks in v2 mode —
   a client-initiated path, out of scope for the enabled-vs-v2 default
   decision but relevant to future UI work.

## What I learned (gate sentence)

The issue's "they may compose — probe to confirm" hypothesis is false:
`v2:true` is a wholesale per-session ownership transfer to KAS (source:
buildSessionHooks; behavior: zero host callbacks while the agent executes
disk hooks itself), so cyril's decision is a genuine either/or between
being the hook executor (org-policy gate) and lighting up users' on-disk
KAS hooks — not a blend.

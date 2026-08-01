# Cyril presents `clientInfo.name = "kiro-cli"` by default on KAS; the honest name becomes the opt-out

Status: accepted (2026-08-01, cyril-df5l) — supersedes [ADR-0006](0006-clientinfo-identity.md)

## Context

ADR-0006 shipped the identity mechanism (`PresentAs`, `[agent] present_as`,
`bridge::client_info`, `protocol::identity`) and defaulted it to cyril's own
name. That default rested on a **judgement call under uncertainty**: the ADR
recorded that KAS's classification "is not exposed by the initialize response
— byte-identical across names", and that the allowlist difference "plausibly
surfaces downstream as available tools". Nobody had measured what the choice
actually costs.

cyril-df5l measured it. A 4-arm live A/B on kiro-cli 2.16.0 / KAS 0.27.8
(`probe-kas-client-persona-2.16.0.py`; captures
`kas-persona-2.16.0-{control,kiro-cli,kiro-ide,kiro-web}.jsonl`, handshake +
`session/new` only, no credits) established:

1. **`kiro-ide` is structurally identical to sending no `clientInfo` at all.**
   The unrecognized-name fall-through is confirmed empirically, not just read
   out of `resolveAgentContext`. Cyril's honest name has always resolved to
   the IDE persona.
2. **The advertised surface is persona-invariant.** `authMethods`, the 7
   modes, `configOptions`, the 7 `extensionMethods`, the 5 advertised
   agent/skill commands, and the session-start push set are byte-identical
   across all four arms. The ADR-0006 worry that persona gates method
   availability is **disproven**; so is the hypothesis that persona explains
   `OrchestrateSubAgent`'s absence or the never-firing `_kiro/userInput` and
   `_kiro/openExternalUrl`.
3. **The system prompt does change**, measurably: context usage on an empty
   session is 0.9% for kiro-ide/control versus 0.8% for kiro-cli and
   kiro-web. The CLI persona is leaner than the IDE persona cyril has been
   receiving — and the IDE-flavored `hooksBlock` that makes up part of that
   difference is an *authoring* guide for a GUI that cyril does not have, and
   confers no authority on hook output anyway (cyril-booz, 0/18).
4. **`kiro-web` additionally runs two tools at session start** before any
   prompt (`get_learnings_for_prompt`, `get_steering_files` — the
   `honorsRepositories()` branch), 21 frames versus 16.

The decisive reframing: there is no neutral option. `clientInfo.name` is a
three-valued enum with a silent fallback, so cyril is *always* wearing one of
Kiro's personas. ADR-0006's default did not decline to pick one — it picked
`kiro-ide` by omission, and picked the one that describes cyril *least*
accurately. Of the three reachable personas, `kiro-cli` is the closest true
statement about what cyril is: a terminal client, not an IDE, not a sandboxed
web session.

## Decision

- **Default: `PresentAs::KiroCli`.** `clientInfo = {name: "kiro-cli", title:
  "Cyril", version: <workspace>}` on KAS. This reverses only the default;
  every other element of ADR-0006 stands.
- **`"cyril"` becomes the opt-out**, not the default. `[agent] present_as =
  "cyril"` still reaches the honest name and its `kiro-ide` fallback, and is
  still advertised at startup. The choice remains the user's; only which arm
  requires an edit has changed.
- **The knob stays KAS-only.** `effective_present_as(V2, _) == Cyril` is
  unchanged: v2 ignores `clientInfo.name` behaviorally, so presenting a Kiro
  name there would be misrepresentation with zero function. The flip is
  scoped to the engine that actually reads the name.
- **`title` still stays `"Cyril"` in every mode.** ADR-0006 called this
  non-negotiable and it remains so — Kiro-side logs and telemetry can always
  identify cyril sessions. This is what keeps the default a *persona
  selection* rather than a disguise.
- **Both KAS arms advise at startup.** A default that silently changes how
  the agent is prompted must stay stated where the user can read it, so the
  `kiro-cli` advisory names the telemetry attribution and the way out — it is
  no longer just an impersonation warning.

## Considered options

- **Keep the honest default, record the measurement** — rejected. It
  preserves the *appearance* of neutrality while actually selecting the
  kiro-ide persona, which is the least accurate of the three and the only one
  carrying prose written for a GUI. "Honest" is the wrong axis when every
  reachable value is one of the vendor's own names and `title` already
  carries the truthful attribution.
- **Make `kiro-ide` explicitly representable** so the fallback is at least
  deliberate — rejected: it is strictly worse than either arm and adds a
  third state with no use case.
- **`kiro-web`** — rejected: measurably provokes two repository/learnings
  tool calls per session that a local TUI has no use for (fact 4).
- **Fix upstream** — still the right long-term answer, still tracked as
  cyril-ctnv (recognize third-party clients, or key persona/allowlist/hooks
  off capabilities). Unchanged by this ADR.

## Consequences

- Kiro telemetry attributes cyril's KAS sessions to `kiro-cli` by default.
  This is a real cost, accepted deliberately: `title` remains `"Cyril"`, so
  attribution is degraded, not falsified, and any Kiro-side consumer that
  looks past `name` can still identify cyril. See
  `reference_kiro_acp_telemetry`.
- Users on KAS get the leaner CLI-persona system prompt and the
  `memoryEnabled` remote-tools branch without configuring anything. The
  `searchMemories` outcome under that branch is **still unverified** —
  ADR-0006 recorded `.cyril-0wyn/probe-c-memory-tools.py` as INCONCLUSIVE
  (both arms died on `TokenExpired`), and this ADR does not discharge it.
  Deferred to cyril-jrl1, now on the *default* path rather than an opt-in
  one, which raises its priority.
- On v2 the configured default is discarded on every startup. That discard
  logs at `debug!`, not `warn!` — under this ADR the discarded value is
  cyril's own default, and warning about it would report the default as user
  misconfiguration for every v2 user.
- The per-release fence is unchanged and now protects a default rather than a
  knob: re-carve `resolveAgentContext` and re-run `.cyril-0wyn/probe-b-name-ab.py`
  each wire audit. If upstream's recognition set ever drops `kiro-cli`, cyril
  silently falls back to `kiro-ide` — the same failure mode ADR-0006
  documented, now on the default path, so the fence is load-bearing.
- ADR-0006 remains the record of the mechanism (the four persona-keyed
  behaviors, the no-override finding, the `title` invariant). Four things
  there are superseded, each marked inline in that document: **Decision
  bullet 1** (the default itself); **Decision bullet 2** on two points —
  which arm is the opt-in, and its claim that the v2 discard carries a
  warning (it logs at `debug!`, per the Consequences above); the **document
  title**, which frames impersonation as an opt-in knob; and the
  **Considered-options** bullet rejecting "impersonate `kiro-cli` by
  default", which this ADR adopts.

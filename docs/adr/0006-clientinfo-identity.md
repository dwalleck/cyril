# Cyril presents its own clientInfo identity; kiro-cli impersonation is an explicit opt-in knob (superseded — `kiro-cli` is now the default)

Status: superseded by [ADR-0008](0008-kiro-cli-persona-default.md) (2026-08-01,
cyril-df5l) — the **default** flipped to `kiro-cli` once the persona A/B
measured what the choice costs. Originally accepted 2026-07-18 (cyril-0wyn).

**Still holds:** the mechanism, the four persona-keyed behaviors, the
no-override finding, the KAS-only scoping, and the `title` invariant.

**Also superseded, beyond the default itself:** this document's title frames
impersonation as "an explicit opt-in knob" — `"cyril"` is now the opt-out arm
instead; and Decision bullet 2's parenthetical says the knob is inert *with a
warning* on v2 — that discard now logs at `debug!`, since the discarded value is
cyril's own default. Both are marked inline below. The original wording stands
as the record of what was decided in July, not as a description of today's
behavior.

## Context

KAS derives its entire client identity from ACP `clientInfo.name` via
`resolveAgentContext` — only `kiro-web` / `kiro-ide` / `kiro-cli` are
recognized; any other name logs `Unrecognized clientInfo.name` server-side
and silently becomes `kiro-ide` (local). The classification is **not exposed
by the initialize response** — byte-identical across names
(`.cyril-0wyn/findings.md` Q3; later session/tool traffic untested, and the
allowlist difference plausibly surfaces downstream as available tools). At least **four behaviors** key off the
resolved client (probe-verified against the shipped 2.13.0 bundle,
`.cyril-0wyn/oracle-*.txt`):

1. **System-prompt persona** (`getIdentity`): "You are Kiro CLI…" vs the IDE
   persona.
2. **Remote-tool allowlist** (`resolveRemoteToolAllowlist`): kiro-ide is
   channel-gated (`stable → [web_search]`); the `memoryEnabled` gate for
   `searchMemories` exists **only** on the kiro-cli branch. The env bypass
   `KIRO_LOAD_ALL_REMOTE_TOOLS=true` forces `*` (debug-grade, tools only).
3. **Hooks briefing** (`hooksBlock`): injected into the system prompt only
   for kiro-ide — see cyril-jiyn (KAS-7) for the coupling with cyril's hooks
   machinery. **cyril _receives_ this briefing** — its unrecognized name
   falls back to kiro-ide, the branch the `hooksBlock` gate selects
   (`client === "kiro-ide" ? hooksBlock : ""`); live-confirmed by the
   `sysprompt` arm of `.cyril-booz/`, where the model quoted its `<hooks>`
   section verbatim. Note the briefing is an _authoring_ guide (how to
   create hooks) and confers **no authority** on injected `HOOK_INSTRUCTION`
   hook output — cyril-booz probed every framing (including KAS's own
   production framing) and none restored compliance (0/18). The refusal
   cyril-tpfd saw is the model's injection defense, structural to the
   user-turn injection point, not a missing briefing. See cyril-booz.
4. **Repository honoring** (`honorsRepositories`): kiro-web/sandbox only.

Probing also established there is **no override**: the fallback inference is
execution-environment-only (`sandbox → kiro-web`, else `kiro-ide`), so an
honest name plus a KAS-side knob selecting the kiro-cli branch is not an
available option.

## Decision

- **Default: honest identity.** `clientInfo = {name: "cyril",
  title: "Cyril", version: <workspace>}` on every engine, single-sourced in
  `bridge::client_info`. Cyril knowingly accepts the kiro-ide fallback on
  KAS and states its standing itself — one `info` advisory at bridge startup
  (`protocol::identity::identity_advisory`), because the wire never will.
- **Opt-in impersonation knob.** `[agent] present_as = "kiro-cli"` presents
  the kiro-cli name to reach the `memoryEnabled` remote-tools branch. The
  knob is KAS-only (inert with a warning on v2, where the name has no
  behavioral effect and impersonation would be pure telemetry
  misrepresentation). `PresentAs` is a two-variant enum: `kiro-ide`,
  `kiro-web`, and free strings are unrepresentable.
  > **Superseded by ADR-0008** on two points: `kiro-cli` is the default and
  > `"cyril"` the opt-in, reversing which arm this bullet describes; and the
  > v2 discard logs at `debug!`, not `warn!`. KAS-only scoping and the
  > two-variant enum are unchanged.
- **Impersonation is never total.** `title` stays `"Cyril"` in every mode —
  Kiro-side logs and telemetry can always identify cyril sessions. This is a
  non-negotiable of the knob's design.

## Considered options

- **Impersonate `kiro-cli` by default** — rejected: `clientInfo` feeds AWS
  telemetry, and cyril's positioning is to *be* a legitimate ACP client, not
  to win a detectability game (the pi-kiro lesson). Also brittle against
  upstream name-keyed changes.
  > **Reversed by ADR-0008.** The rejection assumed a neutral alternative
  > existed. The 4-arm A/B showed there is none — the honest name silently
  > resolves to `kiro-ide`, so this option was never "impersonate vs. don't"
  > but "which of three vendor personas". The telemetry cost is real and is
  > accepted there; `title` stays `"Cyril"`, which is what keeps the
  > detectability-game objection inapplicable.
- **Honest name + env/config override on the KAS side** — does not exist;
  probe-disproven (`.cyril-0wyn/findings.md` fact 1).
- **Fix upstream** — the right long-term answer; tracked as cyril-ctnv
  (recognize third-party clients, or key persona/allowlist/hooks off
  capabilities; at minimum surface the resolved client type in the
  initialize response).

## Consequences

- On KAS under the honest default, cyril runs with the IDE persona,
  channel-gated remote tools, and the IDE hooks briefing — each divergence
  is named in the startup advisory and lives in this ADR rather than in
  anyone's head.
- The `searchMemories` outcome under the knob is **not yet verified**:
  `.cyril-0wyn/probe-c-memory-tools.py` (claim 8) was INCONCLUSIVE — both
  arms failed discovery on `TokenExpired` before any allowlist resolved.
  The live verification under an auth-serviceable session is deferred to
  cyril-jrl1.
- Per release, the wire audit re-carves `resolveAgentContext` and re-runs
  `.cyril-0wyn/probe-b-name-ab.py` (the approved manual regression fence for
  upstream's recognition set — see the checklist in
  `experiments/conductor-spike/README.md`).
- The hooks-briefing mismatch (IDE-flavored prose for a TUI, or none at all
  under the knob) is cyril-jiyn's scope; this ADR only fixes which prose KAS
  picks.

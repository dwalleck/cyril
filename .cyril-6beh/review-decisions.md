# PR #68 — review-feedback decisions

Secondary review, 2026-07-26. 10 findings (5 Standards, 5 Spec). Every finding was
verified against the code/docs before any change was applied.

**Headline: the review was accurate.** All 10 bug claims reproduced. That is unusual
enough to state plainly — the normal expectation is 2–3 of 10 failing verification.
Three fixes are *modified* rather than accepted as written, for reasons below.

| # | Finding | Cat | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | [P1] live bearer token serialized into the capture | Bug | **Yes** — `send()` unconditionally `rec()`s; `rep()` routes `auth_reply()` (returns real `accessToken`) through it | **Accept** | Redact rather than suppress — the committed `kas-live-session-trace-2.11.0.jsonl` already stores `accessToken: "<redacted>"`, so a convention exists and this was a regression against it. Redacting keeps the frame shape, which is the point of a wire capture. Verified no committed capture currently leaks. |
| S2 | [P2] invalid token collapses to `{}` / null fields | Bug | **Yes** — `except Exception: return {}`; `d.get()` yields `None` | **Accept** | Validate once before spawn; exit non-zero with a credential-safe message. |
| S3 | [P2] temp dir, log handle, child process not cleaned up | Bug | **Yes** — `mkdtemp` never removed, `log` never closed, `terminate()` with no `wait`/`kill` | **Modify** | Took `try/finally` + bounded terminate→wait→kill + closed log. **Rejected the `TemporaryDirectory` half**: the workspace holds the files the orchestrated stages create (`alpha.txt`/`beta.txt`) — auto-deleting destroys the capture's own evidence. Path is printed instead. |
| S4 | [P2] cyril-6beh holds incompatible scopes | Design | **Yes** — title/description say "model the future protocol, don't render yet"; `design` prescribes rendering `agent-subtask` now; `notes` say to split | **Accept** | Split: cyril-6beh keeps the deferred protocol scope; near-term rendering filed separately. |
| S5 | [P2] rerun instructions incomplete + `auth_kv` contradiction | Bug | **Yes** — `Usage:` omits `[fresh-token.json]`; doc says `auth_kv` "not plaintext" while the probe comment gives a recipe reading it | **Modify** | Reviewer asked to "document one verified extraction path". Measured it — but see the correction below: the measurement was auth-method-specific and my first write-up overgeneralized it. |
| P1 | [P1] JSON-RPC errors reported as capture progress | Bug | **Yes** — `bool(init)` true for an error response; `sid=None` still prompts; prompt error → `stopReason: None` | **Modify** | Probe fixed as asked. **Did not "regenerate the summary"** — that needs a live authenticated run, which is the very thing that is blocked. Hand-editing captured output to insert an error that was never in it would fabricate evidence. Instead the dead workstation-local path is removed and a clearly-marked annotation records provenance. |
| P2 | [P1] workflow-progress detector misses the documented shape | Bug | **Yes** — audit ln 126–127 documents `_meta.kiro.notification.kind` (nested) and `messageId`/`notifyId` prefix `wf-progress-`; probe checks `_meta.kiro.kind` and `update.kind` | **Accept** | Both documented paths implemented. Note the reported `0` was uninformative regardless — the run produced zero tool frames of any kind — but the detector would have under-reported on a *successful* rerun, which is when it matters. |
| P3 | [P2] audit still calls parse-and-drop scaffolding a "renderer" | Bug | **Yes** — ln 107 says "full client-side parser + renderer"; the added correction says no consumer/renderer exists; tracker description repeats the wording | **Accept** | Corrected in the audit and in the tracker description. |
| P4 | [P2] 2.7.1 audit still says filtered calls render opaquely | Bug | **Yes** — `docs/kiro-2.7.1-wire-audit.md:262` says "They already render as opaque tool calls today; nested-crew UI is the only gap" | **Accept** | Corrected to state they are filtered by the `ToolKind::Other` rule and need the `_meta.kiro.kind` exception first. |
| P5 | [P2] capture terminates without draining trailing frames | Bug | **Yes** — `pump()` returns on the prompt response, then terminates immediately | **Accept** | Close stdin, drain to EOF or a bounded quiet period, then summarize and reap. |

## Deferred work — tracker IDs

The skill requires every deferral to name a tracker ID. Two of the decisions above
defer work; both are now filed.

| From | Deferred work | Tracker |
|---|---|---|
| P1 (Modify) | Regenerate the attempt summary from a real run — needs the live authenticated capture that is itself the blocked task | **cyril-ucii** |
| S1 (scope) | The same credential defect in two *other* probes, found by sweeping the directory after fixing this one | **cyril-hhgw** |

## Follow-on findings from the sweep (not in the review)

Checking whether the probe fixes implied changes elsewhere turned up three things
the review could not have seen from one file:

1. **The credential defect is not unique to this probe.** 49 probes in
   `experiments/conductor-spike` answer `getAccessToken`; two of them —
   `probe-kas-compact-summarization-2.9.0.py` and `probe-kas-orchestrate-wire-2.9.0.py`
   — persist the reply verbatim via the identical `rep→send→rec→file` chain. The other
   45 were checked and do not write the auth reply to a file. No committed capture
   currently leaks (swept; the only committed `accessToken` is the literal
   `<redacted>`). Filed **cyril-hhgw**.
2. **There was no written convention to violate.** `experiments/conductor-spike/README.md`
   documented layout and reproduction but said nothing about credentials or about
   failed probes reporting zeros. Both rules added there, with the `redact()`
   reference implementation, so the next probe author inherits them.
3. **The `auth_kv` measurement is load-bearing for an unrelated open issue.**
   `cyril-taba` (p2, auto-refresh the token before `getAccessToken` in wrapper mode)
   lists refresh candidates as a `kiro-cli whoami/profile` shell-out or KAS's own
   file-auth path, and its own notes call the shell-out "inherently fragile … relies on
   an undocumented side effect". The token is in fact readable directly from
   `auth_kv` as plaintext JSON with `profile_arn` included. Recorded on that issue as
   a third candidate — trading one fragility (undocumented side effect) for another
   (another program's DB schema and locking), so it needs the same falsifier, but it
   was absent from the candidate list entirely.

## Correction to S5 — the "verified path" was n=1

My first fix for S5 asserted flatly that `auth_kv` is plaintext, that the row is
`kirocli:social:token`, that it carries `profile_arn`, and therefore that **both** the
audit doc and the probe comment were wrong. The measurement was real, but it was taken
under **GitHub social auth** — and the repo owner had run those probes under IAM
Identity Center auth. Different login, different store shape.

Binary evidence that the paths genuinely diverge:

| string | implication |
|---|---|
| `social token has no profile ARN, treating as invalid` | social tokens **must** carry `profile_arn` in the row — which is why it was there to read |
| `Lazily resolved profileArn from list_available_profiles` | other methods resolve `profileArn` via an API call; it is **not** in the row |
| `Error getting builder id token from keychain` | Builder ID has an OS-keychain path distinct from the DB row |

So **the probe's original comment was most likely correct for the machine it was written
on**: `kirocli:odic:token` is the right key for an IdC login, and merging `profileArn`
from `kiro-auth-token-cli.json` is the right move when the row does not carry it. Calling
it "wrong" was my error, not the author's. The `auth_kv`-not-plaintext note may likewise
have been accurate for a Builder-ID setup where the token lives in the keychain.

Corrected in the audit doc, the probe docstring/`TOKEN_RECIPE`, the README, and on
`cyril-taba` — the last one materially, because that issue is a *product* feature: an
auto-refresh that assumes one store shape breaks for every user on another login, so its
falsifier needs an arm per auth method rather than one.

**Generalized lesson:** a measurement taken on one machine describes that machine. The
probe's comment and the audit doc did not contradict each other because one was wrong —
they described two different auth methods, and I collapsed them into a single claim
because I only had one environment to look at.

## Observed but deliberately not changed

- `probe-…-2.14.1.py` orchestrate-detection has a redundant clause:
  `"stages" in ri or "orchestrate" in name or "task" in ri and "stages" in ri` — the third
  disjunct is subsumed by the first (and its precedence reads misleadingly). Not a review
  finding, and altering detection semantics unasked would change what a rerun captures.
  Left as-is; flagged here so it is not lost.
- `[[wikilink]]` syntax appears in `docs/*.md` (4 pre-existing occurrences on main). It is
  an established habit in this repo's audit docs, so the one added by this branch was left
  alone. It was removed from `CLAUDE.md` only, where no such precedent exists.

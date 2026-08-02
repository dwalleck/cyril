# cyril-h8zb design — render v2 model-refusal alerts

Probe evidence: `.cyril-h8zb/findings.md`. The wire contract (three-artifact agreement,
2.15.0+2.16.0): `refusal {category, explanation, recommendedModel}` (camelCase, all optional)
rides `_kiro.dev/metadata`; metadata `stopReason` literal `"CONTENT_FILTERED"`; Kiro's own
consumer alerts on `refusal || stopReason === "CONTENT_FILTERED"` (an OR), dedupes to the first
event, uses explanation-or-fallback text, and the refusal-bearing frame lands immediately before
the prompt response.

## Architecture

One new domain type, one field threaded through the existing notification, two state machines
extended, zero new notification variants, zero bridge/routing changes:

1. **`RefusalAlert`** (`cyril-core/src/types/session.rs`) — private fields + getters (TurnSummary
   idiom): `category: Option<String>`, `explanation: Option<String>`,
   `recommended_model: Option<String>`. All three optional; empty string normalizes to `None` at
   the parse boundary ("" is the wire's "not provided" — sentinel doctrine); a present-but-wrong-
   type subfield warns and maps to `None` (corrupt ≠ missing).
2. **Parse** (`convert/kiro.rs` metadata arm) — construct `Some(RefusalAlert)` when the frame has
   a `refusal` object OR `stopReason == "CONTENT_FILTERED"` (Kiro's own OR-condition); `None`
   otherwise. A non-object `refusal` value warns and falls back to the stopReason check alone.
   `refusal`/`stopReason` leave the ignore-list; the unknown-key debug log behavior is unchanged.
   **Review amendment:** `stopReason == "refusal"` is tolerated as a second alert-worthy literal,
   restoring the issue's 2.12.3 addendum instruction ("tolerate stopReason 'refusal'") that the
   original claim 3 silently narrowed — the literal is a first-class zod stopReason on the
   KAS/`_kiro` side and unambiguous if it ever reaches a v2 metadata frame (fenced in
   `to_ext_notification_metadata_content_filtered_no_object`).
3. **`MetadataUpdated`** gains `refusal: Option<RefusalAlert>` — rides the existing routing
   (cyril-fh06 sessionId scoping untouched, so subagent refusal frames never reach main state).
4. **`SessionController` AND `UiState`** — both buffer `pending_refusal: bool` from
   `MetadataUpdated` (same take-at-turn pattern as `pending_tokens`); at `TurnCompleted`, when
   the buffered flag is set AND the ACP stop reason is `EndTurn`, the `TurnSummary` records
   `Refusal` instead — the toolbar then shows the existing red "Refused" chip.
   `Cancelled`/`MaxTokens`/`MaxTurnRequests` are never overridden (they are real, distinct
   outcomes); `Refusal` is idempotent. **Build-time correction (slice 5):** the toolbar reads
   *UiState's* `last_turn`, which UiState assembles independently in its own `TurnCompleted`
   arm — so the reconcile lives in both state machines, keeping the two summaries in agreement.
   The original text named only SessionController; the approved outcome (red chip) dictated the
   UiState half. Safety premise probed: the only production reader of
   `TurnSummary::stop_reason` is `toolbar.rs:181` (display) — no control flow keys off it.
5. **`UiState`** — on `MetadataUpdated` with `Some(refusal)`, commit ONE system chat message at
   arrival (not at turn end — see cyril-9akh ordering race) guarded by a
   `refusal_alerted_this_turn` flag; the flag resets on `TurnCompleted` and on `SessionCreated`.
   Message text:
   - base: the explanation verbatim when present, else the fallback
     `"The model couldn't process this request. Try a different model with /model or start a new session with /new."`
   - when `recommended_model` is present, append `" Recommended model: <m> (switch with /model)."`
   - `category` is carried in the type (Debug/log surface) but never rendered — Kiro's own TUI
     does not display it either.
6. **Stale doc comment** — delete the `types/session.rs:573-575` NOTE claiming the bridge
   hardcodes `EndTurn` (`bridge.rs:1138` has extracted the real value since the parity work).
7. **Fence update** — the cyril-1gim `to_ext_notification_metadata_refusal_and_stop_reason_not_flagged`
   fence's meaning changes from "recognized-but-ignored" to "recognized-and-parsed, still not
   logged as unknown"; it is updated in place, not deleted.

## Input shapes (step 2)

`refusal` key on the metadata frame × `stopReason`:

| # | `refusal` | `stopReason` | outcome |
|---|-----------|--------------|---------|
| a | absent | absent | `None` — byte-identical behavior to today (AC2) |
| b | absent | other string (`"end_turn"` etc.) | `None` |
| c | absent | `"CONTENT_FILTERED"` | `Some` with all subfields `None` → fallback text |
| d | object, all 3 subfields | any | `Some`, all preserved |
| e | object, partial (each of the 7 non-full presence cells) | any | `Some`, present kept, absent `None` |
| f | object, empty `{}` | absent | `Some`, all `None` → fallback text (Kiro alerts on bare `r` too) |
| g | object with empty-string subfield | any | that subfield → `None` |
| h | object with wrong-typed subfield (`explanation: 42`) | any | warn, that subfield → `None` |
| i | non-object (`"x"`, `5`, `null`) | absent | warn, falls back to stopReason check → `None` here |
| j | non-object | `"CONTENT_FILTERED"` | warn + `Some`(all `None`) via stopReason branch |
| k | corrupt `stopReason` (non-string) | — | warn, treated as absent |

Cross-cutting: repeated `Some` frames within one turn (dedupe); `Some` frames in consecutive
turns (one alert each); refusal frame carrying `sessionId` (tag preserved, routing unaffected);
refusal frame also carrying context %/metering/tokens/effort (all still parsed — AC1).

Out of scope: `refusal: null` distinct from absent — JSON `null` is treated as absent (not
corrupt); Kiro's own destructuring (`r?.category`) makes null and undefined indistinguishable.

## Removed-invariant sweep (step 2b)

The change is additive except for one subtractive element: **the invariant
"`TurnSummary.stop_reason` equals the ACP prompt-response value verbatim" stops holding** (it can
now read `Refusal` when ACP said `EndTurn`). Sweep result: production readers of the field =
toolbar display only (falsifier run, passed — see table row 9); tests that assert the verbatim
value use no-refusal turns and stay valid. No other constraint is relaxed: no locks, ordering,
or uniqueness properties change; the notification channel and routing are untouched.

## Claims

1. A metadata frame with a full refusal object parses to `Some(RefusalAlert)` with all three
   subfields preserved verbatim (shape d).
2. A frame without `refusal` and without `CONTENT_FILTERED` parses to `refusal: None` and every
   pre-existing `MetadataUpdated` field identically to today (shapes a, b).
3. A frame with `stopReason:"CONTENT_FILTERED"` and no refusal object parses to `Some` with all
   subfields `None` (shape c).
4. Partial/empty refusal objects preserve exactly the present, non-empty, correctly-typed
   subfields; the rest are `None`; no panic (shapes e, f, g).
5. Corrupt shapes (h, i, j, k) warn and degrade per the table — never error, never fabricate
   values, and corrupt-`refusal`-plus-`CONTENT_FILTERED` (j) still alerts.
6. A refusal-bearing frame that also carries contextUsagePercentage, meteringUsage,
   turnDurationMs, effort, and sessionId parses ALL of them identically to a refusal-free frame
   (AC1 "does not disturb"), and neither `refusal` nor `stopReason` trips the unknown-key log.
7. UiState commits exactly ONE system message for the first `Some(refusal)` frame of a turn;
   a second refusal frame in the same turn adds nothing; a refusal in the next turn (after
   `TurnCompleted`) alerts again.
8. The system message text is: explanation verbatim when present; the exact fallback string when
   absent; with the recommended-model sentence appended iff `recommended_model` is present.
9. A turn whose metadata carried `Some(refusal)` and whose ACP stop reason is `EndTurn` yields
   `TurnSummary.stop_reason == Refusal`; `Cancelled` is never overridden; a no-refusal turn's
   summary is unchanged; the buffered flag does not leak into the following turn.
10. The stale "bridge currently hardcodes EndTurn" NOTE is gone from `types/session.rs`.
11. A refusal-bearing frame with a subagent `sessionId` still carries that tag on the
    notification (routing input preserved).
12. Rendering: after a refusal turn, the chat viewport shows the system message text and the
    toolbar shows "Refused" (render-layer test per the testing-layers convention).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 0 | wire contract (keys/condition/ordering) | probe vs oracle disagreement | tui.js consumer carve vs Rust-binary strings vs embedded docs (3 artifacts) | ran | **passed** | fixtures in fences below encode the carved shapes; live-capture watch = cyril-pz51 |
| 1 | full object parses verbatim | feed carved-shape JSON, assert 3 subfields | hand-written JSON fixture from the tui.js contract (not from cyril code) | 5m | pending | `to_ext_notification_metadata_refusal_full` |
| 2 | absent ⇒ None + today's fields | feed today's fixtures, diff notification | existing 1gim fixtures (pre-date this feature) | 5m | pending | `to_ext_notification_metadata_refusal_absent_unchanged` + existing metadata fences staying green |
| 3 | bare CONTENT_FILTERED alerts | feed shape c, assert Some(all None) | Kiro's OR-condition (carved, site 0) | 5m | pending | `to_ext_notification_metadata_content_filtered_no_object` |
| 4 | partial preserved, no defaults | feed shapes e/f/g matrix | fixture matrix | 10m | pending | `to_ext_notification_metadata_refusal_partial_*` |
| 5 | corrupt warns + degrades | feed h/i/j/k, capture tracing | CaptureWriter log capture (existing 1gim idiom) | 10m | pending | `to_ext_notification_metadata_refusal_corrupt_*` |
| 6 | does not disturb (AC1) | full-frame fixture, assert every field | field values from fixture literals | 5m | pending | `to_ext_notification_metadata_refusal_preserves_existing_fields` (updates the 1gim `_not_flagged` fence) |
| 7 | one alert per turn | apply refusal, refusal, TurnCompleted, refusal; count system messages | message count in committed list | 10m | pending | `ui state: refusal_alert_dedupes_within_turn_resets_on_turn_end` |
| 8 | exact message text | apply each variant, assert full string | spec strings in this doc | 10m | pending | `ui state: refusal_alert_message_wording_*` |
| 9 | TurnSummary reconcile, no leak | EndTurn+refusal ⇒ Refusal; Cancelled+refusal ⇒ Cancelled; next turn clean | SessionController field asserts | 10m | **premise passed** (sole reader = toolbar; grep ran 2026-08-01) | `session: refusal_metadata_reconciles_end_turn_only` |
| 10 | stale comment gone | grep types/session.rs for "hardcodes" | grep | 1m | pending | one-shot (comment deletion; no fence — regression would be a human re-adding a false comment) |
| 11 | sessionId tag preserved | shape d + sessionId fixture | fixture literal | 5m | pending | `to_ext_notification_metadata_refusal_keeps_session_scope` |
| 12 | render layer | TestBackend render after refusal turn | buffer text extraction | 15m | pending | `refusal_alert_renders_in_chat_and_toolbar` |

Non-vacuity (buggy impl each fence kills): 1—parser reads snake_case keys; 2—alert constructed
on every frame; 3—condition is AND not OR; 4—`unwrap_or_default()` filling empty strings;
5—corrupt refusal aborts the whole frame parse (metadata lost); 6—refusal parsing consumes or
reorders sibling fields; 7—no dedupe flag (spam) or flag never resets (one alert per session);
8—fallback text always used / recommendation dropped; 9—override applied to Cancelled, or flag
leaks to next turn; 11—session tag dropped when refusal present; 12—message committed but never
rendered (e.g. wrong message kind).

Claims-to-shapes: a,b→2; c→3; d→1; e,f,g→4; h,i,j,k→5; cross-cutting→6,7,9,11,12.

## Negative space (what this deliberately does not do)

1. **No KAS-side refusal handling** — the KAS adapter path is a different wire surface; tracked
   at **cyril-ker1** (filed from these probes).
2. **No live-capture validation in this PR** — a backend refusal cannot be forced on demand; keys
   are provisional-but-triply-cross-confirmed, and the tolerant parse degrades to fallback text
   on mismatch. Live verification tracked at **cyril-pz51**.
3. **No new toolbar widget** — the existing "Refused" chip is reused via TurnSummary; no second
   status surface.
4. **`category` is never rendered** — carried in the type for logs/future use; Kiro's own TUI
   doesn't display it either (settled rationale, not deferred work).
5. **No `/rewind` mention in the fallback text** — Kiro's fallback offers `/rewind`; cyril has no
   rewind command, so the text offers `/model` and `/new` only (settled scope: adapting wording
   to cyril's actual command surface, not deferred work).

## Open decisions flagged for the design pause

1. **TurnSummary reconcile (claim 9)** — recommended: override `EndTurn` only. Alternative: leave
   TurnSummary alone (chat message only); the toolbar then shows "Done" for a CONTENT_FILTERED
   turn unless ACP itself said refusal.
2. **Message wording (claim 8)** — exact strings above; bikeshed now, not post-merge (they become
   asserted test contracts).
3. **Dedupe reset scope (claim 7)** — recommended: per-turn (reset at TurnCompleted). Kiro's own
   guard scope is not recoverable from the carve; per-turn is the conservative reading that
   still prevents spam.

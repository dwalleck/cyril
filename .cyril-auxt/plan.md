# plan.md — cyril-auxt (+ cyril-gn07)

Inputs: `route.md` (Structural), `design.md` (DG-approved 2026-10-18). One delivery increment: branch `fix/cyril-auxt-consent-echo` → one PR against `main`.

## Slice 1: domain rules + the consent echo on the wire

**Outcome:** a typed Always-allow grant becomes the TUI's `_meta.kiro.consent` echo on a KAS permission response; row derivation, folder validation, and the main/child resolution mirror the TUI (C1–C5).
**Changes:** new `crates/cyril-core/src/always_allow.rs` (+ `lib.rs` declaration); `review/consent.rs` (`PermissionConsent` gains verbatim capability, triggering resource, explicit-ask; builder setters, existing constructor unchanged); `protocol/convert/kas.rs` (`permission_consent` fills them; new `attach_consent_echo`); `protocol/convert/mod.rs` (dispatch for the new `PermissionResponse::AlwaysAllow` variant); `types/event.rs` (variant). Callers matching `PermissionResponse` exhaustively are migrated in the same slice.
**Verification:**
- red-first: convert test for the shell/user echo fails on the pre-fix tree (no variant / no echo) → receipt `receipts/slice1-red.txt`
- `cargo nextest run -p cyril-core --features kas` → new convert + `always_allow` rstest tables pass; existing permission/rejection tests unchanged
- `cargo clippy -p cyril-core --no-default-features --all-targets -D warnings` → clean (non-kas build compiles the variant without the echo)

## Slice 2: approval overlay scope page + folder entry; V2 path off on KAS

**Outcome:** the operator gets the TUI's scope page and folder editor on KAS, and cyril answers with the grant; KAS requests carry no V2 trust tiers (C6–C8).
**Changes:** `cyril-ui` `traits.rs` (`ApprovalState.consent`, two `ApprovalPhase` variants), `state.rs` (show/transition/confirm/cancel/cycle/folder editor), `feedback_editor.rs` (prefilled constructor), `widgets/approval.rs` (two new render fns + dispatch arms only); every `ApprovalState` literal gains `consent: None`; `cyril-core` `domain_mediator/mod.rs` (KAS → no `trust_options`); `cyril` `app.rs` (`s`, folder keys, paste routing).
**Verification:**
- red-first: `UiState` test "allow_always on a KAS-consent request opens the scope page" fails pre-change (it answers immediately) → `receipts/slice2-red.txt`
- `cargo nextest run -p cyril-ui -p cyril` and the KAS lanes → new state/render/mediator tests pass, existing approval tests unchanged
- Dependencies: slice 1.

## Slice 3: live acceptance + full gate

**Outcome:** C9 and C10.
**Changes:** none to production code; receipts under `.cyril-auxt/receipts/`.
**Verification:**
- before (main `10e38e3c`) and after (branch head) hex1b runs with private `HOME`, `SHELL=/usr/bin/fish`, `--agent-engine kas` → as C9 states
- every ci.yml lane (`receipts/gate.txt`) → all `<lane>-OK`
- Dependencies: slices 1–2; a valid kiro token (no token-touching command may be run; a "token expired" failure pauses this slice and is reported).

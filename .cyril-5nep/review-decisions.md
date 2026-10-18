# cyril-5nep — review decisions (PR #168, round 1)

Source review: two-axis `/code-review xhigh PR 168` (2026-10-17) — Standards
sub-agent (S) and Spec sub-agent (P), aggregated in-session. No active Gilfoyle
route/plan exists for this branch, so repairs follow the repository's
reproduce → fix → focused-verification process.

Checked state: base `f3d19b24` + the round-1 repair commit. Logs:
`logs/red-probe-f3d19b24.txt` (pre-fix red), `logs/mutations.txt` (fence
sensitivity, restored byte-identical), `logs/gates.txt` (fmt, clippy, tests).

Blocking rows first: F1, F2.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Malformed present `toolTags` returns InvalidParams; KAS's pre/post-tool caller catches the failure and runs no hooks for the tool, so blocking policy hooks fail open. Proposed: log and treat as absent. | P | Verified | Carve 2.31.1: `catch(d){return S.warn("[AcpPreToolUseHooks] Extract hooks ACP call failed",…),{state:o}}` (same for PostToolUse); `red_probe_malformed_tags_keep_id_hooks` panicked on `unwrap()` of the Err at `f3d19b24` | Modify | R1: `HooksListArgs::parse` is infallible; non-array → warn and no tags; array → warn per non-string member and **keep string members** (a tag can only add hooks, so keeping them is the enforcement-preserving choice). Fence `respond_list_degrades_malformed_tool_tags` + host boundary cases | blocking — fail-open on a policy path |
| F2 | On non-tool triggers KAS's `Kse` matches any valid-regex matcher unconditionally (`case"none":return!0`); cyril required a tool ID so those hooks never fired. | P | Verified | Carve 2.31.1 `w0t`/`Kse`; `red_probe_non_tool_trigger_regex_matcher_serves` failed `left: 0, right: 1` at `f3d19b24` | Accept | R1: `HookMatcher::is_match` returns "not Invalid" for non-tool triggers; `session_start_hooks` shares it (exclusion loop removed). Fence `respond_list_non_tool_trigger_matchers_follow_kas` (also asserts sessionStart membership); mutation M1 red | blocking — spec "mirror KAS 0.66.26's matcher exactly"; widens behavior: matcher hooks on SessionStart/UserPromptSubmit/Stop now fire every invocation (local `.kiro/hooks` and `~/.kiro/hooks` scanned: none affected) |
| F3 | Rust `regex` vs JS `RegExp`: lookaround/backreferences work in KAS but leave the hook never matching in cyril. | P | Verified | `regex` crate rejects lookaround/backrefs by design; KAS `c$r` uses `new RegExp(e)` | Modify | R1: invalid-matcher load warning names the Rust syntax limit; README states it. Engine swap not done | non-blocking — permanent non-goal for engine parity: Host mode documents Rust regex, the `regex` crate guarantees linear-time matching on user-authored patterns, and `kas_hooks = "kas"` keeps KAS's own JS engine for users who need it |
| F4 | Error-wording tests use `.contains("toolTags")`, not exact `assert_eq!` (CLAUDE.md Test Organization). | S | Verified | `hooks.rs:1383`, `host.rs:432` at `f3d19b24` | Modify | R1: superseded by F1 — the error no longer exists; tests now assert exact served-ID lists for every malformed shape | non-blocking |
| F5 | Duplicated test setup: reply→ID extraction ×3 beside `names`; 10-shape malformed list copied in hooks.rs and host.rs. | S | Verified | `hooks.rs:1286,1346`, `host.rs:398`; lists at `hooks.rs:1360-1371` / `host.rs:406-417` | Modify | R1: `hook_ids` + `listed_ids` helpers (and `names` reuses `hook_ids`); host test keeps one ID extraction and two representative malformed cases, the full shape matrix lives once in hooks.rs | non-blocking |
| F6 | `tool_tags.unwrap_or_default()`; `list`/`matching` take `Option<&[String]>` only to collapse it. | S | Verified | `hooks.rs:722` at `f3d19b24` | Accept | R1: `HooksListArgs.tool_tags: Vec<String>` (absent ≡ empty, as KAS's `Eee` skips the tag loop when `tags` is undefined); `list`/`matching` take `&[String]` | non-blocking |
| F7 | `#![allow(clippy::unwrap_used, clippy::expect_used)]` — `expect_used` redundant given `clippy.toml` `allow-expect-in-tests`. | S | Verified | `clippy.toml`; `host.rs:300` | Accept | R1: now `#![allow(clippy::unwrap_used)]` (the repo's established test-module form) | non-blocking |
| F8 | Feature Envy: `parse_hook_tool_tags` lives in callbacks.rs but serves hooks.rs, adding a hooks→callbacks dependency. | S | Verified | `hooks.rs:674` called `super::callbacks::parse_hook_tool_tags` | Modify | R1: function removed; parsing lives in `hooks::HooksListArgs::parse`, beside `respond_list` | non-blocking |
| F9 | Data Clump: trigger/tool_id/tool_tags read and re-inserted by hand in host.rs, callbacks.rs and `respond_list`. | S | Verified | `host.rs:122-136`, `callbacks.rs:453-464` | Accept | R1: typed `HooksListArgs` parsed once in host.rs → `HostCallback::HooksList { args, reply }` → `respond_list(&HooksListArgs)`; no Map rebuild | non-blocking |
| F10 | `HookMatcher` "invalid" implied by two `None`s; `ToolPattern::Literal` carries no data. | S | Verified | `hooks.rs:429-440` | Accept | R1: `enum HookMatcher { Invalid, Selector, Regex, SelectorOrRegex }`, `enum ToolSelector { Literal(String), Glob(Regex) }` | non-blocking |
| F11 | Glob builder escapes only `.`; correct only because of the guard. | S | Verified | `hooks.rs:448-458`; KAS `k5t` escapes every meta char | Accept | R1: `regex::escape` per non-wildcard char; mutation M3 red on matrix | non-blocking |
| F12 | Comment cites unstable minified KAS symbols (`ARr`/`koe`). | S | Verified | `hooks.rs:426` | Accept | R1: comment points at the stable `toolPattern` field instead | non-blocking |
| F13 | Stale "(skip half)" comment — invalid matchers are retained now. | S | Verified | `hooks.rs:1050` | Accept | R1: "(isolation half)", expectation names the retained never-matching hook | non-blocking |
| F14 | README double blank line. | S | Verified | `README.md:198-199` | Accept | R1: removed | non-blocking |

## Review errors

- P(c1) cited README "Other triggers keep their existing behavior" as documenting
  the non-tool matcher rule; that sentence is about execution (exit codes), not
  matching. It is kept, reworded "existing execution behavior".
- P(a) "the live check is only claimed": not re-run this round. Tag selection
  behavior is unchanged by R1 (unit matrix + host boundary test); the
  production path was refactored, so the 2.29.1 live receipt in the tracker note
  is retained for the tag path only. A live check of F1/F2 is not performed.

## Compact repair record — R1 (F1–F14)

- **Delivered behavior:** `hooks.rs` — `is_tool_trigger`, `REGEX_ONLY_CHARS`,
  `HookMatcher` enum, `ToolSelector`, `HooksListArgs`/`parse_tool_tags`,
  `respond_list(&HooksListArgs)`, `list`/`matching(&[String])`,
  `session_start_hooks` simplified; `callbacks.rs` — `HooksList { args }`,
  `parse_hook_tool_tags` removed; `host.rs` — parse via `HooksListArgs`;
  README matching section. One atomic change: the files share the
  `HooksListArgs` interface, so no intermediate state is independently green.
- **Performed checks and results:** pre-fix red for F1/F2 at `f3d19b24`
  (`logs/red-probe-f3d19b24.txt`); mutations M1 (non-tool rule), M2b
  (string-member retention), M3 (glob escaping) each red on their fence and
  restored byte-identical (`logs/mutations.txt`); `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets [--features kas] -- -D warnings`,
  `cargo test -p cyril-core --features kas` (1143 passed, 5 ignored) and
  `--no-default-features` (826 passed, 4 ignored) — `logs/gates.txt`.
- **Repair review:** independent general-purpose reviewer over the uncommitted
  repair diff at `f3d19b24`+R1, checked against the KAS 2.31.1 carve
  (`c$r`, `w0t`, `Kse`, `Eee`, `k5t`, the pre/post-tool list callers) and
  CLAUDE.md. Result: **no blocking defects**. It reran the hook tests
  (50 passed) and confirmed the receipts. Non-blocking notes and disposition:
  - `HooksListArgs` doc understated the non-array case: fixed.
  - Unused `Default`/`PartialEq`/`Eq` derives: removed.
  - No Unicode or metacharacter glob coverage: four matrix cases added
    (`é?lair*`, `a-b #c*`).
  - Stale tracker note ("malformed-tag refusal"): superseded by a new
    cyril-5nep note; the PR body still needs the same correction.
  - Regex dialect drift in the reverse direction (Rust-only `(?i)`/`\A`, Unicode
    `\w`/`\b`) and glob `?` code point vs UTF-16 unit: edge cases inside F3's
    permanent non-goal.
  These follow-ups touch docs and tests only. The production change is
  derive removal; clippy, fmt and the hook tests reran green (50 passed).
- **Unresolved failures:** none known.
  - Native Windows/macOS CI: PENDING (runs on push).
  - Live re-check of F1/F2 against real KAS: PENDING (not run this round).

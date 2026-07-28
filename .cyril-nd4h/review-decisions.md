# cyril-nd4h — review-feedback decisions (PR #70, round 2)

11 findings. Each verified before applying; two were confirmed by running the
alleged failure, not by reading. **10 accept/modify, 1 reject (deferred with a
tracker ID).**

| # | Finding | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|
| S1 | "Full gate" omits `cargo test --workspace` / doctests | Bug (doc accuracy) | Yes — `audit.md` recorded only nextest/clippy/fmt; nextest does not run doctests | **Accept** | Gate record now lists the doctest invocation and states the workspace has **zero** doctests, so the number isn't mistaken for coverage |
| 1 | `for_tests` no-bridge assertions can hang | Bug | Yes — bare `recv()` on a live sender blocks forever; a retained sender is the exact defect the test targets, so it would hang not fail | **Accept** | Wrapped both in `tokio::time::timeout(5s)`. A fence that hangs reads as "still running", never "broken" |
| 2 | C3 fence not bound to the mouse-enable branch | Bug | Yes — the test only required `app.mouse_captured()` to appear *somewhere*; an unused call beside an unconditional `EnableMouseCapture` passed | **Accept** | Now asserts exactly one `EnableMouseCapture` and that it occurs after the `if app.mouse_captured()` guard |
| 3 | Ctrl+M path untested end-to-end | Bug | Yes — the fence drives `UiState::toggle_mouse_capture` directly, never `App::handle_key`, and never observes the emitted command | **Reject (defer)** — **cyril-ttfb** | Bug claim real; proposed fix (injectable terminal-command boundary) is an architectural change to App's terminal I/O, disproportionate to a config-surface ticket. Filed with full context, `discovered-from cyril-nd4h` |
| 4 | Bounded-cache doc false at small capacities | Bug (doc accuracy) | Yes — **measured**: `HashCache::new(1)` settles at **2** live entries, because `order.len() / 2` rounds to 0 | **Accept** | Doc now scopes the sawtooth claim to the production capacity and states outright that `capacity` is not a strict bound at small values |
| 5 | **P1** — probe mistakes destructuring for consumption | Bug | Yes — **ran it**: deleting `set_mouse_captured(mouse_capture)` still yielded `ignored=[]`, `AUDIT PASS`, exit 0 | **Modify** | Root cause is **rustc error recovery**, deeper than stated: a pattern naming a missing field still creates the binding, so use sites emit nothing. The reviewer's fix ("exclude pattern diagnostics, require use evidence") would report *every* field unconsumed, since error recovery leaves no use diagnostics to find. Implemented a second signal instead — `unused_variables` — honored iff named ∧ ¬bound-unused. Verified both directions |
| 6 | probe2 never exercises the live constructors | Bug | Yes — capacities were hardcoded, so changing both statics to 20 left output identical while the audit claimed it measured the live caches | **Accept** | Capacity now **derived** from the production statics; both sites parsed and required to agree. Verified sensitive: flipping one static to 20 makes probe2 exit 1 |
| 7 | probe2 accepts failed/incorrect measurements | Bug | Yes — `return 0 if lines else 1` ignored `returncode`, record count, and the measured values | **Accept** | Now requires cargo success, both expected records present, and each `peak_held` to equal its requested capacity |
| 8 | "byte-exact" restore normalizes CRLF | Bug | Yes — `read_text`/`write_text` use universal newlines, so a CRLF checkout is rewritten to LF and the text comparison passes anyway | **Accept** | Switched to `read_bytes`/`write_bytes`; restore now compares bytes. Thematically the same hazard as cyril-xi4a |
| 9 | Probe docstring still prescribes `--all-targets` | Bug (doc accuracy) | Yes — module docstring contradicted `check()` and `audit.md` | **Accept** | Docstring rewritten; `findings.md` gained a correction note pointing at `audit.md` |
| 10 | Failed audit exits 0 | Bug | Yes — `main()` fell off the end returning `None`; `sys.exit(None)` is status 0 | **Accept** | Returns 1 whenever `ignored` is non-empty. Confirmed: exit 1 in the dead-field scenario |

## Why the P1 matters more than its one-line summary

The probe is the *evidence* behind claim C7 ("no `UiConfig` field lacks a
production consumer"). A green audit that cannot fail proves nothing, and this
one had already corrected itself once (the `--all-targets` false pass in
`audit.md`). That it was still wrong after that correction is the useful lesson:
the failure mode was never the flag, it was trusting a single signal from a
compiler that is *designed* to keep going after an error.

## Rejected in the previous review round, restated

- Rewriting `docs/plans/*.md` — dated archived planning documents are an audit
  trail; editing them would falsify history. The doc fence excludes that
  directory deliberately.
- "The fence checks files the diff never touches" — that is precisely claim C8's
  job (prove the live caches were *not* disturbed).
- "The absent-default test duplicates the explicit-true test" — only the absent
  case catches a flipped default.

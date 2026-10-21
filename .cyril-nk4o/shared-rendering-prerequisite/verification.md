# Shared rendering prerequisite: released-width migration

## Decision and source identity

Adopt the user-approved released pair **ratatui-core 0.1.2 + ratatui-widgets 0.3.2**, retaining the ratatui 0.30.0 facade and existing backend/features. Required transitive resolution is included; there is no vendor fork or new direct dependency. This is a shared prerequisite, **not overall story, native-terminal, C6, MCP-delivery, or full-workspace acceptance**.

| Pin | Value |
|---|---|
| Base commit / HEAD during qualification | `42151d93a87f3a083d96ff3b8b754f944b86785c` |
| Checked code **tree**, not a commit | `d83bf82f35b41b1bbca2cda4d0e35629e49169ad` |
| Receipt tree including retained generated output | `bc049e7782a9bb12ff7277f90353d28767a6a475` |
| Tested executable SHA-256; executable not packaged | `d1fbccafa9d6587f7f8f44597dff257e55d4ee775b36f7c0a86cd4560f916651` |
| Canonical qualifier receipt SHA-256 | `5076709476f8e20e28d9299ee2be1155cc2ddb74a0b0e82eb724935c73069f46` |

`manifest.json` records exact provenance, selected source hashes, archive members/hashes, and excluded material. `evidence.tar.gz` contains only the enumerated owned evidence; it is an inert archive, not a new runner or verification framework. Packaging does not change the eleven code paths or retroactively assign this tree hash to earlier runs. A later commit containing these documents will have a different repository tree.

## Implementation scope

The eight functional UI source paths are `src/text.rs` and `src/widgets/{approval,code_panel,input,markdown,review,toolbar,workflow_plan}.rs`, under `crates/cyril-ui`.

- Shared crate-local `text::width`, `text::line_width`, and `text::char_width` use the released `CellWidth` contract without allocating width queries. String/line accumulation stays in `usize`; line measurement retains painted span boundaries. Borrowed iteration uses the existing `Span::styled_graphemes` API, not a new direct unicode-segmentation dependency.
- FF9E/FF9F receive their physical columns; U3099/U309A retain combining behavior. Truncation retains complete graphemes and original retained-prefix bytes. Table columns, code-language header fill, code backgrounds, and affected popup/status/workflow budgets use the same metric.
- The qualified COMMON-BASE input retains **character wrapping**, scalar editing, raw submitted bytes, and the character-cursor API. Approval's character-wrap preservation here applies **only to this COMMON-BASE qualification**: existing feature-level grapheme wrapping in PRs **#188/#190/#191 must survive integration**, not be reverted to this base's policy. Controls remain zero-width in the scalar helper. No normalization, special-case backend, or continuation-cell workaround was added.
- Hooks and powers consume the existing migrated truncate/pad helpers without a separate functional edit. Semantic character limits remain character limits. This does not claim universal terminal-pixel parity.

## Bounded source qualification

The current canonical qualifier recorded these results on the checked code tree above:

| Gate | Recorded result |
|---|---|
| Focused `floor_tests::roomy_autocomplete_stays_below_input` | Exactly 1 passed |
| Focused preserved `floor_tests::roomy_frame_matches_main_fixture` | Exactly 1 passed |
| `cargo test -p cyril-ui`, including integration and documentation targets | **838 passed, 0 failed, 2 existing ignored** |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo build -p cyril` | Exit 0 |
| Final `cargo fmt --all --check` | Exit 0 |

The UI aggregate is the recorded seven-target sum `797 + 14 + 3 + 3 + 11 + 8 + 2`; it is not a full-workspace test result. The two ignored tests remain existing ignored tests, not silently passing tests. Portable qualification receipts and explicitly labelled log excerpts are under `qualification/` inside the archive. Excerpts omit environment-header records; each retains its original full-log hash and exact source line ranges in the manifest.

### Historical inflow failure and focused replacement

The earlier checked tree `4f671c07c3c07ddb169febd879769f7539569735` retains its original **795 passed / 1 failed / 1 ignored, exit 101** result. The failure was the roomy inflow whole-buffer snapshot: released scrollbar thumb-length rounding changed only cell **(79,5)** from `█` to `║` in the retained frame. The diff's displayed row 9 is not terminal row 9. The source/frame comparison established unchanged scene geometry/content; no production scrollbar correction was warranted.

The repair removed only the inflow scene/assertion from `roomy_frame_matches_main_fixture` and its sole committed `roomy_inflow_80x24.snap`. Idle and approval snapshots remain pinned. The separate root-render test preserves six messages, three draft lines, ten suggestions, selected index five, and 80×24. Its independent literals assert input rows **8–12**, visible draft/cursor/borders, ordered suggestions **13–22**, a visible marker only on the sixth item without pinning marker glyph/palette, status **23**, and the newest chat message above input.

The original failure log and generated `.snap.new` are archived as historical evidence, **not accepted replacement pixels**. Both original retained `.snap.new` copies remain outside this package unchanged. The generated output hash is `f3c976adc7a2c243c79fb8339306fdbbbd29c9a6c065dcf482f076b63924fe97`. Historical `.cyril-a14l/design.md` and `plan.md` records were not rewritten. Routine replacement coverage was source-reviewed and exercised by the ordinary focused/full UI gates; no new mutation driver or claimed fault-injection result was introduced.

## Owned consumer observations: keep command outcomes separate

Two separately authorized owned-V2 attempts used a synthetic ACP peer, not native Kiro, a real account, a network model, permission grants, or tool file operations.

| Record | Original outcome and current meaning |
|---|---|
| Attempt01 | Original `result.json` remains **FAIL**. The first short-input caret locator also counted scrollbar block glyphs; later input/approval scenarios were not reached. It remains attributed to the earlier partial source tree, not relabelled with repaired-source gates. |
| Attempt02 / GO02 | Original command remains **WRAPPER FAIL, exit 1**. Required `run()` predicates completed, but final diagnostic export raised before normal cleanup/result serialization. **No attempt02 `result.json` or original successful final screenshot exists.** |
| Parent's current scoped adjudication | Required existing-UI observations are satisfied; retire **only the redundant mandatory final `Screen.display` export**. Neither original command becomes PASS, and no new overall PASS is asserted. |

The archive retains **19 original coordinate-cell captures**: 4 Markdown, 8 short-input, 4 character-wrap, and 3 approval frames. Coverage includes live/committed Markdown at 160/80/52 columns, halfwidth and combining table alignment, code-label/body sentinels and backgrounds, scalar edits/caret/vacated-column behavior, retained character-wrap boundaries, reject-only approval preview/resize/closure, and draft preservation.

Eight payload JSON receipts preserve expected text, scalars, full UTF-8 hex, and hashes. The independent original wire records the corresponding actual submissions: **all eight byte-exact**, with no Unicode normalization. There are **28 bidirectional ACP frames**, **one owned session / ten prompts**, final `pending_prompt=false`, the exact `selected` / `deny` response, and no filesystem/terminal callback frames. These are bounded observations, not a general editor or permission-system qualification.

### Why the wrapper failure does not erase completed predicates

The retained original traceback reaches `width_smoke.py:323`, the `else: finish("PASS")` branch reachable only after `run()` returns normally. The executed source runs all scenarios and the final no-callback assertion before returning; false checks raise. `finish()` attempts the final capture **before** normal cleanup and result serialization. The archive preserves the exact executed sources and fixtures for this control-flow attribution; it does not fabricate the absent serialized checks/result list.

The existing same-installed-Pyte replay consumed all **321,512 original ANSI bytes** unchanged. A SPACE overwrote a previous hourglass lead at **(0,23)**; Pyte retained its own empty continuation at **(1,23)** and later indexed that empty value while serializing `Screen.display`. Only the final recorded-output boundary, offsets **321214–321512**, raised. This is decoder buffer/export attribution, not evidence that Cyril emitted an empty glyph.

Original raw ANSI, events, wire, captures, traceback, and explicitly **derived, unnormalized** final cells are retained. Empty strings remain literal; no empty-to-space substitution, fabricated screenshot, third product run, new parser, or parser patch was used. The original recording remains at its owner; it is excluded here because its header contains environment metadata. The archived replay receipt preserves the already-observed byte equality without copying that header.

Final capture supplied no additional required functional assertion. The parent retired only that redundant export dependency as coverage-preserving technical verification simplification, not risk acceptance or a waiver of a failed required product behavior. The later diagnostic state is preserved by raw ANSI and labelled derived cells, not claimed identical to the last successful scenario capture.

## Reviews and remaining boundaries

The archive's review receipt preserves the reported scopes and findings:

- Existing-width migration: `ReviewExistingWidthCutover`, no findings in the eight-file functional patch and actual consumers; **reported confidence 0.94**. This is a source-only review, not backend/ANSI or native acceptance.
- Focused inflow replacement: `ReviewRoomyOracleRepair`, no findings in the independent placement/content oracle or preservation of idle/approval fixtures; reported confidence 0.97. This reviewer performed no execution.
- Consumer retirement adjudication: `ReviewSharedSmokeAdjudication`, no missing unique required coverage or findings in the scoped disposition; reported confidence 0.99. Neither original consumer command becomes PASS.

**Working Kiro bootstrap is confirmed.** Native acceptance still requires a **newly qualified MCP binary and explicit bounded parent GO**; this package grants no login, auth, account, credential-change, or automatic native retry authority. C6 reference/history/gutters, MCP-delivery/painter lifecycle, native/platform acceptance, benchmarks and full-workspace/auth tests remain separate. Workspace Clippy and the default build do not substitute for those gates; the common tree's missing shared auth fixture is an explicit scope limit, not an acceptance waiver.

Packaging performed no Cargo, test, lint, format, product, parser-replay, mutation, or native run and made no code changes, commit, push, or tracker update. All gate and consumer results above are retained evidence from their named owners.

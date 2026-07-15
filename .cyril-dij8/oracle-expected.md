# cyril-dij8 — oracle: static source-scan transcription (PRE-REGISTERED)

Written BEFORE the probe ran (nrnq method: different mechanism — raw source
text vs rendered ratatui buffer). Source scanned: `toolbar.rs`,
`crew_panel.rs`, `voice.rs` production sections @ branch point d4f105f.

Normalization expectations (recorded by ghuu/nrnq): unset colors surface as
concrete `Reset` in `Cell`; `Paragraph::style(bg)` paints every cell of the
paragraph area (so separators and trailing blanks carry the chrome bg).

## Toolbar (`toolbar::render`) — Paragraph bg = Rgb(30,30,46)

| # | fg | bg | mod | source line(s) | reached by |
|---|---|---|---|---|---|
| T1 | Reset | Rgb(30,30,46) | NONE | :54 separators, trailing blanks | any segment pair |
| T2 | Yellow | Rgb(30,30,46) | NONE | :18 spinner Sending/Waiting; :75 effort; :85 steer chip | Sending fixture / effort / steers≥1 |
| T3 | Green | Rgb(30,30,46) | NONE | :25 spinner Streaming | Streaming fixture |
| T4 | Cyan | Rgb(30,30,46) | NONE | :32 spinner ToolRunning; :57 mode; :94 code intel | ToolRunning / mode / intel fixtures |
| T5 | White | Rgb(30,30,46) | BOLD | :42-43 session label | session Some |
| T6 | DarkGray | Rgb(30,30,46) | NONE | :48 "No session"; :104 elapsed | session None / elapsed Some |
| T7 | Magenta | Rgb(30,30,46) | NONE | :66 model | model Some |

## Status bar (`toolbar::render_status_bar`) — Paragraph bg = Rgb(30,30,46)

| # | fg | bg | mod | source line(s) | reached by |
|---|---|---|---|---|---|
| S1 | Reset | Rgb(30,30,46) | NONE | separators, blanks | any |
| S2 | Green | Rgb(30,30,46) | NONE | :144 context ≤70 | pct 50 |
| S3 | Yellow | Rgb(30,30,46) | NONE | :142 context >70 | pct 75 |
| S4 | Red | Rgb(30,30,46) | NONE | :140 context >90 | pct 95 |
| S5 | DarkGray | Rgb(30,30,46) | NONE | :172 breakdown; :212 tokens; :225 credits; :242 "cyril" fallback | wide+breakdown / tokens / credits / empty |
| S6 | Yellow | Rgb(30,30,46) | BOLD | :182-183 "Token limit"/"Turn limit"; :236-237 SCROLL hint | MaxTokens / MaxTurnRequests / scroll-back |
| S7 | Red | Rgb(30,30,46) | BOLD | :184 "Refused" | Refusal |
| S8 | DarkGray | Rgb(30,30,46) | BOLD | :185 "Cancelled" | Cancelled |

DEAD STYLING (must NOT appear): `(White, EndTurn)` at :181 pairs White with
an empty label that is never pushed — no White cell may appear in any status
bar scenario.

## Crew panel (`crew_panel::render`) — no paragraph bg; Block borders unstyled

| # | fg | bg | mod | source line(s) | reached by |
|---|---|---|---|---|---|
| C1 | Reset | Reset | NONE | borders; :138 "  " overflow lead-in; blanks | any |
| C2 | Cyan | Reset | NONE | :161 block title | any |
| C3 | Green | Reset | NONE | :80 "●" working icon | Working subagent |
| C4 | DarkGray | Reset | NONE | :83 "◆" terminated; :94 status text; :124 "○"; :129 deps | Terminated / any / pending |
| C5 | White | Reset | BOLD | :91-92 subagent name | any subagent |
| C6 | Magenta | Reset | NONE | :106 "↻ i/max" loop badge | loop_state Some |
| C7 | Gray | Reset | NONE | :127 pending stage name | pending stage |
| C8 | Yellow | Reset | ITALIC | :142-143 "+N more" | >6 rows |

## Voice (`voice::render`) — no paragraph bg

| # | fg | bg | mod | source line(s) | reached by |
|---|---|---|---|---|---|
| V1 | Rgb(138,180,248) | Reset | NONE | :33 "🎙 listening " = palette::USER_BLUE | Listening |
| V2 | Reset | Reset | NONE | :34 meter bar (unstyled); blanks | Listening |
| V3 | Rgb(140,140,140) | Reset | NONE | :35 "/voice to stop" = palette::MUTED_GRAY | Listening |
| V4 | Rgb(180,142,173) | Reset | NONE | :39 "⏳ transcribing…" = palette::SYSTEM_MAUVE | Transcribing |

## Totals

23 distinct styled tuples (6 toolbar + 7 status + 7 crew + 3 voice), plus
the two unstyled flavors (Reset-on-chrome-bg, full Reset).

## Legacy → canonical mapping (ghuu NAMED scheme, to verify post-agreement)

| Legacy literal | Canonical RGB | 31-role contract role |
|---|---|---|
| Rgb(30,30,46) | #1e1e2e | chrome — EXACT |
| Color::White | #ffffff | text |
| Color::DarkGray | #808080 | subdued |
| Color::Gray | #c0c0c0 | text_secondary |
| Color::Yellow | #808000 | emphasis |
| Color::Green | #008000 | subdued_positive |
| Color::Red | #800000 | subdued_negative |
| Color::Cyan | #008080 | accent_quinary |
| Color::Magenta | #800080 | accent_quaternary |
| USER_BLUE Rgb(138,180,248) | #8ab4f8 | soft_accent / user (value twins) |
| MUTED_GRAY Rgb(140,140,140) | #8c8c8c | muted / border (value twins) |
| SYSTEM_MAUVE Rgb(180,142,173) | #b48ead | system / accent_alt (value twins) |

Prediction: every chrome legacy color is already representable in the
31-role contract → dij8 is the first PURE re-mapping batch (no contract
expansion), unlike ghuu (+10) and nrnq (+2).

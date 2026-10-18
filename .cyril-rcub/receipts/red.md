# cyril-rcub red receipts

## 1. Base revision (render fences first)

Base revision: `d6a727975e36e71413f8fea30fc6ee8a8d5a9767` (origin/main at
branch point). Production code unmodified; only the first render fences were
added to `crates/cyril-ui/src/widgets/approval.rs`. At that point the clamped
fence used `input_top=7`; it moved to `input_top=8` with the separator-budget
fix (see §2, M2).

Command:
`env -u CARGO_TARGET_DIR cargo nextest run -p cyril-ui --no-fail-fast -E 'test(/write_path/)'`

Result: 8 tests run, 0 passed, 8 failed. Every case of
`long_write_path_is_fully_visible_when_rows_allow` (ascii 80x24, ascii 40x24,
wide 80x24, wide 30x24) and `clamped_write_path_keeps_file_name` (ascii 80,
ascii 44, wide 80, wide 36) failed.

Screen for `clamped_write_path_keeps_file_name::case_1_ascii_80`. This is the
live 2.32.0 symptom: the file name is cut off at the border.

```
┌ Permission Required ─────────────────────────────────────┐
│Write File                                                │
│                                                          │
│  /home/dwalleck/repos/cyril/experiments/conductor-spike/a│
│                                                          │
└──────────────────────────────────────────────────────────┘
```

The same screen shows a second defect that already existed. When the popup is
clamped and a preview is present, the blank row after the preview is not
counted in the row budget. That pushes the only option row (here the selected
`▸ Allow`) out of the popup, which breaks cyril-a14l C8.

Wide glyphs are clipped mid-name the same way. In
`case_4_wide_chars_narrow_36` the visible text ends at `/home/用户/项目/文档/非常长`.

## 2. Mutations against the final fences (head `14f5dfec`)

Each mutation was applied to the committed file, the fences were run
(`-E 'test(/write_path|clamped_preview_keeps/)'`, 17 tests), and the file was
restored from the commit. The work was committed before each mutation. The
clamped fences run at `input_top=7` (inner 4), which is the base-revision red
geometry from §1.

- **M1, old single-line path layout.** `Preview` emits the path as one
  unwrapped row. This was run on `d0a176d1`, before the inner-4 change.
  13 of 13 rcub fences failed: all four roomy, all four clamped, the reason
  page, and all four snapshots.
- **M2, old separator budget.** `show_separator = preview_rows > 0` and the
  separator is not counted in `option_rows`, which is the base revision's
  budget. 9 of 17 failed: `clamped_preview_keeps_selected_option_visible`
  `inner_4`, `inner_5` and `inner_6` ("selected option clipped"; `inner_7`
  has a spare row), all four `clamped_write_path_keeps_file_name` cases, and
  both clamped snapshots.
- **M3, separator reserved ahead of the path.** This is the reviewer's
  should-fix: `preview.len().min(remaining.saturating_sub(2))`, the
  intermediate `d0a176d1` budget, which hid the path at inner 4. 7 of 17
  failed: `clamped_preview_keeps_selected_option_visible::inner_4`, all four
  `clamped_write_path_keeps_file_name` cases, and both clamped snapshots.

After restoration, the whole cyril-ui suite passes (721 tests).

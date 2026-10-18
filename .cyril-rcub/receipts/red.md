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

## 2. Mutations against the final fences (head `d0a176d1` + fence commit)

Each mutation was applied to the committed file, the fences were run, and the
file was restored with `git checkout --` (the work was committed first).

- **M1, old single-line path layout.** `Preview` emits the path as one
  unwrapped row (`std::iter::once(path)`, `path_rows = 1`). Result: 13 of 13
  failed. This covered all four `long_write_path_is_fully_visible_when_rows_allow`
  cases, all four `clamped_write_path_keeps_file_name` cases,
  `reason_phase_shows_full_write_path`, and all four
  `long_write_path_snapshots`.
- **M2, unbudgeted separator.** The pre-fix
  `preview.len().min(remaining.saturating_sub(1))` is restored, with
  `separator = 0`. Result:
  - `clamped_preview_keeps_selected_option_visible`: `inner_5` and `inner_6`
    FAIL with "selected option clipped"; `inner_7` passes because that height
    has a spare row.
  - The four `clamped_write_path_keeps_file_name` cases and the two clamped
    snapshots also FAIL.

After restoration, the whole cyril-ui suite passes (see `gate.md`).

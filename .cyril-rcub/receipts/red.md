# cyril-rcub red receipt

Base revision: `d6a727975e36e71413f8fea30fc6ee8a8d5a9767` (origin/main at
branch point). Production code unmodified; only the new fences were added to
`crates/cyril-ui/src/widgets/approval.rs`.

Command:
`env -u CARGO_TARGET_DIR cargo nextest run -p cyril-ui --no-fail-fast -E 'test(/write_path/)'`

Result: 8 tests run, 0 passed, 8 failed.

```
FAIL long_write_path_is_fully_visible_when_rows_allow::case_1_ascii_80x24
FAIL long_write_path_is_fully_visible_when_rows_allow::case_2_ascii_narrow_40x24
FAIL long_write_path_is_fully_visible_when_rows_allow::case_3_wide_chars_80x24
FAIL long_write_path_is_fully_visible_when_rows_allow::case_4_wide_chars_narrow_30x24
FAIL clamped_write_path_keeps_file_name::case_1_ascii_80
FAIL clamped_write_path_keeps_file_name::case_2_ascii_narrow_44
FAIL clamped_write_path_keeps_file_name::case_3_wide_chars_80
FAIL clamped_write_path_keeps_file_name::case_4_wide_chars_narrow_36
```

Screen for `clamped_write_path_keeps_file_name::case_1_ascii_80` (this is the
live 2.32.0 symptom: the file name is cut off at the border):

```
┌ Permission Required ─────────────────────────────────────┐
│Write File                                                │
│                                                          │
│  /home/dwalleck/repos/cyril/experiments/conductor-spike/a│
│                                                          │
└──────────────────────────────────────────────────────────┘
```

This same screen shows a second, pre-existing defect. When the popup is
clamped and a preview is present, the blank row after the preview is not
counted in the row budget. That blank pushes the only option row (here the
selected `▸ Allow`) out of the popup, which breaks cyril-a14l C8. The clamped
fence's `▸ Allow` assertion covers it.

Wide glyphs (`case_4_wide_chars_narrow_36`) are clipped mid-name the same way:
`/home/用户/项目/文档/非常长`.

# cyril-q0kg red receipt

Base revision: `d6a727975e36e71413f8fea30fc6ee8a8d5a9767` (origin/main at
branch point). Production code unmodified; only the new fences were added to
`crates/cyril-ui/src/widgets/approval.rs`.

Command:
`env -u CARGO_TARGET_DIR cargo nextest run -p cyril-ui --no-fail-fast -E 'test(/malformed_preview/)'`

```
FAIL malformed_preview_warning_is_not_repeated_per_frame::case_1_non_string_text      (left: 6, right: 1)
FAIL malformed_preview_warning_is_not_repeated_per_frame::case_2_shell_command_shape  (left: 6, right: 1)
PASS malformed_preview_marker_survives_repeated_renders
PASS malformed_preview_warning_is_not_repeated_per_frame::case_3_string_text
PASS malformed_preview_warning_is_not_repeated_per_frame::case_4_string_content
PASS malformed_preview_warning_is_not_repeated_per_frame::case_5_no_raw_input
PASS malformed_preview_warning_is_not_repeated_per_frame::case_6_diff_takes_precedence
Summary 7 tests run: 5 passed, 2 failed
```

Reading: six frames (three option-phase, three reject-with-reason-phase)
produced six identical WARN events — one per frame — for both malformed
shapes. The passing cases are the no-warning guards (they hold before and
after the fix) and the marker-still-renders guard.

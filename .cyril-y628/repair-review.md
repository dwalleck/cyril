# Independent R1 repair review

PASS
Reviewed only `.cyril-y628/r1.diff`, `.cyril-y628/review-decisions.md`, and the current `feedback_editor.rs`, plus the pinned Crossterm source needed to verify the stated parser contract.
The production change is narrowly confined to the `KeyCode::Char` admission guard at `feedback_editor.rs:100-112`.
It admits exactly Ctrl+Alt and Ctrl+Alt+Shift character events in addition to the existing unmodified and Shift-only forms.
The pinned `Cargo.lock` entry is Crossterm 0.29.0 with the checksum recorded in the review decision.
Crossterm's Windows parser derives CONTROL, ALT, and SHIFT independently from `ControlKeyState` and retains those bits when constructing `KeyEvent` values.
Its printable Unicode path produces `KeyCode::Char`, so the new guard matches the dependency's demonstrated AltGr representation.
Ctrl+J remains protected by the earlier exact CONTROL-only branch at `feedback_editor.rs:93-97`, which inserts a newline before the general character branch is considered.
Ctrl+Alt+j deliberately falls through to the new character branch and is inserted as `j`, matching the repair contract.
Plain Ctrl+x and plain Alt+x still miss every insertion guard and reach the consuming catch-all, so neither becomes draft text.
All newly admitted characters still call the existing `insert_char`; the patch introduces no alternate insertion path.
That helper rejects disallowed control characters, enforces the 4,096-scalar ceiling, preserves UTF-8 cursor accounting, and controls the existing limit notice.
The regression test covers Ctrl+Alt `@`, Ctrl+Alt+Shift `€`, and Ctrl+Alt `j`, then verifies exact text, byte cursor, and scalar count.
The Ctrl+Alt+Shift case discriminates against a repair that admits only the base AltGr modifier pair.
The Ctrl+Alt+j case discriminates against broadening the existing Ctrl+J newline rule from equality to modifier containment.
The plain Ctrl+x and Alt+x controls discriminate against admitting either modifier independently or using an overbroad modifier predicate.
The pre-existing Ctrl+J test separately pins newline insertion and non-submission.
Restoring the pre-repair no-modifier/Shift-only guard would leave the new test text empty, so the regression fence directly detects the reported bug.
No unrelated behavior, module boundary, dependency, or platform branch is changed by R1.
This is source-level approval only; no native Windows hardware verification is inferred or claimed.

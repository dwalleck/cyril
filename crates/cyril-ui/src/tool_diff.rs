//! Theme-independent, bounded diff rows borrowing the tracked tool's source.

use std::ops::Range;

use similar::{ChangeTag, DiffTag, TextDiff};

const MAX_DIFF_ROWS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiffSource<'a> {
    pub old: &'a str,
    pub new: &'a str,
}

#[derive(Debug)]
pub(crate) struct DiffRow {
    pub tag: ChangeTag,
    pub line_number: usize,
    range: Range<usize>,
}

impl DiffRow {
    pub fn text<'a>(&self, source: DiffSource<'a>) -> &'a str {
        let text = match self.tag {
            ChangeTag::Delete => source.old,
            ChangeTag::Insert | ChangeTag::Equal => source.new,
        };
        &text[self.range.clone()]
    }
}

#[derive(Debug, Default)]
pub(crate) struct DiffPreview {
    summary: Option<(usize, usize)>,
    rows: Vec<DiffRow>,
    truncated: bool,
}

impl DiffPreview {
    pub fn new(source: Option<DiffSource<'_>>) -> Self {
        let Some(source) = source else {
            return Self::default();
        };

        #[cfg(test)]
        TEXT_DIFF_CONSTRUCTIONS.set(TEXT_DIFF_CONSTRUCTIONS.get() + 1);
        let diff = TextDiff::from_lines(source.old, source.new);
        let (mut added, mut removed) = (0, 0);
        for op in diff.ops() {
            match op.tag() {
                DiffTag::Insert => added += op.new_range().len(),
                DiffTag::Delete => removed += op.old_range().len(),
                DiffTag::Replace => {
                    added += op.new_range().len();
                    removed += op.old_range().len();
                }
                DiffTag::Equal => {}
            }
        }
        if added == 0 && removed == 0 {
            return Self::default();
        }

        let mut preview = Self {
            summary: Some((added, removed)),
            rows: Vec::with_capacity(
                MAX_DIFF_ROWS.min(diff.old_slices().len() + diff.new_slices().len()),
            ),
            truncated: false,
        };
        // similar 2.7's line tokens partition each original source, retaining
        // LF, CRLF and bare CR. Walk their lengths monotonically to obtain
        // source byte ranges without copied text or retained per-line indexes.
        let (mut old_cursor, mut new_cursor) = ((0, 0), (0, 0));
        for group in diff.grouped_ops(1) {
            for op in group {
                for change in diff.iter_changes(&op) {
                    if preview.rows.len() == MAX_DIFF_ROWS {
                        preview.truncated = true;
                        return preview;
                    }
                    let (slices, cursor, index) =
                        match (change.tag(), change.old_index(), change.new_index()) {
                            (ChangeTag::Delete, Some(index), _) => {
                                (diff.old_slices(), &mut old_cursor, index)
                            }
                            (ChangeTag::Insert | ChangeTag::Equal, _, Some(index)) => {
                                (diff.new_slices(), &mut new_cursor, index)
                            }
                            _ => {
                                unreachable!("line changes carry their corresponding source index")
                            }
                        };
                    cursor.1 += slices[cursor.0..index]
                        .iter()
                        .map(|line| line.len())
                        .sum::<usize>();
                    let start = cursor.1;
                    // Preserve the existing presentation: remove LF, not CR.
                    let end = start + change.value().trim_end_matches('\n').len();
                    preview.rows.push(DiffRow {
                        tag: change.tag(),
                        line_number: index + 1,
                        range: start..end,
                    });
                    *cursor = (index + 1, start + slices[index].len());
                }
            }
        }
        preview
    }

    pub fn summary(&self) -> Option<(usize, usize)> {
        self.summary
    }

    pub fn rows(&self) -> &[DiffRow] {
        &self.rows
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

#[cfg(test)]
thread_local! {
    static TEXT_DIFF_CONSTRUCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn text_diff_constructions() -> usize {
    TEXT_DIFF_CONSTRUCTIONS.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_preview_has_exact_counts_context_and_source_ranges() {
        let source = DiffSource {
            old: "before\nsame\nold\nafter\ntail\n",
            new: "before\nsame\nnew\nextra\nafter\ntail\n",
        };
        let before = text_diff_constructions();
        let preview = DiffPreview::new(Some(source));
        assert_eq!(text_diff_constructions() - before, 1);
        assert_eq!(preview.summary(), Some((2, 1)));
        let rows: Vec<_> = preview
            .rows()
            .iter()
            .map(|row| (row.tag, row.line_number, row.text(source)))
            .collect();
        assert_eq!(
            rows,
            vec![
                (ChangeTag::Equal, 2, "same"),
                (ChangeTag::Delete, 3, "old"),
                (ChangeTag::Insert, 3, "new"),
                (ChangeTag::Insert, 4, "extra"),
                (ChangeTag::Equal, 5, "after"),
            ]
        );
        for row in preview.rows() {
            let text = match row.tag {
                ChangeTag::Delete => source.old,
                ChangeTag::Insert | ChangeTag::Equal => source.new,
            };
            assert_eq!(row.text(source).as_ptr(), text[row.range.clone()].as_ptr());
        }
        assert!(!preview.truncated());
    }

    #[test]
    fn diff_preview_handles_absence_unchanged_insertions_and_deletions() {
        for source in [
            None,
            Some(DiffSource { old: "", new: "" }),
            Some(DiffSource {
                old: "unchanged\n",
                new: "unchanged\n",
            }),
        ] {
            let preview = DiffPreview::new(source);
            assert_eq!(preview.summary(), None);
            assert!(preview.rows().is_empty());
            assert!(!preview.truncated());
        }
        for (source, summary, tag) in [
            (
                DiffSource {
                    old: "",
                    new: "a\nb",
                },
                (2, 0),
                ChangeTag::Insert,
            ),
            (
                DiffSource {
                    old: "a\nb",
                    new: "",
                },
                (0, 2),
                ChangeTag::Delete,
            ),
        ] {
            let preview = DiffPreview::new(Some(source));
            assert_eq!(preview.summary(), Some(summary));
            assert_eq!(preview.rows().len(), 2);
            assert_eq!(preview.rows()[0].tag, tag);
            assert_eq!(preview.rows()[1].line_number, 2);
            assert_eq!(preview.rows()[1].text(source), "b");
        }
    }

    #[test]
    fn diff_preview_ranges_preserve_unicode_crlf_bare_cr_and_final_line() {
        let source = DiffSource {
            old: "héllo\r\nold\r終わり",
            new: "héllo\r\nnew\r終わり",
        };
        let preview = DiffPreview::new(Some(source));
        assert_eq!(preview.summary(), Some((1, 1)));
        assert_eq!(
            preview
                .rows()
                .iter()
                .map(|row| row.text(source))
                .collect::<Vec<_>>(),
            ["héllo\r", "old\r", "new\r", "終わり"]
        );
    }

    #[test]
    fn diff_preview_ranges_skip_disjoint_context_gaps() {
        let source = DiffSource {
            old: "0\n1\nold-a\n3\n4\n5\n6\n7\nold-b\n9\n10\n",
            new: "0\n1\nnew-a\n3\n4\n5\n6\n7\nnew-b\n9\n10\n",
        };
        let preview = DiffPreview::new(Some(source));
        assert_eq!(preview.summary(), Some((2, 2)));
        assert_eq!(
            preview
                .rows()
                .iter()
                .map(|row| (row.line_number, row.text(source)))
                .collect::<Vec<_>>(),
            [
                (2, "1"),
                (3, "old-a"),
                (3, "new-a"),
                (4, "3"),
                (8, "7"),
                (9, "old-b"),
                (9, "new-b"),
                (10, "9")
            ]
        );
    }

    #[test]
    fn diff_preview_truncates_only_after_twenty_rows_with_full_summary() {
        for count in [19, 20, 21, 200] {
            let new: String = (1..=count).map(|line| format!("line {line}\n")).collect();
            let source = DiffSource { old: "", new: &new };
            let preview = DiffPreview::new(Some(source));
            assert_eq!(preview.summary(), Some((count, 0)));
            assert_eq!(preview.rows().len(), count.min(20));
            assert_eq!(preview.truncated(), count > 20);
            assert_eq!(
                preview.rows().last().map(|row| row.line_number),
                Some(count.min(20))
            );
        }
    }
}

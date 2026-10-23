//! Physical-row heights for retained conversation blocks, without owning their sources.

/// A Fenwick tree over block heights, with retained-relative block indices.
///
/// Append, point updates and row lookup take `O(log N)`; totals take `O(1)`.
/// Prefix removal leaves tombstones until they outnumber retained blocks, then
/// compacts and rebuilds only height metadata in linear time. No wrapped rows,
/// message data or source text belong to this index.
#[derive(Debug, Default)]
pub(crate) struct RowHeights {
    heights: Vec<usize>,
    // Slot i stores the sum ending at one-based position i + 1, over its low bit.
    tree: Vec<usize>,
    retired: usize,
    retired_rows: usize,
    total: usize,
    #[cfg(test)]
    locate_steps: std::cell::Cell<usize>,
}

impl RowHeights {
    pub(crate) fn len(&self) -> usize {
        self.heights.len() - self.retired
    }

    pub(crate) fn push(&mut self, height: usize) {
        let node = self.tree.len() + 1;
        let start = node & (node - 1);
        let mut previous = node - 1;
        let mut sum = height;
        // An appended Fenwick node may cover older blocks, not just the new one.
        // Gather those existing subtrees without rebuilding earlier nodes.
        while previous > start {
            sum += self.tree[previous - 1];
            previous &= previous - 1;
        }
        self.heights.push(height);
        self.tree.push(sum);
        self.total += height;
    }

    /// Returns whether `index` exists, including an unchanged-height update.
    pub(crate) fn set(&mut self, index: usize, height: usize) -> bool {
        if index >= self.len() {
            return false;
        }
        let stored = self.retired + index;
        let previous = self.heights[stored];
        if previous == height {
            return true;
        }
        self.heights[stored] = height;
        let increase = height > previous;
        let delta = height.abs_diff(previous);
        if increase {
            self.total += delta;
        } else {
            self.total -= delta;
        }
        let mut node = stored + 1;
        while node <= self.tree.len() {
            if increase {
                self.tree[node - 1] += delta;
            } else {
                self.tree[node - 1] -= delta;
            }
            node += node & node.wrapping_neg();
        }
        true
    }

    /// Drops an internally validated number of leading retained blocks.
    pub(crate) fn discard_prefix(&mut self, count: usize) {
        assert!(
            count <= self.len(),
            "cannot discard more than retained blocks"
        );
        if count == 0 {
            return;
        }
        if count == self.len() {
            *self = Self::default();
            return;
        }
        let retired = self.retired + count;
        let retired_rows = self.prefix_sum(retired);
        self.total -= retired_rows - self.retired_rows;
        self.retired = retired;
        self.retired_rows = retired_rows;
        if self.retired > self.len() {
            self.compact();
        }
    }

    pub(crate) fn total(&self) -> usize {
        self.total
    }

    #[cfg(test)]
    pub(crate) fn locate_steps(&self) -> usize {
        self.locate_steps.get()
    }

    /// Finds the retained block and physical row inside it, skipping zero heights.
    pub(crate) fn locate(&self, row: usize) -> Option<(usize, usize)> {
        #[cfg(test)]
        self.locate_steps.set(0);
        if row >= self.total {
            return None;
        }
        let mut remaining = self.retired_rows + row;
        let mut before = 0;
        let mut step = 1usize << self.tree.len().ilog2();
        // Binary lifting finds the longest prefix whose height is <= the target.
        // Equal sums must be skipped too: they represent zero-height blocks.
        while step != 0 {
            #[cfg(test)]
            self.locate_steps.set(self.locate_steps.get() + 1);
            let next = before + step;
            if next <= self.tree.len() && self.tree[next - 1] <= remaining {
                remaining -= self.tree[next - 1];
                before = next;
            }
            step >>= 1;
        }
        Some((before - self.retired, remaining))
    }

    fn prefix_sum(&self, mut count: usize) -> usize {
        let mut sum = 0;
        while count != 0 {
            sum += self.tree[count - 1];
            count &= count - 1;
        }
        sum
    }

    fn compact(&mut self) {
        let retained = self.len();
        self.heights.copy_within(self.retired.., 0);
        self.heights.truncate(retained);
        self.tree.truncate(retained);
        self.tree.copy_from_slice(&self.heights);
        // Each node contributes to its parent once: rebuilding is O(N), not a
        // sequence of O(log N) point updates over all the retained heights.
        for node in 1..=retained {
            let parent = node + (node & node.wrapping_neg());
            if parent <= retained {
                let sum = self.tree[node - 1];
                self.tree[parent - 1] += sum;
            }
        }
        self.retired = 0;
        self.retired_rows = 0;
        // Keep append headroom without retaining an old history-sized allocation
        // after a large prefix discard. Only usize metadata can be relocated.
        let capacity = retained.saturating_mul(2);
        self.heights.shrink_to(capacity);
        self.tree.shrink_to(capacity);
    }
}

#[cfg(test)]
mod tests {
    use super::RowHeights;

    fn index_for(heights: &[usize]) -> RowHeights {
        let mut index = RowHeights::default();
        for &height in heights {
            index.push(height);
        }
        index
    }

    fn assert_matches(index: &RowHeights, heights: &[usize]) {
        assert_eq!(index.len(), heights.len());
        assert_eq!(index.total(), heights.iter().sum::<usize>());
        let mut row = 0;
        for (block, &height) in heights.iter().enumerate() {
            for inside in 0..height {
                assert_eq!(index.locate(row), Some((block, inside)), "row {row}");
                row += 1;
            }
        }
        assert_eq!(index.locate(row), None);
        assert_eq!(index.locate(usize::MAX), None);
    }

    fn assert_window(index: &RowHeights, start: usize, expected: &[(usize, usize)]) {
        for (offset, &location) in expected.iter().enumerate() {
            assert_eq!(index.locate(start + offset), Some(location));
        }
    }

    #[test]
    fn empty_and_zero_heights_have_no_physical_rows() {
        let mut index = RowHeights::default();
        assert_matches(&index, &[]);
        assert!(!index.set(0, 3));
        index.discard_prefix(0);
        for _ in 0..3 {
            index.push(0);
        }
        assert_matches(&index, &[0, 0, 0]);
        assert!(index.set(1, 4));
        assert_matches(&index, &[0, 4, 0]);
        assert!(index.set(1, 0));
        assert_matches(&index, &[0, 0, 0]);
        index.discard_prefix(2);
        assert_matches(&index, &[0]);
        index.discard_prefix(1);
        assert_matches(&index, &[]);
    }

    #[test]
    fn known_heights_locate_exact_first_middle_and_last_windows() {
        let index = index_for(&[0, 3, 0, 1, 4, 0, 2, 0]);
        assert_eq!(index.len(), 8);
        assert_eq!(index.total(), 10);
        assert_window(&index, 0, &[(1, 0), (1, 1), (1, 2)]);
        assert_window(&index, 3, &[(3, 0), (4, 0), (4, 1), (4, 2)]);
        assert_window(&index, 7, &[(4, 3), (6, 0), (6, 1)]);
        assert_eq!(index.locate(10), None);
        assert_eq!(index.locate(usize::MAX), None);
    }

    #[test]
    fn append_initializes_nodes_across_power_of_two_boundaries() {
        let mut index = RowHeights::default();
        let mut heights = Vec::new();
        for block in 0..65 {
            let height = block % 5;
            heights.push(height);
            index.push(height);
            assert_matches(&index, &heights);
        }
    }

    #[test]
    fn point_updates_preserve_later_locations_and_report_bounds() {
        let mut index = index_for(&[2, 0, 3, 1, 0]);
        assert!(index.set(1, 4));
        assert_matches(&index, &[2, 4, 3, 1, 0]);
        assert!(index.set(2, 1));
        assert_matches(&index, &[2, 4, 1, 1, 0]);
        assert!(index.set(0, 0));
        assert_matches(&index, &[0, 4, 1, 1, 0]);
        assert!(index.set(4, 2));
        assert_matches(&index, &[0, 4, 1, 1, 2]);
        assert!(index.set(4, 2));
        assert!(!index.set(5, 9));
        assert!(!index.set(usize::MAX, 9));
        assert_matches(&index, &[0, 4, 1, 1, 2]);
    }

    #[test]
    fn prefix_discard_keeps_retained_indices_across_compaction() {
        let mut index = index_for(&[2, 0, 3, 1, 0, 4, 2, 0]);
        index.discard_prefix(2);
        assert_eq!(index.retired, 2);
        assert_eq!(index.heights.len(), 8);
        assert_matches(&index, &[3, 1, 0, 4, 2, 0]);
        assert!(index.set(1, 5));
        index.push(6);
        assert_matches(&index, &[3, 5, 0, 4, 2, 0, 6]);
        index.discard_prefix(3);
        assert_eq!(index.retired, 0);
        assert_eq!(index.heights.len(), 4);
        assert_eq!(index.tree.len(), 4);
        assert_matches(&index, &[4, 2, 0, 6]);
        index.push(0);
        index.push(3);
        assert!(index.set(0, 1));
        assert_matches(&index, &[1, 2, 0, 6, 0, 3]);
        index.discard_prefix(0);
        assert_matches(&index, &[1, 2, 0, 6, 0, 3]);
    }

    #[test]
    fn discarding_all_blocks_resets_and_reuses_the_index() {
        let mut index = index_for(&[5, 0, 7]);
        index.discard_prefix(1);
        assert_eq!(index.retired, 1);
        assert_matches(&index, &[0, 7]);
        index.discard_prefix(2);
        assert_matches(&index, &[]);
        assert_eq!(index.retired, 0);
        assert!(index.heights.is_empty());
        assert!(index.tree.is_empty());
        assert!(!index.set(0, 7));
        index.push(0);
        index.push(3);
        assert_matches(&index, &[0, 3]);
    }

    #[test]
    fn mixed_updates_append_and_pruning_match_retained_heights() {
        let mut index = RowHeights::default();
        let mut heights = Vec::new();
        let mut compactions = 0;
        for step in 0..2_000 {
            if step % 97 == 96 {
                index.discard_prefix(heights.len());
                heights.clear();
            } else {
                match step % 8 {
                    0..=2 => {
                        let height = step * 17 % 13;
                        index.push(height);
                        heights.push(height);
                    }
                    3 | 4 if !heights.is_empty() => {
                        let block = step * 11 % heights.len();
                        let height = step * 7 % 9;
                        assert!(index.set(block, height));
                        heights[block] = height;
                    }
                    5 => {
                        let count = heights.len() / 3;
                        index.discard_prefix(count);
                        drop(heights.drain(..count));
                        if count != 0 && index.retired == 0 && index.len() != 0 {
                            compactions += 1;
                        }
                    }
                    6 => {
                        index.push(0);
                        heights.push(0);
                    }
                    _ => {
                        assert!(!index.set(heights.len(), 1));
                    }
                }
            }
            assert_matches(&index, &heights);
        }
        assert!(compactions > 10);
    }

    #[test]
    fn large_block_rows_are_not_narrowed_to_u16() {
        let mut index = index_for(&[0, 200_000, 0, 3]);
        assert_eq!(index.locate(65_536), Some((1, 65_536)));
        assert_eq!(index.locate(199_999), Some((1, 199_999)));
        assert_eq!(index.locate(200_000), Some((3, 0)));
        assert!(index.set(1, 300_000));
        assert_eq!(index.total(), 300_003);
        assert_eq!(index.locate(299_999), Some((1, 299_999)));
        index.discard_prefix(2);
        assert_eq!(index.locate(0), Some((1, 0)));
        assert_eq!(index.locate(2), Some((1, 2)));
        assert_eq!(index.locate(3), None);
    }

    #[test]
    fn deep_seek_visits_logarithmic_tree_nodes_before_and_after_pruning() {
        const BLOCKS: usize = 131_073;
        let mut index = RowHeights::default();
        for block in 0..BLOCKS {
            index.push(if block.is_multiple_of(4) { 0 } else { 3 });
        }
        assert_eq!(index.len(), BLOCKS);
        assert_eq!(index.total(), 294_912);
        for (row, location) in [(0, (1, 0)), (147_456, (65_537, 0)), (294_911, (131_071, 2))] {
            assert_eq!(index.locate(row), Some(location));
            assert!(index.locate_steps.get() <= 18);
        }
        index.discard_prefix(50_000);
        assert_eq!(index.retired, 50_000);
        assert_eq!(index.total(), 182_412);
        assert_eq!(index.locate(0), Some((1, 0)));
        assert!(index.locate_steps.get() <= 18);
        assert_eq!(index.locate(182_411), Some((81_071, 2)));
        assert!(index.locate_steps.get() <= 18);
        index.discard_prefix(40_000);
        assert_eq!(index.retired, 0);
        assert_eq!(index.len(), 41_073);
        assert_eq!(index.total(), 92_412);
        assert_eq!(index.locate(0), Some((1, 0)));
        assert!(index.locate_steps.get() <= 16);
        assert_eq!(index.locate(92_411), Some((41_071, 2)));
        assert!(index.locate_steps.get() <= 16);
    }
}

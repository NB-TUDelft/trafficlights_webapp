// SPDX-License-Identifier: AGPL-3.0-or-later
//! The table board of one session: pure state, no I/O.
//!
//! Only non-green in-range tables are stored (green is the default), so a
//! session's memory does not grow with its range, and out-of-range
//! ("unlisted") stations are capped at [`MAX_UNLISTED_TABLES`].

use std::collections::BTreeMap;

use trafficlights_proto::{Color, ColorCounts, TableEntry, TableRange, UnlistedTable};

/// The most out-of-range stations remembered per session; the station
/// heard longest ago is forgotten first.
pub const MAX_UNLISTED_TABLES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lit {
    color: Color,
    red_since_ms: Option<i64>,
}

/// The colors of one session's tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    range: TableRange,
    lit: BTreeMap<u16, Lit>,
    unlisted: BTreeMap<u16, UnlistedTable>,
}

impl Board {
    /// An all-green board over `range` (already validated).
    pub fn new(range: TableRange) -> Self {
        Self {
            range,
            lit: BTreeMap::new(),
            unlisted: BTreeMap::new(),
        }
    }

    /// The displayed inclusive range.
    pub fn range(&self) -> TableRange {
        self.range
    }

    /// The current color of an in-range table.
    pub fn color(&self, table: u16) -> Color {
        self.lit.get(&table).map_or(Color::Green, |lit| lit.color)
    }

    /// Set an in-range table's color at `now_ms`. Returns the previous
    /// color when it changed, `None` when it was already `color` (or the
    /// table is out of range). A table entering red records `now_ms`; one
    /// already red keeps its original anchor.
    pub fn set_color(&mut self, table: u16, color: Color, now_ms: i64) -> Option<Color> {
        if !self.range.contains(table) {
            return None;
        }
        let prev = self.color(table);
        if prev == color {
            return None;
        }
        match color {
            Color::Green => {
                self.lit.remove(&table);
            }
            Color::Orange => {
                self.lit.insert(
                    table,
                    Lit {
                        color,
                        red_since_ms: None,
                    },
                );
            }
            Color::Red => {
                self.lit.insert(
                    table,
                    Lit {
                        color,
                        red_since_ms: Some(now_ms),
                    },
                );
            }
        }
        Some(prev)
    }

    /// When an in-range table turned red, while it is red.
    pub fn red_since_ms(&self, table: u16) -> Option<i64> {
        self.lit.get(&table).and_then(|lit| lit.red_since_ms)
    }

    /// Turn every in-range table green, returning `(table, previous color)`
    /// for each one that was not green, in table order.
    pub fn reset_to_green(&mut self) -> Vec<(u16, Color)> {
        let lit = std::mem::take(&mut self.lit);
        lit.into_iter()
            .map(|(table, lit)| (table, lit.color))
            .collect()
    }

    /// Replace the range with an all-green board over `range`, forgetting
    /// unlisted stations that now fall inside it. Returns the previous
    /// non-green tables (as [`Board::reset_to_green`]) and whether any
    /// unlisted station was pruned.
    pub fn change_range(&mut self, range: TableRange) -> (Vec<(u16, Color)>, bool) {
        let reset = self.reset_to_green();
        self.range = range;
        let before = self.unlisted.len();
        self.unlisted.retain(|table, _| !range.contains(*table));
        (reset, self.unlisted.len() != before)
    }

    /// Record a report from an out-of-range station. Returns `Some(prev)`
    /// when the report is worth logging (first sight, with `prev = None`,
    /// or a color change) and `None` for a repeat of the same color.
    pub fn record_unlisted(
        &mut self,
        table: u16,
        color: Color,
        now_ms: i64,
    ) -> Option<Option<Color>> {
        if let Some(entry) = self.unlisted.get_mut(&table) {
            let prev = entry.color;
            entry.color = color;
            entry.last_seen_ms = now_ms;
            return (prev != color).then_some(Some(prev));
        }
        if self.unlisted.len() >= MAX_UNLISTED_TABLES {
            let stalest = self
                .unlisted
                .values()
                .min_by_key(|entry| entry.last_seen_ms)
                .map(|entry| entry.table);
            if let Some(stalest) = stalest {
                self.unlisted.remove(&stalest);
            }
        }
        self.unlisted.insert(
            table,
            UnlistedTable {
                table,
                color,
                last_seen_ms: now_ms,
            },
        );
        Some(None)
    }

    /// Every remembered out-of-range station, in table order.
    pub fn unlisted_tables(&self) -> Vec<UnlistedTable> {
        self.unlisted.values().copied().collect()
    }

    /// In-range color counts.
    pub fn counts(&self) -> ColorCounts {
        let total = u32::try_from(self.range.len()).unwrap_or(u32::MAX);
        let mut counts = ColorCounts::default();
        for lit in self.lit.values() {
            match lit.color {
                Color::Orange => counts.orange += 1,
                Color::Red => counts.red += 1,
                Color::Green => {}
            }
        }
        counts.green = total.saturating_sub(counts.orange + counts.red);
        counts
    }

    /// One entry per in-range table, in table order.
    pub fn entries(&self) -> Vec<TableEntry> {
        self.range
            .iter()
            .map(|table| TableEntry {
                table,
                color: self.color(table),
                red_since_ms: self.red_since_ms(table),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use trafficlights_proto::{Color, ColorCounts, TableRange};

    use super::{Board, MAX_UNLISTED_TABLES};

    fn board(start: u16, end: u16) -> Board {
        match TableRange::new(start, end) {
            Ok(range) => Board::new(range),
            Err(error) => panic!("bad test range: {error}"),
        }
    }

    #[test]
    fn a_new_board_is_all_green() {
        let board = board(1, 60);
        assert_eq!(
            board.counts(),
            ColorCounts {
                green: 60,
                orange: 0,
                red: 0
            }
        );
        let entries = board.entries();
        assert_eq!(entries.len(), 60);
        assert!(entries
            .iter()
            .all(|e| e.color == Color::Green && e.red_since_ms.is_none()));
        assert_eq!(entries.first().map(|e| e.table), Some(1));
        assert_eq!(entries.last().map(|e| e.table), Some(60));
    }

    #[test]
    fn red_since_is_set_on_entry_kept_while_red_and_cleared_on_exit() {
        let mut board = board(1, 10);
        assert_eq!(board.set_color(4, Color::Red, 100), Some(Color::Green));
        assert_eq!(board.red_since_ms(4), Some(100));
        // Repeating red is no change and keeps the first anchor.
        assert_eq!(board.set_color(4, Color::Red, 200), None);
        assert_eq!(board.red_since_ms(4), Some(100));
        assert_eq!(board.set_color(4, Color::Orange, 300), Some(Color::Red));
        assert_eq!(board.red_since_ms(4), None);
        assert_eq!(board.set_color(4, Color::Red, 400), Some(Color::Orange));
        assert_eq!(board.red_since_ms(4), Some(400));
        assert_eq!(board.set_color(4, Color::Green, 500), Some(Color::Red));
        assert_eq!(board.red_since_ms(4), None);
        assert_eq!(board.set_color(4, Color::Green, 600), None);
    }

    #[test]
    fn out_of_range_tables_never_change_the_board() {
        let mut board = board(5, 10);
        assert_eq!(board.set_color(4, Color::Red, 1), None);
        assert_eq!(board.set_color(11, Color::Red, 1), None);
        assert_eq!(board.counts().green, 6);
    }

    #[test]
    fn counts_track_colors() {
        let mut board = board(1, 5);
        board.set_color(1, Color::Red, 1);
        board.set_color(2, Color::Orange, 1);
        board.set_color(3, Color::Orange, 1);
        assert_eq!(
            board.counts(),
            ColorCounts {
                green: 2,
                orange: 2,
                red: 1
            }
        );
    }

    #[test]
    fn reset_reports_only_non_green_tables() {
        let mut board = board(1, 5);
        board.set_color(3, Color::Red, 1);
        board.set_color(1, Color::Orange, 1);
        assert_eq!(
            board.reset_to_green(),
            vec![(1, Color::Orange), (3, Color::Red)]
        );
        assert_eq!(board.counts().green, 5);
        assert!(board.reset_to_green().is_empty());
    }

    #[test]
    fn unlisted_tables_log_first_sight_and_changes_only() {
        let mut board = board(1, 5);
        assert_eq!(board.record_unlisted(0, Color::Orange, 10), Some(None));
        assert_eq!(board.record_unlisted(0, Color::Orange, 20), None);
        assert_eq!(
            board.record_unlisted(0, Color::Red, 30),
            Some(Some(Color::Orange))
        );
        let tables = board.unlisted_tables();
        assert_eq!(tables.len(), 1);
        assert_eq!(
            tables.first().map(|t| (t.color, t.last_seen_ms)),
            Some((Color::Red, 30))
        );
    }

    #[test]
    fn unlisted_tables_are_capped_forgetting_the_stalest() {
        let mut board = board(1, 1);
        for (index, table) in (100..100 + MAX_UNLISTED_TABLES as u16).enumerate() {
            board.record_unlisted(table, Color::Red, index as i64);
        }
        assert_eq!(board.unlisted_tables().len(), MAX_UNLISTED_TABLES);
        // Table 100 was heard first, so it goes when a new one arrives.
        board.record_unlisted(5000, Color::Orange, 1_000);
        let tables = board.unlisted_tables();
        assert_eq!(tables.len(), MAX_UNLISTED_TABLES);
        assert!(tables.iter().all(|t| t.table != 100));
        assert!(tables.iter().any(|t| t.table == 5000));
    }

    #[test]
    fn changing_the_range_resets_and_prunes_unlisted_tables_inside_it() {
        let mut board = board(1, 5);
        board.set_color(2, Color::Red, 1);
        board.record_unlisted(8, Color::Red, 1);
        board.record_unlisted(0, Color::Red, 1);
        let next = TableRange::new(1, 10).unwrap_or_default();
        let (reset, pruned) = board.change_range(next);
        assert_eq!(reset, vec![(2, Color::Red)]);
        assert!(pruned);
        assert_eq!(board.range(), next);
        assert_eq!(board.counts().green, 10);
        let tables: Vec<u16> = board.unlisted_tables().iter().map(|t| t.table).collect();
        assert_eq!(tables, vec![0]);
        let (_, pruned_again) = board.change_range(next);
        assert!(!pruned_again);
    }
}

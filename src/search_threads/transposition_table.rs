use crate::move_generation::mov::Move;
use crate::search_threads::is_mate_score;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
struct Entry {
    zobrist_key_xor_data: AtomicU64,
    data: AtomicU64,
}

fn score_to_tt(score: i16, search_plies: u8) -> i16 {
    if is_mate_score(score) {
        score + (search_plies as i16) * score.signum()
    } else {
        score
    }
}

fn tt_to_score(score: i16, search_plies: u8) -> i16 {
    if is_mate_score(score) {
        score - (search_plies as i16) * score.signum()
    } else {
        score
    }
}

impl Entry {
    fn data_from_entry_result(entry_result: EntryResult, search_plies: u8) -> u64 {
        entry_result.best_move.data as u64
            | ((score_to_tt(entry_result.search_score, search_plies) as u16 as u64) << 16)
            | ((score_to_tt(entry_result.static_score, search_plies) as u16 as u64) << 32)
            | (entry_result.depth as u64) << (16 + 32)
            | (entry_result.node_type as u64) << (16 + 32 + 8)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct EntryResult {
    pub search_score: i16,
    pub static_score: i16,
    pub depth: u8,
    pub node_type: i8,
    pub best_move: Move,
}

impl EntryResult {
    pub fn new(
        depth: u8,
        search_score: i16,
        static_score: i16,
        node_type: i8,
        best_move: Move,
    ) -> Self {
        Self {
            search_score,
            static_score,
            depth,
            node_type,
            best_move,
        }
    }

    fn from_u64(data: u64, search_plies: u8) -> Self {
        Self {
            best_move: Move { data: data as u16 },
            search_score: tt_to_score((data >> 16) as i16, search_plies),
            static_score: tt_to_score((data >> 32) as i16, search_plies),
            depth: (data >> 48) as u8,
            node_type: (data >> 56) as i8,
        }
    }
}

pub struct TranspositionTable {
    size: usize,
    table: Vec<Entry>,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let size = size_mb * 1024 * 1024 / size_of::<Entry>();
        let mut table = Vec::with_capacity(size);
        for _ in 0..size {
            table.push(Entry {
                zobrist_key_xor_data: AtomicU64::new(0),
                data: AtomicU64::new(0),
            });
        }
        Self { size, table }
    }

    pub fn get(&self, zobrist_key: u64, search_plies: u8) -> Option<EntryResult> {
        let index = zobrist_key as usize % self.size;
        let entry = &self.table[index];
        let key_xor_data = entry.zobrist_key_xor_data.load(Ordering::Acquire);
        let data = entry.data.load(Ordering::Acquire);
        if key_xor_data == zobrist_key ^ data {
            Some(EntryResult::from_u64(data, search_plies))
        } else {
            None
        }
    }

    pub fn insert(&self, zobrist_key: u64, entry_result: EntryResult, search_plies: u8) {
        let index = zobrist_key as usize % self.size;
        let entry = &self.table[index];
        let data = Entry::data_from_entry_result(entry_result, search_plies);

        // Keep a strictly deeper entry for the same position. An equal-depth entry is replaced so
        // that a PVS re-search can overwrite the null-window bound it just produced with the
        // exact score (and so fresher bounds win over stale ones).
        let current_data = entry.data.load(Ordering::Relaxed);
        if entry.zobrist_key_xor_data.load(Ordering::Relaxed) == zobrist_key ^ current_data
            && EntryResult::from_u64(current_data, search_plies).depth > entry_result.depth
        {
            return;
        }

        entry.data.store(data, Ordering::Release);
        entry
            .zobrist_key_xor_data
            .store(zobrist_key ^ data, Ordering::Release);
    }

    pub fn clear(&self) {
        for entry in &self.table {
            entry.zobrist_key_xor_data.store(0, Ordering::Relaxed);
            entry.data.store(0, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::move_generation::mov::Move;
    use crate::search_threads::searcher::{MAX_PLIES, Search};
    use crate::search_threads::transposition_table::{EntryResult, TranspositionTable};

    #[test]
    fn test_load_unload() {
        let table = TranspositionTable::new(1);
        let entry_result = EntryResult {
            best_move: Move::left_en_passant(10),
            search_score: 10,
            static_score: 5,
            depth: 4,
            node_type: 0,
        };
        table.insert(0, entry_result, 0);
        assert_eq!(table.get(0, 0), Some(entry_result));
        let entry_result = EntryResult {
            best_move: Move::left_en_passant(10),
            search_score: -10,
            static_score: -5,
            depth: 4,
            node_type: 0,
        };
        table.insert(0, entry_result, 0);
        assert_eq!(table.get(0, 0), Some(entry_result));
    }

    #[test]
    fn deeper_entry_is_not_replaced_by_shallower_entry() {
        let table = TranspositionTable::new(1);
        let deep = EntryResult::new(8, 100, 100, 0, Move::left_en_passant(10));
        let shallow = EntryResult::new(4, -100, -100, 1, Move::null());

        table.insert(0, deep, 0);
        table.insert(0, shallow, 0);

        assert_eq!(table.get(0, 0), Some(deep));
    }

    #[test]
    fn mate_scores_round_trip_at_max_search_ply() {
        let table = TranspositionTable::new(1);
        for sign in [1, -1] {
            let score = (Search::IMMEDIATE_MATE_SCORE - 1) * sign;
            let entry = EntryResult::new(1, score, 0, 0, Move::null());
            table.insert(0, entry, MAX_PLIES);
            assert_eq!(table.get(0, MAX_PLIES), Some(entry));
        }
    }

    #[test]
    fn equal_depth_entry_replaces_existing_entry() {
        let table = TranspositionTable::new(1);
        let bound = EntryResult::new(6, 50, 50, 1, Move::null());
        let exact = EntryResult::new(6, 42, 50, 0, Move::left_en_passant(10));

        table.insert(0, bound, 0);
        table.insert(0, exact, 0);

        assert_eq!(table.get(0, 0), Some(exact));
    }
}

use crate::move_generation::mov::Move;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
struct Entry {
    zobrist_key_xor_data: AtomicU64,
    data: AtomicU64,
}

impl Entry {
    fn data_from_entry_result(entry_result: EntryResult) -> u64 {
        entry_result.best_move.data as u64
            | (((entry_result.score as u64) & 0xFFFFFFFF) << 16)
            | (entry_result.depth as u64) << (16 + 32)
            | (entry_result.node_type as u64) << (16 + 32 + 8)
    }
}

impl Entry {
    pub fn new() -> Self {
        Self {
            zobrist_key_xor_data: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct EntryResult {
    pub score: i32,
    pub depth: u8,
    pub node_type: i8,
    pub best_move: Move,
}

impl EntryResult {
    pub fn new(depth: u8, score: i32, node_type: i8, best_move: Move) -> Self {
        Self {
            score,
            depth,
            node_type,
            best_move,
        }
    }

    fn from_u64(data: u64) -> Self {
        Self {
            best_move: Move { data: data as u16 },
            score: (data >> 16) as i32,
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

    pub fn get(&self, zobrist_key: u64) -> Option<EntryResult> {
        let index = zobrist_key as usize % self.size;
        let entry = &self.table[index];
        let data = entry.data.load(Ordering::Relaxed);
        if entry.zobrist_key_xor_data.load(Ordering::Relaxed) == zobrist_key ^ data {
            Some(EntryResult::from_u64(data))
        } else {
            None
        }
    }

    pub fn insert(&self, zobrist_key: u64, entry_result: EntryResult) {
        let index = zobrist_key as usize % self.size;
        let entry = &self.table[index];
        let data = Entry::data_from_entry_result(entry_result);
        entry
            .zobrist_key_xor_data
            .store(zobrist_key ^ data, Ordering::Relaxed);
        entry.data.store(data, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use crate::move_generation::mov::Move;
    use crate::search::transposition_table::{EntryResult, TranspositionTable};

    #[test]
    fn test_load_unload() {
        let entry_result = EntryResult {
            best_move: Move::left_en_passant(10),
            score: 10,
            depth: 4,
            node_type: 0,
        };
        let mut table = TranspositionTable::new(1);
        table.insert(0, entry_result);
        assert_eq!(table.get(0), Some(entry_result));
    }
}

use crate::move_generation::mov::Move;

#[derive(Copy, Clone)]
struct Entry {
    zobrist_key: u64,
    node_type: i8,
    result: Option<EntryResult>,
}

#[derive(Copy, Clone)]
pub struct EntryResult {
    pub score: i64,
    pub depth: u16,
    pub node_type: i8,
    pub best_move: Move,
}

pub struct TranspositionTable {
    size: usize,
    table: Vec<Entry>,
}

impl TranspositionTable {
    pub fn new(size_mb: usize) -> Self {
        let size = size_mb * 1024 * 1024 / std::mem::size_of::<Entry>();
        Self {
            size,
            table: vec![
                Entry {
                    zobrist_key: 0,
                    node_type: 0,
                    result: None,
                };
                size
            ],
        }
    }

    pub fn get(&self, zobrist_key: u64) -> Option<EntryResult> {
        let index = zobrist_key as usize % self.size;
        let entry = &self.table[index];
        if entry.zobrist_key == zobrist_key {
            entry.result
        } else {
            None
        }
    }

    pub fn insert(&mut self, zobrist_key: u64, entry_result: EntryResult) {
        let index = zobrist_key as usize % self.size;
        let entry = &mut self.table[index];
        if entry.result.is_none() || entry.result.unwrap().depth < entry_result.depth {
            entry.zobrist_key = zobrist_key;
            entry.node_type = entry_result.node_type;
            entry.result = Some(entry_result);
        }
    }
}

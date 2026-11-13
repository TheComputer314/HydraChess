mod evaluation;
mod searcher;
mod transposition_table;

use super::move_generation;
use super::move_generation::Board;
use crate::move_generation::mov::Move;
use crate::search_threads::searcher::Search;
use crate::search_threads::transposition_table::TranspositionTable;
use std::io::Write;
use std::io::stdout;
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

struct SearchResult {
    depth: u8,
    score: i32,
    best_move: Move,
    // pv: Vec<Move>, // TODO
}

struct SharedState {
    time_left: AtomicU64,
    increment: AtomicU64,
    start_barrier: Barrier,
    end_barrier: Barrier,
    root_board: Mutex<Board>,
    // Transposition table is lock-free to allow TT improvements in one thread to improve search_threads in another
    transposition_table: TranspositionTable,
    results: Vec<Mutex<SearchResult>>,
    nodes_searched: AtomicU64,
}

pub struct Signals {
    pub stop_signal: Arc<AtomicBool>,
    pub quit_signal: Arc<AtomicBool>,
}

pub struct SearchThreads {
    shared_state: Arc<SharedState>,
    moves_played: u32,
}

// TODO: Don't hardcode
const SEARCH_THREADS: usize = 4;

impl SearchThreads {
    pub fn new(signals: Signals) -> Self {
        // Warm up the LazyLock'd lookups
        move_generation::move_lookup::initialize();

        // Coordinator thread is +1
        let total_threads = SEARCH_THREADS + 1;

        let mut results = Vec::with_capacity(SEARCH_THREADS);
        for _ in 0..SEARCH_THREADS {
            results.push(Mutex::new(SearchResult {
                depth: 0,
                score: 0,
                best_move: Move::null(),
            }));
        }

        let shared_state = Arc::new(SharedState {
            time_left: AtomicU64::new(0),
            increment: AtomicU64::new(0),
            start_barrier: Barrier::new(total_threads),
            end_barrier: Barrier::new(total_threads),
            root_board: Mutex::new(Board::starting()),
            transposition_table: TranspositionTable::new(64),
            results,
            nodes_searched: AtomicU64::new(0),
        });

        for i in 0..SEARCH_THREADS {
            let state = Arc::clone(&shared_state);
            let stop_signal = Arc::clone(&signals.stop_signal);
            let quit_signal = Arc::clone(&signals.quit_signal);
            thread::spawn(move || {
                let mut search = Search::new();
                loop {
                    state.start_barrier.wait();
                    sleep(Duration::from_millis((i as u64) * 2));
                    if quit_signal.load(Ordering::Acquire) {
                        break;
                    }
                    let mut board = state.root_board.lock().unwrap().clone();

                    let result = search.search(
                        &mut board,
                        &state.transposition_table,
                        &stop_signal,
                        state.time_left.load(Relaxed),
                        state.increment.load(Relaxed),
                        &state.nodes_searched,
                        i,
                    );

                    *state.results[i].lock().unwrap() = result;
                    if quit_signal.load(Ordering::Acquire) {
                        break;
                    }
                    state.end_barrier.wait();
                }
            });
        }

        Self {
            shared_state,
            moves_played: 0,
        }
    }

    pub fn new_game(&mut self) {
        *self.shared_state.root_board.lock().unwrap() = Board::starting();
        self.shared_state.transposition_table.clear();
        self.moves_played = 0;
    }

    pub fn position(&mut self, string: &str) {
        let parts: Vec<&str> = string.split_whitespace().collect();

        if parts[0] == "startpos" {
            *self.shared_state.root_board.lock().unwrap() = Board::starting();
            self.moves_played = 0;
            if parts.len() > 1 && parts[1] == "moves" {
                self.position_moves(&parts[2..]);
            }
        } else if parts[0] == "fen" {
            *self.shared_state.root_board.lock().unwrap() = Board::from_fen(&parts[1..7].join(" "));
            self.moves_played = 0;
            if parts.len() > 7 && parts[7] == "moves" {
                self.position_moves(&parts[8..]);
            }
        }
    }

    fn position_moves(&mut self, moves: &[&str]) {
        let mut board = self.shared_state.root_board.lock().unwrap();
        for mov in moves {
            board.move_from_uci_lan(mov);
            self.moves_played += 1;
        }
    }

    pub fn debug(&self) {
        println!("Board: {}", self.shared_state.root_board.lock().unwrap());
        println!("Moves played: {}", self.moves_played);
    }

    pub fn go(
        &mut self,
        white_time_left_ms: u64,
        white_increment: u64,
        black_time_left_ms: u64,
        black_increment: u64,
    ) {
        let (time_left_ms, increment) =
            if self.shared_state.root_board.lock().unwrap().white_to_move() {
                (white_time_left_ms, white_increment)
            } else {
                (black_time_left_ms, black_increment)
            };
        self.shared_state.nodes_searched.store(0, Relaxed);
        self.shared_state.time_left.store(time_left_ms, Relaxed);
        self.shared_state.increment.store(increment, Relaxed);
        self.shared_state.start_barrier.wait();
        self.shared_state.end_barrier.wait();

        let best_result = self
            .shared_state
            .results
            .iter()
            .map(|m| m.lock().unwrap())
            .max_by_key(|r| (r.depth, r.score))
            .unwrap()
            .best_move;
        let mut out = stdout().lock();
        writeln!(out, "bestmove {}", best_result).unwrap();
        out.flush().unwrap();
        self.shared_state
            .root_board
            .lock()
            .unwrap()
            .make_move(&best_result);
    }
}

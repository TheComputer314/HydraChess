mod evaluation;
mod searcher;
mod transposition_table;

use super::move_generation;
use super::move_generation::Board;
use crate::move_generation::mov::Move;
use crate::search_threads::searcher::{Search, SearchState};
use crate::search_threads::transposition_table::TranspositionTable;
use minstant::Instant;
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
    best_ply: u8,
    score: i32,
    best_move: Move,
    // pv: Vec<Move>, // TODO
}

struct SharedState {
    time_left: AtomicU64,
    increment: AtomicU64,
    start_barrier: Barrier,
    end_barrier: Barrier,
    // Set only by the coordinator when workers must leave for a resize or shutdown.
    // This is deliberately separate from UCI signals, which are reset asynchronously
    // by the input thread and can therefore change while a resize is in progress.
    quit: AtomicBool,
    root_board: Mutex<Board>,
    // Transposition table is lock-free to allow TT improvements in one thread to improve search_threads in another
    transposition_table: TranspositionTable,
    results: Vec<Mutex<SearchResult>>,
    nodes_searched: AtomicU64,
}

pub struct Signals {
    pub stop_signal: Arc<AtomicBool>,
    pub ready_signal: Arc<AtomicBool>,
    pub main_thread: thread::Thread,
}

pub struct SearchThreads {
    shared_state: Arc<SharedState>,
    worker_handles: Vec<thread::JoinHandle<()>>,
    stop_signal: Arc<AtomicBool>,
    moves_played: u32,
}

pub(crate) const DEFAULT_SEARCH_THREADS: usize = 1;

impl SearchThreads {
    pub fn new(signals: &Signals) -> Self {
        signals.ready_signal.store(false, Ordering::Release);
        // Warm up the LazyLock'd lookups
        move_generation::move_lookup::initialize();

        let shared_state = Arc::new(Self::build_state(DEFAULT_SEARCH_THREADS));
        let mut new = Self {
            shared_state,
            worker_handles: Vec::new(),
            stop_signal: Arc::clone(&signals.stop_signal),
            moves_played: 0,
        };

        new.spawn_threads(DEFAULT_SEARCH_THREADS);
        signals.ready_signal.store(true, Ordering::Release);
        new
    }

    /// Set the number of search threads.
    pub fn set_threads(&mut self, threads: usize) {
        // Tell existing workers to quit, then release them from the start barrier.
        self.shared_state.quit.store(true, Ordering::Release);
        self.shared_state.start_barrier.wait();
        // Join them: this drops their Arc clones, restoring unique ownership so we can resize.
        for handle in self.worker_handles.drain(..) {
            handle.join().unwrap();
        }

        let state_mut = Arc::get_mut(&mut self.shared_state)
            .expect("Search threads joined; no other Arc clones should remain");
        state_mut.quit.store(false, Ordering::Release);
        state_mut.start_barrier = Barrier::new(threads + 1);
        state_mut.end_barrier = Barrier::new(threads + 1);
        state_mut.results = Self::empty_results(threads);

        self.spawn_threads(threads);
    }

    fn build_state(search_threads: usize) -> SharedState {
        SharedState {
            time_left: AtomicU64::new(0),
            increment: AtomicU64::new(0),
            // Coordinator thread is +1
            start_barrier: Barrier::new(search_threads + 1),
            end_barrier: Barrier::new(search_threads + 1),
            quit: AtomicBool::new(false),
            root_board: Mutex::new(Board::starting()),
            transposition_table: TranspositionTable::new(256),
            results: Self::empty_results(search_threads),
            nodes_searched: AtomicU64::new(0),
        }
    }

    fn empty_results(search_threads: usize) -> Vec<Mutex<SearchResult>> {
        (0..search_threads)
            .map(|_| {
                Mutex::new(SearchResult {
                    depth: 0,
                    best_ply: 0,
                    score: 0,
                    best_move: Move::null(),
                })
            })
            .collect()
    }

    fn spawn_threads(&mut self, search_threads: usize) {
        let mut handles = Vec::with_capacity(search_threads);
        for i in 0..search_threads {
            let state = Arc::clone(&self.shared_state);
            let stop_signal = Arc::clone(&self.stop_signal);
            handles.push(thread::spawn(move || {
                let mut search = Search::new();
                loop {
                    state.start_barrier.wait();
                    if state.quit.load(Ordering::Acquire) {
                        break;
                    }
                    sleep(Duration::from_millis((i as u64) * 2));
                    let mut board = state.root_board.lock().unwrap().clone();

                    let result = search.search(
                        &mut board,
                        &SearchState {
                            transposition_table: &state.transposition_table,
                            stop_signal: &stop_signal,
                            nodes_searched: &state.nodes_searched,
                            time_left_ms: state.time_left.load(Relaxed),
                            increment: state.increment.load(Relaxed),
                            timer: Instant::now(),
                        },
                        i,
                    );

                    *state.results[i].lock().unwrap() = result;
                    state.end_barrier.wait();
                    if state.quit.load(Ordering::Acquire) {
                        break;
                    }
                }
                println!("Thread {} finished", i);
            }));
        }

        println!("Successfully spawned {} threads", search_threads);
        self.worker_handles = handles;
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
        let time = Instant::now();
        self.shared_state.start_barrier.wait();
        self.shared_state.end_barrier.wait();

        let best_result = self
            .shared_state
            .results
            .iter()
            .map(|m| m.lock().unwrap())
            .max_by_key(|r| (r.depth, r.score))
            .unwrap();
        print_info(
            best_result.depth,
            best_result.best_ply,
            best_result.score,
            time.elapsed().as_secs_f64(),
            self.shared_state.nodes_searched.load(Relaxed),
        );
        let mut out = stdout().lock();
        writeln!(out, "bestmove {}", best_result.best_move).unwrap();
        out.flush().unwrap();
        self.shared_state
            .root_board
            .lock()
            .unwrap()
            .make_move(&best_result.best_move);
    }
}

impl Drop for SearchThreads {
    fn drop(&mut self) {
        // Signal workers to stop, then release them from the start barrier and wait for them.
        self.shared_state.quit.store(true, Ordering::Release);
        self.shared_state.start_barrier.wait();
        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn is_mate_score(score: i32) -> bool {
    score.abs() > Search::IMMEDIATE_MATE_SCORE - 100000
}

fn print_info(depth: u8, depth_best_ply: u8, score: i32, time: f64, nodes: u64) {
    let mut out = stdout().lock();
    if is_mate_score(score) {
        let mate_in_ply = Search::IMMEDIATE_MATE_SCORE - score.abs();
        writeln!(
            out,
            "info depth {} seldepth {} score mate {} time {} nodes {} nps {}",
            depth + 1,
            depth_best_ply + 1,
            (mate_in_ply / 2 + 1) * score.signum(),
            (time * 1000.0).round() as u64,
            nodes,
            (nodes as f64 / time).round() as u64
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "info depth {} seldepth {} score cp {} time {} nodes {} nps {}",
            depth + 1,
            depth_best_ply + 1,
            score,
            (time * 1000.0).round() as u64,
            nodes,
            (nodes as f64 / time).round() as u64
        )
        .unwrap();
    }
    out.flush().unwrap();
}

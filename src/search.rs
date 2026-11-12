mod evaluation;
mod transposition_table;

use super::move_generation;
use super::move_generation::Board;
use crate::move_generation::mov::Move;
use crate::move_generation::mov::piece_type::PAWN;
use crate::search::transposition_table::{EntryResult, TranspositionTable};
use minstant::Instant;
use std::cmp::Reverse;
use std::io::stdout;
use std::io::{Stdout, Write};
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::thread::JoinHandle;

struct SearchResult {
    // TODO
    // depth: usize,
    // score: i32,
    best_move: Move,
    // pv: Vec<Move>,
}

struct SharedState {
    time_left: AtomicU64,
    increment: AtomicU64,
    start_barrier: Barrier,
    end_barrier: Barrier,
    root_board: Mutex<Board>,
    // Transposition table is lock-free to allow TT improvements in one thread to improve search in another
    transposition_table: TranspositionTable,
    results: Vec<Mutex<SearchResult>>,
}

pub struct Signals {
    pub stop_signal: Arc<AtomicBool>,
    pub quit_signal: Arc<AtomicBool>,
}

pub struct SearchThreads {
    shared_state: Arc<SharedState>,
    handles: Vec<JoinHandle<()>>,
    moves_played: u32,
}

const SEARCH_THREADS: usize = 4;

impl SearchThreads {
    pub fn new(signals: Signals) -> Self {
        // Coordinator thread is +1
        let total_threads = SEARCH_THREADS + 1;

        let shared_state = Arc::new(SharedState {
            time_left: AtomicU64::new(0),
            increment: AtomicU64::new(0),
            start_barrier: Barrier::new(total_threads),
            end_barrier: Barrier::new(total_threads),
            root_board: Mutex::new(Board::starting()),
            transposition_table: TranspositionTable::new(20),
            results: Vec::with_capacity(SEARCH_THREADS),
        });

        let mut handles = Vec::with_capacity(SEARCH_THREADS);

        for i in 0..SEARCH_THREADS {
            let state = Arc::clone(&shared_state);
            let stop_signal = Arc::clone(&signals.stop_signal);
            let quit_signal = Arc::clone(&signals.quit_signal);
            handles.push(thread::spawn(move || {
                let search = Search {};
                loop {
                    state.start_barrier.wait();
                    if quit_signal.load(Ordering::Acquire) {
                        break;
                    }
                    let mut board = state.root_board.lock().unwrap().clone();

                    let search = search.search(
                        &mut board,
                        &state.transposition_table,
                        &stop_signal,
                        state.time_left.load(Relaxed),
                        state.increment.load(Relaxed),
                    );

                    state.results[i].lock().unwrap().best_move = search;
                    if quit_signal.load(Ordering::Acquire) {
                        break;
                    }
                    state.end_barrier.wait();
                }
            }));
        }

        Self {
            shared_state,
            handles,
            moves_played: 0,
        }
    }

    pub fn new_game(&mut self) {
        *self.shared_state.root_board.lock().unwrap() = Board::starting();
        Arc::get_mut(&mut self.shared_state)
            .unwrap()
            .transposition_table = TranspositionTable::new(20);
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
        self.shared_state.time_left.store(time_left_ms, Relaxed);
        self.shared_state.increment.store(increment, Relaxed);
    }
}

struct Search {}

impl Search {
    fn search(
        &self,
        board: &mut Board,
        transposition_table: &TranspositionTable,
        stop_signal: &AtomicBool,
        time_left_ms: u64,
        increment: u64,
    ) -> Move {
        let legal_moves = move_generation::legal_moves(board);
        if legal_moves.is_empty() {
            panic!(
                "Attempted to search and eval board {} when we're checkmated",
                board
            );
        }
        if legal_moves.len() == 1 {
            return legal_moves[0];
        }

        let color = if board.white_to_move() { 1 } else { -1 };
        let transposition_table_result = transposition_table.get(board.zobrist_hash);
        let mut tt_move = None;
        if let Some(result) = transposition_table_result {
            tt_move = Some(result.best_move);
        }
        let legal_moves = self.sort_moves(board, &legal_moves, tt_move, color);

        let mut best_move = &legal_moves[0];
        let max_depth = 16;
        let timer = Instant::now();
        let time_ok = || {
            let elapsed = timer.elapsed().as_millis() as u64;
            !stop_signal.load(Relaxed)
                && elapsed < (time_left_ms / 20 + increment / 2)
                && elapsed < 10000
        };
        'iterative_deepening: for depth in 0..max_depth {
            let mut depth_best_score = -i32::MAX;
            let mut depth_best_move = best_move;
            for mov in &legal_moves {
                let metadata = board.make_move(mov);

                let eval = -self.negamax(
                    board,
                    transposition_table,
                    depth,
                    1,
                    -i32::MAX,
                    i32::MAX,
                    -color,
                    &time_ok,
                );
                board.unmake_move(mov, metadata);
                if eval > depth_best_score {
                    depth_best_score = eval;
                    depth_best_move = mov;
                }

                if !time_ok() {
                    break 'iterative_deepening;
                }
            }
            best_move = depth_best_move;
        }

        *best_move
    }

    const IMMEDIATE_MATE_SCORE: i32 = i32::MAX / 2;

    fn is_mate_score(&self, score: i32) -> bool {
        score.abs() > Self::IMMEDIATE_MATE_SCORE - 100000
    }

    fn negamax<F>(
        &self,
        board: &mut Board,
        transposition_table: &TranspositionTable,
        depth: u8,
        search_ply: i32,
        alpha: i32,
        beta: i32,
        color: i32,
        time_ok: &F,
    ) -> i32
    where
        F: Fn() -> bool,
    {
        if board.get_halfmove_clock() > 100 || board.times_played() >= 3 {
            return 0;
        }
        let mut evaluation_type = 1;

        // Alpha can't possibly be worse than getting checkmated right now, if it is it means we already found a
        // faster mate elsewhere
        // Same for beta
        let mut alpha = alpha.max(-Self::IMMEDIATE_MATE_SCORE + search_ply);
        let mut beta = beta.min(Self::IMMEDIATE_MATE_SCORE - search_ply);
        if alpha >= beta {
            return alpha;
        }

        let (moves, check) = move_generation::legal_moves_with_metadata(board);
        if moves.is_empty() {
            return if check {
                search_ply - Self::IMMEDIATE_MATE_SCORE // Checkmate
            } else {
                0 // Draw by no legal moves
            };
        }

        let tt_result = transposition_table.get(board.zobrist_hash);
        let mut tt_move = None;
        if let Some(tt_result) = tt_result {
            tt_move = Some(tt_result.best_move);
            if tt_result.depth >= depth {
                match tt_result.node_type {
                    1 => beta = beta.min(tt_result.score),    // Upper bound
                    0 => return tt_result.score,              // Exact
                    -1 => alpha = alpha.max(tt_result.score), // Lower bound
                    _ => {}
                }
                if alpha >= beta {
                    return tt_result.score;
                }
            }
        }

        if depth == 0 {
            return self.quiescence(
                board,
                (moves, check),
                alpha,
                beta,
                search_ply + 1,
                color,
                time_ok,
            );
        }

        let moves = moves.into_iter().map(|mov| mov.0).collect();
        let sorted_moves = self.sort_moves(board, &moves, tt_move, color);

        let mut best_move = sorted_moves[0];
        for mov in sorted_moves {
            if !time_ok() {
                return alpha;
            }
            let metadata = board.make_move(&mov);
            let value = -self.negamax(
                board,
                transposition_table,
                depth - 1,
                search_ply + 1,
                -beta,
                -alpha,
                -color,
                time_ok,
            );
            board.unmake_move(&mov, metadata);
            if value > alpha {
                alpha = value;
                evaluation_type = 0;
                best_move = mov;
                if alpha >= beta {
                    alpha = beta;
                    evaluation_type = -1;
                    break;
                }
            }
        }

        transposition_table.insert(
            board.zobrist_hash,
            EntryResult::new(depth, alpha, evaluation_type, best_move),
        );
        alpha
    }

    fn quiescence<F>(
        &self,
        board: &mut Board,
        moves: (Vec<(Move, Option<u8>)>, bool),
        alpha: i32,
        beta: i32,
        search_ply: i32,
        color: i32,
        time_ok: &F,
    ) -> i32
    where
        F: Fn() -> bool,
    {
        if !time_ok() {
            return 0;
        }

        if board.get_halfmove_clock() > 100 || board.times_played() >= 3 {
            return 0;
        }

        let (mut moves, check) = moves;
        if moves.is_empty() {
            return if check {
                search_ply - Self::IMMEDIATE_MATE_SCORE // Checkmate
            } else {
                0 // Draw by no legal moves
            };
        } else {
            moves.retain(|mov| mov.1.is_some());
        }

        let mut alpha = alpha;

        let static_eval = evaluation::evaluate(board) * color;
        if static_eval >= beta {
            return beta;
        }
        if static_eval > alpha {
            alpha = static_eval;
        }

        let moves = moves.into_iter().map(|mov| mov.0).collect();
        let moves = self.sort_moves(board, &moves, None, color);

        for mov in moves {
            if !time_ok() {
                return alpha;
            }
            let see = evaluation::static_exchange_evaluation(board, &mov);
            if see < 0 {
                continue;
            }

            let metadata = board.make_move(&mov);
            let eval = -self.quiescence(
                board,
                move_generation::legal_moves_with_metadata(board),
                -beta,
                -alpha,
                search_ply + 1,
                -color,
                time_ok,
            );
            board.unmake_move(&mov, metadata);
            if eval >= beta {
                return beta;
            }
            if eval > alpha {
                alpha = eval;
            }
        }

        alpha
    }

    fn sort_moves(
        &self,
        board: &mut Board,
        moves: &Vec<Move>,
        tt_move: Option<Move>,
        color: i32,
    ) -> Vec<Move> {
        let mut sorted_moves: Vec<(&Move, i32)> = Vec::with_capacity(moves.len());
        for mov in moves {
            let value;
            if let Some(tt_move) = tt_move
                && &tt_move == mov
            {
                value = 1 << 20;
            } else {
                let capture_type = if mov.flags() == 0b0001 {
                    Some(PAWN)
                } else {
                    board.get_piece_at_square(mov.ending_index())
                };

                value = if let Some(capture_type) = capture_type {
                    let eval = evaluation::static_exchange_evaluation(board, mov);
                    if eval != 0 {
                        eval + (100_000 * eval.signum())
                    } else {
                        20_000
                            + evaluation::mvv_lva(
                                board.get_piece_at_square(mov.starting_index()).unwrap() as usize,
                                capture_type as usize,
                            )
                    }
                } else {
                    let metadata = board.make_move(mov);
                    let eval = evaluation::evaluate(board) * color;
                    board.unmake_move(mov, metadata);
                    eval
                };
            }
            sorted_moves.push((mov, value));
        }
        sorted_moves.sort_by_key(|mov| Reverse(mov.1));
        sorted_moves.iter().map(|mov| *mov.0).collect()
    }
}

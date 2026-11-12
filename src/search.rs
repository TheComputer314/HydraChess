mod evaluation;
mod transposition_table;

use super::move_generation;
use super::move_generation::Board;
use crate::move_generation::mov::Move;
use crate::move_generation::mov::piece_type::PAWN;
use crate::search::transposition_table::{EntryResult, TranspositionTable};
use minstant::Instant;
use rand::seq::SliceRandom;
use std::cmp::Reverse;
use std::io::stdout;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

struct SearchResult {
    depth: usize,
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
    // Transposition table is lock-free to allow TT improvements in one thread to improve search in another
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
                let mut search = Search { nodes_searched: 0 };
                loop {
                    state.start_barrier.wait();
                    sleep(Duration::from_millis((i as u64) * 2));
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
                        &state.nodes_searched,
                        i,
                    );

                    state.results[i].lock().unwrap().best_move = search;
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

struct Search {
    nodes_searched: u64,
}

impl Search {
    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        board: &mut Board,
        transposition_table: &TranspositionTable,
        stop_signal: &AtomicBool,
        time_left_ms: u64,
        increment: u64,
        nodes_searched: &AtomicU64,
        search_id: usize,
    ) -> Move {
        self.nodes_searched = 0;
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
        let mut legal_moves = self.sort_moves(board, &legal_moves, tt_move, color);
        if search_id > 0 {
            legal_moves.shuffle(&mut rand::rng());
        }
        let legal_moves = legal_moves;

        let mut best_score = -i32::MAX;
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
                    nodes_searched,
                );
                board.unmake_move(mov, metadata);
                if eval > depth_best_score {
                    depth_best_score = eval;
                    depth_best_move = mov;
                }

                if search_id == 0 {
                    let score = if depth > 0 { best_score } else { 0 };
                    let time = timer.elapsed().as_secs_f64();
                    let nodes =
                        nodes_searched.load(Ordering::Relaxed) + (self.nodes_searched & 2047);
                    let mut out = stdout().lock();
                    if self.is_mate_score(score) {
                        let mate_in_ply = Self::IMMEDIATE_MATE_SCORE - depth_best_score.abs();
                        writeln!(
                            out,
                            "info depth {} score mate {} time {} nodes {} nps {}",
                            depth + 1,
                            ((mate_in_ply + 1) / 2) * depth_best_score.signum(),
                            time,
                            nodes,
                            nodes as f64 / time
                        ).unwrap();
                    } else {
                        writeln!(
                            out,
                            "info depth {} score cp {} time {} nodes {} nps {}",
                            depth + 1,
                            score,
                            time,
                            nodes,
                            nodes as f64 / time
                        ).unwrap();
                    }
                    out.flush().unwrap();
                }

                if !time_ok() {
                    break 'iterative_deepening;
                }
            }
            best_score = depth_best_score;
            best_move = depth_best_move;
        }

        nodes_searched.fetch_add(self.nodes_searched % 2047, Ordering::Relaxed);
        *best_move
    }

    const IMMEDIATE_MATE_SCORE: i32 = i32::MAX / 2;

    fn is_mate_score(&self, score: i32) -> bool {
        score.abs() > Self::IMMEDIATE_MATE_SCORE - 100000
    }

    #[allow(clippy::too_many_arguments)]
    fn negamax<F>(
        &mut self,
        board: &mut Board,
        transposition_table: &TranspositionTable,
        depth: u8,
        search_ply: i32,
        alpha: i32,
        beta: i32,
        color: i32,
        time_ok: &F,
        nodes_searched: &AtomicU64,
    ) -> i32
    where
        F: Fn() -> bool,
    {
        if board.get_halfmove_clock() > 100 || board.times_played() >= 3 {
            return 0;
        }
        self.count_nodes(nodes_searched);
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
                nodes_searched,
            );
        }

        let moves = moves.into_iter().map(|mov| mov.0).collect();
        let mut sorted_moves = self.sort_moves(board, &moves, tt_move, color);

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
                nodes_searched,
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

    #[allow(clippy::too_many_arguments)]
    fn quiescence<F>(
        &mut self,
        board: &mut Board,
        moves: (Vec<(Move, Option<u8>)>, bool),
        alpha: i32,
        beta: i32,
        search_ply: i32,
        color: i32,
        time_ok: &F,
        nodes_searched: &AtomicU64,
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

        self.count_nodes(nodes_searched);

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
                nodes_searched,
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

    fn count_nodes(&mut self, nodes_searched: &AtomicU64) {
        self.nodes_searched += 1;
        if self.nodes_searched % 2047 == 0 {
            nodes_searched.fetch_add(2048, Ordering::Relaxed);
        }
    }
}

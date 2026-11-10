mod evaluation;
mod transposition_table;

use super::move_generation::{Board, MoveGenerator};
use crate::move_generation::mov::Move;
use crate::move_generation::mov::piece_type::PAWN;
use crate::search::evaluation::Evaluation;
use crate::search::transposition_table::{EntryResult, TranspositionTable};
use minstant::Instant;
use std::cmp::Reverse;
use std::io::stdout;
use std::io::{Stdout, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct SearchThread {
    search: Search,
}

impl SearchThread {
    pub fn new() -> Self {
        Self {
            search: Search::new(),
        }
    }

    pub fn new_game(&mut self) {
        self.search = Search::new();
    }

    pub fn position(&mut self, string: &str) {
        let parts: Vec<&str> = string.split_whitespace().collect();

        if parts[0] == "startpos" {
            self.search.board = Board::starting();
            self.search.moves_played = 0;
            if parts.len() > 1 && parts[1] == "moves" {
                self.position_moves(&parts[2..]);
            }
        } else if parts[0] == "fen" {
            self.search.board = Board::from_fen(&parts[1..7].join(" "));
            self.search.moves_played = 0;
            if parts.len() > 7 && parts[7] == "moves" {
                self.position_moves(&parts[8..]);
            }
        }
    }

    fn position_moves(&mut self, moves: &[&str]) {
        for mov in moves {
            self.search.board.move_from_uci_lan(mov);
            self.search.moves_played += 1;
        }
    }

    pub fn debug(&self) {
        println!("Board: {}", self.search.board);
        println!("Moves played: {}", self.search.moves_played);
    }

    pub fn go(
        &mut self,
        stop_signal: &Arc<AtomicBool>,
        white_time_left_ms: u128,
        white_increment: u128,
        black_time_left_ms: u128,
        black_increment: u128,
    ) {
        let out = stdout();

        let (time_left_ms, increment) = if self.search.board.white_to_move() {
            (white_time_left_ms, white_increment)
        } else {
            (black_time_left_ms, black_increment)
        };
        let best_move = self
            .search
            .search(stop_signal, &out, time_left_ms, increment);
        self.search.board.make_move(&best_move);
        let mut out = stdout().lock();
        writeln!(out, "bestmove {}", best_move).unwrap();
        out.flush().unwrap();
        self.search.moves_played += 1;
    }
}

struct Search {
    board: Board,
    move_generator: MoveGenerator,
    transposition_table: TranspositionTable,
    moves_played: usize,
    nodes_searched: u64,
}

impl Search {
    pub fn new() -> Self {
        Self {
            board: Board::starting(),
            move_generator: MoveGenerator::new(),
            transposition_table: TranspositionTable::new(20),
            moves_played: 0,
            nodes_searched: 0,
        }
    }

    fn search(
        &mut self,
        stop_signal: &Arc<AtomicBool>,
        out: &Stdout,
        time_left_ms: u128,
        increment: u128,
    ) -> Move {
        let legal_moves = self.move_generator.legal_moves(&self.board);
        if legal_moves.is_empty() {
            panic!(
                "Attempted to search and eval board {} when we're checkmated",
                self.board
            );
        }
        if legal_moves.len() == 1 {
            return legal_moves[0];
        }

        let color = if self.board.white_to_move() { 1 } else { -1 };
        let transposition_table_result = self.transposition_table.get(self.board.zobrist_hash);
        let mut tt_move = None;
        if let Some(result) = transposition_table_result {
            tt_move = Some(result.best_move);
        }
        let legal_moves = self.sort_moves(&legal_moves, tt_move, color);

        let mut best_move = &legal_moves[0];
        let max_depth = 16;
        self.nodes_searched = 0;
        let timer = Instant::now();
        let time_ok = || {
            let elapsed = timer.elapsed().as_millis();
            !stop_signal.load(Relaxed)
                && elapsed < (time_left_ms / 20 + increment / 2)
                && elapsed < 10000
        };
        'iterative_deepening: for depth in 0..max_depth {
            let mut depth_best_score = -i64::MAX;
            let mut depth_best_move = best_move;
            for mov in &legal_moves {
                let metadata = self.board.make_move(mov);

                let eval = -self.negamax(depth, 1, -i64::MAX, i64::MAX, -color, &time_ok);
                self.board.unmake_move(mov, metadata);
                if eval > depth_best_score {
                    depth_best_score = eval;
                    depth_best_move = mov;
                }

                if depth_best_score.abs() > Self::IMMEDIATE_MATE_SCORE - 10000 {
                    let mate_in_ply = Self::IMMEDIATE_MATE_SCORE - depth_best_score.abs();
                    let mut lock = out.lock();
                    writeln!(
                        lock,
                        "info depth {} score mate {} nodes {}",
                        depth + 1,
                        ((mate_in_ply + 1) / 2) * depth_best_score.signum() * color,
                        self.nodes_searched
                    )
                    .unwrap();
                    lock.flush().unwrap();
                } else {
                    let mut lock = out.lock();
                    writeln!(
                        lock,
                        "info depth {} score cp {} nodes {}",
                        depth + 1,
                        depth_best_score * color,
                        self.nodes_searched
                    )
                    .unwrap();
                    lock.flush().unwrap();
                }

                if !time_ok() {
                    break 'iterative_deepening;
                }
            }
            best_move = depth_best_move;
        }

        *best_move
    }

    const IMMEDIATE_MATE_SCORE: i64 = i64::MAX / 2;

    fn negamax<F>(
        &mut self,
        depth: u16,
        search_ply: i64,
        alpha: i64,
        beta: i64,
        color: i64,
        time_ok: &F,
    ) -> i64
    where
        F: Fn() -> bool,
    {
        self.nodes_searched += 1;
        if self.board.get_halfmove_clock() > 100 || self.board.times_played() >= 3 {
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

        let (moves, check) = self.move_generator.legal_moves_with_metadata(&self.board);
        if moves.is_empty() {
            return if check {
                search_ply - Self::IMMEDIATE_MATE_SCORE // Checkmate
            } else {
                0 // Draw by no legal moves
            };
        }

        let tt_result = self.transposition_table.get(self.board.zobrist_hash);
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
            return self.quiescence((moves, check), alpha, beta, search_ply + 1, color, time_ok);
        }

        let moves = moves.into_iter().map(|mov| mov.0).collect();
        let sorted_moves = self.sort_moves(&moves, tt_move, color);

        let mut best_move = sorted_moves[0];
        for mov in sorted_moves {
            if !time_ok() {
                return alpha;
            }
            let metadata = self.board.make_move(&mov);
            let value = -self.negamax(depth - 1, search_ply + 1, -beta, -alpha, -color, time_ok);
            self.board.unmake_move(&mov, metadata);
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

        self.transposition_table.insert(
            self.board.zobrist_hash,
            EntryResult::new(depth, alpha, evaluation_type, best_move),
        );
        alpha
    }

    fn quiescence<F>(
        &mut self,
        moves: (Vec<(Move, Option<u8>)>, bool),
        alpha: i64,
        beta: i64,
        search_ply: i64,
        color: i64,
        time_ok: &F,
    ) -> i64
    where
        F: Fn() -> bool,
    {
        if !time_ok() {
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

        let static_eval = Evaluation::evaluate(&self.board) * color;
        if static_eval >= beta {
            return beta;
        }
        if static_eval > alpha {
            alpha = static_eval;
        }

        let moves = moves.into_iter().map(|mov| mov.0).collect();
        let moves = self.sort_moves(&moves, None, color);

        for mov in moves {
            if !time_ok() {
                return alpha;
            }
            let metadata = self.board.make_move(&mov);
            let eval = -self.quiescence(
                self.move_generator.legal_moves_with_metadata(&self.board),
                -beta,
                -alpha,
                search_ply + 1,
                -color,
                time_ok,
            );
            self.board.unmake_move(&mov, metadata);
            if eval >= beta {
                return beta;
            }
            if eval > alpha {
                alpha = eval;
            }
        }

        alpha
    }

    fn sort_moves(&mut self, moves: &Vec<Move>, tt_move: Option<Move>, color: i64) -> Vec<Move> {
        let mut sorted_moves: Vec<(&Move, i64)> = Vec::with_capacity(moves.len());
        for mov in moves {
            let value;
            if let Some(tt_move) = tt_move
                && &tt_move == mov
            {
                value = 1 << 12;
            } else {
                let capture_type = if mov.flags() == 0b0001 {
                    Some(PAWN)
                } else {
                    self.board.get_piece_at_square(mov.ending_index())
                };

                value = if let Some(capture_type) = capture_type {
                    let attacker_type = self
                        .board
                        .get_piece_at_square(mov.starting_index())
                        .unwrap();
                    Evaluation::mvv_lva(attacker_type as usize, capture_type as usize) + (1 << 10)
                } else {
                    let metadata = self.board.make_move(mov);
                    let eval = Evaluation::evaluate(&self.board) * color;
                    self.board.unmake_move(mov, metadata);
                    eval
                };
            }
            sorted_moves.push((mov, value));
        }
        sorted_moves.sort_by_key(|mov| Reverse(mov.1));
        sorted_moves.iter().map(|mov| *mov.0).collect()
    }
}

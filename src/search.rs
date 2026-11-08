mod evaluation;
mod transposition_table;

use super::move_generation::{Board, MoveGenerator};
use crate::move_generation::mov::Move;
use crate::search::evaluation::Evaluation;
use crate::search::transposition_table::{EntryResult, TranspositionTable};
use std::cmp::Reverse;
use std::io::stdout;
use std::io::{Stdout, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

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
}

impl Search {
    pub fn new() -> Self {
        Self {
            board: Board::starting(),
            move_generator: MoveGenerator::new(),
            transposition_table: TranspositionTable::new(20),
            moves_played: 0,
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
            panic!("Attempted to search and eval board {} when we're checkmated", self.board);
        }
        let color = if self.board.white_to_move() { 1 } else { -1 };
        let mut best_move = &legal_moves[0];
        let max_depth = 5;
        let timer = Instant::now();
        let time_ok = || {
            !stop_signal.load(Relaxed)
                && timer.elapsed().as_millis() < (time_left_ms / 20 + increment / 2)
        };
        'iterative_deepening: for depth in 1..(max_depth + 1) {
            let mut depth_best_score = -i64::MAX;
            let mut depth_best_move = best_move;
            for mov in &legal_moves {
                let metadata = self.board.make_move(mov);
                {
                    let eval = -self.negamax(depth, 1, -i64::MAX, i64::MAX, -color, &time_ok);
                    self.board.unmake_move(mov, metadata);
                    if eval > depth_best_score {
                        depth_best_score = eval;
                        depth_best_move = mov;

                        if depth_best_score.abs() > Self::IMMEDIATE_MATE_SCORE - 10000 {
                            let mate_in_ply =
                                Self::IMMEDIATE_MATE_SCORE - depth_best_score.abs();
                            let mut lock = out.lock();
                            writeln!(
                                lock,
                                "info depth {} score mate {}",
                                depth,
                                ((mate_in_ply + 1) / 2) * depth_best_score.signum()
                            )
                            .unwrap();
                            lock.flush().unwrap();
                        } else {
                            let mut lock = out.lock();
                            writeln!(lock, "info depth {} score cp {}", depth, depth_best_score * color)
                                .unwrap();
                            lock.flush().unwrap();
                        }
                    }
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
        search_ply: u16,
        alpha: i64,
        beta: i64,
        color: i64,
        time_ok: &F,
    ) -> i64
    where
        F: Fn() -> bool,
    {
        if self.board.get_halfmove_clock() > 100 || self.board.times_played() >= 3 {
            return 0;
        }
        let alpha_original = alpha;
        let mut alpha = alpha;
        let mut beta = beta;

        // Alpha can't possibly be worse than getting checkmated right now, if it is it means we already found a
        // faster mate elsewhere
        // Same for beta
        alpha = alpha.max(-Self::IMMEDIATE_MATE_SCORE + search_ply as i64);
        beta = beta.min(Self::IMMEDIATE_MATE_SCORE - search_ply as i64);
        if alpha >= beta {
            return alpha;
        }

        let moves = self.move_generator.legal_moves(&self.board);
        if moves.is_empty() {
            if self.move_generator.king_in_check(&self.board) {
                return search_ply as i64 - i64::MAX / 2;
            }
            return 0;
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
            return self.quiescence(alpha, beta, color, time_ok);
        }

        let sorted_moves = self.sort_moves(&moves, tt_move, color);

        let mut best_value = -i64::MAX;
        let mut best_move = sorted_moves[0];
        for mov in sorted_moves {
            if !time_ok() {
                return 0;
            }
            let metadata = self.board.make_move(&mov);
            let value = -self.negamax(depth - 1, search_ply + 1, -beta, -alpha, -color, time_ok);
            self.board.unmake_move(&mov, metadata);
            if value > best_value {
                best_value = value;
                best_move = mov;
            }
            alpha = alpha.max(best_value);
            if alpha >= beta {
                break;
            }
        }

        let node_type = if alpha <= alpha_original {
            1
        } else if alpha >= beta {
            -1
        } else {
            0
        };
        self.transposition_table.insert(
            self.board.zobrist_hash,
            EntryResult::new(depth, alpha, node_type, best_move),
        );
        alpha
    }

    fn quiescence<F>(&mut self, alpha: i64, beta: i64, color: i64, time_ok: &F) -> i64 where F: Fn() -> bool {
        if !time_ok() {
            return 0;
        }
        let mut alpha = alpha;
        let mut beta = beta;

        let static_eval = Evaluation::evaluate(&self.board) * color;
        if static_eval >= beta {
            return beta;
        }
        if static_eval > alpha {
            alpha = static_eval;
        }

        let check = self.move_generator.king_in_check(&self.board);

        let moves;
        if check {
            moves = self.move_generator.legal_moves(&self.board);
            if moves.is_empty() {
                return Self::IMMEDIATE_MATE_SCORE;
            }
        } else {
            moves = self.move_generator.legal_captures(&self.board);
        }
        let moves = self.sort_moves(&moves, None, color);

        for mov in moves {
            if !time_ok() {
                return 0;
            }
            let metadata = self.board.make_move(&mov);
            let eval = -self.quiescence(-beta, -alpha, -color, time_ok);
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
                value = 1 << 10;
            } else {
                let metadata = self.board.make_move(mov);
                value = Evaluation::evaluate(&self.board) * color; // TODO: Implement MVV-LVA
                self.board.unmake_move(mov, metadata);
            }
            sorted_moves.push((mov, value));
        }
        sorted_moves.sort_by_key(|mov| Reverse(mov.1));
        sorted_moves.iter().map(|mov| *mov.0).collect()
    }


}

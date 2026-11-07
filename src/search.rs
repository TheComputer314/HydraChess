mod evaluation;

use super::move_generation::{Board, MoveGenerator};
use crate::move_generation::mov::Move;
use crate::search::evaluation::Evaluation;
use rand::random_range;
use std::io::{Stdout, Write};
use std::io::{StdoutLock, stdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const TRANSPOSITION_TABLE_SIZE: usize = 1 << 20;

pub struct Search {
    board: Board,
    move_generator: MoveGenerator,
    transposition_table: Vec<u8>,
    moves_played: usize,
}

impl Search {
    pub fn new() -> Self {
        Self {
            board: Board::starting(),
            move_generator: MoveGenerator::new(),
            transposition_table: vec![0; TRANSPOSITION_TABLE_SIZE],
            moves_played: 0,
        }
    }

    pub fn new_game(&mut self) {
        self.board = Board::starting();
        self.transposition_table = vec![0; TRANSPOSITION_TABLE_SIZE];
        self.moves_played = 0;
    }

    pub fn position(&mut self, string: &str) {
        let parts: Vec<&str> = string.split_whitespace().collect();

        if parts[0] == "startpos" {
            if self.moves_played == 0 {
                self.board = Board::starting();
            }
            if parts.len() > 1 && parts[1] == "moves" {
                self.position_moves(&parts[2..]);
            }
        } else if parts[0] == "fen" {
            if self.moves_played == 0 {
                self.board = Board::from_fen(&parts[1..7].join(" "));
            }
            if parts.len() > 7 && parts[7] == "moves" {
                self.position_moves(&parts[8..]);
            }
        }
    }

    fn position_moves(&mut self, moves: &[&str]) {
        for mov in &moves[self.moves_played..] {
            self.board.move_from_uci_lan(mov);
            self.moves_played += 1;
        }
    }

    pub fn debug(&self) {
        println!("Board: {}", self.board);
        println!("Moves played: {}", self.moves_played);
    }

    pub fn go(&mut self, stop_signal: &Arc<AtomicBool>) {
        let out = stdout();

        let best_move = self.search(stop_signal, &out);
        self.board.make_move(&best_move);
        let mut out = stdout().lock();
        writeln!(out, "bestmove {}", best_move).unwrap();
        out.flush().unwrap();
        self.moves_played += 1;
    }

    fn search(&mut self, stop_signal: &Arc<AtomicBool>, out: &Stdout) -> Move {
        let legal_moves = self.move_generator.legal_moves(&self.board);
        if legal_moves.is_empty() {
            panic!("Attempted to search and eval when we're checkmated")
        }
        let color = if self.board.white_to_move() { 1 } else { -1 };
        let mut best_score = -i64::MAX;
        let mut best_move = &legal_moves[random_range(0..legal_moves.len())];
        let max_depth = 10;
        let timer = Instant::now();
        'iterative_deepening: for depth in 1..(max_depth + 1) {
            for mov in &legal_moves {
                let metadata = self.board.make_move(mov);
                let eval = self.negamax(depth, -i64::MAX, i64::MAX, color);
                self.board.unmake_move(mov, metadata);
                if eval > best_score {
                    best_score = eval;
                    best_move = mov;
                    let mut lock = out.lock();
                    writeln!(lock, "info depth {} score cp {}", depth, best_score).unwrap();
                    lock.flush().unwrap();
                }
                if timer.elapsed().as_secs() > 5 { // TODO: Do actual time management
                    break 'iterative_deepening;
                }
            }
        }

        *best_move
    }

    fn negamax(&mut self, depth: u16, alpha: i64, beta: i64, color: i64) -> i64 {
        if depth == 0 {
            return Evaluation::evaluate(&self.board) * color;
        }

        let moves = self.move_generator.legal_moves(&self.board);
        if moves.is_empty() {
            return i64::MAX;
        }
        // TODO: Move ordering

        let mut alpha = alpha;
        for mov in moves {
            let metadata = self.board.make_move(&mov);
            let value = -self.negamax(depth - 1, -beta, -alpha, -color);
            self.board.unmake_move(&mov, metadata);
            alpha = alpha.max(value);
            if alpha >= beta {
                break;
            }
        }
        alpha
    }
}

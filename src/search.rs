mod evaluation;

use super::move_generation::{Board, MoveGenerator};
use crate::move_generation::mov::Move;
use crate::search::evaluation::Evaluation;
use std::io::{Stdout, Write};
use std::io::{StdoutLock, stdout};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
        let mut scored_moves: Vec<(Move, i64)> = legal_moves
            .into_iter()
            .map(|mov| {
                let metadata = self.board.make_move(&mov);
                let eval = self.negamax(3, -i64::MAX, i64::MAX, color);
                self.board.unmake_move(&mov, metadata);
                (mov, eval)
            })
            .collect();
        scored_moves.sort_by_key(|a| std::cmp::Reverse(a.1));
        let best_score = scored_moves[0].1;

        let tied_count = scored_moves
            .iter()
            .filter(|(_, score)| *score == best_score)
            .count();
        let best_move = scored_moves[rand::random_range(0..tied_count)].0;

        let mut lock = out.lock();
        writeln!(lock, "info depth 3 score cp {}", best_score).unwrap();
        lock.flush().unwrap();

        best_move
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

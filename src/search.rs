use super::move_generation::{Board, MoveGenerator};
use std::io::Write;
use std::io::stdout;
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
            self.board.move_from_string(mov);
            self.moves_played += 1;
        }
    }

    pub fn debug(&self) {
        println!("Board: {}", self.board);
        println!("Moves played: {}", self.moves_played);
    }

    pub fn search(&mut self, stop_signal: &Arc<AtomicBool>) {
        // Temp: Random move selection
        let legal_moves = self.move_generator.legal_moves(&self.board);
        let mov = legal_moves[rand::random_range(0..legal_moves.len())];
        self.board.make_move(&mov);
        let mut out = stdout().lock();
        writeln!(out, "bestmove {}", mov).unwrap();
        out.flush().unwrap();
        self.moves_played += 1;
    }
}
use crate::move_generation::mov::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::move_generation::mov::squares::*;
use board_state::Board;
use mov::Move;
use move_lookup::MoveLookup;

mod board_state;
mod mov;
mod move_lookup;

pub struct MoveGenerator {
    lookup: MoveLookup,
}

impl MoveGenerator {
    pub fn new() -> Self {
        Self {
            lookup: MoveLookup::new(),
        }
    }

    pub fn legal_moves(&self, board: &mut Board) -> Vec<Move> {
        let mut moves = Vec::new();

        let white = board.white_to_move();
        let friendly_pieces = if white { board.white } else { board.black };
        let enemy_pieces = if white { board.black } else { board.white };

        let king_mask = friendly_pieces & board.kings;
        let mut king_danger_mask: u64 = 0;
        let mut checkers = 0;
        let mut checker_index = 0;
        // King is not included in the blockers, so that the king can't block a
        // move that would leave it in check post-move
        let blockers = board.all_pieces() & !king_mask;
        for i in MoveGenerator::get_indices_of_bits(enemy_pieces) {
            let mask = 1 << i;
            if board.pawns & mask != 0 {
                // TODO
            } else if board.knights & mask != 0 {
                let possible_knight_moves = self.lookup.possible_knight_moves(i);
                if king_mask & possible_knight_moves != 0 {
                    checkers += 1;
                    checker_index = i;
                }
                king_danger_mask |= possible_knight_moves;
            } else if board.bishops & mask != 0 {
                let possible_bishop_moves = self.lookup.possible_bishop_moves(i, blockers);
                if king_mask & possible_bishop_moves != 0 {
                    checkers += 1;
                    checker_index = i;
                }
                king_danger_mask |= possible_bishop_moves;
            } else if board.rooks & mask != 0 {
                let possible_rook_moves = self.lookup.possible_rook_moves(i, blockers);
                if king_mask & possible_rook_moves != 0 {
                    checkers += 1;
                    checker_index = i;
                }
                king_danger_mask |= possible_rook_moves;
            } else if board.queens & mask != 0 {
                let possible_queen_moves = self.lookup.possible_queen_moves(i, blockers);
                if king_mask & possible_queen_moves != 0 {
                    checkers += 1;
                    checker_index = i;
                }
                king_danger_mask |= possible_queen_moves;
            } else if board.kings & mask != 0 {
                // Kings can't attack each other legally, so no need to handle checkers counting for that
                king_danger_mask |= self.lookup.possible_king_moves(i);
            }
        }
        let king_danger_mask = king_danger_mask;

        // King moves are restricted by the danger mask
        let king_index = king_mask.trailing_zeros() as u8;
        let pseudo_legal_king_moves =
            self.lookup.possible_king_moves(king_index) & !friendly_pieces;
        let legal_king_moves = pseudo_legal_king_moves & !king_danger_mask;
        for i in Self::get_indices_of_bits(legal_king_moves) {
            moves.push(Move::regular(king_index, i));
        }

        // If in double check, only legal moves are king moves
        if checkers > 1 {
            return moves;
        }

        // If in single check, capturing or blocking attackers are the only legal non-king moves
        // So restrict possible moves by creating a mask of legal captures and legal blocks
        let mut capture_mask = u64::MAX;
        let mut block_mask = u64::MAX;
        if checkers > 0 {
            let checker_mask = 1 << checker_index;
            capture_mask = 1 << checker_index;
            if board.rooks & checker_mask != 0 {
                block_mask = self.lookup.possible_rook_moves(checker_index, 0)
                    & self.lookup.possible_rook_moves(king_index, 0);
            } else if board.bishops & checker_mask != 0 {
                block_mask = self.lookup.possible_bishop_moves(checker_index, 0)
                    & self.lookup.possible_bishop_moves(king_index, 0);
            } else if board.queens & checker_mask != 0 {
                block_mask = self.lookup.possible_queen_moves(checker_index, 0)
                    & self.lookup.possible_queen_moves(king_index, 0);
            }
        }

        // If king is not in check, all other moves are allowed
        // TODO

        moves
    }

    fn get_indices_of_bits(bits: u64) -> Vec<u8> {
        let mut indices = Vec::new();
        let mut bits = bits;
        while bits != 0 {
            let trailing_zeros = bits.trailing_zeros();
            bits ^= 1 << trailing_zeros;
            indices.push(trailing_zeros as u8);
        }
        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_get_squares_of_moves() {
        let moves = 4462742033;
        let squares = MoveGenerator::get_indices_of_bits(moves);
        assert_eq!(squares, vec![0, 4, 9, 11, 25, 27, 32]);
    }

    #[test]
    fn perft_starting() {
        test_perft(Board::starting(), &[20, 400, 8902, 197281, 4865609]);
    }

    #[test]
    fn perft_starting_a2_a4() {
        test_perft(
            Board::from_fen("rnbqkbnr/pppppppp/8/8/P7/8/1PPPPPPP/RNBQKBNR b KQkq - 0 1"),
            &[20, 420, 9329, 217832],
        );
    }

    #[test]
    fn perft_kiwipete() {
        test_perft(
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1"),
            &[48, 2039, 97862, 4085603],
        )
    }

    #[test]
    fn perft_kiwipete_e5_c6() {
        test_perft(
            Board::from_fen("r3k2r/p1ppqpb1/bnN1pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 1 1"),
            &[41, 2027],
        )
    }

    fn test_perft(starting_board: Board, expected: &[usize]) {
        let move_generator = MoveGenerator::new();
        let mut board = starting_board.clone();
        for depth in 0..expected.len() {
            let timer = Instant::now();
            let result = perft(&move_generator, &mut board, depth + 1, true);
            let elapsed = timer.elapsed();
            println!(
                "Depth {}: Expected {} moves, calculated {} moves in {:?} milliseconds",
                depth + 1,
                expected[depth],
                result,
                elapsed.as_secs_f64() * 1000.0
            );
            assert_eq!(result, expected[depth]);
        }
    }

    fn perft(
        move_generator: &MoveGenerator,
        board: &mut Board,
        depth: usize,
        divide: bool,
    ) -> usize {
        let moves = move_generator.legal_moves(board);
        if moves.is_empty() {
            return 0;
        }
        if depth == 1 {
            return moves.len();
        }
        let mut total_moves = 0;
        for mov in moves {
            let (last_flag, capture) = board.make_move(&mov);
            let nodes = perft(move_generator, board, depth - 1, false);
            if divide {
                println!("Move {}: {} nodes", mov, nodes);
            }
            total_moves += nodes;
            board.unmake_move(&mov, last_flag, &capture);
        }
        total_moves
    }
}

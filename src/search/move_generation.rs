use crate::search::move_generation::mov::Square;
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
        let white = board.white_to_move();

        let friendly_pieces = if white { board.white } else { board.black };

        let friendly_piece_indices = Self::get_indices_of_bits(friendly_pieces);

        let mut moves = Vec::new();

        for square in friendly_piece_indices {
            let start_square = Square::from_index(square);
            let square_mask = start_square.mask();
            if board.pawns & square_mask != 0 {
                todo!()
            } else if board.knights & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup.possible_knight_moves(start_square),
                    friendly_pieces,
                );
            } else if board.bishops & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup.possible_bishop_moves(start_square, 0),
                    friendly_pieces,
                );
            } else if board.rooks & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup.possible_rook_moves(start_square, 0),
                    friendly_pieces,
                );
            } else if board.queens & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup.possible_queen_moves(start_square, 0),
                    friendly_pieces,
                );
            } else if board.kings & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup.possible_king_moves(start_square),
                    friendly_pieces,
                );
            } else {
                panic!("Invalid piece type");
            }
        }

        moves
    }

    /// Makes the supplied pseudo-legal moves, checks for king-in-check, and if not adds it to the list of moves.
    /// Unmakes the move before returning
    fn try_moves(
        &self,
        board: &mut Board,
        white: bool,
        moves: &mut Vec<Move>,
        start_square: Square,
        move_mask: u64,
        friendly_pieces: u64,
    ) {
        // Friendly pieces should not be captured
        for end in Self::get_indices_of_bits(move_mask & !friendly_pieces) {
            let end_square = Square::from_index(end);
            let mov = Move::regular(start_square, end_square);

            let (flags, capture) = board.make_move(&mov);
            if !self.king_in_check(board, white) {
                moves.push(mov);
            }
            board.unmake_move(&mov, flags, &capture);
        }
    }

    fn king_in_check(&self, board: &Board, white: bool) -> bool {
        todo!()
    }

    fn get_indices_of_bits(bits: u64) -> Vec<u8> {
        let mut indices = Vec::new();
        let mut i = 0;
        let mut bits = bits;
        while bits != 0 {
            let trailing_zeros = bits.trailing_zeros();
            bits >>= trailing_zeros + 1;
            i += trailing_zeros as u8;
            indices.push(i);
            i += 1;
        }
        indices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_squares_of_moves() {
        let moves = 4462742033;
        let squares = MoveGenerator::get_indices_of_bits(moves);
        assert_eq!(squares, vec![0, 4, 9, 11, 25, 27, 32]);
    }
}

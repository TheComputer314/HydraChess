use crate::search::move_generation::mov::PieceType::{Bishop, King, Knight, Pawn, Queen, Rook};
use crate::search::move_generation::mov::squares::*;
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
        let enemy_pieces = if white { board.black } else { board.white };

        let friendly_piece_indices = Self::get_indices_of_bits(friendly_pieces);

        let mut moves = Vec::new();

        for start_square in friendly_piece_indices {
            let square_mask = mask(start_square);
            if board.pawns & square_mask != 0 {
                let mut pawn_moves = 0;

                // Single pawn push
                let forward = if white {
                    north(start_square)
                } else {
                    south(start_square)
                };
                pawn_moves |= mask(forward) & !board.all_pieces();
                // Captures
                if file(start_square) > 0 {
                    let left_capture = west(forward);
                    pawn_moves |= mask(left_capture) & enemy_pieces;
                }
                if file(start_square) < 7 {
                    let right_capture = east(forward);
                    pawn_moves |= mask(right_capture) & enemy_pieces;
                }

                if (white && rank(start_square) == 6) || (!white && rank(start_square) == 1) {
                    self.try_promotions(
                        board,
                        white,
                        &mut moves,
                        start_square,
                        pawn_moves,
                        friendly_pieces,
                    );
                } else {
                    self.try_moves(
                        board,
                        white,
                        &mut moves,
                        start_square,
                        pawn_moves,
                        friendly_pieces,
                    )
                };

                // Double pawn push
                if (white && rank(start_square) == 1) || (!white && rank(start_square) == 6) {
                    let file_mask: u64 = if white { 0x1010000 } else { 0x10100000000 };
                    if (file_mask << file(start_square)) & board.all_pieces() == 0 {
                        let mov = Move::double_pawn_push(start_square);
                        self.try_move(board, white, &mut moves, &mov);
                    }
                }

                // En passant handling
                if let Some(en_passant_target) = board.en_passant_target() {
                    let target_rank = en_passant_target / 8;
                    let required_rank = if white { 4 } else { 3 };
                    let target_file = en_passant_target % 8;
                    if rank(start_square) == required_rank && target_rank == required_rank {
                        // Left en passant
                        if file(start_square) > 0 && target_file == file(start_square) - 1 {
                            let mov = Move::left_en_passant(start_square);
                            self.try_move(board, white, &mut moves, &mov);
                        }
                        // Right en passant
                        if file(start_square) < 7 && target_file == file(start_square) + 1 {
                            let mov = Move::right_en_passant(start_square);
                            self.try_move(board, white, &mut moves, &mov);
                        }
                    }
                }
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
                    self.lookup
                        .possible_bishop_moves(start_square, board.all_pieces()),
                    friendly_pieces,
                );
            } else if board.rooks & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup
                        .possible_rook_moves(start_square, board.all_pieces()),
                    friendly_pieces,
                );
            } else if board.queens & square_mask != 0 {
                self.try_moves(
                    board,
                    white,
                    &mut moves,
                    start_square,
                    self.lookup
                        .possible_queen_moves(start_square, board.all_pieces()),
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

                let kingside_castle_blocker_mask = if white { 0x60 } else { 0x60 << 56 };
                if board.kingside_castling_available(white) // If kingside castling available
                        && board.all_pieces() & kingside_castle_blocker_mask == 0 // And no pieces blocking it
                        // And king is not currently in check
                        && !self.king_in_check(board, white)
                {
                    // A bit disgusting: Redoing king-in-check on the move to the F file
                    // TODO: Figure out a way to combine the normal move checks with the castling checks
                    let right_mov = &Move::regular(start_square, east(start_square));
                    let (flags, capture) = board.make_move(right_mov);
                    let castling_ok = !self.king_in_check(board, white);
                    board.unmake_move(right_mov, flags, &capture);
                    if castling_ok {
                        self.try_move(board, white, &mut moves, &Move::kingside_castle(white))
                    }
                }
                let queenside_castle_blocker_mask = if white { 0xe } else { 0xe << 56 };
                if board.queenside_castling_available(white) // If queenside castling available
                        && board.all_pieces() & queenside_castle_blocker_mask == 0 // And no pieces blocking it
                        // And king is not currently in check
                        && !self.king_in_check(board, white)
                {
                    // A bit disgusting: Redoing king-in-check on the move to the C file
                    // TODO: Figure out a way to combine the normal move checks with the castling checks
                    let left_mov = &Move::regular(start_square, west(start_square));
                    let (flags, capture) = board.make_move(left_mov);
                    let castling_ok = !self.king_in_check(board, white);
                    board.unmake_move(left_mov, flags, &capture);
                    if castling_ok {
                        self.try_move(board, white, &mut moves, &Move::queenside_castle(white))
                    }
                }
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
        start_square: u8,
        move_mask: u64,
        friendly_pieces: u64,
    ) {
        // Friendly pieces should not be captured
        for end_square in Self::get_indices_of_bits(move_mask & !friendly_pieces) {
            let mov = Move::regular(start_square, end_square);

            self.try_move(board, white, moves, &mov);
        }
    }

    fn try_promotions(
        &self,
        board: &mut Board,
        white: bool,
        moves: &mut Vec<Move>,
        start_square: u8,
        move_mask: u64,
        friendly_pieces: u64,
    ) {
        // Friendly pieces should not be captured
        for end_square in Self::get_indices_of_bits(move_mask & !friendly_pieces) {
            for promotion in [Queen, Rook, Bishop, Knight] {
                let mov = Move::pawn_promotion(start_square, end_square, promotion);

                self.try_move(board, white, moves, &mov);
            }
        }
    }

    fn try_move(&self, board: &mut Board, white: bool, moves: &mut Vec<Move>, mov: &Move) {
        let (flags, capture) = board.make_move(mov);
        if !self.king_in_check(board, white) {
            moves.push(*mov);
        }
        board.unmake_move(mov, flags, &capture);
    }

    pub fn king_in_check(&self, board: &Board, white: bool) -> bool {
        let king_square = board.get_bitboard(white, King).trailing_zeros() as u8;
        let friendly_pieces = if white { board.white } else { board.black };

        let horizontal_attacks = self
            .lookup
            .possible_rook_moves(king_square, board.all_pieces())
            & !friendly_pieces;
        if horizontal_attacks & (board.rooks | board.queens) != 0 {
            return true;
        }

        let diagonal_attacks = self
            .lookup
            .possible_bishop_moves(king_square, board.all_pieces())
            & !friendly_pieces;
        if diagonal_attacks & (board.bishops | board.queens) != 0 {
            return true;
        }

        let knight_attacks =
            self.lookup.possible_knight_moves(king_square) & board.get_bitboard(!white, Knight);
        if knight_attacks != 0 {
            return true;
        }

        let king_attacks =
            self.lookup.possible_king_moves(king_square) & board.get_bitboard(!white, King);
        if king_attacks != 0 {
            return true;
        }

        let final_rank = if white { 7 } else { 0 };
        if rank(king_square) != final_rank {
            let mut pawn_attacks: u64 = 0;
            let backwards = if white {
                north(king_square)
            } else {
                south(king_square)
            };
            if file(king_square) != 0 {
                pawn_attacks |= mask(west(backwards));
            }
            if file(king_square) != 7 {
                pawn_attacks |= mask(east(backwards));
            }
            if pawn_attacks & board.get_bitboard(!white, Pawn) != 0 {
                return true;
            }
        }

        false
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
    fn test_king_in_check() {
        let move_generator = MoveGenerator::new();
        // Starting board
        let board = Board::starting();
        assert!(!move_generator.king_in_check(&board, true));
        assert!(!move_generator.king_in_check(&board, false));
        // Black rook checks white king
        let board = Board::from_fen("4k3/8/8/4r3/8/8/8/4K3 w - - 0 1");
        assert!(move_generator.king_in_check(&board, true));
        assert!(!move_generator.king_in_check(&board, false));
        // White pawn blocks black rook
        let board = Board::from_fen("4k3/8/8/4r3/8/4P3/8/4K3 w - - 0 1");
        assert!(!move_generator.king_in_check(&board, true));
        assert!(!move_generator.king_in_check(&board, false));
        // White queen checks black king
        let board = Board::from_fen("4k3/8/8/8/8/4Q3/8/4K3 w - - 0 1");
        assert!(!move_generator.king_in_check(&board, true));
        assert!(move_generator.king_in_check(&board, false));
        // Black pawn blocks white queen
        let board = Board::from_fen("4k3/8/4p3/8/8/4Q3/8/4K3 w - - 0 1");
        assert!(!move_generator.king_in_check(&board, true));
        assert!(!move_generator.king_in_check(&board, false));
        // White pawn checks black king
        let board = Board::from_fen("4k3/5P2/8/8/8/8/8/4K3 w - - 0 1");
        assert!(!move_generator.king_in_check(&board, true));
        assert!(move_generator.king_in_check(&board, false));
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
        if depth == 0 {
            return 1;
        }
        let moves = move_generator.legal_moves(board);
        if moves.is_empty() {
            return 0;
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

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
        let checkers = checkers;
        let checker_index = checker_index;
        let king_danger_mask = king_danger_mask;

        let mut moves = Vec::new();

        // King moves are restricted by the danger mask
        let king_index = king_mask.trailing_zeros() as u8;
        let pseudo_legal_king_moves =
            self.lookup.possible_king_moves(king_index) & !friendly_pieces;
        let legal_king_moves = pseudo_legal_king_moves & !king_danger_mask;
        for i in Self::get_indices_of_bits(legal_king_moves) {
            moves.push(Move::regular(king_index, i));
        }

        // If in double check, only legal moves are non-castling king moves
        if checkers > 1 {
            return moves;
        }

        let king_rook_mask = self.lookup.possible_rook_moves(king_index, 0);
        let king_bishop_mask = self.lookup.possible_bishop_moves(king_index, 0);

        // If in single check, capturing or blocking attackers are the only legal non-king moves
        // So restrict possible moves by creating a mask of legal captures and legal blocks
        let mut capture_mask = u64::MAX;
        let mut block_mask = u64::MAX;
        if checkers > 0 {
            let checker_mask = 1 << checker_index;
            capture_mask = 1 << checker_index;
            if board.rooks & checker_mask != 0 {
                block_mask = self.lookup.possible_rook_moves(checker_index, 0) & king_rook_mask;
            } else if board.bishops & checker_mask != 0 {
                block_mask = self.lookup.possible_bishop_moves(checker_index, 0) & king_bishop_mask;
            } else if board.queens & checker_mask != 0 {
                if checker_mask & king_rook_mask != 0 {
                    block_mask = self.lookup.possible_rook_moves(checker_index, 0) & king_rook_mask;
                } else if checker_mask & king_bishop_mask != 0 {
                    block_mask = self.lookup.possible_bishop_moves(checker_index, 0) & king_bishop_mask;
                }
            }
        } else {
            // Only if we're not in check are we allowed to castle
            let kingside_castle_blocker_mask = if white { 0x60 } else { 0x60 << 56 };
            if board.kingside_castling_available(white) // If kingside castling available
                && board.all_pieces() & kingside_castle_blocker_mask == 0 // And no pieces blocking it
                // And no pieces attacking the squares between the king and the rook
                && king_danger_mask & kingside_castle_blocker_mask == 0
            {
                moves.push(Move::kingside_castle(white));
            }
            let queenside_castle_blocker_mask = if white { 0xe } else { 0xe << 56 };
            if board.queenside_castling_available(white) // If queenside castling available
                && board.all_pieces() & queenside_castle_blocker_mask == 0 // And no pieces blocking it
                // And no pieces attacking the squares between the king and the rook
                && king_danger_mask & queenside_castle_blocker_mask == 0
            {
                moves.push(Move::queenside_castle(white));
            }
        }

        // Calculate pin rays
        // A piece is pinned if it is the only piece on the ray between a sliding attacker and the king
        let mut horizontal_pin_rays = Vec::new();
        for i in Self::get_indices_of_bits(enemy_pieces & board.rooks & king_rook_mask) {
            let attack_mask = self.lookup.possible_rook_moves(i, enemy_pieces);
            let ray = attack_mask & king_rook_mask;
            if ray != 0 && (ray & friendly_pieces).count_ones() == 1 {
                horizontal_pin_rays.push(ray | (1 << i));
            }
        }
        let mut diagonal_pin_rays = Vec::new();
        for i in Self::get_indices_of_bits(enemy_pieces & board.bishops & king_bishop_mask) {
            let attack_mask = self.lookup.possible_bishop_moves(i, enemy_pieces);
            let ray = attack_mask & king_bishop_mask;
            if ray != 0 && (ray & friendly_pieces).count_ones() == 1 {
                diagonal_pin_rays.push(ray | (1 << i));
            }
        }
        for i in Self::get_indices_of_bits(
            enemy_pieces & board.queens & (king_rook_mask | king_bishop_mask),
        ) {
            let horizontal_attack_mask = self.lookup.possible_rook_moves(i, enemy_pieces);
            let horizontal_ray = horizontal_attack_mask & king_rook_mask;
            if horizontal_ray != 0 && (horizontal_ray & friendly_pieces).count_ones() == 1 {
                horizontal_pin_rays.push(horizontal_ray | (1 << i));
            }
            let diagonal_attack_mask = self.lookup.possible_bishop_moves(i, enemy_pieces);
            let diagonal_ray = diagonal_attack_mask & king_bishop_mask;
            if diagonal_ray != 0 && (diagonal_ray & friendly_pieces).count_ones() == 1 {
                diagonal_pin_rays.push(diagonal_ray | (1 << i));
            }
        }

        // All other moves are restricted by the capture and block masks
        'friendly_piece_loop: for i in Self::get_indices_of_bits(friendly_pieces & !king_mask) {
            let mask = 1 << i;
            if board.pawns & mask != 0 {
                let mut horizontal_pin_mask = u64::MAX;
                for ray in &horizontal_pin_rays {
                    if ray & mask != 0 {
                        horizontal_pin_mask = *ray;
                        break;
                    }
                }
                let mut diagonal_pin_mask = u64::MAX;
                for ray in &diagonal_pin_rays {
                    if ray & mask != 0 {
                        diagonal_pin_mask = *ray;
                        break;
                    }
                }

                if diagonal_pin_mask == u64::MAX {
                    let push_square = if white { i + 8 } else { i - 8 };
                    let push_mask = 1 << push_square;
                    if push_mask & !board.all_pieces() != 0 {
                        if push_mask & horizontal_pin_mask & block_mask != 0 {
                            let final_rank = if white { 7 } else { 0 };
                            if push_square / 8 == final_rank {
                                moves.push(Move::pawn_promotion(i, push_square, Knight));
                                moves.push(Move::pawn_promotion(i, push_square, Bishop));
                                moves.push(Move::pawn_promotion(i, push_square, Rook));
                                moves.push(Move::pawn_promotion(i, push_square, Queen));
                            } else {
                                moves.push(Move::regular(i, push_square));
                            }
                        }
                        let starting_rank = if white { 1 } else { 6 };
                        if i / 8 == starting_rank {
                            // Double pawn push
                            let double_push_square = if white { i + 16 } else { i - 16 };
                            let double_push_mask = 1 << double_push_square;
                            if double_push_mask & horizontal_pin_mask & block_mask & !board.all_pieces() != 0 {
                                moves.push(Move::double_pawn_push(i));
                            }
                        }
                    }
                }

                if horizontal_pin_mask == u64::MAX {
                    let push_square = if white { i + 8 } else { i - 8 };
                    if i % 8 > 0 {
                        let left_capture_square = push_square - 1;
                        let left_capture_mask = 1 << left_capture_square;
                        if left_capture_mask & diagonal_pin_mask & block_mask & enemy_pieces != 0 {
                            moves.push(Move::regular(i, left_capture_square));
                        }
                    }
                    if i % 8 < 7 {
                        let right_capture_square = push_square + 1;
                        let right_capture_mask = 1 << right_capture_square;
                        if right_capture_mask & diagonal_pin_mask & block_mask & enemy_pieces != 0 {
                            moves.push(Move::regular(i, right_capture_square));
                        }
                    }

                    if let Some(target) = board.en_passant_target() {
                        let target_file = target % 8;
                        let target_rank = target / 8;
                        if i / 8 == target_rank {
                            // If target is to the right of the current square
                            if target_file != 0 && i % 8 == target_file - 1 {
                                moves.push(Move::right_en_passant(i));
                            } else if target_file != 7 && i % 8 == target_file + 1 {
                                moves.push(Move::left_en_passant(i));
                            }
                        }
                        // TODO: Handle en passant discovered check
                    }
                }


            } else if board.knights & mask != 0 {
                // If pinned by a ray, the knight cannot move
                for ray in &horizontal_pin_rays {
                    if ray & mask != 0 {
                        continue 'friendly_piece_loop;
                    }
                }
                for ray in &diagonal_pin_rays {
                    if ray & mask != 0 {
                        continue 'friendly_piece_loop;
                    }
                }

                let legal_end_squares = capture_mask & block_mask;
                let knight_moves =
                    self.lookup.possible_knight_moves(i) & legal_end_squares & !friendly_pieces;
                for j in Self::get_indices_of_bits(knight_moves) {
                    moves.push(Move::regular(i, j));
                }
            } else if board.bishops & mask != 0 {
                // If pinned by a horizontal ray, the bishop cannot move
                for ray in &horizontal_pin_rays {
                    if ray & mask != 0 {
                        continue 'friendly_piece_loop;
                    }
                }
                // If pinned by a diagonal ray, the bishop can only move along the pin ray
                let mut move_mask = u64::MAX;
                for ray in &diagonal_pin_rays {
                    if ray & mask != 0 {
                        move_mask = *ray;
                        break;
                    }
                }

                let legal_end_squares = capture_mask & block_mask & move_mask;
                let bishop_moves = self.lookup.possible_bishop_moves(i, board.all_pieces())
                    & legal_end_squares
                    & !friendly_pieces;
                for j in Self::get_indices_of_bits(bishop_moves) {
                    moves.push(Move::regular(i, j));
                }
            } else if board.rooks & mask != 0 {
                // If pinned by a diagonal ray, the rook cannot move
                for ray in &diagonal_pin_rays {
                    if ray & mask != 0 {
                        continue 'friendly_piece_loop;
                    }
                }
                // If pinned by a horizontal ray, the rook can only move along the ray
                let mut move_mask = u64::MAX;
                for ray in &horizontal_pin_rays {
                    if ray & mask != 0 {
                        move_mask = *ray;
                        break;
                    }
                }

                let legal_end_squares = capture_mask & block_mask & move_mask;
                let rook_moves = self.lookup.possible_rook_moves(i, board.all_pieces())
                    & legal_end_squares
                    & !friendly_pieces;
                for j in Self::get_indices_of_bits(rook_moves) {
                    moves.push(Move::regular(i, j));
                }
            } else if board.queens & mask != 0 {
                let mut move_mask = u64::MAX;
                for ray in &diagonal_pin_rays {
                    if ray & mask != 0 {
                        move_mask = *ray;
                        break;
                    }
                }
                if move_mask == u64::MAX {
                    for ray in &horizontal_pin_rays {
                        if ray & mask != 0 {
                            move_mask = *ray;
                            break;
                        }
                    }
                }

                let legal_end_squares = capture_mask & block_mask & move_mask;
                let queen_moves = self.lookup.possible_queen_moves(i, board.all_pieces())
                    & legal_end_squares
                    & !friendly_pieces;
                for j in Self::get_indices_of_bits(queen_moves) {
                    moves.push(Move::regular(i, j));
                }
            }
        }

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
    fn test_bishop_pin() {
        let move_generator = MoveGenerator::new();
        let mut board = Board::from_fen("7b/8/8/8/8/8/1B6/K7 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 8);
        let mut board = Board::from_fen("8/8/8/8/8/8/KB5r/8 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 4);
        let mut board = Board::from_fen("8/8/8/8/8/8/KB2b2r/8 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 13);
        let mut board = Board::from_fen("8/8/8/5k2/8/8/KB2B2r/8 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 22);
    }

    #[test]
    fn test_pawn_pin() {
        let move_generator = MoveGenerator::new();
        let mut board = Board::from_fen("8/8/8/8/8/8/K1P1r3/8 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 5);
        let mut board = Board::from_fen("8/8/8/8/8/K3r3/2P5/8 w - - 0 1");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 5);
        let mut board = Board::from_fen("7k/8/7r/K3pP1r/7r/8/8/8 w - - 0 2");
        let moves = move_generator.legal_moves(&mut board);
        assert_eq!(moves.len(), 2);
    }

    #[test]
    fn perft_starting() {
        test_perft(Board::starting(), &[20, 400, 8902, 197281, 4865609]);
    }

    #[test]
    fn perft_fools() {
        test_perft(Board::from_fen("rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 1"), &[1])
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
            // assert_eq!(result, expected[depth]);
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

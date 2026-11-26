use crate::move_generation::mov::piece_type;
use crate::move_generation::mov::squares::mask;
pub(crate) use board_state::Board;
use mov::Move;
use std::ops::{Index, IndexMut};

pub mod board_state;
pub mod mov;
pub mod move_lookup;

pub struct Moves {
    moves: [Move; 218],
    pub length: usize,
    pub check: bool,
}

impl Moves {
    pub fn empty() -> Self {
        Moves {
            moves: [Move::null(); 218],
            length: 0,
            check: false,
        }
    }

    fn push(&mut self, mov: Move) {
        self.moves[self.length] = mov;
        self.length += 1;
    }

    pub fn filter_tactical_moves(&self) -> Moves {
        let mut moves = Moves::empty();
        for i in 0..self.length {
            // If capture or queen promotion
            if self.moves[i].capture() || self.moves[i].flags() == 0b100 {
                moves.push(self.moves[i]);
            }
        }
        moves
    }
}

impl Index<usize> for Moves {
    type Output = Move;

    fn index(&self, index: usize) -> &Self::Output {
        &self.moves[index]
    }
}

impl IndexMut<usize> for Moves {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.moves[index]
    }
}

pub fn legal_moves(board: &Board) -> Moves {
    let white = board.white_to_move();
    let friendly_pieces = if white { board.white } else { board.black };
    let enemy_pieces = if white { board.black } else { board.white };

    let king_mask = friendly_pieces & board.kings;
    // King is not included in the blockers, so that the king can't block a
    // move that would leave it in check post-move
    let blockers = board.all_pieces() & !king_mask;
    let (king_danger_mask, checkers, checker_index) =
        king_danger_mask(board, white, enemy_pieces, king_mask, blockers);

    let mut moves = Moves::empty();

    // King moves are restricted by the danger mask
    let king_index = king_mask.trailing_zeros() as u8;
    let pseudo_legal_king_moves = move_lookup::possible_king_moves(king_index) & !friendly_pieces;
    let legal_king_moves = pseudo_legal_king_moves & !king_danger_mask;
    for i in get_indices_of_bits(legal_king_moves) {
        moves.push(Move::regular(king_index, i, enemy_pieces & mask(i) != 0));
    }

    // If in double check, only legal moves are non-castling king moves
    if checkers > 1 {
        moves.check = true;
        return moves;
    }

    // Mask of what enemy pieces can attack the king
    let king_rook_mask = move_lookup::possible_rook_moves(king_index, enemy_pieces);
    let king_bishop_mask = move_lookup::possible_bishop_moves(king_index, enemy_pieces);

    // If in single check, capturing or blocking attackers are the only legal non-king moves
    // So restrict possible moves by creating a mask of legal captures and legal blocks
    let mut capture_mask = u64::MAX;
    let mut block_mask = u64::MAX;
    if checkers > 0 {
        let checker_mask = 1 << checker_index;
        capture_mask = checker_mask;
        if board.rooks & checker_mask != 0 {
            block_mask =
                move_lookup::possible_rook_moves(checker_index, king_mask) & king_rook_mask;
        } else if board.bishops & checker_mask != 0 {
            block_mask =
                move_lookup::possible_bishop_moves(checker_index, king_mask) & king_bishop_mask;
        } else if board.queens & checker_mask != 0 {
            if checker_mask & king_rook_mask != 0 {
                block_mask =
                    move_lookup::possible_rook_moves(checker_index, king_mask) & king_rook_mask;
            } else if checker_mask & king_bishop_mask != 0 {
                block_mask =
                    move_lookup::possible_bishop_moves(checker_index, king_mask) & king_bishop_mask;
            }
        } else if board.pawns & checker_mask != 0 || board.knights & checker_mask != 0 {
            block_mask = 0;
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
        let queenside_castle_danger_mask = if white { 0xc } else { 0xc << 56 };
        if board.queenside_castling_available(white) // If queenside castling available
            && board.all_pieces() & queenside_castle_blocker_mask == 0 // And no pieces blocking it
            // And no pieces attacking the squares between the king and the rook
            && king_danger_mask & queenside_castle_danger_mask == 0
        {
            moves.push(Move::queenside_castle(white));
        }
    }

    // Calculate pin rays
    // A piece is pinned if it is the only piece on the ray between a sliding attacker and the king
    let mut cardinal_pin_rays = Vec::new();
    let mut diagonal_pin_rays = Vec::new();

    let attacking_cardinal_sliders = king_rook_mask & enemy_pieces & (board.rooks | board.queens);
    for i in get_indices_of_bits(attacking_cardinal_sliders) {
        let attacked_squares = move_lookup::
        // We invert the king rook mask to block any moves that are not on the ray between the king and the attacker
        // But also add the king mask to block the ray from going past the king
        possible_rook_moves(i, !king_rook_mask | king_mask)
            & king_rook_mask;
        if (attacked_squares & friendly_pieces).count_ones() == 1 {
            cardinal_pin_rays.push(attacked_squares | (1 << i));
        }
    }
    let attacking_diagonal_sliders =
        king_bishop_mask & enemy_pieces & (board.bishops | board.queens);
    for i in get_indices_of_bits(attacking_diagonal_sliders) {
        let attacked_squares =
            move_lookup::possible_bishop_moves(i, !king_bishop_mask | king_mask) & king_bishop_mask;
        if (attacked_squares & friendly_pieces).count_ones() == 1 {
            diagonal_pin_rays.push(attacked_squares | (1 << i));
        }
    }

    // All other moves are restricted by the capture and block masks
    'friendly_piece_loop: for i in get_indices_of_bits(friendly_pieces & !king_mask) {
        let start_mask = 1 << i;
        if board.pawns & start_mask != 0 {
            let mut cardinal_pin_mask = u64::MAX;
            for ray in &cardinal_pin_rays {
                if ray & start_mask != 0 {
                    cardinal_pin_mask = *ray;
                    break;
                }
            }
            let mut diagonal_pin_mask = u64::MAX;
            for ray in &diagonal_pin_rays {
                if ray & start_mask != 0 {
                    diagonal_pin_mask = *ray;
                    break;
                }
            }

            // We can only push if we're not pinned diagonally
            if diagonal_pin_mask == u64::MAX {
                let push_square = if white { i + 8 } else { i - 8 };
                let push_mask = 1 << push_square;
                if push_mask & !board.all_pieces() != 0 {
                    if push_mask & cardinal_pin_mask & block_mask != 0 {
                        let final_rank = if white { 7 } else { 0 };
                        if push_square / 8 == final_rank {
                            moves.push(Move::pawn_promotion(
                                i,
                                push_square,
                                piece_type::KNIGHT,
                                false,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                push_square,
                                piece_type::BISHOP,
                                false,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                push_square,
                                piece_type::ROOK,
                                false,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                push_square,
                                piece_type::QUEEN,
                                false,
                            ));
                        } else {
                            moves.push(Move::regular(i, push_square, false));
                        }
                    }
                    let starting_rank = if white { 1 } else { 6 };
                    if i / 8 == starting_rank {
                        // Double pawn push
                        let double_push_square = if white { i + 16 } else { i - 16 };
                        let double_push_mask = 1 << double_push_square;
                        if double_push_mask & cardinal_pin_mask & block_mask & !board.all_pieces()
                            != 0
                        {
                            moves.push(Move::double_pawn_push(i));
                        }
                    }
                }
            }

            // We can only push if we're not pinned cardinally
            if cardinal_pin_mask == u64::MAX {
                let push_square = if white { i + 8 } else { i - 8 };
                let final_rank = if white { 7 } else { 0 };
                if i % 8 > 0 {
                    let left_capture_square = push_square - 1;
                    let left_capture_mask = 1 << left_capture_square;
                    if left_capture_mask & diagonal_pin_mask & capture_mask & enemy_pieces != 0 {
                        if push_square / 8 == final_rank {
                            moves.push(Move::pawn_promotion(
                                i,
                                left_capture_square,
                                piece_type::KNIGHT,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                left_capture_square,
                                piece_type::BISHOP,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                left_capture_square,
                                piece_type::ROOK,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                left_capture_square,
                                piece_type::QUEEN,
                                true,
                            ));
                        } else {
                            moves.push(Move::regular(i, left_capture_square, true));
                        }
                    }
                }
                if i % 8 < 7 {
                    let right_capture_square = push_square + 1;
                    let right_capture_mask = 1 << right_capture_square;
                    if right_capture_mask & diagonal_pin_mask & capture_mask & enemy_pieces != 0 {
                        if push_square / 8 == final_rank {
                            moves.push(Move::pawn_promotion(
                                i,
                                right_capture_square,
                                piece_type::KNIGHT,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                right_capture_square,
                                piece_type::BISHOP,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                right_capture_square,
                                piece_type::ROOK,
                                true,
                            ));
                            moves.push(Move::pawn_promotion(
                                i,
                                right_capture_square,
                                piece_type::QUEEN,
                                true,
                            ));
                        } else {
                            moves.push(Move::regular(i, right_capture_square, true));
                        }
                    }
                }

                // If en passant is possible
                if let Some(target) = board.en_passant_target() {
                    let target_file = target % 8;
                    let capture_square = if white { target - 8 } else { target + 8 };
                    let capture_rank = capture_square / 8;
                    if i / 8 == capture_rank // If pawn is at the same rank as the captured pawn
                        && (king_index / 8 != capture_rank // And either the king is not on the same rank
                        || (move_lookup::possible_rook_moves(
                        king_index,
                        // Or: excluding this pawn and the captured pawn from blockers,
                        board.all_pieces() & !start_mask & !(1 << capture_square),
                    ) & 0xFF << (capture_rank * 8)) // and only looking at horizontal attacks
                        // Is there a horizontal attack from a rook or queen
                        & (enemy_pieces & (board.rooks | board.queens)) == 0)
                    {
                        // If target is to the right of the current square
                        if target_file != 0 && i % 8 == target_file - 1 {
                            let end_mask = if white {
                                start_mask << 9
                            } else {
                                start_mask >> 7
                            };
                            if end_mask & block_mask & diagonal_pin_mask != 0 // If we can block the check
                                || (end_mask & diagonal_pin_mask != 0 // Or if we can capture the piece without
                                // moving off a pin
                                && capture_mask & (1 << capture_square) != 0)
                            {
                                moves.push(Move::right_en_passant(i));
                            }
                        } else if target_file != 7 && i % 8 == target_file + 1 {
                            let end_mask = if white {
                                start_mask << 7
                            } else {
                                start_mask >> 9
                            };
                            if end_mask & block_mask & diagonal_pin_mask != 0
                                || (end_mask & diagonal_pin_mask != 0
                                    && capture_mask & (1 << capture_square) != 0)
                            {
                                moves.push(Move::left_en_passant(i));
                            }
                        }
                    }
                }
            }
        } else if board.knights & start_mask != 0 {
            // If pinned by a ray, the knight cannot move
            for ray in &cardinal_pin_rays {
                if ray & start_mask != 0 {
                    continue 'friendly_piece_loop;
                }
            }
            for ray in &diagonal_pin_rays {
                if ray & start_mask != 0 {
                    continue 'friendly_piece_loop;
                }
            }

            let legal_end_squares = capture_mask | block_mask;
            let knight_moves =
                move_lookup::possible_knight_moves(i) & legal_end_squares & !friendly_pieces;
            for j in get_indices_of_bits(knight_moves) {
                moves.push(Move::regular(i, j, enemy_pieces & mask(j) != 0));
            }
        } else if board.bishops & start_mask != 0 {
            // If pinned by a horizontal ray, the bishop cannot move
            for ray in &cardinal_pin_rays {
                if ray & start_mask != 0 {
                    continue 'friendly_piece_loop;
                }
            }
            // If pinned by a diagonal ray, the bishop can only move along the pin ray
            let mut move_mask = u64::MAX;
            for ray in &diagonal_pin_rays {
                if ray & start_mask != 0 {
                    move_mask = *ray;
                    break;
                }
            }

            let legal_end_squares = (capture_mask | block_mask) & move_mask;
            let bishop_moves = move_lookup::possible_bishop_moves(i, board.all_pieces())
                & legal_end_squares
                & !friendly_pieces;
            for j in get_indices_of_bits(bishop_moves) {
                moves.push(Move::regular(i, j, enemy_pieces & mask(j) != 0));
            }
        } else if board.rooks & start_mask != 0 {
            // If pinned by a diagonal ray, the rook cannot move
            for ray in &diagonal_pin_rays {
                if ray & start_mask != 0 {
                    continue 'friendly_piece_loop;
                }
            }
            // If pinned by a horizontal ray, the rook can only move along the ray
            let mut move_mask = u64::MAX;
            for ray in &cardinal_pin_rays {
                if ray & start_mask != 0 {
                    move_mask = *ray;
                    break;
                }
            }

            let legal_end_squares = (capture_mask | block_mask) & move_mask;
            let rook_moves = move_lookup::possible_rook_moves(i, board.all_pieces())
                & legal_end_squares
                & !friendly_pieces;
            for j in get_indices_of_bits(rook_moves) {
                moves.push(Move::regular(i, j, enemy_pieces & mask(j) != 0));
            }
        } else if board.queens & start_mask != 0 {
            // If pinned by a ray, the queen can only move along that ray
            let mut move_mask = u64::MAX;
            for ray in &diagonal_pin_rays {
                if ray & start_mask != 0 {
                    move_mask = *ray;
                    break;
                }
            }
            // No need to check cardinal_pin_rays if we already have a move_mask from the diagonal pins check
            if move_mask == u64::MAX {
                for ray in &cardinal_pin_rays {
                    if ray & start_mask != 0 {
                        move_mask = *ray;
                        break;
                    }
                }
            }

            let legal_end_squares = (capture_mask | block_mask) & move_mask;
            let queen_moves = move_lookup::possible_queen_moves(i, board.all_pieces())
                & legal_end_squares
                & !friendly_pieces;
            for j in get_indices_of_bits(queen_moves) {
                moves.push(Move::regular(i, j, enemy_pieces & mask(j) != 0));
            }
        }
    }

    moves.check = checkers > 0;
    moves
}

fn king_danger_mask(
    board: &Board,
    white: bool,
    enemy_pieces: u64,
    king_mask: u64,
    blockers: u64,
) -> (u64, u32, u8) {
    // For every enemy piece, create a mask of the squares it attacks
    // If the king is currently sitting in an attacked square, it is in check
    let mut king_danger_mask: u64 = 0;
    let mut checkers = 0;
    let mut checker_index = 0;
    for i in get_indices_of_bits(enemy_pieces) {
        let mask = 1 << i;
        if board.pawns & mask != 0 {
            let mut pawn_danger_mask = 0;
            let backwards = if white { i - 8 } else { i + 8 };
            if i % 8 != 0 {
                pawn_danger_mask |= 1 << (backwards - 1);
            }
            if i % 8 != 7 {
                pawn_danger_mask |= 1 << (backwards + 1);
            }
            if king_mask & pawn_danger_mask != 0 {
                checkers += 1;
                checker_index = i;
            }
            king_danger_mask |= pawn_danger_mask;
        } else if board.knights & mask != 0 {
            let possible_knight_moves = move_lookup::possible_knight_moves(i);
            if king_mask & possible_knight_moves != 0 {
                checkers += 1;
                checker_index = i;
            }
            king_danger_mask |= possible_knight_moves;
        } else if board.bishops & mask != 0 {
            let possible_bishop_moves = move_lookup::possible_bishop_moves(i, blockers);
            if king_mask & possible_bishop_moves != 0 {
                checkers += 1;
                checker_index = i;
            }
            king_danger_mask |= possible_bishop_moves;
        } else if board.rooks & mask != 0 {
            let possible_rook_moves = move_lookup::possible_rook_moves(i, blockers);
            if king_mask & possible_rook_moves != 0 {
                checkers += 1;
                checker_index = i;
            }
            king_danger_mask |= possible_rook_moves;
        } else if board.queens & mask != 0 {
            let possible_queen_moves = move_lookup::possible_queen_moves(i, blockers);
            if king_mask & possible_queen_moves != 0 {
                checkers += 1;
                checker_index = i;
            }
            king_danger_mask |= possible_queen_moves;
        } else if board.kings & mask != 0 {
            // Kings can't attack each other legally, so no need to handle checkers counting for that
            king_danger_mask |= move_lookup::possible_king_moves(i);
        }
    }
    (king_danger_mask, checkers, checker_index)
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

/// Check if a move puts the king in check.
///
/// Roughly 90-95% accurate, since this doesn't account for discovered check, en passant, etc.,
/// but doing a full check is slow
pub fn move_puts_king_in_check(board: &Board, mov: &Move) -> bool {
    let white = board.white & mov.starting_mask() != 0;
    let opposing_king = board.kings & if white { board.black } else { board.white };
    let occupied = board.all_pieces() & !mov.starting_mask();
    let ending_square = mov.ending_index();
    let danger_mask = match board.get_piece_at_square(mov.starting_index()) {
        Some(piece_type::PAWN) => match mov.flags() {
            0b0100 => move_lookup::possible_queen_moves(ending_square, occupied),
            0b0101 => move_lookup::possible_rook_moves(ending_square, occupied),
            0b0110 => move_lookup::possible_bishop_moves(ending_square, occupied),
            0b0111 => move_lookup::possible_knight_moves(ending_square),
            _ => {
                let file = ending_square % 8;
                let mut pawn_danger_mask = 0;
                if white && ending_square < 56 {
                    if file > 0 {
                        pawn_danger_mask |= 1 << (ending_square + 7);
                    }
                    if file < 7 {
                        pawn_danger_mask |= 1 << (ending_square + 9);
                    }
                } else if !white && ending_square >= 8 {
                    if file > 0 {
                        pawn_danger_mask |= 1 << (ending_square - 9);
                    }
                    if file < 7 {
                        pawn_danger_mask |= 1 << (ending_square - 7);
                    }
                }
                pawn_danger_mask
            }
        },
        Some(piece_type::KNIGHT) => move_lookup::possible_knight_moves(ending_square),
        Some(piece_type::BISHOP) => move_lookup::possible_bishop_moves(ending_square, occupied),
        Some(piece_type::ROOK) => move_lookup::possible_rook_moves(ending_square, occupied),
        Some(piece_type::QUEEN) => move_lookup::possible_queen_moves(ending_square, occupied),
        _ => return false,
    };
    opposing_king & danger_mask != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_get_squares_of_moves() {
        let moves = 4462742033;
        let squares = get_indices_of_bits(moves);
        assert_eq!(squares, vec![0, 4, 9, 11, 25, 27, 32]);
    }

    #[test]
    fn test_bishop_pin() {
        let board = Board::from_fen("7b/8/8/8/8/8/1B6/K7 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 8);
        let board = Board::from_fen("8/8/8/8/8/8/KB5r/8 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 4);
        let board = Board::from_fen("8/8/8/8/8/8/KB2b2r/8 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 13);
        let board = Board::from_fen("8/8/8/5k2/8/8/KB2B2r/8 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 22);
    }

    #[test]
    fn test_pawn_pin() {
        let board = Board::from_fen("8/8/8/8/8/8/K1P1r3/8 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 5);
        let board = Board::from_fen("8/8/8/8/8/K3r3/2P5/8 w - - 0 1");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 5);
        let board = Board::from_fen("7k/8/7r/K3pP1r/7r/8/8/8 w - - 0 2");
        let moves = legal_moves(&board);
        assert_eq!(moves.length, 2);
    }

    #[test]
    fn promotion_checks_are_detected_for_both_colors() {
        let board = Board::from_fen("7k/6P1/8/8/8/8/8/K7 w - - 0 1");
        let white_promotion = Move::pawn_promotion(54, 62, piece_type::QUEEN, false);
        assert!(move_puts_king_in_check(&board, &white_promotion));

        let board = Board::from_fen("k7/8/8/8/8/8/p7/7K b - - 0 1");
        let black_promotion = Move::pawn_promotion(8, 0, piece_type::QUEEN, false);
        assert!(move_puts_king_in_check(&board, &black_promotion));
    }

    #[test]
    fn perft_starting() {
        test_perft(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            &[20, 400, 8902, 197281, 4865609, 119060324],
        );
    }

    #[test]
    fn perft_fools() {
        test_perft(
            "rnbqkbnr/ppppp1pp/5p2/7Q/8/4P3/PPPP1PPP/RNB1KBNR b KQkq - 1 1",
            &[1],
        )
    }

    #[test]
    fn perft_starting_a2a4() {
        test_perft(
            "rnbqkbnr/pppppppp/8/8/P7/8/1PPPPPPP/RNBQKBNR b KQkq - 0 1",
            &[20, 420, 9329, 217832],
        );
    }

    #[test]
    fn perft_starting_c2c3_b7b5_d1a4() {
        test_perft(
            "rnbqkbnr/p1pppppp/8/1p6/Q7/2P5/PP1PPPPP/RNB1KBNR b KQkq - 1 2",
            &[22],
        );
    }

    #[test]
    fn perft_starting_c2c3_a7a5_d1a4() {
        test_perft(
            "rnbqkbnr/1ppppppp/8/p7/Q7/2P5/PP1PPPPP/RNB1KBNR b KQkq - 1 2",
            &[18],
        )
    }

    #[test]
    fn perft_starting_c2c3_d7d5_d1a4() {
        test_perft(
            "rnbqkbnr/ppp1pppp/8/3p4/Q7/2P5/PP1PPPPP/RNB1KBNR b KQkq - 1 2",
            &[6, 206],
        )
    }

    #[test]
    fn perft_starting_d2d4() {
        test_perft(
            "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1",
            &[20, 560, 12435, 361790],
        )
    }

    #[test]
    fn perft_starting_d2d4_c7c5_e1d2() {
        test_perft(
            "rnbqkbnr/pp1ppppp/8/2p5/3P4/8/PPPKPPPP/RNBQ1BNR b kq - 1 2",
            &[23, 548],
        )
    }

    #[test]
    fn perft_starting_d2d4_c7c5_e1d2_d8a5_c5c4() {
        test_perft(
            "rnbqkbnr/pp1ppppp/8/8/2pP4/8/PPPKPPPP/RNBQ1BNR w kq - 0 3",
            &[22],
        )
    }

    #[test]
    fn perft_starting_d2d4_c7c5_e1d2_d8a5() {
        test_perft(
            "rnb1kbnr/pp1ppppp/8/q1p5/3P4/8/PPPKPPPP/RNBQ1BNR w kq - 2 3",
            &[5],
        )
    }

    #[test]
    fn perft_pawn_blocks_queen() {
        test_perft("r3k3/3p4/8/p7/Q7/8/8/4K3 b q - 0 1", &[10]);
    }

    #[test]
    fn perft_kiwipete() {
        test_perft(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            &[48, 2039, 97862, 4085603],
        )
    }

    #[test]
    fn perft_kiwipete_e5c6() {
        test_perft(
            "r3k2r/p1ppqpb1/bnN1pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 1 1",
            &[41, 2027],
        )
    }

    #[test]
    fn perft_kiwipete_e5d7() {
        test_perft(
            "r3k2r/p1pNqpb1/bn2pnp1/3P4/1p2P3/2N2Q1p/PPPBBPPP/R3K2R b KQkq - 0 1",
            &[45],
        )
    }

    #[test]
    fn perft_kiwipete_c3b1_a6b5() {
        test_perft(
            "r3k2r/p1ppqpb1/1n2pnp1/1b1PN3/1p2P3/5Q1p/PPPBBPPP/RN2K2R w KQkq - 2 2",
            &[48],
        );
    }

    #[test]
    fn perft_kiwipete_a2a3_h3g2_a1a2() {
        test_perft(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/P1N2Q2/RPPBBPpP/4K2R b Kkq - 1 2",
            &[53],
        );
    }

    #[test]
    fn perft_position3() {
        test_perft(
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            &[14, 191, 2812, 43238, 674624],
        );
    }

    #[test]
    fn perft_position3_a5a6_h5h7_a6a7() {
        test_perft(
            "8/K1p4r/3p4/1P6/1R3p1k/8/4P1P1/8 b - - 3 2",
            &[14, 207, 3184],
        )
    }

    #[test]
    fn perft_position3_a5a6_h5h7_a6a7_h4h5_g2g4() {
        test_perft("8/K1p4r/3p4/1P5k/1R3pP1/8/4P3/8 b - g3 0 3", &[6])
    }

    #[test]
    fn perft_en_passant_discovered_check() {
        test_perft(
            "5Q2/p1p1Rppp/2k5/8/3Pp3/6PP/PP4B1/RNB3K1 b - d3 0 1",
            &[13, 512],
        )
    }

    #[test]
    fn perft_en_passant_capture_checker() {
        test_perft(
            "1rr5/1p6/1P1p1nk1/p2Ppb1p/2n2Kp1/2N3P1/P3NPBP/2RR4 w - e6 0 1",
            &[1, 32],
        )
    }

    const TABLE_SIZE: usize = 1 << 20;

    fn test_perft(fen: &str, expected: &[usize]) {
        let mut board = Board::from_fen(fen);
        let mut transposition_tables = vec![vec![(0, 0); TABLE_SIZE]; expected.len()];
        for depth in 0..expected.len() {
            let timer = Instant::now();
            let result = perft(&mut board, depth + 1, &mut transposition_tables, true);
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
        board: &mut Board,
        depth: usize,
        transposition_tables: &mut Vec<Vec<(u64, usize)>>,
        divide: bool,
    ) -> usize {
        let moves = legal_moves(board);
        if moves.length == 0 {
            return 0;
        }
        if depth == 1 {
            return moves.length;
        }
        let mut total_moves = 0;
        for i in 0..moves.length {
            let mov = &moves.moves[i];
            let metadata = board.make_move(&mov);
            let table_key = board.zobrist_hash as usize % TABLE_SIZE;
            let (entry_hash, entry_nodes) = transposition_tables[depth - 1][table_key];
            if entry_hash == board.zobrist_hash {
                total_moves += entry_nodes;
            } else {
                let nodes = perft(board, depth - 1, transposition_tables, false);
                if divide {
                    println!("Move {}: {} nodes", mov, nodes);
                }
                total_moves += nodes;
                transposition_tables[depth - 1][table_key] = (board.zobrist_hash, nodes);
            }
            board.unmake_move(&mov, metadata);
        }
        total_moves
    }
}

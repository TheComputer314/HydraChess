use crate::move_generation::mov::Move;
use crate::move_generation::mov::squares::mask;
use crate::move_generation::{Board, move_lookup};
use std::cmp::max;

macro_rules! piece_square_table {
    (
            $a8:expr, $b8:expr, $c8:expr, $d8:expr, $e8:expr, $f8:expr, $g8:expr, $h8:expr;
            $a7:expr, $b7:expr, $c7:expr, $d7:expr, $e7:expr, $f7:expr, $g7:expr, $h7:expr;
            $a6:expr, $b6:expr, $c6:expr, $d6:expr, $e6:expr, $f6:expr, $g6:expr, $h6:expr;
            $a5:expr, $b5:expr, $c5:expr, $d5:expr, $e5:expr, $f5:expr, $g5:expr, $h5:expr;
            $a4:expr, $b4:expr, $c4:expr, $d4:expr, $e4:expr, $f4:expr, $g4:expr, $h4:expr;
            $a3:expr, $b3:expr, $c3:expr, $d3:expr, $e3:expr, $f3:expr, $g3:expr, $h3:expr;
            $a2:expr, $b2:expr, $c2:expr, $d2:expr, $e2:expr, $f2:expr, $g2:expr, $h2:expr;
            $a1:expr, $b1:expr, $c1:expr, $d1:expr, $e1:expr, $f1:expr, $g1:expr, $h1:expr;
        ) => {
        [
            $a1, $b1, $c1, $d1, $e1, $f1, $g1, $h1, $a2, $b2, $c2, $d2, $e2, $f2, $g2, $h2, $a3,
            $b3, $c3, $d3, $e3, $f3, $g3, $h3, $a4, $b4, $c4, $d4, $e4, $f4, $g4, $h4, $a5, $b5,
            $c5, $d5, $e5, $f5, $g5, $h5, $a6, $b6, $c6, $d6, $e6, $f6, $g6, $h6, $a7, $b7, $c7,
            $d7, $e7, $f7, $g7, $h7, $a8, $b8, $c8, $d8, $e8, $f8, $g8, $h8,
        ]
    };
}

const OPENING_PIECE_VALUES: [i32; 6] = [82, 337, 365, 477, 1025, 0];
const ENDGAME_PIECE_VALUES: [i32; 6] = [94, 281, 297, 512, 936, 0];

const OPENING_SQUARE_VALUES: [[i32; 64]; 6] = [
    piece_square_table!(
          0,   0,   0,   0,   0,   0,  0,   0;
         98, 134,  61,  95,  68, 126, 34, -11;
         -6,   7,  26,  31,  65,  56, 25, -20;
        -14,  13,   6,  21,  23,  12, 17, -23;
        -27,  -2,  -5,  12,  17,   6, 10, -25;
        -26,  -4,  -4, -10,   3,   3, 33, -12;
        -35,  -1, -20, -23, -15,  24, 38, -22;
          0,   0,   0,   0,   0,   0,  0,   0;
    ),
    piece_square_table!(
        -167, -89, -34, -49,  61, -97, -15, -107;
         -73, -41,  72,  36,  23,  62,   7,  -17;
         -47,  60,  37,  65,  84, 129,  73,   44;
          -9,  17,  19,  53,  37,  69,  18,   22;
         -13,   4,  16,  13,  28,  19,  21,   -8;
         -23,  -9,  12,  10,  19,  17,  25,  -16;
         -29, -53, -12,  -3,  -1,  18, -14,  -19;
        -105, -21, -58, -33, -17, -28, -19,  -23;
    ),
    piece_square_table!(
        -29,   4, -82, -37, -25, -42,   7,  -8;
        -26,  16, -18, -13,  30,  59,  18, -47;
        -16,  37,  43,  40,  35,  50,  37,  -2;
         -4,   5,  19,  50,  37,  37,   7,  -2;
         -6,  13,  13,  26,  34,  12,  10,   4;
          0,  15,  15,  15,  14,  27,  18,  10;
          4,  15,  16,   0,   7,  21,  33,   1;
        -33,  -3, -14, -21, -13, -12, -39, -21;
    ),
    piece_square_table!(
        32,  42,  32,  51, 63,  9,  31,  43;
        27,  32,  58,  62, 80, 67,  26,  44;
        -5,  19,  26,  36, 17, 45,  61,  16;
       -24, -11,   7,  26, 24, 35,  -8, -20;
       -36, -26, -12,  -1,  9, -7,   6, -23;
       -45, -25, -16, -17,  3,  0,  -5, -33;
       -44, -16, -20,  -9, -1, 11,  -6, -71;
       -19, -13,   1,  17, 16,  7, -37, -26;
    ),
    piece_square_table!(
        -28,   0,  29,  12,  59,  44,  43,  45;
        -24, -39,  -5,   1, -16,  57,  28,  54;
        -13, -17,   7,   8,  29,  56,  47,  57;
        -27, -27, -16, -16,  -1,  17,  -2,   1;
         -9, -26,  -9, -10,  -2,  -4,   3,  -3;
        -14,   2, -11,  -2,  -5,   2,  14,   5;
        -35,  -8,  11,   2,   8,  15,  -3,   1;
         -1, -18,  -9,  10, -15, -25, -31, -50;
    ),
    piece_square_table!(
        -65,  23,  16, -15, -56, -34,   2,  13;
         29,  -1, -20,  -7,  -8,  -4, -38, -29;
         -9,  24,   2, -16, -20,   6,  22, -22;
        -17, -20, -12, -27, -30, -25, -14, -36;
        -49,  -1, -27, -39, -46, -44, -33, -51;
        -14, -14, -22, -46, -44, -30, -15, -27;
          1,   7,  -8, -64, -43, -16,   9,   8;
        -15,  36,  12, -54,   8, -28,  24,  14;
    ),
];

const ENDGAME_SQUARE_VALUES: [[i32; 64]; 6] = [
    piece_square_table!(
        0,   0,   0,   0,   0,   0,   0,   0;
      178, 173, 158, 134, 147, 132, 165, 187;
       94, 100,  85,  67,  56,  53,  82,  84;
       32,  24,  13,   5,  -2,   4,  17,  17;
       13,   9,  -3,  -7,  -7,  -8,   3,  -1;
        4,   7,  -6,   1,   0,  -5,  -1,  -8;
       13,   8,   8,  10,  13,   0,   2,  -7;
        0,   0,   0,   0,   0,   0,   0,   0;
    ),
    piece_square_table!(
        -58, -38, -13, -28, -31, -27, -63, -99;
        -25,  -8, -25,  -2,  -9, -25, -24, -52;
        -24, -20,  10,   9,  -1,  -9, -19, -41;
        -17,   3,  22,  22,  22,  11,   8, -18;
        -18,  -6,  16,  25,  16,  17,   4, -18;
        -23,  -3,  -1,  15,  10,  -3, -20, -22;
        -42, -20, -10,  -5,  -2, -20, -23, -44;
        -29, -51, -23, -15, -22, -18, -50, -64;
    ),
    piece_square_table!(
        -14, -21, -11,  -8, -7,  -9, -17, -24;
        -8,  -4,   7, -12, -3, -13,  -4, -14;
         2,  -8,   0,  -1, -2,   6,   0,   4;
        -3,   9,  12,   9, 14,  10,   3,   2;
        -6,   3,  13,  19,  7,  10,  -3,  -9;
       -12,  -3,   8,  10, 13,   3,  -7, -15;
       -14, -18,  -7,  -1,  4,  -9, -15, -27;
       -23,  -9, -23,  -5, -9, -16,  -5, -17;
    ),
    piece_square_table!(
        13, 10, 18, 15, 12,  12,   8,   5;
        11, 13, 13, 11, -3,   3,   8,   3;
         7,  7,  7,  5,  4,  -3,  -5,  -3;
         4,  3, 13,  1,  2,   1,  -1,   2;
         3,  5,  8,  4, -5,  -6,  -8, -11;
        -4,  0, -5, -1, -7, -12,  -8, -16;
        -6, -6,  0,  2, -9,  -9, -11,  -3;
        -9,  2,  3, -1, -5, -13,   4, -20;
    ),
    piece_square_table!(
         -9,  22,  22,  27,  27,  19,  10,  20;
        -17,  20,  32,  41,  58,  25,  30,   0;
        -20,   6,   9,  49,  47,  35,  19,   9;
          3,  22,  24,  45,  57,  40,  57,  36;
        -18,  28,  19,  47,  31,  34,  39,  23;
        -16, -27,  15,   6,   9,  17,  10,   5;
        -22, -23, -30, -16, -16, -23, -36, -32;
        -33, -28, -22, -43,  -5, -32, -20, -41;
    ),
    piece_square_table!(
        -74, -35, -18, -18, -11,  15,   4, -17;
        -12,  17,  14,  17,  17,  38,  23,  11;
         10,  17,  23,  15,  20,  45,  44,  13;
         -8,  22,  24,  27,  26,  33,  26,   3;
        -18,  -4,  21,  24,  27,  23,   9, -11;
        -19,  -3,  11,  21,  23,  16,   7,  -9;
        -27, -11,   4,  13,  14,   4,  -5, -17;
        -53, -34, -21, -11, -28, -14, -24, -43;
    ),
];

pub(crate) fn evaluate(board: &Board) -> i32 {
    let mut score = 0;
    let pieces_left = board.white.count_ones() + board.black.count_ones();
    for square in 0..64 {
        let mask = mask(square);
        if mask & board.all_pieces() == 0 {
            continue;
        }
        let white = mask & board.white != 0;
        let color_mult = if white { 1 } else { -1 };
        let piece_square_lookup = if white {
            square as usize
        } else {
            // Flip rank but keep file
            ((7 - (square / 8)) * 8 + square % 8) as usize
        };
        let piece_type = if mask & board.pawns != 0 {
            0
        } else if mask & board.knights != 0 {
            1
        } else if mask & board.bishops != 0 {
            2
        } else if mask & board.rooks != 0 {
            3
        } else if mask & board.queens != 0 {
            4
        } else if mask & board.kings != 0 {
            5
        } else {
            continue;
        };
        let value = get_value(piece_type, piece_square_lookup, pieces_left);
        score += value * color_mult;
    }
    score
}

fn get_value(piece_type: usize, square: usize, pieces_left: u32) -> i32 {
    lerp_value(
        OPENING_PIECE_VALUES[piece_type] + OPENING_SQUARE_VALUES[piece_type][square],
        ENDGAME_PIECE_VALUES[piece_type] + ENDGAME_SQUARE_VALUES[piece_type][square],
        pieces_left,
    )
}

fn lerp_value(opening: i32, endgame: i32, pieces_left: u32) -> i32 {
    let a = opening;
    let b = endgame;
    // Remove kings from count
    let t = pieces_left as i32 - 2;
    (a * (30 - t) + b * t) / 30
}

const PIECE_VALUES: [i32; 6] = [100, 220, 430, 600, 1000, 20_000];

pub fn mvv_lva(attacker: usize, victim: usize) -> i32 {
    PIECE_VALUES[victim] * 10 - PIECE_VALUES[attacker]
}

pub fn static_exchange_evaluation(board: &Board, mov: &Move) -> i32 {
    let mut gain = [0; 32];
    let mut depth = 0;

    let mut white = !board.white_to_move();
    let mut occupied_squares = board.white | board.black;

    let capture_type = if mov.flags() == 0b0001 {
        0
    } else {
        match board.get_piece_at_square(mov.ending_index()) {
            Some(piece_type) => piece_type as usize,
            None => return 0,
        }
    };
    gain[0] = PIECE_VALUES[capture_type];

    let mut current_attacker_type =
        board.get_piece_at_square(mov.starting_index()).unwrap() as usize;
    if current_attacker_type == 0 {
        current_attacker_type = match mov.flags() {
            0b0100 => 4,
            0b0101 => 3,
            0b0110 => 2,
            0b0111 => 1,
            _ => 0,
        };
    }

    // The moving piece leaves its origin, and an en-passant capture also
    // removes the pawn that is not on the destination square.
    occupied_squares &= !mov.starting_mask();
    if mov.flags() == 0b0001 {
        let captured_pawn_square = if mov.ending_index() % 8 < mov.starting_index() % 8 {
            mov.starting_index() - 1
        } else {
            mov.starting_index() + 1
        };
        occupied_squares &= !(1 << captured_pawn_square);
    }
    let mut attackers = get_attackers(board, mov.ending_index(), occupied_squares);

    loop {
        let friendly_pieces = if white { board.white } else { board.black };
        let active_attackers = friendly_pieces & attackers & occupied_squares;

        // Find the least valuable attacker available for this side
        let mut chosen_square = None;
        let mut selected_attacker_type = None;
        for piece_type in 0..6 {
            let piece_mask = match piece_type {
                0 => board.pawns,
                1 => board.knights,
                2 => board.bishops,
                3 => board.rooks,
                4 => board.queens,
                5 => board.kings,
                _ => u64::MAX,
            };

            let subset = active_attackers & piece_mask;
            if subset != 0 {
                selected_attacker_type = Some(piece_type);
                chosen_square = Some(subset.trailing_zeros() as u8);
                break;
            }
        }

        // If no captures to be made, break
        let attacker_sq = match chosen_square {
            Some(sq) => sq,
            None => break,
        };
        let selected_attacker_type = selected_attacker_type.unwrap();

        depth += 1;
        gain[depth] = PIECE_VALUES[current_attacker_type] - gain[depth - 1];
        if max(-gain[depth - 1], gain[depth]) < 0 {
            break;
        }

        // Remove the capturing piece from the virtual board state
        occupied_squares &= !(1 << attacker_sq);

        // Re-trigger Bishop/Queen rays through the newly vacant square
        attackers |= move_lookup::possible_bishop_moves(mov.ending_index(), occupied_squares)
            & (board.bishops | board.queens);
        // Re-trigger Rook/Queen rays through the newly vacant square
        attackers |= move_lookup::possible_rook_moves(mov.ending_index(), occupied_squares)
            & (board.rooks | board.queens);

        // If king capture
        if selected_attacker_type == 5 {
            let enemy_pieces = if white { board.black } else { board.white };
            // And enemy pieces can attack the king post-move
            if attackers & enemy_pieces & occupied_squares != 0 {
                depth -= 1; // Roll back this move, we're putting ourselves in check
            }
            break; // Break either way since king capture is always a last resort
        }

        current_attacker_type = selected_attacker_type;
        if current_attacker_type == 0
            && ((white && mov.ending_index() / 8 == 7) || (!white && mov.ending_index() / 8 == 0))
        {
            current_attacker_type = 4;
        }
        white = !white;
    }

    while depth > 0 {
        gain[depth - 1] = -max(-gain[depth - 1], gain[depth]);
        depth -= 1;
    }

    gain[0]
}

fn get_attackers(board: &Board, square: u8, occupied_squares: u64) -> u64 {
    let mut attackers = 0;

    if square / 8 > 0 {
        if square % 8 != 0 {
            attackers |= (1 << (square - 9)) & board.pawns & board.white;
        }
        if square % 8 != 7 {
            attackers |= (1 << (square - 7)) & board.pawns & board.white;
        }
    }
    if square / 8 < 7 {
        if square % 8 != 0 {
            attackers |= (1 << (square + 7)) & board.pawns & board.black;
        }
        if square % 8 != 7 {
            attackers |= (1 << (square + 9)) & board.pawns & board.black;
        }
    }

    attackers |= move_lookup::possible_knight_moves(square) & board.knights;
    attackers |= move_lookup::possible_bishop_moves(square, occupied_squares) & board.bishops;
    attackers |= move_lookup::possible_rook_moves(square, occupied_squares) & board.rooks;
    attackers |= move_lookup::possible_queen_moves(square, occupied_squares) & board.queens;
    attackers |= move_lookup::possible_king_moves(square) & board.kings;
    attackers
}

#[cfg(test)]
mod tests {
    use crate::move_generation::Board;
    use crate::move_generation::mov::Move;
    use crate::search::evaluation::static_exchange_evaluation;

    fn see(fen: &str, starting_square: u8, ending_square: u8) -> i32 {
        let board = Board::from_fen(fen);
        static_exchange_evaluation(&board, &Move::regular(starting_square, ending_square))
    }

    #[test]
    fn see_values_capture_without_recapture() {
        assert_eq!(see("4k3/8/8/8/3r4/2B5/8/4K3 w - - 0 1", 18, 27), 600);
    }

    #[test]
    fn see_accounts_for_a_recapture() {
        assert_eq!(see("4k3/8/8/4p3/3r4/2B5/8/4K3 w - - 0 1", 18, 27), 170);
    }

    #[test]
    fn see_uses_the_correct_pawn_attack_direction() {
        assert_eq!(see("4k3/8/8/3r4/4P3/8/8/4K3 w - - 0 1", 28, 35), 600);
        assert_eq!(see("4k3/8/8/4p3/3R4/8/8/4K3 b - - 0 1", 36, 27), 600);
    }
}

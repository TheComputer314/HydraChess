use crate::move_generation::Board;
use crate::move_generation::mov::squares::mask;

pub struct Evaluation;

impl Evaluation {
    const PIECE_VALUES: [i64; 5] = [100, 300, 300, 500, 900];

    pub(crate) fn evaluate(board: &Board) -> i64 {
        let mut score = 0;
        for square in 0..64 {
            let mask = mask(square);
            if mask & board.all_pieces() == 0 {
                continue;
            }
            let color_mult = if mask & board.white != 0 { 1 } else { -1 };
            if mask & board.pawns != 0 {
                score += Evaluation::PIECE_VALUES[0] * color_mult;
            } else if mask & board.knights != 0 {
                score += Evaluation::PIECE_VALUES[1] * color_mult;
            } else if mask & board.bishops != 0 {
                score += Evaluation::PIECE_VALUES[2] * color_mult;
            } else if mask & board.rooks != 0 {
                score += Evaluation::PIECE_VALUES[3] * color_mult;
            } else if mask & board.queens != 0 {
                score += Evaluation::PIECE_VALUES[4] * color_mult;
            }
        }
        score
    }
}
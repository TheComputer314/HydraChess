use core::arch::x86_64::_pext_u64;

pub struct MoveLookup {
    rook_moves: [Vec<u64>; 64],
    bishop_moves: [Vec<u64>; 64],
    // Bishop occupancy masks are slow as fuck to generate, so we precompute them
    // Rooks aren't as bad, but for consistency we also precompute them
    rook_occupancy_masks: [u64; 64],
    bishop_occupancy_masks: [u64; 64],
    knight_moves: [u64; 64],
    king_moves: [u64; 64],
}
impl MoveLookup {
    pub fn new() -> Self {
        let mut rook_moves = [const { Vec::new() }; 64];
        let mut bishop_moves = [const { Vec::new() }; 64];
        let mut rook_occupancy_masks = [0; 64];
        let mut bishop_occupancy_masks = [0; 64];
        let mut knight_moves = [0; 64];
        let mut king_moves = [0; 64];
        for i in 0..64 {
            rook_occupancy_masks[i] = MoveLookup::rook_occupancy_mask(i as u8);
            bishop_occupancy_masks[i] = MoveLookup::bishop_occupancy_mask(i as u8);

            for attack_mask in MoveLookup::get_mask_subsets(rook_occupancy_masks[i]) {
                rook_moves[i].push(Self::rook_move_mask(i as u8, attack_mask));
            }
            for attack_mask in MoveLookup::get_mask_subsets(bishop_occupancy_masks[i]) {
                bishop_moves[i].push(Self::bishop_move_mask(i as u8, attack_mask));
            }
            knight_moves[i] = Self::knight_move_mask(i as u8);
            king_moves[i] = Self::king_move_mask(i as u8);
        }
        Self {
            rook_moves,
            bishop_moves,
            rook_occupancy_masks,
            bishop_occupancy_masks,
            knight_moves,
            king_moves,
        }
    }

    // The very last square on a sliding direction is ignored, since they aren't relevant for determining if a move
    // is blocked by a piece.
    fn rook_occupancy_mask(square: u8) -> u64 {
        ((0x7e << ((square / 8) * 8)) | (0x1010101010100 << (square % 8))) & !(1 << square)
    }

    fn bishop_occupancy_mask(square: u8) -> u64 {
        let mut mask = 0;
        let mut northeast_square = square;
        loop {
            if northeast_square % 8 >= 6 || northeast_square / 8 >= 6 {
                break;
            }
            northeast_square += 9;
            mask |= 1 << northeast_square;
        }
        let mut northwest_square = square;
        loop {
            if northwest_square % 8 <= 1 || northwest_square / 8 >= 6 {
                break;
            }
            northwest_square += 7;
            mask |= 1 << northwest_square;
        }
        let mut southeast_square = square;
        loop {
            if southeast_square % 8 >= 6 || southeast_square / 8 <= 1 {
                break;
            }
            southeast_square -= 7;
            mask |= 1 << southeast_square;
        }
        let mut southwest_square = square;
        loop {
            if southwest_square % 8 <= 1 || southwest_square / 8 <= 1 {
                break;
            }
            southwest_square -= 9;
            mask |= 1 << southwest_square;
        }

        mask
    }

    fn get_mask_subsets(blocker: u64) -> Vec<u64> {
        let mut subsets = Vec::new();
        let mut subset: u64 = 0;
        loop {
            subsets.push(subset);
            subset = subset.wrapping_sub(blocker) & blocker;
            if subset == 0 {
                break;
            }
        }
        subsets
    }

    fn rook_move_mask(square: u8, attack_mask: u64) -> u64 {
        let start_rank = square / 8;
        let start_file = square % 8;
        let mut mask = 0;
        if start_rank < 7 {
            for rank in (start_rank + 1)..8 {
                let north_mask = 1 << (rank * 8 + start_file);
                mask |= north_mask;
                if north_mask & attack_mask != 0 {
                    break;
                }
            }
        }
        if start_file < 7 {
            for file in (start_file + 1)..8 {
                let east_mask = 1 << (start_rank * 8 + file);
                mask |= east_mask;
                if east_mask & attack_mask != 0 {
                    break;
                }
            }
        }
        if start_rank > 0 {
            for rank in (0..start_rank).rev() {
                let south_mask = 1 << (rank * 8 + start_file);
                mask |= south_mask;
                if south_mask & attack_mask != 0 {
                    break;
                }
            }
        }
        if start_file > 0 {
            for file in (0..start_file).rev() {
                let west_mask = 1 << (start_rank * 8 + file);
                mask |= west_mask;
                if west_mask & attack_mask != 0 {
                    break;
                }
            }
        }

        mask
    }

    fn bishop_move_mask(square: u8, attack_mask: u64) -> u64 {
        let mut mask = 0;
        let mut northeast_square = square;
        loop {
            if northeast_square % 8 >= 7 || northeast_square / 8 >= 7 {
                break;
            }
            northeast_square += 9;
            let northeast_mask = 1 << northeast_square;
            mask |= northeast_mask;
            if northeast_mask & attack_mask != 0 {
                break;
            }
        }
        let mut northwest_square = square;
        loop {
            if northwest_square % 8 == 0 || northwest_square / 8 >= 7 {
                break;
            }
            northwest_square += 7;
            let northwest_mask = 1 << northwest_square;
            mask |= northwest_mask;
            if northwest_mask & attack_mask != 0 {
                break;
            }
        }
        let mut southeast_square = square;
        loop {
            if southeast_square % 8 >= 7 || southeast_square / 8 == 0 {
                break;
            }
            southeast_square -= 7;
            let southeast_mask = 1 << southeast_square;
            mask |= southeast_mask;
            if southeast_mask & attack_mask != 0 {
                break;
            }
        }
        let mut southwest_square = square;
        loop {
            if southwest_square % 8 == 0 || southwest_square / 8 == 0 {
                break;
            }
            southwest_square -= 9;
            let southwest_mask = 1 << southwest_square;
            mask |= southwest_mask;
            if southwest_mask & attack_mask != 0 {
                break;
            }
        }

        mask
    }

    fn knight_move_mask(square: u8) -> u64 {
        let b = 1 << square;
        ((b << 17) & !0x101010101010101) | // NNE
            ((b << 10) & !0x303030303030303) | // ENE
            ((b >> 6) & !0x303030303030303) | // ESE
            ((b >> 15) & !0x101010101010101) | // SSE
            ((b >> 17) & !0x8080808080808080) | // SSW
                ((b >> 10) & !0xc0c0c0c0c0c0c0c0) | // WSW
            ((b << 6) & !0xc0c0c0c0c0c0c0c0) | // WNW
            ((b << 15) & !0x8080808080808080) // NNW
    }

    fn king_move_mask(square: u8) -> u64 {
        let b = 1 << square;
        let mut mask = b;
        if square % 8 != 0 {
            mask |= b >> 1;
        }
        if square % 8 != 7 {
            mask |= b << 1;
        }
        if square / 8 != 0 {
            mask |= mask >> 8;
        }
        if square / 8 != 7 {
            mask |= mask << 8;

        }
        mask & !b
    }

    pub fn possible_rook_moves(&self, square: u8, blocker_pieces: u64) -> u64 {
        let mask;
        unsafe {
            mask = self.rook_moves[square as usize]
                [_pext_u64(blocker_pieces, self.rook_occupancy_masks[square as usize]) as usize];
        }
        mask
    }

    pub fn possible_bishop_moves(&self, square: u8, blocker_pieces: u64) -> u64 {
        let mask;
        unsafe {
            mask = self.bishop_moves[square as usize]
                [_pext_u64(blocker_pieces, self.bishop_occupancy_masks[square as usize]) as usize];
        }
        mask
    }

    pub fn possible_queen_moves(&self, square: u8, blocker_pieces: u64) -> u64 {
        self.possible_bishop_moves(square, blocker_pieces) | self.possible_rook_moves(square, blocker_pieces)
    }

    pub fn possible_knight_moves(&self, square: u8) -> u64 {
        self.knight_moves[square as usize]
    }

    pub fn possible_king_moves(&self, square: u8) -> u64 {
        self.king_moves[square as usize]
    }
}

#[cfg(test)]
mod tests {
    use crate::move_generation::mov::squares::{A1, C3, G3, H4, H8};
    use crate::move_generation::move_lookup::MoveLookup;
    use std::time::Instant;

    #[test]
    fn time_loading() {
        let start = Instant::now();
        let _ = MoveLookup::new();
        println!("Time taken to load MoveLookup: {:?}", start.elapsed());
    }

    #[test]
    fn test_bishop_occupancy_mask() {
        assert_eq!(MoveLookup::bishop_occupancy_mask(28), 637888545440768);
    }

    #[test]
    fn test_rook_move_mask() {
        assert_eq!(MoveLookup::rook_move_mask(0, 0), 0x1010101010101fe);
        assert_eq!(MoveLookup::rook_move_mask(0, 4), 0x101010101010106);
        assert_eq!(MoveLookup::rook_move_mask(27, 0), 0x8080808f7080808);
    }

    #[test]
    fn test_lookup() {
        let lookup = MoveLookup::new();
        assert_eq!(lookup.possible_rook_moves(C3, 0), 289360691368494084);
        assert_eq!(lookup.possible_rook_moves(C3, 17867064999936), 289360691353814020);
        assert_eq!(lookup.possible_rook_moves(C3, 17867069194240), 289360691353814020);
        assert_eq!(lookup.possible_bishop_moves(C3, 0), 9241421692918565393);
        assert_eq!(lookup.possible_bishop_moves(C3, 134217728), 4462742033);
        assert_eq!(lookup.possible_bishop_moves(C3, 35184506306560), 4462742033);
    }

    #[test]
    fn test_knight_moves() {
        let lookup = MoveLookup::new();
        assert_eq!(lookup.possible_knight_moves(C3), 0xa1100110a);
        assert_eq!(lookup.possible_knight_moves(A1), 0x20400);
        assert_eq!(lookup.possible_knight_moves(G3), 0xa0100010a0);
    }

    #[test]
    fn test_king_moves() {
        let lookup = MoveLookup::new();
        assert_eq!(lookup.possible_king_moves(C3), 0xe0a0e00);
        assert_eq!(lookup.possible_king_moves(A1), 0x302);
        assert_eq!(lookup.possible_king_moves(H8), 0x40c0000000000000);
        assert_eq!(lookup.possible_king_moves(H4), 0xc040c00000);
    }
}

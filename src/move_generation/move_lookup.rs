use core::arch::x86_64::_pext_u64;
use std::sync::LazyLock;

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

static ROOK_MOVES: LazyLock<[Vec<u64>; 64]> = LazyLock::new(|| {
    let mut moves = [const { Vec::new() }; 64];
    let mut i = 0;
    while i < 64 {
        for subset in get_mask_subsets(ROOK_OCCUPANCY_MASKS[i]) {
            moves[i].push(rook_move_mask(i as u8, subset));
        }
        i += 1;
    }
    moves
});
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
const ROOK_OCCUPANCY_MASKS: [u64; 64] = {
    let mut masks = [0; 64];
    let mut i = 0;
    while i < 64 {
        masks[i] = ((0x7e << ((i / 8) * 8)) | (0x1010101010100 << (i % 8))) & !(1 << i);
        i += 1;
    }
    masks
};
pub fn possible_rook_moves(square: u8, blocker_pieces: u64) -> u64 {
    let mask;
    unsafe {
        mask = ROOK_MOVES[square as usize]
            [_pext_u64(blocker_pieces, ROOK_OCCUPANCY_MASKS[square as usize]) as usize];
    }
    mask
}

static BISHOP_MOVES: LazyLock<[Vec<u64>; 64]> = LazyLock::new(|| {
    let mut moves = [const { Vec::new() }; 64];
    let mut i = 0;
    while i < 64 {
        for subset in get_mask_subsets(BISHOP_OCCUPANCY_MASKS[i]) {
            moves[i].push(bishop_move_mask(i as u8, subset));
        }
        i += 1;
    }
    moves
});
const fn bishop_move_mask(square: u8, attack_mask: u64) -> u64 {
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
const BISHOP_OCCUPANCY_MASKS: [u64; 64] = {
    let mut masks = [0; 64];
    let mut i = 0;
    while i < 64 {
        masks[i] = bishop_occupancy_mask(i as u8);
        i += 1;
    }
    masks
};
const fn bishop_occupancy_mask(square: u8) -> u64 {
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

pub fn initialize() {
    LazyLock::force(&ROOK_MOVES);
    LazyLock::force(&BISHOP_MOVES);
}

pub fn possible_bishop_moves(square: u8, blocker_pieces: u64) -> u64 {
    let mask;
    unsafe {
        mask = BISHOP_MOVES[square as usize]
            [_pext_u64(blocker_pieces, BISHOP_OCCUPANCY_MASKS[square as usize]) as usize];
    }
    mask
}

pub fn possible_queen_moves(square: u8, blocker_pieces: u64) -> u64 {
    possible_bishop_moves(square, blocker_pieces) | possible_rook_moves(square, blocker_pieces)
}

const KNIGHT_MOVES: [u64; 64] = {
    let mut moves = [0; 64];
    let mut i = 0;
    while i < 64 {
        let b = 1 << i;
        moves[i] = ((b << 17) & !0x101010101010101) | // NNE
            ((b << 10) & !0x303030303030303) | // ENE
            ((b >> 6) & !0x303030303030303) | // ESE
            ((b >> 15) & !0x101010101010101) | // SSE
            ((b >> 17) & !0x8080808080808080) | // SSW
            ((b >> 10) & !0xc0c0c0c0c0c0c0c0) | // WSW
            ((b << 6) & !0xc0c0c0c0c0c0c0c0) | // WNW
            ((b << 15) & !0x8080808080808080); // NNW
        i += 1;
    }
    moves
};
pub const fn possible_knight_moves(square: u8) -> u64 {
    KNIGHT_MOVES[square as usize]
}

const KING_MOVES: [u64; 64] = {
    let mut moves = [0; 64];
    let mut i = 0;
    while i < 64 {
        let b = 1 << i;
        let mut mask = b;
        if i % 8 != 0 {
            mask |= b >> 1;
        }
        if i % 8 != 7 {
            mask |= b << 1;
        }
        if i / 8 != 0 {
            mask |= mask >> 8;
        }
        if i / 8 != 7 {
            mask |= mask << 8;
        }
        moves[i] = mask & !b;
        i += 1;
    }
    moves
};
pub const fn possible_king_moves(square: u8) -> u64 {
    KING_MOVES[square as usize]
}

#[cfg(test)]
mod tests {
    use crate::move_generation::mov::squares::{A1, C3, G3, H4, H8};
    use crate::move_generation::move_lookup::*;
    use std::time::Instant;

    #[test]
    fn time_loading() {
        let start = Instant::now();
        let _ = possible_queen_moves(0, 0);
        println!("Time taken to load MoveLookup: {:?}", start.elapsed());
    }

    #[test]
    fn test_bishop_occupancy_mask() {
        assert_eq!(bishop_occupancy_mask(28), 637888545440768);
    }

    #[test]
    fn test_rook_move_mask() {
        assert_eq!(rook_move_mask(0, 0), 0x1010101010101fe);
        assert_eq!(rook_move_mask(0, 4), 0x101010101010106);
        assert_eq!(rook_move_mask(27, 0), 0x8080808f7080808);
    }

    #[test]
    fn test_lookup() {
        assert_eq!(possible_rook_moves(C3, 0), 289360691368494084);
        assert_eq!(possible_rook_moves(C3, 17867064999936), 289360691353814020);
        assert_eq!(possible_rook_moves(C3, 17867069194240), 289360691353814020);
        assert_eq!(possible_bishop_moves(C3, 0), 9241421692918565393);
        assert_eq!(possible_bishop_moves(C3, 134217728), 4462742033);
        assert_eq!(possible_bishop_moves(C3, 35184506306560), 4462742033);
    }

    #[test]
    fn test_knight_moves() {
        assert_eq!(possible_knight_moves(C3), 0xa1100110a);
        assert_eq!(possible_knight_moves(A1), 0x20400);
        assert_eq!(possible_knight_moves(G3), 0xa0100010a0);
    }

    #[test]
    fn test_king_moves() {
        assert_eq!(possible_king_moves(C3), 0xe0a0e00);
        assert_eq!(possible_king_moves(A1), 0x302);
        assert_eq!(possible_king_moves(H8), 0x40c0000000000000);
        assert_eq!(possible_king_moves(H4), 0xc040c00000);
    }
}

use crate::move_generation::mov::squares::*;
use std::fmt;
use std::fmt::Formatter;

/// Represents a move in the game.
///
/// From least significant bit:
/// - 6 bits: starting square
/// - 6 bits: ending square
/// - 4 bits: flags
///   - 0b0000: None
///   - 0b0001: En passant
///   - 0b0010: Castle
///   - 0b0011: Double pawn push
///   - 0b0100: Queen promotion
///   - 0b0101: Rook promotion
///   - 0b0110: Bishop promotion
///   - 0b0111: Knight promotion
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Move {
    pub data: u16,
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let starting_square = to_name(self.starting_index());
        let ending_square = to_name(self.ending_index());
        write!(f, "{}{}", starting_square, ending_square)?;
        match self.flags() {
            0b0100 => write!(f, "q")?,
            0b0101 => write!(f, "r")?,
            0b0110 => write!(f, "b")?,
            0b0111 => write!(f, "n")?,
            _ => {}
        }

        Ok(())
    }
}

impl Move {
    pub fn regular(starting_index: u8, ending_index: u8) -> Self {
        Self::new(starting_index, ending_index, 0)
    }

    pub fn pawn_promotion(starting_index: u8, ending_index: u8, promotion: u8) -> Self {
        let flags = match promotion {
            piece_type::QUEEN => 0b0100,
            piece_type::ROOK => 0b0101,
            piece_type::BISHOP => 0b0110,
            piece_type::KNIGHT => 0b0111,
            _ => panic!("Invalid promotion piece type"),
        };
        Self::new(starting_index, ending_index, flags)
    }

    pub fn double_pawn_push(starting_index: u8) -> Self {
        if squares::rank(starting_index) == 1 {
            Self::new(starting_index, north(north(starting_index)), 0b0011)
        } else {
            Self::new(starting_index, south(south(starting_index)), 0b0011)
        }
    }

    pub fn left_en_passant(starting_index: u8) -> Self {
        if squares::rank(starting_index) == 4 {
            Self::new(starting_index, northwest(starting_index), 0b0001)
        } else {
            Self::new(starting_index, southwest(starting_index), 0b0001)
        }
    }

    pub fn right_en_passant(starting_index: u8) -> Self {
        if squares::rank(starting_index) == 4 {
            Self::new(starting_index, northeast(starting_index), 0b0001)
        } else {
            Self::new(starting_index, southeast(starting_index), 0b0001)
        }
    }

    pub fn kingside_castle(white: bool) -> Self {
        if white {
            Self::new(E1, G1, 0b0010)
        } else {
            Self::new(E8, G8, 0b0010)
        }
    }

    pub fn queenside_castle(white: bool) -> Self {
        if white {
            Self::new(E1, C1, 0b0010)
        } else {
            Self::new(E8, C8, 0b0010)
        }
    }

    fn new(starting_index: u8, ending_index: u8, flags: u8) -> Self {
        let data = (starting_index as u16) | ((ending_index as u16) << 6) | ((flags as u16) << 12);
        Self { data }
    }

    pub fn null() -> Self {
        Self { data: 0 }
    }

    pub fn starting_index(&self) -> u8 {
        (self.data & 0x3F) as u8
    }

    pub fn starting_mask(&self) -> u64 {
        1 << self.starting_index()
    }

    pub fn ending_index(&self) -> u8 {
        ((self.data >> 6) & 0x3F) as u8
    }

    pub fn ending_mask(&self) -> u64 {
        1 << self.ending_index()
    }

    pub fn flags(&self) -> u8 {
        (self.data >> 12) as u8
    }
}

pub mod piece_type {
    pub const PAWN: u8 = 0;
    pub const KNIGHT: u8 = 1;
    pub const BISHOP: u8 = 2;
    pub const ROOK: u8 = 3;
    pub const QUEEN: u8 = 4;
    pub const KING: u8 = 5;
}

// Shut clippy up about unused
#[allow(unused)]
pub mod squares {
    pub const A1: u8 = 0;
    pub const B1: u8 = 1;
    pub const C1: u8 = 2;
    pub const D1: u8 = 3;
    pub const E1: u8 = 4;
    pub const F1: u8 = 5;
    pub const G1: u8 = 6;
    pub const H1: u8 = 7;

    pub const A2: u8 = 8;
    pub const B2: u8 = 9;
    pub const C2: u8 = 10;
    pub const D2: u8 = 11;
    pub const E2: u8 = 12;
    pub const F2: u8 = 13;
    pub const G2: u8 = 14;
    pub const H2: u8 = 15;

    pub const A3: u8 = 16;
    pub const B3: u8 = 17;
    pub const C3: u8 = 18;
    pub const D3: u8 = 19;
    pub const E3: u8 = 20;
    pub const F3: u8 = 21;
    pub const G3: u8 = 22;
    pub const H3: u8 = 23;

    pub const A4: u8 = 24;
    pub const B4: u8 = 25;
    pub const C4: u8 = 26;
    pub const D4: u8 = 27;
    pub const E4: u8 = 28;
    pub const F4: u8 = 29;
    pub const G4: u8 = 30;
    pub const H4: u8 = 31;

    pub const A5: u8 = 32;
    pub const B5: u8 = 33;
    pub const C5: u8 = 34;
    pub const D5: u8 = 35;
    pub const E5: u8 = 36;
    pub const F5: u8 = 37;
    pub const G5: u8 = 38;
    pub const H5: u8 = 39;

    pub const A6: u8 = 40;
    pub const B6: u8 = 41;
    pub const C6: u8 = 42;
    pub const D6: u8 = 43;
    pub const E6: u8 = 44;
    pub const F6: u8 = 45;
    pub const G6: u8 = 46;
    pub const H6: u8 = 47;

    pub const A7: u8 = 48;
    pub const B7: u8 = 49;
    pub const C7: u8 = 50;
    pub const D7: u8 = 51;
    pub const E7: u8 = 52;
    pub const F7: u8 = 53;
    pub const G7: u8 = 54;
    pub const H7: u8 = 55;

    pub const A8: u8 = 56;
    pub const B8: u8 = 57;
    pub const C8: u8 = 58;
    pub const D8: u8 = 59;
    pub const E8: u8 = 60;
    pub const F8: u8 = 61;
    pub const G8: u8 = 62;
    pub const H8: u8 = 63;

    pub const fn rank(index: u8) -> u8 {
        index / 8
    }

    pub const fn file(index: u8) -> u8 {
        index % 8
    }

    pub const fn mask(index: u8) -> u64 {
        1 << index
    }

    pub const fn north(index: u8) -> u8 {
        index + 8
    }

    pub const fn northeast(index: u8) -> u8 {
        index + 9
    }

    pub const fn east(index: u8) -> u8 {
        index + 1
    }

    pub const fn southeast(index: u8) -> u8 {
        index - 7
    }

    pub const fn south(index: u8) -> u8 {
        index - 8
    }

    pub const fn southwest(index: u8) -> u8 {
        index - 9
    }

    pub const fn west(index: u8) -> u8 {
        index - 1
    }

    pub const fn northwest(index: u8) -> u8 {
        index + 7
    }

    pub fn to_name(index: u8) -> String {
        let file = file(index);
        let rank = rank(index);
        let file_char = (b'a' + file) as char;
        let rank_char = (b'1' + rank) as char;
        format!("{}{}", file_char, rank_char)
    }
}

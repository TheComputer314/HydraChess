use strum_macros::FromRepr;
use crate::search::move_generation::mov::Square::{C1, C8, E1, E8, G1, G8};

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
#[derive(Clone, Copy)]
pub struct Move {
    data: u16,
}

impl Move {
    pub fn regular(starting_index: Square, ending_index: Square) -> Self {
        Self::new(starting_index, ending_index, 0)
    }

    pub fn pawn_promotion(starting_index: Square, ending_index: Square, promotion: PieceType) -> Self {
        let flags = match promotion {
            PieceType::Queen => 0b0100,
            PieceType::Rook => 0b0101,
            PieceType::Bishop => 0b0110,
            PieceType::Knight => 0b0111,
            _ => panic!("Invalid promotion piece type"),
        };
        Self::new(starting_index, ending_index, flags)
    }

    pub fn double_pawn_push(starting_index: Square) -> Self {
        if starting_index.rank() == 1 {
            Self::new(starting_index, starting_index.north(2), 0b0011)
        } else {
            Self::new(starting_index, starting_index.south(2), 0b0011)
        }
    }

    pub fn left_en_passant(starting_index: Square) -> Self {
        if starting_index.rank() == 4 {
            Self::new(starting_index, starting_index.northwest(1), 0b0001)
        } else {
            Self::new(starting_index, starting_index.southwest(1), 0b0001)
        }
    }

    pub fn right_en_passant(starting_index: Square) -> Self {
        if starting_index.rank() == 4 {
            Self::new(starting_index, starting_index.northeast(1), 0b0001)
        } else {
            Self::new(starting_index, starting_index.southeast(1), 0b0001)
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

    fn new(starting_index: Square, ending_index: Square, flags: u8) -> Self {
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

#[derive(PartialEq, Eq)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, FromRepr)]
// Shut clippy up about unused
#[allow(unused)]
pub enum Square {
    A1 = 0,
    B1 = 1,
    C1 = 2,
    D1 = 3,
    E1 = 4,
    F1 = 5,
    G1 = 6,
    H1 = 7,

    A2 = 8,
    B2 = 9,
    C2 = 10,
    D2 = 11,
    E2 = 12,
    F2 = 13,
    G2 = 14,
    H2 = 15,

    A3 = 16,
    B3 = 17,
    C3 = 18,
    D3 = 19,
    E3 = 20,
    F3 = 21,
    G3 = 22,
    H3 = 23,

    A4 = 24,
    B4 = 25,
    C4 = 26,
    D4 = 27,
    E4 = 28,
    F4 = 29,
    G4 = 30,
    H4 = 31,

    A5 = 32,
    B5 = 33,
    C5 = 34,
    D5 = 35,
    E5 = 36,
    F5 = 37,
    G5 = 38,
    H5 = 39,

    A6 = 40,
    B6 = 41,
    C6 = 42,
    D6 = 43,
    E6 = 44,
    F6 = 45,
    G6 = 46,
    H6 = 47,

    A7 = 48,
    B7 = 49,
    C7 = 50,
    D7 = 51,
    E7 = 52,
    F7 = 53,
    G7 = 54,
    H7 = 55,

    A8 = 56,
    B8 = 57,
    C8 = 58,
    D8 = 59,
    E8 = 60,
    F8 = 61,
    G8 = 62,
    H8 = 63,
}

impl Square {
    pub fn index(&self) -> u8 {
        *self as u8
    }

    pub fn mask(&self) -> u64 {
        1 << self.index()
    }

    pub fn rank(&self) -> u8 {
        self.index() / 8
    }

    pub fn file(&self) -> u8 {
        self.index() % 8
    }

    pub fn north(&self, squares: u8) -> Square {
        Square::from_repr(self.index() + 8*squares).unwrap()
    }

    pub fn northeast(&self, squares: u8) -> Square {
        Square::from_repr(self.index() + 9*squares).unwrap()
    }

    pub fn east(&self, squares: u8) -> Square {
        Square::from_repr(self.index() + squares).unwrap()
    }

    pub fn southeast(&self, squares: u8) -> Square {
        Square::from_repr(self.index() - 7*squares).unwrap()
    }

    pub fn south(&self, squares: u8) -> Square {
        Square::from_repr(self.index() - 8*squares).unwrap()
    }

    pub fn southwest(&self, squares: u8) -> Square {
        Square::from_repr(self.index() - 9*squares).unwrap()
    }

    pub fn west(&self, squares: u8) -> Square {
        Square::from_repr(self.index() - squares).unwrap()
    }

    pub fn northwest(&self, squares: u8) -> Square {
        Square::from_repr(self.index() + 7*squares).unwrap()
    }

    pub fn from_index(index: u8) -> Square {
        Square::from_repr(index).unwrap()
    }
}

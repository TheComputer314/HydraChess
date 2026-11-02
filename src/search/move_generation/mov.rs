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
    pub fn regular(starting_index: u8, ending_index: u8) -> Self {
        Self::new(starting_index, ending_index, 0)
    }

    pub fn pawn_promotion(starting_index: u8, ending_index: u8, promotion: PieceType) -> Self {
        let flags = match promotion {
            PieceType::Queen => 0b0100,
            PieceType::Rook => 0b0101,
            PieceType::Bishop => 0b0110,
            PieceType::Knight => 0b0111,
            _ => panic!("Invalid promotion piece type"),
        };
        Self::new(starting_index, ending_index, flags)
    }

    pub fn double_pawn_push(starting_index: u8) -> Self {
        if starting_index / 8 == 1 {
            Self::new(starting_index, starting_index + 16, 0b0011)
        } else {
            Self::new(starting_index, starting_index - 16, 0b0011)
        }
    }

    pub fn left_en_passant(starting_index: u8) -> Self {
        if starting_index % 8 == 4 {
            Self::new(starting_index, starting_index + 7, 0b0001)
        } else {
            Self::new(starting_index, starting_index - 9, 0b0001)
        }
    }

    pub fn right_en_passant(starting_index: u8) -> Self {
        if starting_index % 8 == 4 {
            Self::new(starting_index, starting_index + 9, 0b0001)
        } else {
            Self::new(starting_index, starting_index - 7, 0b0001)
        }
    }

    pub fn kingside_castle(white: bool) -> Self {
        if white {
            Self::new(4, 6, 0b0010)
        } else {
            Self::new(60, 62, 0b0010)
        }
    }

    pub fn queenside_castle(white: bool) -> Self {
        if white {
            Self::new(4, 2, 0b0010)
        } else {
            Self::new(60, 58, 0b0010)
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

#[derive(PartialEq, Eq)]
pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

#[derive(PartialEq, Eq)]
pub struct Square {
    pub index: u8,
}

impl Square {
    pub fn mask(&self) -> u64 {
        1 << self.index
    }

    pub fn rank(&self) -> u8 {
        self.index / 8
    }

    pub fn file(&self) -> u8 {
        self.index % 8
    }
}

// Shut clippy up about unused
#[allow(unused)]
pub mod squares {
    use crate::search::move_generation::mov::Square;

    pub const A1: Square = Square { index: 0 };
    pub const B1: Square = Square { index: 1 };
    pub const C1: Square = Square { index: 2 };
    pub const D1: Square = Square { index: 3 };
    pub const E1: Square = Square { index: 4 };
    pub const F1: Square = Square { index: 5 };
    pub const G1: Square = Square { index: 6 };
    pub const H1: Square = Square { index: 7 };

    pub const A2: Square = Square { index: 8 };
    pub const B2: Square = Square { index: 9 };
    pub const C2: Square = Square { index: 10 };
    pub const D2: Square = Square { index: 11 };
    pub const E2: Square = Square { index: 12 };
    pub const F2: Square = Square { index: 13 };
    pub const G2: Square = Square { index: 14 };
    pub const H2: Square = Square { index: 15 };

    pub const A3: Square = Square { index: 16 };
    pub const B3: Square = Square { index: 17 };
    pub const C3: Square = Square { index: 18 };
    pub const D3: Square = Square { index: 19 };
    pub const E3: Square = Square { index: 20 };
    pub const F3: Square = Square { index: 21 };
    pub const G3: Square = Square { index: 22 };
    pub const H3: Square = Square { index: 23 };

    pub const A4: Square = Square { index: 24 };
    pub const B4: Square = Square { index: 25 };
    pub const C4: Square = Square { index: 26 };
    pub const D4: Square = Square { index: 27 };
    pub const E4: Square = Square { index: 28 };
    pub const F4: Square = Square { index: 29 };
    pub const G4: Square = Square { index: 30 };
    pub const H4: Square = Square { index: 31 };

    pub const A5: Square = Square { index: 32 };
    pub const B5: Square = Square { index: 33 };
    pub const C5: Square = Square { index: 34 };
    pub const D5: Square = Square { index: 35 };
    pub const E5: Square = Square { index: 36 };
    pub const F5: Square = Square { index: 37 };
    pub const G5: Square = Square { index: 38 };
    pub const H5: Square = Square { index: 39 };

    pub const A6: Square = Square { index: 40 };
    pub const B6: Square = Square { index: 41 };
    pub const C6: Square = Square { index: 42 };
    pub const D6: Square = Square { index: 43 };
    pub const E6: Square = Square { index: 44 };
    pub const F6: Square = Square { index: 45 };
    pub const G6: Square = Square { index: 46 };
    pub const H6: Square = Square { index: 47 };

    pub const A7: Square = Square { index: 48 };
    pub const B7: Square = Square { index: 49 };
    pub const C7: Square = Square { index: 50 };
    pub const D7: Square = Square { index: 51 };
    pub const E7: Square = Square { index: 52 };
    pub const F7: Square = Square { index: 53 };
    pub const G7: Square = Square { index: 54 };
    pub const H7: Square = Square { index: 55 };

    pub const A8: Square = Square { index: 56 };
    pub const B8: Square = Square { index: 57 };
    pub const C8: Square = Square { index: 58 };
    pub const D8: Square = Square { index: 59 };
    pub const E8: Square = Square { index: 60 };
    pub const F8: Square = Square { index: 61 };
    pub const G8: Square = Square { index: 62 };
    pub const H8: Square = Square { index: 63 };
}

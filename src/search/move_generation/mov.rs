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
pub struct Move {
    data: u16,
}

impl Move {
    pub fn regular(starting_index: u8, ending_index: u8) -> Self {
        Self::new(starting_index, ending_index, 0)
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

    fn new(starting_index: u8, ending_index: u8, flags: u8) -> Self {
        let data = (starting_index as u16) | ((ending_index as u16) << 6) | ((flags as u16) << 12);
        Self { data }
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

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
    pub fn new(starting_index: u8, ending_index: u8, flags: u8) -> Self {
        let data = (starting_index as u16) | ((ending_index as u16) << 6) | ((flags as u16) << 12);
        Self { data }
    }

    pub fn starting_index(&self) -> u8 {
        (self.data & 0x3F) as u8
    }

    pub fn ending_index(&self) -> u8 {
        ((self.data >> 6) & 0x3F) as u8
    }

    pub fn flags(&self) -> u8 {
        (self.data >> 12) as u8
    }
}

pub enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}
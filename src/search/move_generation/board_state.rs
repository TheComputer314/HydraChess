/// Represents the state of a chess board.
///
/// Board state is represented by a Little-Endian Rank-File order u64 for each piece color and each piece type.
struct Board {
    white: u64,
    black: u64,
    pawns: u64,
    knights: u64,
    bishops: u64,
    rooks: u64,
    queens: u64,
    kings: u64,
    /// Miscellaneous information about the board state.
    ///
    /// From least significant bit:
    /// - 6 bits: Halfmove clock
    /// - 1 bit: White's queenside castling rights
    /// - 1 bit: White's kingside castling rights
    /// - 1 bit: Black's queenside castling rights
    /// - 1 bit: Black's kingside castling rights
    other: u16,
}

enum PieceType {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl Board {
    pub fn starting() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR")
    }

    pub fn from_fen(fen_string: &str) -> Self {
        let mut rank = 7; // Start at the 8th rank
        let mut file = 0; // Start at the A-file

        let mut bitboards = [0u64; 8];
        for c in fen_string.chars() {
            if c.is_ascii_digit() {
                file += c.to_digit(10).unwrap() as usize;
                continue;
            } else if c == '/' {
                rank -= 1;
                file = 0;
                continue;
            }
            if file > 7 {
                panic!("Invalid FEN string: Too many files in rank {}", rank + 1);
            }

            let mask: u64 = 1 << (rank * 8 + file);
            if c.is_uppercase() {
                bitboards[0] |= mask;
            } else {
                bitboards[1] |= mask;
            }
            match c {
                'P' | 'p' => bitboards[2] |= mask,
                'N' | 'n' => bitboards[3] |= mask,
                'B' | 'b' => bitboards[4] |= mask,
                'R' | 'r' => bitboards[5] |= mask,
                'Q' | 'q' => bitboards[6] |= mask,
                'K' | 'k' => bitboards[7] |= mask,
                _ => panic!("Invalid FEN string: Unknown char"),
            }
            file += 1;
        }

        let [white, black, pawns, knights, bishops, rooks, queens, kings] = bitboards;
        Board {
            white,
            black,
            pawns,
            knights,
            bishops,
            rooks,
            queens,
            kings,
            other: 0b1111,
        }
    }

    pub fn get_bitboard(&self, white: bool, piece_type: PieceType) -> u64 {
        (if white { self.white } else { self.black })
            & match piece_type {
                PieceType::Pawn => self.pawns,
                PieceType::Knight => self.knights,
                PieceType::Bishop => self.bishops,
                PieceType::Rook => self.rooks,
                PieceType::Queen => self.queens,
                PieceType::King => self.kings,
            }
    }

    pub fn make_move(mut self, mov: &Move) {
        let start_mask = 1 << mov.starting_index();
        let end_mask = 1 << mov.ending_index();
        let mut halfmove_reset = false;

        // Check piece color
        if start_mask & self.white != 0 {
            self.white &= !start_mask; // Remove from start square
            self.white |= end_mask; // Add to end square
            // If capture
            if self.black & end_mask != 0 {
                // Reset halfmove clock to 0
                self.other &= 0b1111 << 6;
                self.black &= !end_mask;
                halfmove_reset = true;
            }
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
            // If capture
            if self.white & end_mask != 0 {
                self.other &= 0b1111 << 4;
                self.white &= !end_mask;
                halfmove_reset = true;
            }
        }

        // Clear piece from end square for captures
        self.pawns &= !end_mask;
        self.knights &= !end_mask;
        self.bishops &= !end_mask;
        self.rooks &= !end_mask;
        self.queens &= !end_mask;
        self.kings &= !end_mask;

        // Check piece type
        if self.pawns & start_mask != 0 {
            self.other &= 0b1111 << 6;
            self.pawns &= !start_mask;
            self.pawns |= end_mask;
            halfmove_reset = true;
        } else if self.knights & start_mask != 0 {
            self.knights &= !start_mask;
            self.knights |= end_mask;
        } else if self.bishops & start_mask != 0 {
            self.bishops &= !start_mask;
            self.bishops |= end_mask;
        } else if self.rooks & start_mask != 0 {
            self.rooks &= !start_mask;
            self.rooks |= end_mask;
        } else if self.queens & start_mask != 0 {
            self.queens &= !start_mask;
            self.queens |= end_mask;
        } else if self.kings & start_mask != 0 {
            self.kings &= !start_mask;
            self.kings |= end_mask;
        }

        if !halfmove_reset {
            self.other += 1;
        }
    }
}

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
struct Move {
    data: u16,
}

impl Move {
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

#[cfg(test)]
mod tests {
    use crate::search::move_generation::board_state::{Board, PieceType};

    #[test]
    fn test_starting() {
        let board = Board::starting();
        test_bitboard(0x000000000000FFFF, board.white, "all white");
        test_bitboard(0xFFFF000000000000, board.black, "all black");
        test_bitboard(0x000000000000FF00, board.get_bitboard(true, PieceType::Pawn), "white pawns");
        test_bitboard(0x00FF000000000000, board.get_bitboard(false, PieceType::Pawn), "black pawns");
        test_bitboard(0x0000000000000042, board.get_bitboard(true, PieceType::Knight), "white knights");
        test_bitboard(0x4200000000000000, board.get_bitboard(false, PieceType::Knight), "black knights");
        test_bitboard(0x0000000000000024, board.get_bitboard(true, PieceType::Bishop), "white bishops");
        test_bitboard(0x2400000000000000, board.get_bitboard(false, PieceType::Bishop), "black bishops");
        test_bitboard(0x0000000000000081, board.get_bitboard(true, PieceType::Rook), "white rooks");
        test_bitboard(0x8100000000000000, board.get_bitboard(false, PieceType::Rook), "black rooks");
        test_bitboard(0x0000000000000008, board.get_bitboard(true, PieceType::Queen), "white queen");
        test_bitboard(0x0800000000000000, board.get_bitboard(false, PieceType::Queen), "black queen");
        test_bitboard(0x0000000000000010, board.get_bitboard(true, PieceType::King), "white king");
        test_bitboard(0x1000000000000000, board.get_bitboard(false, PieceType::King), "black king");
    }

    #[test]
    fn test_kiwipete() {
        let board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R");
        test_bitboard(0x000000081000E700, board.get_bitboard(true, PieceType::Pawn), "white pawns");
        test_bitboard(0x002D500002800000, board.get_bitboard(false, PieceType::Pawn), "black pawns");
        test_bitboard(0x0000001000040000, board.get_bitboard(true, PieceType::Knight), "white knights");
        test_bitboard(0x0000220000000000, board.get_bitboard(false, PieceType::Knight), "black knights");
        test_bitboard(0x0000000000001800, board.get_bitboard(true, PieceType::Bishop), "white bishops");
        test_bitboard(0x0040010000000000, board.get_bitboard(false, PieceType::Bishop), "black bishops");
        test_bitboard(0x0000000000000081, board.get_bitboard(true, PieceType::Rook), "white rooks");
        test_bitboard(0x8100000000000000, board.get_bitboard(false, PieceType::Rook), "black rooks");
        test_bitboard(0x0000000000200000, board.get_bitboard(true, PieceType::Queen), "white queens");
        test_bitboard(0x0010000000000000, board.get_bitboard(false, PieceType::Queen), "black queens");
        test_bitboard(0x0000000000000010, board.get_bitboard(true, PieceType::King), "white king");
        test_bitboard(0x1000000000000000, board.get_bitboard(false, PieceType::King), "black king");
    }

    fn test_bitboard(expected: u64, actual: u64, name: &str) {
        let mut display: String = String::new();
        display.push_str("    Expected      |      Actual\n");
        for rank in (0..8).rev() {
            for file in 0..8 {
                let mask: u64 = 1 << (rank * 8 + file);
                if expected & mask == 0 {
                    display.push_str("0 ");
                } else {
                    display.push_str("1 ");
                }
            }
            display.push_str("  |  ");
            for file in 0..8 {
                let mask: u64 = 1 << (rank * 8 + file);
                if actual & mask == 0 {
                    display.push_str("0 ");
                } else {
                    display.push_str("1 ");
                }
            }
            display.push('\n');
        }
        assert_eq!(
            expected, actual,
            "Bitboard for {} did not match: \n{}",
            name, display
        );
    }
}

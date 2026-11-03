use super::mov::PieceType::*;
use super::mov::{Move, PieceType};
use crate::search::move_generation::mov::Square::*;

/// Represents the state of a chess board.
///
/// Board state is represented by a Little-Endian Rank-File order u64 for each piece color and each piece type.
#[derive(Debug, Clone)]
pub struct Board {
    pub white: u64,
    pub black: u64,
    pub pawns: u64,
    pub knights: u64,
    pub bishops: u64,
    pub rooks: u64,
    pub queens: u64,
    pub kings: u64,
    /// Miscellaneous information about the board state.
    ///
    /// From least significant bit:
    /// - 6 bits: Halfmove clock
    /// - 1 bit: White's kingside castling rights
    /// - 1 bit: White's queenside castling rights
    /// - 1 bit: Black's kingside castling rights
    /// - 1 bit: Black's queenside castling rights
    /// - 1 bit: White to move
    /// - 1 bit: En passant performed
    /// - 3 bits: En passant target file
    /// - 1 bit: En passant target rank (white = 1)
    flags: u16,
}

// Custom PartialEq impl to avoid checking the halfmove clock in equality comparisons
impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        self.white == other.white
            && self.black == other.black
            && self.pawns == other.pawns
            && self.knights == other.knights
            && self.bishops == other.bishops
            && self.rooks == other.rooks
            && self.queens == other.queens
            && self.kings == other.kings
            && if self.flags & 0b1 << 11 == 0 {
                // If en passant not performed
                self.flags & (u16::MAX >> 4) == other.flags & (u16::MAX >> 4)
            } else {
                self.flags == other.flags
            }
    }
}

impl Board {
    pub fn starting() -> Self {
        Self::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
    }

    pub fn from_fen(fen_string: &str) -> Self {
        let mut substrings = fen_string.split_whitespace();
        let board_state = substrings.next().unwrap();
        let white_to_move = substrings.next().unwrap();
        let castling_rights = substrings.next().unwrap();
        let en_passant_target = substrings.next().unwrap();
        let halfmove_clock = substrings.next().unwrap();
        // Fullmove clock is disregarded

        // Load pieces into bitboards
        let mut rank = 7; // Start at the 8th rank
        let mut file = 0; // Start at the A-file
        let mut bitboards = [0u64; 8];
        for c in board_state.chars() {
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

        // Load miscellaneous data
        let mut other: u16 = 0;

        // Load halfmove clock into first 6 bits
        other |= halfmove_clock.parse::<u16>().unwrap();

        // Load castling rights into next 4 bits
        for c in castling_rights.chars() {
            match c {
                'K' => other |= 0b0001 << 6,
                'Q' => other |= 0b0010 << 6,
                'k' => other |= 0b0100 << 6,
                'q' => other |= 0b1000 << 6,
                _ => {}
            }
        }

        // Load white to move into next bit
        if white_to_move == "w" {
            other |= 1 << 10
        }

        // Load if en passant was performed into next bit
        let mut en_passant_target = en_passant_target.chars();
        let en_passant_target_file = en_passant_target.next().unwrap();
        if en_passant_target_file != '-' {
            other |= 1 << 11;
            // Load en passant target into next 4 bits
            other |= match en_passant_target_file {
                'a' => 0,
                'b' => 1,
                'c' => 2,
                'd' => 3,
                'e' => 4,
                'f' => 5,
                'g' => 6,
                'h' => 7,
                _ => panic!(),
            } << 12;
            let en_passant_target_rank = en_passant_target.next().unwrap();
            other |= (if en_passant_target_rank == '3' { 1 } else { 0 }) << 15;
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
            flags: other,
        }
    }

    pub fn get_bitboard(&self, white: bool, piece_type: PieceType) -> u64 {
        (if white { self.white } else { self.black })
            & match piece_type {
                Pawn => self.pawns,
                Knight => self.knights,
                Bishop => self.bishops,
                Rook => self.rooks,
                Queen => self.queens,
                King => self.kings,
            }
    }

    pub fn all_pieces(&self) -> u64 {
        self.white | self.black
    }

    pub fn get_halfmove_clock(&self) -> u8 {
        (self.flags & 0b111111) as u8
    }

    pub fn get_castling_rights(&self) -> u8 {
        ((self.flags >> 6) & 0b1111) as u8
    }

    pub fn white_to_move(&self) -> bool {
        (self.flags >> 10) & 0b1 == 1
    }

    pub fn en_passant_target(&self) -> Option<u8> {
        if (self.flags >> 11) & 0b1 == 1 {
            let target = self.flags >> 12;
            let file = (target & 0b111) as u8;
            let rank = if target & 0b1000 == 0 { 5 } else { 2 };
            Some(rank * 8 + file)
        } else {
            None
        }
    }

    /// Makes a non-special move on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move was made.
    /// If the move captures a piece, the captured piece is returned. Otherwise, None is returned.
    pub fn make_move(&mut self, mov: &Move) -> (u16, Option<PieceType>) {
        let last_flags = self.flags;
        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        // Get type of the piece being moved
        let moved_piece = if start_mask & self.pawns != 0 {
            Pawn
        } else if start_mask & self.knights != 0 {
            Knight
        } else if start_mask & self.bishops != 0 {
            Bishop
        } else if start_mask & self.rooks != 0 {
            Rook
        } else if start_mask & self.queens != 0 {
            Queen
        } else if start_mask & self.kings != 0 {
            King
        } else {
            panic!("No piece at the start square");
        };
        // Get color of piece
        let white = start_mask & self.white != 0;
        // If the move is a capture, get the type of the captured piece
        let capture = if end_mask & self.pawns != 0 {
            Some(Pawn)
        } else if end_mask & self.knights != 0 {
            Some(Knight)
        } else if end_mask & self.bishops != 0 {
            Some(Bishop)
        } else if end_mask & self.rooks != 0 {
            Some(Rook)
        } else if end_mask & self.queens != 0 {
            Some(Queen)
        } else if end_mask & self.kings != 0 {
            panic!("King capture??????")
        } else {
            None
        };

        // If a piece was captured or if the piece to move is a pawn, reset the halfmove clock
        if capture.is_some() || moved_piece == Pawn {
            self.flags &= !0b111111;
        } else {
            // Otherwise, increment it
            self.flags += 1;
        }

        // Remove the captured piece from the board
        match &capture {
            Some(Pawn) => self.pawns &= !end_mask,
            Some(Knight) => self.knights &= !end_mask,
            Some(Bishop) => self.bishops &= !end_mask,
            Some(Rook) => self.rooks &= !end_mask,
            Some(Queen) => self.queens &= !end_mask,
            Some(King) => panic!("King capture??????"),
            None => {}
        }
        if capture.is_some() {
            if white {
                self.black &= !end_mask;
            } else {
                self.white &= !end_mask;
            }
        }

        // Remove the piece from the start square and add it to the end square
        match moved_piece {
            Pawn => {
                self.pawns &= !start_mask;
                // Promotions
                match mov.flags() {
                    0b0100 => self.queens |= end_mask,
                    0b0101 => self.rooks |= end_mask,
                    0b0110 => self.bishops |= end_mask,
                    0b0111 => self.knights |= end_mask,
                    _ => self.pawns |= end_mask,
                }
            }
            Knight => {
                self.knights &= !start_mask;
                self.knights |= end_mask
            }
            Bishop => {
                self.bishops &= !start_mask;
                self.bishops |= end_mask
            }
            Rook => {
                self.rooks &= !start_mask;
                self.rooks |= end_mask
            }
            Queen => {
                self.queens &= !start_mask;
                self.queens |= end_mask
            }
            King => {
                self.kings &= !start_mask;
                self.kings |= end_mask
            }
        }
        if white {
            self.white &= !start_mask;
            self.white |= end_mask;
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
        }

        // Toggle white to move
        self.flags ^= 1 << 10;
        // Clear en passant flags
        self.flags &= !(0b11111 << 11);

        if white {
            if self.flags & 1 << 6 != 0 // If white's kingside castling is available,
                // and either the H file rook moved, or the king moved
                && ((moved_piece == Rook && mov.starting_index() == H1.index()) || moved_piece == King)
            {
                // Then clear the flag for white's kingside castling
                self.flags &= !(1 << 6);
            }
            if self.flags & 1 << 7 != 0 // If white's queenside castling is available,
                // and either the A file rook or the king moved
                && ((moved_piece == Rook && mov.starting_index() == A1.index()) || moved_piece == King)
            {
                // Then clear the flag for white's queenside castling
                self.flags &= !(1 << 7);
            }
            if self.flags & 1 << 8 != 0 // If black's kingside castling is available, 
                // and we're capturing the rook on H8
            && mov.ending_index() == H8.index()
            {
                // Then clear the flag for black's kingside castling
                self.flags &= !(1 << 8);
            }
            if self.flags & 1 << 9 != 0 // If black's queenside castling is available,
                // and we're capturing the rook on A8
            && mov.ending_index() == A8.index()
            {
                // Then clear the flag for black's queenside castling
                self.flags &= !(1 << 9);
            }
        } else {
            if self.flags & 1 << 8 != 0 // If black's kingside castling is available,
                // and either the H file rook moved, or the king moved
                && ((moved_piece == Rook && mov.starting_index() == H8.index()) || moved_piece == King)
            {
                // Then clear the flag for black's kingside castling
                self.flags &= !(1 << 8);
            }
            if self.flags & 1 << 9 != 0 // If black's queenside castling is available,
                // and either the A file rook moved, or the king moved
            && mov.ending_index() == A8.index()
            {
                // Then clear the flag for black's queenside castling
                self.flags &= !(1 << 9);
            }
            if self.flags & 1 << 6 != 0 // If white's kingside castling is available,
            // and we're capturing the rook on H1,
            && mov.ending_index() == H1.index()
            {
                // Then clear the flag for white's kingside castling
                self.flags &= !(1 << 6);
            }
            if self.flags & 1 << 7 != 0 // If white's queenside castling is available,
            // and we're capturing the rook on A1,
            && mov.ending_index() == A1.index()
            {
                // Then clear the flag for white's queenside castling
                self.flags &= !(1 << 7);
            }
        }

        (last_flags, capture)
    }

    /// Unmakes a non-special move on the board state.
    ///
    /// Takes the last flag and the captured piece type as input.
    pub fn unmake_move(&mut self, mov: &Move, last_flag: u16, capture: &Option<PieceType>) {
        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        // Get type of the piece being moved
        // End mask since we're unmaking the move from end to start
        let moved_piece = if mov.flags() != 0 || end_mask & self.pawns != 0 {
            // If flags are set, it's a promotion, so the move must be a pawn
            Pawn
        } else if end_mask & self.knights != 0 {
            Knight
        } else if end_mask & self.bishops != 0 {
            Bishop
        } else if end_mask & self.rooks != 0 {
            Rook
        } else if end_mask & self.queens != 0 {
            Queen
        } else if end_mask & self.kings != 0 {
            King
        } else {
            panic!("No piece at the end square");
        };
        // Get color of piece
        let white = end_mask & self.white != 0;

        match moved_piece {
            Pawn => {
                self.pawns |= start_mask;
                // Promotions
                match mov.flags() {
                    0b0100 => self.queens &= !end_mask,
                    0b0101 => self.rooks &= !end_mask,
                    0b0110 => self.bishops &= !end_mask,
                    0b0111 => self.knights &= !end_mask,
                    _ => self.pawns &= !end_mask,
                }
            }
            Knight => {
                self.knights |= start_mask;
                self.knights &= !end_mask;
            }
            Bishop => {
                self.bishops |= start_mask;
                self.bishops &= !end_mask;
            }
            Rook => {
                self.rooks |= start_mask;
                self.rooks &= !end_mask;
            }
            Queen => {
                self.queens |= start_mask;
                self.queens &= !end_mask;
            }
            King => {
                self.kings |= start_mask;
                self.kings &= !end_mask;
            }
        }
        if white {
            self.white |= start_mask;
            self.white &= !end_mask;
        } else {
            self.black |= start_mask;
            self.black &= !end_mask;
        }

        match capture {
            Some(Pawn) => self.pawns |= end_mask,
            Some(Knight) => self.knights |= end_mask,
            Some(Bishop) => self.bishops |= end_mask,
            Some(Rook) => self.rooks |= end_mask,
            Some(Queen) => self.queens |= end_mask,
            Some(King) => panic!("King capture??????"),
            None => {}
        }
        if capture.is_some() {
            if white {
                self.black |= end_mask;
            } else {
                self.white |= end_mask;
            }
        }

        self.flags = last_flag;
    }

    /// Makes a double pawn push on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    pub fn make_double_pawn_push(&mut self, mov: &Move) -> u16 {
        debug_assert!(
            mov.flags() == 0b0011,
            "Double pawn push must have flags 0b0011"
        );
        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        let white = self.white & start_mask != 0;
        if white {
            self.white &= !start_mask;
            self.white |= end_mask;
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
        }

        self.pawns &= !start_mask;
        self.pawns |= end_mask;

        // Double pawn pushes can't capture, so capture handling is not necessary

        let last_flag = self.flags;

        // Reset halfmove clock
        self.flags &= !0b111111;
        // Toggle white to move
        self.flags ^= 1 << 10;
        // Clear en passant flags
        self.flags &= !(0b11111 << 11);
        // Flag en passant
        self.flags |= 1 << 11;
        // Flag file
        self.flags |= ((mov.starting_index() % 8) as u16) << 12;
        // Flag rank
        self.flags |= if white { 1 << 15 } else { 0 };

        last_flag
    }

    /// Unmakes a double pawn push.
    ///
    /// Takes the last flag value before the move.
    pub fn unmake_double_pawn_push(&mut self, mov: &Move, last_flag: u16) {
        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        let white = self.white & end_mask != 0;
        if white {
            self.white |= start_mask;
            self.white &= !end_mask;
        } else {
            self.black |= start_mask;
            self.black &= !end_mask;
        }

        self.pawns |= start_mask;
        self.pawns &= !end_mask;

        self.flags = last_flag;
    }

    /// Makes an en passant move on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    pub fn make_en_passant(&mut self, mov: &Move) -> u16 {
        debug_assert!(mov.flags() == 0b0001, "En passant must have flag 0b0001");

        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        let white = self.white & start_mask != 0;
        let capture_mask = if white {
            1 << (mov.ending_index() - 8)
        } else {
            1 << (mov.ending_index() + 8)
        };

        if white {
            self.white &= !start_mask;
            self.white |= end_mask;
            self.black &= !capture_mask;
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
            self.white &= !capture_mask;
        }

        self.pawns &= !(start_mask | capture_mask);
        self.pawns |= end_mask;

        let flags = self.flags;

        // Reset halfmove clock
        self.flags &= !0b111111;
        // Toggle white to move
        self.flags ^= 1 << 10;
        // Clear en passant flags
        self.flags &= !(0b11111 << 11);

        flags
    }

    /// Unmakes an en passant.
    ///
    /// Takes the last flag value before the move.
    pub fn unmake_en_passant(&mut self, mov: &Move, last_flag: u16) {
        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        let white = self.white & end_mask != 0;
        let capture_mask = if white {
            1 << (mov.ending_index() - 8)
        } else {
            1 << (mov.ending_index() + 8)
        };

        if white {
            self.white |= start_mask;
            self.white &= !end_mask;
            self.black |= capture_mask;
        } else {
            self.black |= start_mask;
            self.black &= !end_mask;
            self.white |= capture_mask;
        }

        self.pawns |= start_mask | capture_mask;
        self.pawns &= !end_mask;

        self.flags = last_flag;
    }

    /// Makes a castling move on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    pub fn make_castle(&mut self, mov: &Move) -> u16 {
        let white = self.white & mov.starting_mask() != 0;
        let kingside = mov.ending_index() % 8 == 6;

        let king_start_mask = mov.starting_mask();
        let king_end_mask = mov.ending_mask();
        let rook_start_mask;
        let rook_end_mask;
        if white {
            if kingside {
                rook_start_mask = H1.mask();
                rook_end_mask = F1.mask();
            } else {
                rook_start_mask = A1.mask();
                rook_end_mask = D1.mask();
            }
        } else {
            if kingside {
                rook_start_mask = H8.mask();
                rook_end_mask = F8.mask();
            } else {
                rook_start_mask = A8.mask();
                rook_end_mask = D8.mask();
            }
        }

        self.kings &= !king_start_mask;
        self.kings |= king_end_mask;
        self.rooks &= !rook_start_mask;
        self.rooks |= rook_end_mask;
        if white {
            self.white &= !(king_start_mask | rook_start_mask);
            self.white |= king_end_mask | rook_end_mask;
        } else {
            self.black &= !(king_start_mask | rook_start_mask);
            self.black |= king_end_mask | rook_end_mask;
        }

        let flags = self.flags;

        // Increment halfmove clock
        self.flags += 1;
        // Toggle white to move
        self.flags ^= 1 << 10;

        if white {
            self.flags &= !((1 << 6) + (1 << 7));
        } else {
            self.flags &= !((1 << 8) + (1 << 9));
        }

        flags
    }

    pub fn unmake_castle(&mut self, mov: &Move, last_flags: u16) {
        let white = self.white & mov.ending_mask() != 0;
        let kingside = mov.ending_index() % 8 == 6;

        let king_start_mask = mov.starting_mask();
        let king_end_mask = mov.ending_mask();
        let rook_start_mask;
        let rook_end_mask;
        if white {
            if kingside {
                rook_start_mask = H1.mask();
                rook_end_mask = F1.mask();
            } else {
                rook_start_mask = A1.mask();
                rook_end_mask = D1.mask();
            }
        } else {
            if kingside {
                rook_start_mask = H8.mask();
                rook_end_mask = F8.mask();
            } else {
                rook_start_mask = A8.mask();
                rook_end_mask = D8.mask();
            }
        }

        self.kings |= king_start_mask;
        self.kings &= !king_end_mask;
        self.rooks |= rook_start_mask;
        self.rooks &= !rook_end_mask;
        if white {
            self.white |= king_start_mask | rook_start_mask;
            self.white &= !(king_end_mask | rook_end_mask);
        } else {
            self.black |= king_start_mask | rook_start_mask;
            self.black &= !(king_end_mask | rook_end_mask);
        }

        self.flags = last_flags;
    }
}

#[cfg(test)]
mod tests {
    use super::super::mov::{Move, PieceType};
    use super::Board;
    use crate::search::move_generation::mov::Square::{A1, A2, A3, A7, B8, C3, E4, E5};

    #[test]
    fn test_load_starting() {
        let board = Board::starting();
        test_bitboard(0x000000000000FFFF, board.white, "all white");
        test_bitboard(0xFFFF000000000000, board.black, "all black");
        test_bitboard(
            0x000000000000FF00,
            board.get_bitboard(true, PieceType::Pawn),
            "white pawns",
        );
        test_bitboard(
            0x00FF000000000000,
            board.get_bitboard(false, PieceType::Pawn),
            "black pawns",
        );
        test_bitboard(
            0x0000000000000042,
            board.get_bitboard(true, PieceType::Knight),
            "white knights",
        );
        test_bitboard(
            0x4200000000000000,
            board.get_bitboard(false, PieceType::Knight),
            "black knights",
        );
        test_bitboard(
            0x0000000000000024,
            board.get_bitboard(true, PieceType::Bishop),
            "white bishops",
        );
        test_bitboard(
            0x2400000000000000,
            board.get_bitboard(false, PieceType::Bishop),
            "black bishops",
        );
        test_bitboard(
            0x0000000000000081,
            board.get_bitboard(true, PieceType::Rook),
            "white rooks",
        );
        test_bitboard(
            0x8100000000000000,
            board.get_bitboard(false, PieceType::Rook),
            "black rooks",
        );
        test_bitboard(
            0x0000000000000008,
            board.get_bitboard(true, PieceType::Queen),
            "white queen",
        );
        test_bitboard(
            0x0800000000000000,
            board.get_bitboard(false, PieceType::Queen),
            "black queen",
        );
        test_bitboard(
            0x0000000000000010,
            board.get_bitboard(true, PieceType::King),
            "white king",
        );
        test_bitboard(
            0x1000000000000000,
            board.get_bitboard(false, PieceType::King),
            "black king",
        );
        assert_eq!(board.get_halfmove_clock(), 0);
        assert!(board.en_passant_target().is_none());
        assert_eq!(board.get_castling_rights(), 0b1111);
        assert!(board.white_to_move());
    }

    #[test]
    fn test_load_kiwipete() {
        let board =
            Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0");
        test_bitboard(
            0x000000081000E700,
            board.get_bitboard(true, PieceType::Pawn),
            "white pawns",
        );
        test_bitboard(
            0x002D500002800000,
            board.get_bitboard(false, PieceType::Pawn),
            "black pawns",
        );
        test_bitboard(
            0x0000001000040000,
            board.get_bitboard(true, PieceType::Knight),
            "white knights",
        );
        test_bitboard(
            0x0000220000000000,
            board.get_bitboard(false, PieceType::Knight),
            "black knights",
        );
        test_bitboard(
            0x0000000000001800,
            board.get_bitboard(true, PieceType::Bishop),
            "white bishops",
        );
        test_bitboard(
            0x0040010000000000,
            board.get_bitboard(false, PieceType::Bishop),
            "black bishops",
        );
        test_bitboard(
            0x0000000000000081,
            board.get_bitboard(true, PieceType::Rook),
            "white rooks",
        );
        test_bitboard(
            0x8100000000000000,
            board.get_bitboard(false, PieceType::Rook),
            "black rooks",
        );
        test_bitboard(
            0x0000000000200000,
            board.get_bitboard(true, PieceType::Queen),
            "white queens",
        );
        test_bitboard(
            0x0010000000000000,
            board.get_bitboard(false, PieceType::Queen),
            "black queens",
        );
        test_bitboard(
            0x0000000000000010,
            board.get_bitboard(true, PieceType::King),
            "white king",
        );
        test_bitboard(
            0x1000000000000000,
            board.get_bitboard(false, PieceType::King),
            "black king",
        );
        assert_eq!(board.get_halfmove_clock(), 0);
        assert!(board.en_passant_target().is_none());
        assert_eq!(board.get_castling_rights(), 0b1111);
        assert!(board.white_to_move());
    }

    #[test]
    fn test_load_en_passant() {
        let board =
            Board::from_fen("rnbqkbnr/pp1p1ppp/8/1Pp1p3/8/8/P1PPPPPP/RNBQKBNR w KQkq c6 0 3");
        assert!(board.en_passant_target().is_some());
        assert_eq!(board.en_passant_target().unwrap(), 42);
    }

    #[test]
    fn test_load_castling_rights() {
        let board =
            Board::from_fen("rnbqkbnr/pp1p1ppp/8/1Pp1p3/8/8/P1PPPPPP/RNBQKBNR w Kkq c6 0 3");
        assert_eq!(board.get_castling_rights(), 0b1101);
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

    #[test]
    fn test_make_unmake_move() {
        // Test making a move
        let start_board = Board::starting();
        let mut board = start_board.clone();
        let mov = Move::regular(A2, A3);
        let (flags, capture) = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/P7/1PPPPPPP/RNBQKBNR b KQkq - 0 1")
        );
        // Test unmaking a move
        board.unmake_move(&mov, flags, &capture);
        assert_eq!(board, start_board);
        // Test capture
        let start_board =
            Board::from_fen("rnbqkb1r/pppppppp/8/8/4n3/2N5/PPPPPPPP/R1BQKBNR w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::regular(C3,E4);
        let (flags, capture) = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkb1r/pppppppp/8/8/4N3/8/PPPPPPPP/R1BQKBNR b KQkq - 0 1")
        );
        // Test unmaking a capture
        board.unmake_move(&mov, flags, &capture);
        assert_eq!(board, start_board);
        // Test increment halfmove clock & test castling rights
        let start_board =
            Board::from_fen("rnbqkbnr/1ppppppp/8/p7/P7/8/1PPPPPPP/RNBQKBNR w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::regular(A1, A3);
        let (flags, capture) = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/1ppppppp/8/p7/P7/R7/1PPPPPPP/1NBQKBNR b Kkq - 1 1")
        );
        // Test reset flags
        board.unmake_move(&mov, flags, &capture);
        assert_eq!(board, start_board);
        // Test pawn promotion
        let start_board = Board::from_fen("1b2k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let mut board = start_board.clone();
        let mov = Move::pawn_promotion(A7, B8, PieceType::Queen);
        let (flags, capture) = board.make_move(&mov);
        assert_eq!(board, Board::from_fen("1Q2k3/8/8/8/8/8/8/4K3 b - - 0 1"));
        board.unmake_move(&mov, flags, &capture);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_double_pawn_push() {
        let start_board = Board::starting();
        let mut board = start_board.clone();
        let mov = Move::double_pawn_push(A2);
        let flags = board.make_double_pawn_push(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pppppppp/8/8/P7/8/1PPPPPPP/RNBQKBNR b KQkq a3 0 1")
        );
        board.unmake_double_pawn_push(&mov, flags);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_en_passant() {
        let start_board =
            Board::from_fen("rnbqkbnr/pp2pppp/2p5/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3");
        let mut board = start_board.clone();
        let mov = Move::left_en_passant(E5);
        let flags = board.make_en_passant(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pp2pppp/2pP4/8/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3")
        );
        board.unmake_en_passant(&mov, flags);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_castle() {
        // Test make white kingside castle
        let start_board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::kingside_castle(true);
        let flags = board.make_castle(&mov);
        assert_eq!(
            board,
            Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R4RK1 b kq - 1 1")
        );
        // Test unmake white kingside castle
        board.unmake_castle(&mov, flags);
        assert_eq!(board, start_board);
        // Test make white queenside castle
        let mov = Move::queenside_castle(true);
        let flags = board.make_castle(&mov);
        assert_eq!(
            board,
            Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/2KR3R b kq - 1 1")
        );
        // Test unmake white queenside castle
        board.unmake_castle(&mov, flags);
        assert_eq!(board, start_board);
        // Test make black kingside castle
        let start_board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R b KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::kingside_castle(false);
        let flags = board.make_castle(&mov);
        assert_eq!(
            board,
            Board::from_fen("r4rk1/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQ - 1 2")
        );
        // Test unmake black kingside castle
        board.unmake_castle(&mov, flags);
        assert_eq!(board, start_board);
        // Test make black queenside castle
        let mov = Move::queenside_castle(false);
        let flags = board.make_castle(&mov);
        assert_eq!(
            board,
            Board::from_fen("2kr3r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQ - 1 2")
        );
        // Test unmake black queenside castle
        board.unmake_castle(&mov, flags);
        assert_eq!(board, start_board);
    }
}

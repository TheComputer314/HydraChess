use super::mov::PieceType::*;
use super::mov::squares::*;
use super::mov::{Move, PieceType, squares};
use std::fmt;
use std::fmt::Debug;

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
    /// Halfmove clock
    halfmove_clock: u8,
    /// Miscellaneous information about the board state.
    ///
    /// From least significant bit:
    /// - 1 bit: White's kingside castling rights
    /// - 1 bit: White's queenside castling rights
    /// - 1 bit: Black's kingside castling rights
    /// - 1 bit: Black's queenside castling rights
    /// - 1 bit: White to move
    /// - 1 bit: En passant performed
    /// - 3 bits: En passant target file
    /// - 1 bit: En passant target rank (white = 1)
    flags: u16,
    pub zobrist_hash: u64,
    zobrist_numbers: ZobristNumbers,
}

#[derive(Debug, Clone)]
struct ZobristNumbers {
    /// Usage: piece_square_hash\[piece\]\[color\]\[square\]
    /// White is 0.
    piece_square_hash: [[[u64; 6]; 2]; 64],
    white_kingside_castling_rights: u64,
    white_queenside_castling_rights: u64,
    black_kingside_castling_rights: u64,
    black_queenside_castling_rights: u64,
    white_to_move: u64,
    /// Usage: en_passant_target\[color\]\[file\]
    /// White is 0.
    en_passant_target: [[u64; 8]; 2],
}

impl ZobristNumbers {
    fn new() -> Self {
        Self {
            piece_square_hash: {
                let mut piece_square_hash = [[[0; 6]; 2]; 64];
                for piece in 0..6 {
                    for color in 0..2 {
                        for square in 0..64 {
                            piece_square_hash[square][color][piece] = rand::random();
                        }
                    }
                }
                piece_square_hash
            },
            white_kingside_castling_rights: rand::random(),
            white_queenside_castling_rights: rand::random(),
            black_kingside_castling_rights: rand::random(),
            black_queenside_castling_rights: rand::random(),
            white_to_move: rand::random(),
            en_passant_target: {
                let mut en_passant_target = [[0; 8]; 2];
                for color in 0..2 {
                    for square in 0..8 {
                        en_passant_target[color][square] = rand::random();
                    }
                }
                en_passant_target
            },
        }
    }
}

// Custom PartialEq impl to only compare Zobrist hashes
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
            && self.halfmove_clock == other.halfmove_clock
            && self.flags == other.flags
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let square_mask = 1 << (rank * 8 + file);
                if self.all_pieces() & square_mask == 0 {
                    empty += 1;
                } else {
                    if empty != 0 {
                        write!(f, "{}", empty)?;
                        empty = 0;
                    }
                    if self.get_bitboard(true, Pawn) & square_mask != 0 {
                        write!(f, "P")?;
                    } else if self.get_bitboard(true, Knight) & square_mask != 0 {
                        write!(f, "N")?;
                    } else if self.get_bitboard(true, Bishop) & square_mask != 0 {
                        write!(f, "B")?;
                    } else if self.get_bitboard(true, Rook) & square_mask != 0 {
                        write!(f, "R")?;
                    } else if self.get_bitboard(true, Queen) & square_mask != 0 {
                        write!(f, "Q")?;
                    } else if self.get_bitboard(true, King) & square_mask != 0 {
                        write!(f, "K")?;
                    } else if self.get_bitboard(false, Pawn) & square_mask != 0 {
                        write!(f, "p")?;
                    } else if self.get_bitboard(false, Knight) & square_mask != 0 {
                        write!(f, "n")?;
                    } else if self.get_bitboard(false, Bishop) & square_mask != 0 {
                        write!(f, "b")?;
                    } else if self.get_bitboard(false, Rook) & square_mask != 0 {
                        write!(f, "r")?;
                    } else if self.get_bitboard(false, Queen) & square_mask != 0 {
                        write!(f, "q")?;
                    } else if self.get_bitboard(false, King) & square_mask != 0 {
                        write!(f, "k")?;
                    }
                }
                if file == 7 && empty != 0 {
                    write!(f, "{}", empty)?;
                }
            }
            if rank != 0 {
                write!(f, "/")?;
            }
        }

        write!(f, " ")?;

        if self.white_to_move() {
            write!(f, "w")?;
        } else {
            write!(f, "b")?;
        }

        write!(f, " ")?;

        if self.get_castling_rights() == 0 {
            write!(f, "-")?;
        } else {
            let castling = self.get_castling_rights();
            if castling & 1 != 0 {
                write!(f, "K")?;
            }
            if castling & 2 != 0 {
                write!(f, "Q")?;
            }
            if castling & 4 != 0 {
                write!(f, "k")?;
            }
            if castling & 8 != 0 {
                write!(f, "q")?;
            }
        }

        write!(f, " ")?;

        if let Some(target) = self.en_passant_target() {
            write!(f, "{}", to_name(target))?;
        } else {
            write!(f, "-")?;
        }

        write!(f, " ")?;

        write!(f, "{}", self.get_halfmove_clock())?;

        write!(f, " 1")?;

        Ok(())
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
        let halfmove_clock = substrings.next().unwrap().parse::<u8>().unwrap();
        // Fullmove clock is disregarded

        let zobrist_numbers = ZobristNumbers::new();
        // Load pieces into bitboards
        let mut rank = 7; // Start at the 8th rank
        let mut file = 0; // Start at the A-file
        let mut bitboards = [0u64; 8];
        let mut zobrist_hash = 0u64;
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
            let white = if c.is_uppercase() { 0 } else { 1 };
            bitboards[white] |= mask;
            let piece_type = match c {
                'P' | 'p' => 0,
                'N' | 'n' => 1,
                'B' | 'b' => 2,
                'R' | 'r' => 3,
                'Q' | 'q' => 4,
                'K' | 'k' => 5,
                _ => panic!("Invalid FEN string: Unknown char"),
            };
            bitboards[2 + piece_type] |= mask;
            zobrist_hash ^= zobrist_numbers.piece_square_hash[rank * 8 + file][white][piece_type];
            file += 1;
        }

        // Load miscellaneous data
        let mut flags: u16 = 0;

        // Load castling rights into next 4 bits
        for c in castling_rights.chars() {
            match c {
                'K' => {
                    flags |= 0b0001;
                    zobrist_hash ^= zobrist_numbers.white_kingside_castling_rights
                }
                'Q' => {
                    flags |= 0b0010;
                    zobrist_hash ^= zobrist_numbers.white_queenside_castling_rights
                }
                'k' => {
                    flags |= 0b0100;
                    zobrist_hash ^= zobrist_numbers.black_kingside_castling_rights
                }
                'q' => {
                    flags |= 0b1000;
                    zobrist_hash ^= zobrist_numbers.black_queenside_castling_rights
                }
                _ => {}
            }
        }

        // Load white to move into next bit
        if white_to_move == "w" {
            flags |= 1 << 4;
            zobrist_hash ^= zobrist_numbers.white_to_move;
        }

        // Load if en passant was performed into next bit
        let mut en_passant_target = en_passant_target.chars();
        let en_passant_target_file = en_passant_target.next().unwrap();
        if en_passant_target_file != '-' {
            flags |= 1 << 5;
            // Load en passant target into next 4 bits
            let file = match en_passant_target_file {
                'a' => 0,
                'b' => 1,
                'c' => 2,
                'd' => 3,
                'e' => 4,
                'f' => 5,
                'g' => 6,
                'h' => 7,
                _ => panic!(),
            };
            flags |= file << 6;
            let en_passant_target_rank = en_passant_target.next().unwrap();
            let white = if en_passant_target_rank == '3' { 1 } else { 0 };
            flags |= white << 9;
            zobrist_hash ^= zobrist_numbers.en_passant_target[1 - white as usize][file as usize];
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
            halfmove_clock,
            flags,
            zobrist_hash,
            zobrist_numbers,
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
        self.halfmove_clock
    }

    pub fn get_castling_rights(&self) -> u8 {
        (self.flags & 0b1111) as u8
    }

    pub fn kingside_castling_available(&self, white: bool) -> bool {
        if white {
            self.flags & 0b1 == 1
        } else {
            (self.flags >> 2) & 0b1 == 1
        }
    }

    pub fn queenside_castling_available(&self, white: bool) -> bool {
        if white {
            (self.flags >> 1) & 0b1 == 1
        } else {
            (self.flags >> 3) & 0b1 == 1
        }
    }

    pub fn white_to_move(&self) -> bool {
        (self.flags >> 4) & 0b1 == 1
    }

    pub fn en_passant_target(&self) -> Option<u8> {
        if (self.flags >> 5) & 0b1 == 1 {
            let target = self.flags >> 6;
            let file = (target & 0b111) as u8;
            let rank = if target & 0b1000 == 0 { 4 } else { 3 };
            Some(rank * 8 + file)
        } else {
            None
        }
    }

    /// Parse a UCI Long Algebraic Notation move string, and apply that move to the board.
    pub fn move_from_uci_lan(&mut self, mov: &str) {
        let mut chars = mov.chars();
        let start_file = chars.next().unwrap() as u8 - b'a';
        let start_rank = chars.next().unwrap() as u8 - b'1';
        let end_file = chars.next().unwrap() as u8 - b'a';
        let end_rank = chars.next().unwrap() as u8 - b'1';
        let start_square = start_rank * 8 + start_file;
        let end_square = end_rank * 8 + end_file;
        let mov;
        if self.pawns & mask(start_square) != 0 {
            if start_rank == 1 && end_rank == 3 {
                mov = Move::double_pawn_push(start_square);
            } else if start_rank == 6 && end_rank == 4 {
                mov = Move::double_pawn_push(start_square);
            } else if let Some(char) = chars.next() {
                match char {
                    'n' => mov = Move::pawn_promotion(start_square, end_square, Knight),
                    'b' => mov = Move::pawn_promotion(start_square, end_square, Bishop),
                    'r' => mov = Move::pawn_promotion(start_square, end_square, Rook),
                    'q' => mov = Move::pawn_promotion(start_square, end_square, Queen),
                    _ => panic!("Invalid promotion character"),
                }
            } else {
                if start_file != end_file && let Some(target) = self.en_passant_target() {
                    let target_rank = target / 8;
                    if start_rank == target_rank {
                        let target_file = target % 8;
                        if start_file != 0 && start_file - 1 == target_file && end_file == target_file {
                            mov = Move::left_en_passant(start_square);
                        } else if start_file != 7 && start_file + 1 == target_file && end_file == target_file {
                            mov = Move::right_en_passant(start_square);
                        } else {
                            mov = Move::regular(start_square, end_square);
                        }
                    } else {
                        mov = Move::regular(start_square, end_square);
                    }
                } else {
                    mov = Move::regular(start_square, end_square);
                }
            }
        } else if self.kings & mask(start_square) != 0 {
            if self.kingside_castling_available(true) && start_square == E1 && end_square == G1 {
                mov = Move::kingside_castle(true);
            } else if self.queenside_castling_available(true)
                && start_square == E1
                && end_square == C1
            {
                mov = Move::queenside_castle(true);
            } else if self.kingside_castling_available(false)
                && start_square == E8
                && end_square == G8
            {
                mov = Move::kingside_castle(false);
            } else if self.queenside_castling_available(false)
                && start_square == E8
                && end_square == C8
            {
                mov = Move::queenside_castle(false);
            } else {
                mov = Move::regular(start_square, end_square);
            }
        } else {
            mov = Move::regular(start_square, end_square);
        }
        self.make_move(&mov);
    }

    /// Makes a move on the board state, assuming that the move is legal.
    ///
    /// Returns a tuple of:
    /// - The last halfmove clock
    /// - The last flags
    /// - The last Zobrist hash
    /// - The captured piece type, if any
    pub fn make_move(&mut self, mov: &Move) -> (u8, u16, u64, Option<PieceType>) {
        let last_halfmove_clock = self.halfmove_clock;
        let last_zobrist_hash = self.zobrist_hash;
        // Handle special moves
        match mov.flags() {
            0b0001 => {
                return (
                    last_halfmove_clock,
                    self.make_en_passant(&mov),
                    last_zobrist_hash,
                    None,
                );
            }
            0b0010 => {
                return (
                    last_halfmove_clock,
                    self.make_castle(&mov),
                    last_zobrist_hash,
                    None,
                );
            }
            0b0011 => {
                return (
                    last_halfmove_clock,
                    self.make_double_pawn_push(&mov),
                    last_zobrist_hash,
                    None,
                );
            }
            _ => {}
        };

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
            panic!("Move {:?} at position {:?} has no piece at the start square",
                mov.to_string(),
                self.to_string()
            );
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
            panic!(
                "Move {:?} at position {:?} is a king capture??????",
                mov.to_string(),
                self.to_string()
            )
        } else {
            None
        };

        // If a piece was captured or if the piece to move is a pawn, reset the halfmove clock
        if capture.is_some() || moved_piece == Pawn {
            self.halfmove_clock = 0;
        } else {
            // Otherwise, increment it
            self.halfmove_clock += 1;
        }

        // Remove the captured piece from the board
        match &capture {
            Some(Pawn) => {
                self.pawns &= !end_mask;
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                    [mov.ending_index() as usize][if white { 1 } else { 0 }][0];
            }
            Some(Knight) => {
                self.knights &= !end_mask;
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                    [mov.ending_index() as usize][if white { 1 } else { 0 }][1];
            }
            Some(Bishop) => {
                self.bishops &= !end_mask;
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                    [mov.ending_index() as usize][if white { 1 } else { 0 }][2];
            }
            Some(Rook) => {
                self.rooks &= !end_mask;
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                    [mov.ending_index() as usize][if white { 1 } else { 0 }][3];
            }
            Some(Queen) => {
                self.queens &= !end_mask;
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                    [mov.ending_index() as usize][if white { 1 } else { 0 }][4];
            }
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
        let start_piece_type_index;
        let end_piece_type_index;
        match moved_piece {
            Pawn => {
                self.pawns &= !start_mask;
                start_piece_type_index = 0;
                // Promotions
                match mov.flags() {
                    0b0100 => {
                        self.queens |= end_mask;
                        end_piece_type_index = 4
                    }
                    0b0101 => {
                        self.rooks |= end_mask;
                        end_piece_type_index = 3
                    }
                    0b0110 => {
                        self.bishops |= end_mask;
                        end_piece_type_index = 2
                    }
                    0b0111 => {
                        self.knights |= end_mask;
                        end_piece_type_index = 1
                    }
                    _ => {
                        self.pawns |= end_mask;
                        end_piece_type_index = 0
                    }
                }
            }
            Knight => {
                self.knights &= !start_mask;
                self.knights |= end_mask;
                start_piece_type_index = 1;
                end_piece_type_index = 1;
            }
            Bishop => {
                self.bishops &= !start_mask;
                self.bishops |= end_mask;
                start_piece_type_index = 2;
                end_piece_type_index = 2;
            }
            Rook => {
                self.rooks &= !start_mask;
                self.rooks |= end_mask;
                start_piece_type_index = 3;
                end_piece_type_index = 3;
            }
            Queen => {
                self.queens &= !start_mask;
                self.queens |= end_mask;
                start_piece_type_index = 4;
                end_piece_type_index = 4;
            }
            King => {
                self.kings &= !start_mask;
                self.kings |= end_mask;
                start_piece_type_index = 5;
                end_piece_type_index = 5;
            }
        }
        if white {
            self.white &= !start_mask;
            self.white |= end_mask;
            self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                [mov.starting_index() as usize][0][start_piece_type_index];
            self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                [mov.ending_index() as usize][0][end_piece_type_index];
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
            self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                [mov.starting_index() as usize][1][start_piece_type_index];
            self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash
                [mov.ending_index() as usize][1][end_piece_type_index];
        }

        // Toggle white to move
        self.flags ^= 1 << 4;
        self.zobrist_hash ^= self.zobrist_numbers.white_to_move;

        // Clear en passant flags
        if let Some(target) = self.en_passant_target() {
            self.zobrist_hash ^= self.zobrist_numbers.en_passant_target[if white { 1 } else { 0 }]
                [(target % 8) as usize];
        }
        self.flags &= !(0b11111 << 5);

        if white {
            if self.kingside_castling_available(true) // If white's kingside castling is available,
                // and either the H file rook moved, or the king moved
                && ((moved_piece == Rook && mov.starting_index() == H1) || moved_piece == King)
            {
                // Then clear the flag for white's kingside castling
                self.flags &= !1;
                self.zobrist_hash ^= self.zobrist_numbers.white_kingside_castling_rights;
            }
            if self.queenside_castling_available(true) // If white's queenside castling is available,
                // and either the A file rook or the king moved
                && ((moved_piece == Rook && mov.starting_index() == A1) || moved_piece == King)
            {
                // Then clear the flag for white's queenside castling
                self.flags &= !(1 << 1);
                self.zobrist_hash ^= self.zobrist_numbers.white_queenside_castling_rights;
            }
            if self.kingside_castling_available(false) // If black's kingside castling is available,
                // and we're capturing the rook on H8
            && mov.ending_index() == H8
            {
                // Then clear the flag for black's kingside castling
                self.flags &= !(1 << 2);
                self.zobrist_hash ^= self.zobrist_numbers.black_kingside_castling_rights;
            }
            if self.queenside_castling_available(false) // If black's queenside castling is available,
                // and we're capturing the rook on A8
            && mov.ending_index() == A8
            {
                // Then clear the flag for black's queenside castling
                self.flags &= !(1 << 3);
                self.zobrist_hash ^= self.zobrist_numbers.black_queenside_castling_rights;
            }
        } else {
            if self.kingside_castling_available(false) // If black's kingside castling is available,
                // and either the H file rook moved, or the king moved
                && ((moved_piece == Rook && mov.starting_index() == H8) || moved_piece == King)
            {
                // Then clear the flag for black's kingside castling
                self.flags &= !(1 << 2);
                self.zobrist_hash ^= self.zobrist_numbers.black_kingside_castling_rights;
            }
            if self.queenside_castling_available(false) // If black's queenside castling is available,
                // and either the A file rook moved, or the king moved
            && ((moved_piece == Rook && mov.starting_index() == A8) || moved_piece == King)
            {
                // Then clear the flag for black's queenside castling
                self.flags &= !(1 << 3);
                self.zobrist_hash ^= self.zobrist_numbers.black_queenside_castling_rights;
            }
            if self.kingside_castling_available(true) // If white's kingside castling is available,
            // and we're capturing the rook on H1,
            && mov.ending_index() == H1
            {
                // Then clear the flag for white's kingside castling
                self.flags &= !1;
                self.zobrist_hash ^= self.zobrist_numbers.white_kingside_castling_rights;
            }
            if self.queenside_castling_available(true) // If white's queenside castling is available,
            // and we're capturing the rook on A1,
            && mov.ending_index() == A1
            {
                // Then clear the flag for white's queenside castling
                self.flags &= !(1 << 1);
                self.zobrist_hash ^= self.zobrist_numbers.white_queenside_castling_rights;
            }
        }

        assert_eq!(
            self.white & self.black,
            0,
            "A black and white piece on the same square"
        );

        (last_halfmove_clock, last_flags, last_zobrist_hash, capture)
    }

    /// Unmakes a move on the board state.
    ///
    /// Takes the last flag and the captured piece type as input.
    pub fn unmake_move(&mut self, mov: &Move, metadata: (u8, u16, u64, Option<PieceType>)) {
        let (last_halfmove_clock, last_flags, last_zobrist_hash, capture) = metadata;
        self.halfmove_clock = last_halfmove_clock;
        self.zobrist_hash = last_zobrist_hash;
        self.flags = last_flags;
        match mov.flags() {
            0b0001 => return self.unmake_en_passant(mov),
            0b0010 => return self.unmake_castle(mov),
            0b0011 => return self.unmake_double_pawn_push(mov),
            _ => {}
        }

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
    }

    /// Makes a double pawn push on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    fn make_double_pawn_push(&mut self, mov: &Move) -> u16 {
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
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.starting_index() as usize][0][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.ending_index() as usize][0][0];
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.starting_index() as usize][1][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.ending_index() as usize][1][0];
        }

        self.pawns &= !start_mask;
        self.pawns |= end_mask;

        // Double pawn pushes can't capture, so capture handling is not necessary

        let last_flag = self.flags;

        // Toggle off last en passant target
        if let Some(target) = self.en_passant_target() {
            self.zobrist_hash ^= self.zobrist_numbers.en_passant_target[if white { 1 } else { 0 }]
                [(target % 8) as usize];
        }
        // Toggle on this en passant target
        self.zobrist_hash ^= self.zobrist_numbers.en_passant_target[if white { 0 } else { 1 }]
            [(mov.starting_index() % 8) as usize];

        // Reset halfmove clock
        self.halfmove_clock = 0;
        // Toggle white to move
        self.flags ^= 1 << 4;
        self.zobrist_hash ^= self.zobrist_numbers.white_to_move;
        // Clear en passant flags
        self.flags &= !(0b11111 << 5);
        // Flag en passant
        self.flags |= 1 << 5;
        // Flag file
        self.flags |= ((mov.starting_index() % 8) as u16) << 6;
        // Flag rank
        self.flags |= if white { 1 << 9 } else { 0 };

        last_flag
    }

    /// Unmakes a double pawn push.
    ///
    /// Takes the last flag value before the move.
    fn unmake_double_pawn_push(&mut self, mov: &Move) {
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
    }

    /// Makes an en passant move on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    fn make_en_passant(&mut self, mov: &Move) -> u16 {
        debug_assert!(mov.flags() == 0b0001, "En passant must have flag 0b0001");

        let start_mask = mov.starting_mask();
        let end_mask = mov.ending_mask();

        let white = self.white & start_mask != 0;
        let capture_index = if white {
            mov.ending_index() - 8
        } else {
            mov.ending_index() + 8
        };
        let capture_mask = mask(capture_index);

        if white {
            self.white &= !start_mask;
            self.white |= end_mask;
            self.black &= !capture_mask;
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.starting_index() as usize][0][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.ending_index() as usize][0][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[capture_index as usize][1][0];
        } else {
            self.black &= !start_mask;
            self.black |= end_mask;
            self.white &= !capture_mask;
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.starting_index() as usize][1][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[mov.ending_index() as usize][1][0];
            self.zobrist_hash ^=
                self.zobrist_numbers.piece_square_hash[capture_index as usize][0][0];
        }

        self.pawns &= !(start_mask | capture_mask);
        self.pawns |= end_mask;
        // Toggle off this en passant target
        self.zobrist_hash ^= self.zobrist_numbers.en_passant_target[if white { 1 } else { 0 }]
            [(mov.ending_index() % 8) as usize];

        let flags = self.flags;

        // Reset halfmove clock
        self.halfmove_clock = 0;
        // Toggle white to move
        self.flags ^= 1 << 4;
        self.zobrist_hash ^= self.zobrist_numbers.white_to_move;
        // Clear en passant flags
        self.flags &= !(0b11111 << 5);

        flags
    }

    /// Unmakes an en passant.
    ///
    /// Takes the last flag value before the move.
    fn unmake_en_passant(&mut self, mov: &Move) {
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
    }

    /// Makes a castling move on the board state, assuming that the move is legal.
    ///
    /// Returns the state of the board's flags before the move.
    fn make_castle(&mut self, mov: &Move) -> u16 {
        let white = self.white & mov.starting_mask() != 0;
        let kingside = mov.ending_index() % 8 == 6;

        let king_start_mask = mov.starting_mask();
        let king_end_mask = mov.ending_mask();
        let rook_start_mask;
        let rook_end_mask;
        if white {
            if kingside {
                rook_start_mask = mask(H1);
                rook_end_mask = mask(F1);
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[H1 as usize][0][3];
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[F1 as usize][0][3];
            } else {
                rook_start_mask = mask(A1);
                rook_end_mask = mask(D1);
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[A1 as usize][0][3];
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[D1 as usize][0][3];
            }
        } else {
            if kingside {
                rook_start_mask = mask(H8);
                rook_end_mask = mask(F8);
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[H8 as usize][1][3];
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[F8 as usize][1][3];
            } else {
                rook_start_mask = mask(A8);
                rook_end_mask = mask(D8);
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[A8 as usize][1][3];
                self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[D8 as usize][1][3];
            }
        }
        self.zobrist_hash ^= self.zobrist_numbers.piece_square_hash[mov.starting_index() as usize]
            [!white as usize][5];
        self.zobrist_hash ^=
            self.zobrist_numbers.piece_square_hash[mov.ending_index() as usize][!white as usize][5];

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

        // Toggle off en passant target
        if let Some(target) = self.en_passant_target() {
            self.zobrist_hash ^= self.zobrist_numbers.en_passant_target[if white { 1 } else { 0 }]
                [(target % 8) as usize];
        }

        // Increment halfmove clock
        self.halfmove_clock += 1;
        // Toggle white to move
        self.flags ^= 1 << 4;
        self.zobrist_hash ^= self.zobrist_numbers.white_to_move;
        // Clear en passant flags
        self.flags &= !(0b11111 << 5);

        if white {
            self.flags &= !0b11;
            self.zobrist_hash ^= self.zobrist_numbers.white_kingside_castling_rights;
            self.zobrist_hash ^= self.zobrist_numbers.white_queenside_castling_rights;
        } else {
            self.flags &= !0b1100;
            self.zobrist_hash ^= self.zobrist_numbers.black_kingside_castling_rights;
            self.zobrist_hash ^= self.zobrist_numbers.black_queenside_castling_rights;
        }

        flags
    }

    fn unmake_castle(&mut self, mov: &Move) {
        let white = self.white & mov.ending_mask() != 0;
        let kingside = mov.ending_index() % 8 == 6;

        let king_start_mask = mov.starting_mask();
        let king_end_mask = mov.ending_mask();
        let rook_start_mask;
        let rook_end_mask;
        if white {
            if kingside {
                rook_start_mask = mask(H1);
                rook_end_mask = mask(F1);
            } else {
                rook_start_mask = mask(A1);
                rook_end_mask = mask(D1);
            }
        } else {
            if kingside {
                rook_start_mask = mask(H8);
                rook_end_mask = mask(F8);
            } else {
                rook_start_mask = mask(A8);
                rook_end_mask = mask(D8);
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
    }
}

#[cfg(test)]
mod tests {
    use super::super::mov::squares::{A1, A2, A3, A7, B8, C3, E4, E5};
    use super::super::mov::{Move, PieceType};
    use super::Board;

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
        assert_eq!(board.en_passant_target().unwrap(), 34);
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
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pppppppp/8/8/8/P7/1PPPPPPP/RNBQKBNR b KQkq - 0 1")
        );
        // Test unmaking a move
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test capture
        let start_board =
            Board::from_fen("rnbqkb1r/pppppppp/8/8/4n3/2N5/PPPPPPPP/R1BQKBNR w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::regular(C3, E4);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkb1r/pppppppp/8/8/4N3/8/PPPPPPPP/R1BQKBNR b KQkq - 0 1")
        );
        // Test unmaking a capture
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test increment halfmove clock & test castling rights
        let start_board =
            Board::from_fen("rnbqkbnr/1ppppppp/8/p7/P7/8/1PPPPPPP/RNBQKBNR w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::regular(A1, A3);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/1ppppppp/8/p7/P7/R7/1PPPPPPP/1NBQKBNR b Kkq - 1 1")
        );
        // Test reset flags
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test pawn promotion
        let start_board = Board::from_fen("1b2k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let mut board = start_board.clone();
        let mov = Move::pawn_promotion(A7, B8, PieceType::Queen);
        let metadata = board.make_move(&mov);
        assert_eq!(board, Board::from_fen("1Q2k3/8/8/8/8/8/8/4K3 b - - 0 1"));
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_double_pawn_push() {
        let start_board = Board::starting();
        let mut board = start_board.clone();
        let mov = Move::double_pawn_push(A2);
        let flags = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pppppppp/8/8/P7/8/1PPPPPPP/RNBQKBNR b KQkq a3 0 1")
        );
        board.unmake_move(&mov, flags);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_en_passant() {
        let start_board =
            Board::from_fen("rnbqkbnr/pp2pppp/2p5/3pP3/8/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 3");
        let mut board = start_board.clone();
        let mov = Move::left_en_passant(E5);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("rnbqkbnr/pp2pppp/2pP4/8/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 3")
        );
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
    }

    #[test]
    fn test_make_unmake_castle() {
        // Test make white kingside castle
        let start_board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::kingside_castle(true);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R4RK1 b kq - 1 1")
        );
        // Test unmake white kingside castle
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test make white queenside castle
        let mov = Move::queenside_castle(true);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/2KR3R b kq - 1 1")
        );
        // Test unmake white queenside castle
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test make black kingside castle
        let start_board = Board::from_fen("r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R b KQkq - 0 1");
        let mut board = start_board.clone();
        let mov = Move::kingside_castle(false);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("r4rk1/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQ - 1 2")
        );
        // Test unmake black kingside castle
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
        // Test make black queenside castle
        let mov = Move::queenside_castle(false);
        let metadata = board.make_move(&mov);
        assert_eq!(
            board,
            Board::from_fen("2kr3r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQ - 1 2")
        );
        // Test unmake black queenside castle
        board.unmake_move(&mov, metadata);
        assert_eq!(board, start_board);
    }
}

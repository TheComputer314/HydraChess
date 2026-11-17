use crate::move_generation;
use crate::move_generation::mov::Move;
use crate::move_generation::mov::piece_type::PAWN;
use crate::move_generation::{Board, Moves, MovesWithMetadata};
use crate::search_threads::transposition_table::{EntryResult, TranspositionTable};
use crate::search_threads::{SearchResult, evaluation, print_info};
use minstant::Instant;
use rand::seq::SliceRandom;
use std::cmp::{Reverse, max};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const MAX_DEPTH: u8 = 16;

pub struct Search {
    pub(crate) nodes_searched: u64,
    killer_moves: [Option<Move>; MAX_DEPTH as usize],
}

pub struct SearchState<'a> {
    pub transposition_table: &'a TranspositionTable,
    pub stop_signal: &'a AtomicBool,
    pub nodes_searched: &'a AtomicU64,
    pub time_left_ms: u64,
    pub increment: u64,
    pub timer: Instant,
}

impl Search {
    pub fn new() -> Self {
        Self {
            nodes_searched: 0,
            killer_moves: [None; MAX_DEPTH as usize],
        }
    }

    fn time_ok_hard(search_state: &SearchState) -> bool {
        let elapsed = search_state.timer.elapsed().as_millis() as u64;
        !search_state.stop_signal.load(Relaxed)
            && elapsed < (search_state.time_left_ms / 20 + search_state.increment / 2)
            && elapsed < 10000
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search(
        &mut self,
        board: &mut Board,
        search_state: &SearchState,
        search_id: usize,
    ) -> SearchResult {
        self.nodes_searched = 0;
        self.killer_moves.fill(None);
        let legal_moves = move_generation::legal_moves(board);
        if legal_moves.length == 0 {
            panic!(
                "Attempted to search_threads and eval board {} when we're checkmated",
                board
            );
        }
        if legal_moves.length == 1 {
            return SearchResult {
                depth: 0,
                best_ply: 0,
                score: 0,
                best_move: legal_moves.moves[0],
            };
        }

        let color = if board.white_to_move() { 1 } else { -1 };
        let transposition_table_result =
            search_state.transposition_table.get(board.zobrist_hash, 0);
        let mut tt_move = None;
        if let Some(result) = transposition_table_result {
            tt_move = Some(result.best_move);
        }
        let mut sorted_moves = self.sort_moves(board, &legal_moves, tt_move, None, color);
        if search_id > 0 {
            sorted_moves[1..].shuffle(&mut rand::rng());
        }
        let mut sorted_moves: Vec<(Move, i32)> = sorted_moves.iter().map(|&m| (m, 0)).collect();

        let mut best_score = -i32::MAX;
        let mut best_move = sorted_moves[0].0;
        let mut best_depth = 0;
        let mut best_ply = 0;
        let mut stability = 0;
        let mut last_best_move = Move::null();
        let timer = Instant::now();
        let mut last_print_time = timer.elapsed().as_secs_f64();

        'iterative_deepening: for depth in 0..MAX_DEPTH {
            if depth > 0 {
                sorted_moves.sort_by_key(|&(_, eval)| Reverse(eval));
                if search_id > 0 {
                    sorted_moves[1..].shuffle(&mut rand::rng());
                }
            }

            let mut delta = 50;
            let (mut alpha, mut beta) = if depth <= 4 {
                (-i32::MAX, i32::MAX)
            } else {
                (
                    best_score.saturating_sub(delta).max(-i32::MAX),
                    best_score.saturating_add(delta),
                )
            };

            let mut alpha_start = alpha;
            'aspiration: loop {
                let mut depth_best_score = -i32::MAX;
                let mut depth_best_move = best_move;
                let mut depth_best_ply = 0;
                for i in 0..sorted_moves.len() {
                    let (mov, sort_eval) = &mut sorted_moves[i];
                    let metadata = board.make_move(mov);

                    let result = if i == 0 || depth < 2 {
                        self.negamax(board, search_state, depth, 0, -beta, -alpha, -color)
                    } else {
                        // Moves that aren't the first one probably aren't as good, so do a null window search
                        // to confirm that they're not better than the best move
                        let mut res =
                            self.negamax(board, search_state, depth, 0, -alpha - 1, -alpha, -color);
                        if let Some((score, _)) = res
                            && -score > alpha
                            && -score < beta
                        {
                            res =
                                self.negamax(board, search_state, depth, 0, -beta, -alpha, -color);
                        }
                        res
                    };

                    if result.is_none() {
                        board.unmake_move(mov, metadata);
                        break 'iterative_deepening;
                    }

                    let (eval, searched_ply) = result.unwrap();
                    let eval = -eval;

                    *sort_eval = eval;

                    board.unmake_move(mov, metadata);

                    if eval > depth_best_score {
                        depth_best_score = eval;
                        depth_best_move = *mov;
                        depth_best_ply = searched_ply;
                    }
                    if eval > alpha {
                        alpha = eval;
                    }
                    if alpha >= beta {
                        // Fail high: we need to redo the search with a larger window, and to look at this new move
                        // first
                        sorted_moves.swap(0, i);
                        break;
                    }

                    if search_id == 0 {
                        let score = if depth > 0 { best_score } else { 0 };
                        let time = timer.elapsed().as_secs_f64();
                        let nodes = search_state.nodes_searched.load(Ordering::Relaxed)
                            + (self.nodes_searched & 2047);
                        if timer.elapsed().as_secs_f64() - last_print_time > 0.05 {
                            print_info(depth, depth_best_ply, score, time, nodes);
                            last_print_time = timer.elapsed().as_secs_f64();
                        }
                    }

                    if !Self::time_ok_hard(search_state) {
                        break 'iterative_deepening;
                    }
                }

                // Check if the root search failed outside our aspiration window
                if depth_best_score <= alpha_start {
                    // (save original alpha before loop!)
                    // Fail-low: widen alpha downward and re-search
                    alpha = depth_best_score.saturating_sub(delta).max(-i32::MAX);
                    alpha_start = alpha;
                    delta *= 2;
                    if delta > 500 {
                        alpha = -i32::MAX;
                        alpha_start = -i32::MAX;
                    }
                } else if depth_best_score >= beta {
                    // Fail-high: widen beta upward and re-search
                    beta = depth_best_score.saturating_add(delta);
                    alpha = alpha_start; // Reset alpha to start of window
                    delta *= 2;
                    if delta > 500 {
                        beta = i32::MAX;
                    }
                } else {
                    // Exact score found
                    best_score = depth_best_score;
                    best_move = depth_best_move;
                    best_depth = depth;
                    best_ply = depth_best_ply;
                    break 'aspiration;
                }
            }
            // We've reached the maximum depth, any further search is meaningless so exit early
            if depth + 1 == MAX_DEPTH {
                search_state.stop_signal.store(true, Ordering::Relaxed);
                break 'iterative_deepening;
            }

            let mult;
            if best_move == last_best_move {
                stability += 1;
                mult = 1.0;
            } else {
                stability = 0;
                last_best_move = best_move;
                mult = 1.5;
            }

            const MULTIPLIERS: [f64; 7] = [2.0, 1.5, 1.2, 1.0, 0.8, 0.75, 0.5];
            let time_mult = MULTIPLIERS[stability.min(6) as usize] * mult;
            if search_state.timer.elapsed().as_millis() as f64
                > ((search_state.time_left_ms / 40 + search_state.increment / 3) as f64)
                    .min(5000.0)
                    * time_mult
            {
                break 'iterative_deepening;
            }
        }

        search_state
            .nodes_searched
            .fetch_add(self.nodes_searched % 2047, Ordering::Relaxed);
        SearchResult {
            depth: best_depth,
            best_ply,
            score: best_score,
            best_move,
        }
    }

    pub(super) const IMMEDIATE_MATE_SCORE: i32 = i32::MAX / 2;

    #[allow(clippy::too_many_arguments)]
    fn negamax(
        &mut self,
        board: &mut Board,
        search_state: &SearchState,
        depth: u8,
        search_ply: u8,
        alpha: i32,
        beta: i32,
        color: i32,
    ) -> Option<(i32, u8)> {
        if board.get_halfmove_clock() > 100 || board.times_played() >= 3 {
            return Some((0, search_ply));
        }
        self.count_nodes(search_state.nodes_searched);

        // Alpha can't possibly be worse than getting checkmated right now, if it is it means we already found a
        // faster mate elsewhere
        // Same for beta
        let mut alpha = alpha.max(-Self::IMMEDIATE_MATE_SCORE + search_ply as i32);
        let mut beta = beta.min(Self::IMMEDIATE_MATE_SCORE - search_ply as i32);
        if alpha >= beta {
            return Some((alpha, search_ply));
        }

        let alpha_original = alpha;

        let moves = move_generation::legal_moves_with_metadata(board);
        if moves.length == 0 {
            return if moves.check {
                Some((search_ply as i32 - Self::IMMEDIATE_MATE_SCORE, search_ply)) // Checkmate
            } else {
                Some((0, search_ply)) // Draw by no legal moves
            };
        }

        let tt_result = search_state
            .transposition_table
            .get(board.zobrist_hash, search_ply);
        let mut tt_move = None;
        if let Some(tt_result) = tt_result {
            tt_move = Some(tt_result.best_move);
            let non_pv = search_ply > 0 && beta - alpha == 1;
            if tt_result.depth >= depth {
                match tt_result.node_type {
                    1 if non_pv && tt_result.score <= alpha => {
                        return Some((tt_result.score, search_ply));
                    } // Upper bound
                    0 => return Some((tt_result.score, search_ply)), // Exact
                    -1 if non_pv && tt_result.score >= beta => {
                        return Some((tt_result.score, search_ply));
                    } // Lower bound
                    _ => {}
                }
                if alpha >= beta {
                    return Some((tt_result.score, search_ply));
                }
            }
        }

        if depth == 0 || search_ply >= MAX_DEPTH {
            return self.quiescence(
                board,
                search_state,
                moves,
                alpha,
                beta,
                search_ply + 1,
                color,
            );
        }

        let mut extensions = -1;
        if moves.length == 1 {
            extensions += 1;
        }
        if moves.check {
            extensions += 1;
        }

        let moves = moves.strip_metadata();
        let sorted_moves = self.sort_moves(
            board,
            &moves,
            tt_move,
            self.killer_moves[search_ply as usize],
            color,
        );

        let mut best_move = sorted_moves[0];
        let mut best_value = -i32::MAX;
        let mut best_searched_ply = search_ply;
        for i in 0..sorted_moves.len() {
            if !Self::time_ok_hard(search_state) {
                return None;
            }
            let mov = sorted_moves[i];

            let metadata = board.make_move(&mov);

            let search = if i == 0 || depth < 2 {
                self.negamax(
                    board,
                    search_state,
                    max(0, depth as i8 + extensions) as u8,
                    search_ply + 1,
                    -beta,
                    -alpha,
                    -color,
                )
            } else {
                let full_depth = max(0, depth as i8 + extensions) as u8;
                let reduced = depth > 2 && sorted_moves.len() > 4 && i > 2;
                let search_depth = if reduced {
                    max(0, depth as i8 + extensions - 1) as u8
                } else {
                    full_depth
                };
                // Moves that aren't first probably aren't worth looking at, so do a null-window
                // search (at reduced depth for late moves) to confirm that's the case
                let mut result = self.negamax(
                    board,
                    search_state,
                    search_depth,
                    search_ply + 1,
                    -alpha - 1,
                    -alpha,
                    -color,
                );
                // A reduced search beat alpha: verify at full depth, still with a null window
                if reduced && result.is_some_and(|r| -r.0 > alpha) {
                    result = self.negamax(
                        board,
                        search_state,
                        full_depth,
                        search_ply + 1,
                        -alpha - 1,
                        -alpha,
                        -color,
                    );
                }
                // Only a move that lands strictly inside (alpha, beta) needs an exact score.
                // In null-window nodes beta == alpha + 1, so this never re-searches there.
                if result.is_some_and(|r| -r.0 > alpha && -r.0 < beta) {
                    result = self.negamax(
                        board,
                        search_state,
                        full_depth,
                        search_ply + 1,
                        -beta,
                        -alpha,
                        -color,
                    );
                }
                result
            };

            if search.is_none() {
                board.unmake_move(&mov, metadata);
                return None;
            }

            let (value, searched_ply) = search.unwrap();

            let value = -value;

            board.unmake_move(&mov, metadata);
            if value > best_value {
                best_value = value;
                best_searched_ply = searched_ply;
            }
            if value > alpha {
                alpha = value;
                // Only trust moves that actually raised alpha. In fail-low nodes every child score
                // is just an upper bound, so storing those in TT poisons the move ordering.
                best_move = mov;
                if alpha >= beta {
                    self.killer_moves[search_ply as usize] = Some(mov);
                    break;
                }
            }
        }

        let evaluation_type = if best_value <= alpha_original {
            1
        } else if best_value >= beta {
            -1
        } else {
            0
        };
        search_state.transposition_table.insert(
            board.zobrist_hash,
            EntryResult::new(depth, best_value, evaluation_type, best_move),
            search_ply,
        );
        Some((best_value, best_searched_ply))
    }

    #[allow(clippy::too_many_arguments)]
    fn quiescence(
        &mut self,
        board: &mut Board,
        search_state: &SearchState,
        moves: MovesWithMetadata,
        alpha: i32,
        beta: i32,
        search_ply: u8,
        color: i32,
    ) -> Option<(i32, u8)> {
        if !Self::time_ok_hard(search_state) {
            return None;
        }

        if board.get_halfmove_clock() > 100 || board.times_played() >= 3 {
            return Some((0, search_ply));
        }

        self.count_nodes(search_state.nodes_searched);

        if moves.length == 0 {
            return if moves.check {
                Some((search_ply as i32 - Self::IMMEDIATE_MATE_SCORE, search_ply)) // Checkmate
            } else {
                Some((0, search_ply)) // Draw by no legal moves
            };
        }

        let static_eval = evaluation::evaluate(board) * color;
        if static_eval >= beta {
            return Some((static_eval, search_ply));
        }
        let mut alpha = alpha;
        if static_eval > alpha {
            alpha = static_eval;
        }
        let mut best_value = static_eval;

        let moves = moves.filter_captures();
        if moves.length == 0 {
            return Some((best_value, search_ply));
        }
        let moves = self.sort_moves(board, &moves, None, None, color);

        let mut best_search_ply = search_ply;
        for mov in moves {
            if !Self::time_ok_hard(search_state) {
                return None;
            }

            let see = evaluation::static_exchange_evaluation(board, &mov);
            if see < 0 {
                continue;
            }

            let metadata = board.make_move(&mov);
            let result = self.quiescence(
                board,
                search_state,
                move_generation::legal_moves_with_metadata(board),
                -beta,
                -alpha,
                search_ply + 1,
                -color,
            );
            board.unmake_move(&mov, metadata);
            result?;

            let (eval, searched_ply) = result.unwrap();
            let eval = -eval;

            if eval > best_value {
                best_value = eval;
                best_search_ply = searched_ply;
            }
            if eval > alpha {
                alpha = eval;
                if alpha >= beta {
                    break;
                }
            }
        }

        Some((best_value, best_search_ply))
    }

    fn sort_moves(
        &self,
        board: &mut Board,
        moves: &Moves,
        tt_move: Option<Move>,
        killer_move: Option<Move>,
        color: i32,
    ) -> Vec<Move> {
        let mut sorted_moves: Vec<(&Move, i32)> = Vec::with_capacity(moves.length);
        for i in 0..moves.length {
            let mov = &moves.moves[i];
            let value;
            if let Some(tt_move) = tt_move
                && tt_move.eq(mov)
            {
                value = 1 << 20;
            } else if let Some(killer_move) = killer_move
                && killer_move.eq(mov)
            {
                if mov.flags() != 0b0001 && board.get_piece_at_square(mov.ending_index()).is_none()
                {
                    value = 10_000; // If killer move is quiet, score below captures but above normal quiets
                } else {
                    value = 1 << 19; // If capture, score below TT best move but above everything else
                }
            } else {
                let capture_type = if mov.flags() == 0b0001 {
                    Some(PAWN)
                } else {
                    board.get_piece_at_square(mov.ending_index())
                };

                value = if let Some(capture_type) = capture_type {
                    let eval = evaluation::static_exchange_evaluation(board, mov);
                    if eval != 0 {
                        eval + (100_000 * eval.signum()) // Bad captures are scored below quiet moves
                    } else {
                        30_000
                            + evaluation::mvv_lva(
                                board.get_piece_at_square(mov.starting_index()).unwrap() as usize,
                                capture_type as usize,
                            )
                    }
                } else {
                    let metadata = board.make_move(mov);
                    let eval = evaluation::evaluate(board) * color;
                    board.unmake_move(mov, metadata);
                    eval
                };
            }
            sorted_moves.push((mov, value));
        }
        sorted_moves.sort_by_key(|mov| Reverse(mov.1));
        sorted_moves.iter().map(|mov| *mov.0).collect()
    }

    fn count_nodes(&mut self, nodes_searched: &AtomicU64) {
        self.nodes_searched += 1;
        if self.nodes_searched % 2047 == 0 {
            nodes_searched.fetch_add(2048, Ordering::Relaxed);
        }
    }
}

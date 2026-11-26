use crate::move_generation;
use crate::move_generation::mov::{Move, piece_type};
use crate::move_generation::{Board, Moves};
use crate::search_threads::transposition_table::{EntryResult, TranspositionTable};
use crate::search_threads::{SearchResult, evaluation, is_mate_score, print_info};
use minstant::Instant;
use rand::seq::SliceRandom;
use std::cmp::{Reverse, max};
use std::io::{Write, stdout};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(super) const MAX_DEPTH: u8 = 64;
pub(super) const MAX_PLIES: u8 = 128;

pub struct Search {
    pub(crate) nodes_searched: u64,
    killer_moves: [Option<Move>; MAX_PLIES as usize],
    singular_exclude_moves: [Option<Move>; MAX_PLIES as usize],
    history: [[[i16; 64]; 64]; 2],
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
            killer_moves: [None; MAX_PLIES as usize],
            singular_exclude_moves: [None; MAX_PLIES as usize],
            history: [[[0; 64]; 64]; 2],
        }
    }

    pub(super) fn reset_history(&mut self) {
        self.history = [[[0; 64]; 64]; 2];
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
        self.singular_exclude_moves.fill(None);
        for side in &mut self.history {
            for start in side {
                for end in start {
                    *end /= 2;
                }
            }
        }
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
                score: evaluation::evaluate(board),
                best_move: legal_moves[0],
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
        let mut sorted_moves: Vec<(Move, i16)> = sorted_moves.iter().map(|&m| (m, 0)).collect();

        let mut best_score = -i16::MAX;
        let mut best_move = sorted_moves[0].0;
        let mut best_depth = 0;
        let mut best_ply = 0;
        let mut stability = 0;
        let mut last_best_move = Move::null();

        let mut mate_finding = false;

        // Iterative deepening: Information from previous searches can make the next search faster, so searching the
        // same position over and over with increasing depth is faster than doing a fixed depth search
        'iterative_deepening: for depth in 0..MAX_DEPTH {
            // Sort moves by evaluation from previous search
            if depth > 0 {
                sorted_moves.sort_by_key(|&(_, eval)| Reverse(eval));
                if search_id > 0 {
                    // Shuffle moves if not on main thread to encourage exploration of different lines
                    sorted_moves[1..].shuffle(&mut rand::rng());
                }
            }

            let mut delta = 20;
            // On shallow depths, use a full window since the previous depth's evaluation is
            // likely to be too unstable
            let (mut alpha, mut beta) = if depth <= 4 {
                (-i16::MAX, i16::MAX)
            } else {
                // The evaluation with this current depth is likely
                // to be close to the evaluation from the previous depth
                (
                    best_score.saturating_sub(delta).max(-i16::MAX),
                    best_score.saturating_add(delta),
                )
            };

            let mut alpha_start = alpha;
            'aspiration: loop {
                let mut depth_best_score = -i16::MAX;
                let mut depth_best_move = best_move;
                let mut depth_best_ply = 0;
                for i in 0..sorted_moves.len() {
                    let (mov, sort_eval) = &mut sorted_moves[i];
                    let metadata = board.make_move(mov);

                    if search_id == 0 && search_state.timer.elapsed().as_secs_f64() > 1.5 {
                        let mut out = stdout().lock();
                        writeln!(
                            out,
                            "info depth {} currmove {} currmovenumber {}",
                            depth + 1,
                            mov,
                            i + 1
                        )
                        .unwrap();
                        out.flush().unwrap();
                    }

                    // The first move is likely to be the best move, so do a full search
                    let result = if i == 0 || depth < 2 {
                        self.negamax(
                            board,
                            search_state,
                            depth,
                            0,
                            -beta,
                            -alpha,
                            -color,
                            false,
                            mate_finding,
                        )
                    } else {
                        // Moves that aren't the first one probably aren't as good, so do a null window search
                        // to confirm that they're not better than the best move
                        let mut res = self.negamax(
                            board,
                            search_state,
                            depth,
                            0,
                            -alpha - 1,
                            -alpha,
                            -color,
                            false,
                            mate_finding,
                        );
                        if let Some((score, _)) = res
                            && -score > alpha
                            && -score < beta
                        {
                            // If it's worth looking at, do a full search
                            res = self.negamax(
                                board,
                                search_state,
                                depth,
                                0,
                                -beta,
                                -alpha,
                                -color,
                                false,
                                mate_finding,
                            );
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

                    // Update best move
                    if eval > depth_best_score {
                        depth_best_score = eval;
                        depth_best_move = *mov;
                        depth_best_ply = searched_ply;
                    }
                    // Update alpha
                    // Moves must be at least as good as the best score we've seen so far
                    if eval > alpha {
                        alpha = eval;
                    }
                    if alpha >= beta {
                        // Fail high: we need to redo the search with a larger window, and to look at this new move
                        // first
                        sorted_moves.swap(0, i);
                        break;
                    }

                    if !Self::time_ok_hard(search_state) {
                        break 'iterative_deepening;
                    }
                }

                // Check if root search failed outside the aspiration window
                if depth_best_score <= alpha_start {
                    // Fail-low: widen alpha downward and re-search
                    alpha = depth_best_score.saturating_sub(delta).max(-i16::MAX);
                    alpha_start = alpha;
                    delta *= 2;
                    if delta > 500 {
                        alpha = -i16::MAX;
                        alpha_start = -i16::MAX;
                    }
                } else if depth_best_score >= beta {
                    // Fail-high: widen beta upward and re-search
                    beta = depth_best_score.saturating_add(delta);
                    alpha = alpha_start; // Reset alpha
                    delta *= 2;
                    if delta > 500 {
                        beta = i16::MAX;
                    }
                } else {
                    // Exact score found, break
                    best_score = depth_best_score;
                    best_move = depth_best_move;
                    best_depth = depth;
                    best_ply = depth_best_ply;
                    break 'aspiration;
                }
            }
            if search_id == 0 {
                let score = if depth > 0 { best_score } else { 0 };
                let time = search_state.timer.elapsed().as_secs_f64();
                let nodes = search_state.nodes_searched.load(Ordering::Relaxed)
                    + (self.nodes_searched & 2047);
                print_info(depth, best_ply, score, time, nodes);
            }

            // We've reached the maximum depth, any further search is meaningless, so exit all threads early
            if depth + 1 == MAX_DEPTH {
                search_state.stop_signal.store(true, Ordering::Relaxed);
                break 'iterative_deepening;
            }

            // If the best move stayed the same, we don't need to spend that much more time on it
            let mult;
            if best_move == last_best_move {
                stability += 1;
                mult = 1.0;
            } else {
                stability = 0;
                last_best_move = best_move;
                mult = 1.5;
            }
            // So progressively shrink the soft time limit as stability increases
            const MULTIPLIERS: [f64; 7] = [2.0, 1.5, 1.2, 1.0, 0.8, 0.75, 0.5];
            let time_mult = MULTIPLIERS[stability.min(6) as usize] * mult;
            if search_state.timer.elapsed().as_millis() as f64
                > ((search_state.time_left_ms / 40 + search_state.increment / 3) as f64).min(5000.0)
                    * time_mult
            {
                break 'iterative_deepening;
            }

            if !mate_finding && is_mate_score(best_score) {
                mate_finding = true;
            }
        }

        // self.count_nodes only adds to the global total mod 2047, so the remainder is added after the search
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

    pub(super) const IMMEDIATE_MATE_SCORE: i16 = i16::MAX - MAX_PLIES as i16;

    #[allow(clippy::too_many_arguments)]
    fn negamax(
        &mut self,
        board: &mut Board,
        search_state: &SearchState,
        depth: u8,
        search_ply: u8,
        alpha: i16,
        beta: i16,
        color: i16,
        cut_node: bool,
        mate_finding: bool,
    ) -> Option<(i16, u8)> {
        // Draw by FIDE rules
        if board.get_halfmove_clock() >= 100
            || board.times_played() >= 3
            || board.draw_by_insufficient_material()
        {
            return Some((0, search_ply));
        }
        self.count_nodes(search_state.nodes_searched);

        // Mate distance pruning
        // Alpha can't possibly be worse than getting checkmated right now, if it is it means we already found a
        // faster mate elsewhere
        // Same for beta
        let mut alpha = alpha.max(-Self::IMMEDIATE_MATE_SCORE + search_ply as i16);
        let beta = beta.min(Self::IMMEDIATE_MATE_SCORE - search_ply as i16);
        if alpha >= beta {
            return Some((alpha, search_ply));
        }

        let alpha_original = alpha;

        let moves = move_generation::legal_moves(board);
        if moves.length == 0 {
            return if moves.check {
                Some((search_ply as i16 - Self::IMMEDIATE_MATE_SCORE, search_ply)) // Checkmate
            } else {
                Some((0, search_ply)) // Draw by no legal moves
            };
        }
        let in_check = moves.check;

        let excluded_move = if search_ply >= MAX_PLIES {
            None
        } else {
            self.singular_exclude_moves[search_ply as usize]
        };

        let pv_node = search_ply == 0 || i32::from(beta) - i32::from(alpha) > 1;
        let all_node = !(pv_node || cut_node);

        let tt_result = search_state
            .transposition_table
            .get(board.zobrist_hash, search_ply);
        let mut tt_move = None;
        if let Some(tt_result) = tt_result {
            tt_move = Some(tt_result.best_move);
            // Don't do TT cutoffs if doing a singular extension scout search
            if excluded_move.is_none() && tt_result.depth >= depth {
                let non_pv = !pv_node;
                match tt_result.node_type {
                    1 if non_pv && tt_result.search_score <= alpha => {
                        return Some((tt_result.search_score, search_ply));
                    } // Upper bound
                    0 => return Some((tt_result.search_score, search_ply)), // Exact
                    -1 if non_pv && tt_result.search_score >= beta => {
                        return Some((tt_result.search_score, search_ply));
                    } // Lower bound
                    _ => {}
                }
            }
        }

        let mut static_eval = tt_result.map_or_else(
            || evaluation::evaluate(board) * color,
            |tt_result| tt_result.static_score,
        );
        if let Some(result) = tt_result
            && !is_mate_score(result.search_score)
            && match result.node_type {
                0 => true,
                -1 => result.search_score > static_eval,
                1 => result.search_score < static_eval,
                _ => false,
            }
        {
            static_eval = result.search_score;
        }

        // If at max depth or max plies, start the quiescence search
        if depth == 0 || search_ply >= MAX_PLIES ||
            // Razoring: If the eval is *really* bad, just skip and go straight to quiescence search.
            (!mate_finding
                && all_node
                && i32::from(static_eval) < i32::from(alpha) - 350 * i32::from(depth))
        {
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

        // Reverse futility pruning: If the static eval is *really* good, just skip searching since
        // any possible move is unlikely to fall below beta.
        if !mate_finding
            && excluded_move.is_none()
            && !pv_node
            && !in_check
            && depth <= 6
            && i32::from(static_eval) >= i32::from(beta) + 80 * i32::from(depth)
        {
            return Some((static_eval, search_ply));
        }

        let maybe_singular = tt_result.filter(|tt_result| {
            excluded_move.is_none()
                && search_ply > 0
                && depth > 6
                && tt_result.depth >= depth - 3
                && tt_result.node_type != 1
        });

        if cut_node
            && !in_check
            && !mate_finding
            && excluded_move.is_none()
            && i32::from(static_eval) >= i32::from(beta) + 70 * i32::from(depth)
            && search_ply > 5
            && depth >= 3
            // Require at least a couple of non-pawn pieces
            && (board.all_pieces() & !board.kings & !board.pawns).count_ones() > 2
        {
            let reduction = ((1792
                + 200 * depth as u32
                + 477 * (i32::from(static_eval) - i32::from(beta)).clamp(0, 900) as u32 / 128)
                / 1024) as u8;

            let metadata = board.make_move(&Move::null());
            let result = self.negamax(
                board,
                search_state,
                depth.saturating_sub(reduction),
                search_ply + 1,
                -beta - 1,
                -beta,
                -color,
                false,
                mate_finding,
            );
            board.unmake_move(&Move::null(), metadata);
            let (eval, _) = result?;
            let eval = -eval;

            if eval >= beta {
                return Some((eval, search_ply));
            }
        }

        let mut extensions = -1;
        // Single reply extension
        if moves.length == 1 || moves.check {
            extensions += 1;
        }
        // Internal Iterative Reduction: A move without a TT move is likely to be a bad move.
        if !all_node && depth >= 4 && tt_move.is_none() {
            extensions -= 1;
        }

        let sorted_moves = self.sort_moves(
            board,
            &moves,
            tt_move,
            self.killer_moves[search_ply as usize],
            color,
        );

        let mut best_move = sorted_moves[0];
        let mut best_value = -i16::MAX;
        let mut best_searched_ply = search_ply;
        for i in 0..sorted_moves.len() {
            if !Self::time_ok_hard(search_state) {
                return None;
            }
            let mov = sorted_moves[i];
            // Singular verification: search every move except the one being tested.
            if Some(mov) == excluded_move {
                continue;
            }

            // Futility pruning: If the static eval is *really* bad, then skip this move
            if !mate_finding
                && excluded_move.is_none()
                && !pv_node
                && !in_check // Only prune if not currently in check and move won't deliver a check
                && !move_generation::move_puts_king_in_check(board, &mov)
                && depth <= 10
                && i > 0
                && !mov.capture()
                && i32::from(static_eval) + 80 * i32::from(depth) <= i32::from(alpha)
            {
                continue;
            }

            let mut move_extension = extensions;

            // Singular extension
            // If all other moves are worse than the TT move by some margin, extend the move
            if search_ply > 0
                && depth >= 5
                && !mate_finding
                && let Some(tt_result) = maybe_singular
                && tt_result.best_move == mov
            {
                let tt_score = tt_result.search_score;
                let singular_beta = tt_score - depth as i16 / 2;
                let singular_depth = depth / 4;

                // Do a search at this board state but exclude the TT move
                self.singular_exclude_moves[search_ply as usize] = Some(mov);
                let score = self.negamax(
                    board,
                    search_state,
                    singular_depth,
                    search_ply,
                    singular_beta - 1,
                    singular_beta,
                    color,
                    cut_node,
                    mate_finding,
                );
                self.singular_exclude_moves[search_ply as usize] = None;
                let (score, _) = score?;

                // If this move is singular, extend it
                if score < singular_beta {
                    move_extension += 1;
                }
                // If this move isn't singular and our margin is better than beta, we already know it's at least
                // as good as beta
                else if singular_beta >= beta {
                    return Some((singular_beta, search_ply));
                }
                // If other moves failed high over the margin, but the margin is less than beta,
                // It can't be determined if the TT move is singular, so reduce the TT move in favor of other moves
                else if tt_score >= beta || cut_node {
                    move_extension = -4;
                }
            }

            let metadata = board.make_move(&mov);
            let search = if i == 0 || depth < 2 {
                self.negamax(
                    board,
                    search_state,
                    max(0, depth as i8 + move_extension) as u8,
                    search_ply + 1,
                    -beta,
                    -alpha,
                    -color,
                    // If PV node, it's not a cut node
                    // If all-node, invert cut node
                    if pv_node { false } else { !cut_node },
                    mate_finding,
                )
            } else {
                let full_depth = max(0, depth as i8 + extensions) as u8;
                let reduced = depth > 2 && sorted_moves.len() > 3 && i > 1 && !mov.capture();
                let search_depth = if reduced {
                    let reduction = ((5.5 * i as f32 / sorted_moves.len() as f32).ceil() as i8
                        - pv_node as i8)
                        .max(0);
                    max(0, depth as i8 + extensions - reduction) as u8
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
                    true,
                    mate_finding,
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
                        !cut_node,
                        mate_finding,
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
                        false,
                        mate_finding,
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
                best_move = mov;
                if alpha >= beta {
                    // If a move is so good it causes a beta cutoff, then it should be given
                    // priority in move ordering
                    self.killer_moves[search_ply as usize] = Some(mov);
                    self.update_history(board, &mov, (depth * depth) as i16);
                    for mov in &sorted_moves[0..i] {
                        self.update_history(board, mov, -(depth as i16));
                    }
                    break;
                }
            }
        }

        // Don't write to TT if doing a singular extension scout search
        if excluded_move.is_none() {
            let evaluation_type = if best_value <= alpha_original {
                1
            } else if best_value >= beta {
                -1
            } else {
                0
            };
            search_state.transposition_table.insert(
                board.zobrist_hash,
                EntryResult::new(depth, best_value, static_eval, evaluation_type, best_move),
                search_ply,
            );
        }
        Some((best_value, best_searched_ply))
    }

    #[allow(clippy::too_many_arguments)]
    fn quiescence(
        &mut self,
        board: &mut Board,
        search_state: &SearchState,
        moves: Moves,
        alpha: i16,
        beta: i16,
        search_ply: u8,
        color: i16,
    ) -> Option<(i16, u8)> {
        if !Self::time_ok_hard(search_state) {
            return None;
        }

        // Draw by FIDE rules
        if board.get_halfmove_clock() >= 100
            || board.times_played() >= 3
            || board.draw_by_insufficient_material()
        {
            return Some((0, search_ply));
        }

        self.count_nodes(search_state.nodes_searched);

        if moves.length == 0 {
            return if moves.check {
                Some((search_ply as i16 - Self::IMMEDIATE_MATE_SCORE, search_ply)) // Checkmate
            } else {
                Some((0, search_ply)) // Draw by no legal moves
            };
        }

        // Evaluate if doing nothing is better
        let static_eval = evaluation::evaluate(board) * color;
        // If it is, return immediately
        if static_eval >= beta {
            return Some((static_eval, search_ply));
        }
        // Otherwise, no move that could be made should be worse than doing nothing
        let mut alpha = alpha.max(static_eval);
        let mut best_value = static_eval;

        // Search all check evasions in check,
        let moves = if moves.check {
            moves
        } else {
            // Otherwise only search captures
            moves.filter_captures()
        };
        if moves.length == 0 {
            return Some((best_value, search_ply));
        }
        let moves = self.sort_moves(board, &moves, None, None, color);

        let mut best_search_ply = search_ply;
        for mov in moves {
            if !Self::time_ok_hard(search_state) {
                return None;
            }

            // Do static exchange evaluation
            let see = evaluation::static_exchange_evaluation(board, &mov);
            // If it turns out to be bad, don't search it
            if see < 0 {
                continue;
            }

            let metadata = board.make_move(&mov);
            let result = self.quiescence(
                board,
                search_state,
                move_generation::legal_moves(board),
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
        color: i16,
    ) -> Vec<Move> {
        let mut sorted_moves: Vec<(&Move, i16)> = Vec::with_capacity(moves.length);
        for i in 0..moves.length {
            let mov = &moves[i];
            let value;
            if let Some(tt_move) = tt_move
                && tt_move.eq(mov)
            {
                // Moves previously evaluated to be the best in the TT are placed first
                value = i16::MAX;
            } else if let Some(killer_move) = killer_move
                && killer_move.eq(mov)
            {
                if mov.flags() != 0b0001 && board.get_piece_at_square(mov.ending_index()).is_none()
                {
                    value = 7_000; // If killer move is non-capturing, score below captures but above normal quiets
                } else {
                    value = i16::MAX - 1; // If capture, score below TT best move but above everything else
                }
            } else {
                value = if mov.capture() {
                    // En passant moves don't have the captured piece at the end square, so special case it
                    let capture_type = if mov.flags() == 0b0001 {
                        piece_type::PAWN
                    } else {
                        board.get_piece_at_square(mov.ending_index()).unwrap()
                    };

                    let eval = evaluation::static_exchange_evaluation(board, mov);
                    if eval != 0 {
                        eval + (15_000 * eval.signum()) // Bad captures are scored below quiet moves
                    } else {
                        12_000
                            + evaluation::mvv_lva(
                                board.get_piece_at_square(mov.starting_index()).unwrap() as usize,
                                capture_type as usize,
                            )
                    }
                } else {
                    let metadata = board.make_move(mov);
                    let eval = evaluation::evaluate(board) * color;
                    board.unmake_move(mov, metadata);
                    eval + self.history[board.white_to_move() as usize]
                        [mov.starting_index() as usize][mov.ending_index() as usize]
                };
            }
            sorted_moves.push((mov, value));
        }
        sorted_moves.sort_by_key(|mov| Reverse(mov.1));
        sorted_moves.iter().map(|mov| *mov.0).collect()
    }

    fn count_nodes(&mut self, nodes_searched: &AtomicU64) {
        self.nodes_searched += 1;
        // Only add to global counter every 2048 nodes to reduce contention
        if self.nodes_searched % 2047 == 0 {
            nodes_searched.fetch_add(2048, Ordering::Relaxed);
        }
    }

    fn update_history(&mut self, board: &Board, mov: &Move, bonus: i16) {
        const MAX_HISTORY: i16 = 768;
        let clamped_bonus = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
        let score = &mut self.history[board.white_to_move() as usize]
            [mov.starting_index() as usize][mov.ending_index() as usize];
        *score += clamped_bonus - *score * clamped_bonus.abs() / MAX_HISTORY;
    }
}
